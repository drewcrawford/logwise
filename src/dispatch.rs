// SPDX-License-Identifier: MIT OR Apache-2.0

//! The dispatch ABI: the single seam between instrumented crates and a runtime.
//!
//! An application installs one process-wide [`Dispatch`] via
//! [`install_dispatcher`]; every call site routes through it. Until that
//! happens each operation is a non-allocating no-op, which is what lets a
//! library instrument itself without taxing consumers who never read a log
//! line.
//!
//! The performance contract lives here. [`Interest`] is a bitmask of the field
//! groups some active view has actually asked for, and each [`Callsite`]
//! caches the mask it was told alongside the runtime generation that mask was
//! computed for. A call site consults the cache *before* evaluating any field
//! expression, so unwanted work is never done rather than done and discarded.
//! Packing the generation into the mask's upper bits is what makes a torn
//! two-word update detectable: a mismatched pair recomputes instead of serving
//! a stale interest forever.

use crate::{ContextToken, Detail, EventRef, Metadata, Privacy, SpanGuard, SpanRef, SpanToken};

#[cfg(target_has_atomic = "ptr")]
use core::sync::atomic::{AtomicUsize, Ordering};

/// The field groups requested by currently active views.
///
/// `Hash` is deliberately not implemented. An interest is a transient answer
/// to "what does anyone want from this call site right now", not an identity;
/// keying a map by one would be keying it by a value that changes whenever a
/// view is added or removed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Interest(usize);

impl Interest {
    /// Nothing is wanted. The call site evaluates no field expressions.
    pub const NONE: Self = Self(0);

    /// Some view wants core support-safe fields.
    pub const CORE_SUPPORT: Self = Self(1 << 0);
    /// Some view wants core local-only fields.
    pub const CORE_LOCAL: Self = Self(1 << 1);
    /// Some trusted view wants core secret fields.
    pub const CORE_SECRET: Self = Self(1 << 2);
    /// Some view wants deferred support-safe detail.
    pub const DETAIL_SUPPORT: Self = Self(1 << 3);
    /// Some view wants deferred local-only detail.
    pub const DETAIL_LOCAL: Self = Self(1 << 4);
    /// Some trusted view wants deferred secret detail.
    pub const DETAIL_SECRET: Self = Self(1 << 5);
    /// The runtime must refine this call site's interest against the current
    /// context before any fields are evaluated.
    pub const CONTEXTUAL: Self = Self(1 << 6);

    const ALL_BITS: usize = (1 << 7) - 1;

    /// Reconstructs an interest from its bit representation, discarding any
    /// bit this version of the facade does not define.
    pub const fn from_bits(bits: usize) -> Self {
        Self(bits & Self::ALL_BITS)
    }

    /// The bit representation, for a runtime that stores or transmits it.
    pub const fn bits(self) -> usize {
        self.0
    }

    /// Whether any view wants anything from this call site.
    pub const fn any(self) -> bool {
        self.0 != 0
    }

    /// The interest satisfying both, for a runtime combining several views.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Whether the runtime must refine this against the current context
    /// before the call site evaluates anything.
    pub const fn is_contextual(self) -> bool {
        self.0 & Self::CONTEXTUAL.0 != 0
    }

    /// This interest with the contextual-refinement request cleared, leaving
    /// only the field groups themselves.
    pub const fn without_contextual(self) -> Self {
        Self(self.0 & !Self::CONTEXTUAL.0)
    }

    /// Whether a field with this privacy and detail should be evaluated.
    ///
    /// This is the check a call site makes before running a field expression,
    /// and the reason unwanted work is never done rather than done and thrown
    /// away.
    pub const fn wants(self, privacy: Privacy, detail: Detail) -> bool {
        let bit = match (detail, privacy) {
            (Detail::Core, Privacy::SupportSafe) => Self::CORE_SUPPORT.0,
            (Detail::Core, Privacy::LocalOnly) => Self::CORE_LOCAL.0,
            (Detail::Core, Privacy::Secret) => Self::CORE_SECRET.0,
            (Detail::Detail, Privacy::SupportSafe) => Self::DETAIL_SUPPORT.0,
            (Detail::Detail, Privacy::LocalOnly) => Self::DETAIL_LOCAL.0,
            (Detail::Detail, Privacy::Secret) => Self::DETAIL_SECRET.0,
        };
        self.0 & bit != 0
    }
}

/// Runtime-side implementation of the facade dispatch ABI.
///
/// The facade installs exactly one dispatcher. The dispatcher may mutate its
/// own filters and sinks; changing interest must advance `generation`.
/// Generations must not use `usize::MAX`, which is reserved for an unpopulated
/// call-site cache, and must not repeat while stale cache entries can exist.
#[doc(hidden)]
pub trait Dispatch: Sync + 'static {
    fn generation(&self) -> usize;
    fn interest(&self, metadata: &'static Metadata) -> Interest;
    fn contextual_interest(&self, metadata: &'static Metadata, _context: ContextToken) -> Interest {
        self.interest(metadata).without_contextual()
    }
    fn emit(&self, event: EventRef<'_>);

    fn capture_context(&self) -> ContextToken {
        ContextToken::NONE
    }

    fn create_context(&self, _parent: ContextToken, _name: &'static str) -> ContextToken {
        ContextToken::NONE
    }

    fn link_context(&self, _context: ContextToken, _related: ContextToken) {}

    fn enter_context(&self, _context: ContextToken) -> ContextToken {
        ContextToken::NONE
    }

    fn exit_context(&self, _previous: ContextToken) {}

    fn start_span(&self, span: SpanRef<'_>) -> SpanToken {
        self.emit(span.event);
        SpanToken::NONE
    }

    fn end_span(&self, _span: SpanToken, _context: ContextToken) {}
}

/// Failure to install the process dispatcher.
///
/// The variants are exhaustive on purpose: installation has exactly these two
/// outcomes, and a caller matching on them should get a compile error rather
/// than a silent fallthrough if that ever changes.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum InstallError {
    /// A dispatcher is already installed. Installation is once per process,
    /// so that every call site in the program observes the same one.
    AlreadyInstalled,
    /// This target cannot safely install a process-global dispatcher because it
    /// lacks pointer-width atomics. The facade remains a no-op on such targets.
    UnsupportedTarget,
}

impl core::fmt::Display for InstallError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let message = match self {
            Self::AlreadyInstalled => "a logwise dispatcher is already installed",
            Self::UnsupportedTarget => {
                "this target lacks pointer-width atomics, so no dispatcher can be installed"
            }
        };
        formatter.write_str(message)
    }
}

impl core::error::Error for InstallError {}

#[cfg(target_has_atomic = "ptr")]
#[derive(Debug)]
struct Cache {
    generation: AtomicUsize,
    /// The cached mask, carrying the generation it was computed for in its
    /// upper bits. See [`Cache::tag`].
    tagged_interest: AtomicUsize,
}

#[cfg(target_has_atomic = "ptr")]
impl Cache {
    const fn new() -> Self {
        Self {
            generation: AtomicUsize::new(usize::MAX),
            tagged_interest: AtomicUsize::new(0),
        }
    }

    /// Pairs a mask with the generation it was computed for, in one word.
    ///
    /// The two cache words cannot be published together, and two threads that
    /// miss at different generations can interleave their four stores so the
    /// newer generation is left sitting next to the older mask -- a stale
    /// interest that then looks current until the generation moves again. The
    /// mask carries its own generation so that pairing is detectable; a
    /// mismatch is treated as a miss and recomputed.
    ///
    /// Only the upper `usize::BITS - INTEREST_WIDTH` bits are left for the
    /// generation, so this
    /// detects everything except a collision between two generations that are
    /// exactly a multiple of `2^(usize::BITS - INTEREST_WIDTH)` apart. The dispatcher
    /// contract already forbids reusing a generation while stale entries can
    /// exist; this is the same requirement, one shift weaker.
    const fn tag(generation: usize, interest: Interest) -> usize {
        (generation << INTEREST_WIDTH) | interest.bits()
    }
}

/// Number of low bits [`Interest`] occupies in a tagged cache word. Derived
/// from the mask itself so adding an interest bit cannot silently start
/// overlapping the generation tag.
#[cfg(target_has_atomic = "ptr")]
const INTEREST_WIDTH: u32 = usize::BITS - Interest::ALL_BITS.leading_zeros();

#[cfg(not(target_has_atomic = "ptr"))]
#[derive(Debug)]
struct Cache;

#[cfg(not(target_has_atomic = "ptr"))]
impl Cache {
    const fn new() -> Self {
        Self
    }
}

/// A static call site with a generation-keyed interest cache.
#[derive(Debug)]
pub struct Callsite {
    metadata: &'static Metadata,
    cache: Cache,
}

impl Callsite {
    /// Creates the call site for one piece of static metadata.
    ///
    /// This is `const` so a call site can live in a static and pay nothing to
    /// come into existence.
    pub const fn new(metadata: &'static Metadata) -> Self {
        Self {
            metadata,
            cache: Cache::new(),
        }
    }

    /// The static metadata this call site was built from.
    pub const fn metadata(&self) -> &'static Metadata {
        self.metadata
    }

    /// Returns the field groups requested by the installed dispatcher.
    ///
    /// With no runtime this is `Interest::NONE`. Call-site macros must check
    /// this before evaluating any dynamic field expression.
    #[cfg(target_has_atomic = "ptr")]
    pub fn interest(&self) -> Interest {
        let Some(dispatcher) = global::dispatcher() else {
            return Interest::NONE;
        };

        let generation = dispatcher.generation();
        // Acquire here pairs with the Release below, so a matching generation
        // also publishes the mask stored before it.
        if self.cache.generation.load(Ordering::Acquire) == generation {
            let cached = self.cache.tagged_interest.load(Ordering::Relaxed);
            let interest = Interest::from_bits(cached);
            if Cache::tag(generation, interest) == cached {
                return interest;
            }
        }

        let interest = dispatcher.interest(self.metadata);
        self.cache
            .tagged_interest
            .store(Cache::tag(generation, interest), Ordering::Relaxed);
        self.cache.generation.store(generation, Ordering::Release);
        interest
    }

    #[cfg(not(target_has_atomic = "ptr"))]
    pub const fn interest(&self) -> Interest {
        Interest::NONE
    }

    /// Delivers one borrowed event synchronously.
    pub fn emit(&self, event: EventRef<'_>) {
        debug_assert!(core::ptr::eq(self.metadata, event.metadata));
        if let Some(dispatcher) = global::dispatcher() {
            dispatcher.emit(event);
        }
    }

    /// Refines a cached mask when a runtime has context-targeted activation.
    pub fn contextual_interest(&self, interest: Interest, context: ContextToken) -> Interest {
        if !interest.is_contextual() {
            return interest;
        }
        global::dispatcher().map_or(Interest::NONE, |dispatcher| {
            dispatcher.contextual_interest(self.metadata, context)
        })
    }

    /// Starts a runtime-owned span from a borrowed observation.
    pub fn start_span(&self, span: SpanRef<'_>) -> SpanGuard {
        debug_assert!(core::ptr::eq(self.metadata, span.event.metadata));
        let context = span.event.context;
        let Some(dispatcher) = global::dispatcher() else {
            return SpanGuard::disabled();
        };
        SpanGuard::new(dispatcher.start_span(span), context)
    }
}

/// Installs the process dispatcher once.
///
/// Runtime configuration remains mutable behind the dispatcher; the global ABI
/// pointer itself is never replaced.
pub fn install_dispatcher(dispatcher: &'static dyn Dispatch) -> Result<(), InstallError> {
    global::install(dispatcher)
}

pub(crate) fn capture_context() -> ContextToken {
    global::dispatcher().map_or(ContextToken::NONE, Dispatch::capture_context)
}

pub(crate) fn create_context(parent: ContextToken, name: &'static str) -> ContextToken {
    global::dispatcher().map_or(ContextToken::NONE, |dispatcher| {
        dispatcher.create_context(parent, name)
    })
}

pub(crate) fn link_context(context: ContextToken, related: ContextToken) {
    if let Some(dispatcher) = global::dispatcher() {
        dispatcher.link_context(context, related);
    }
}

pub(crate) fn enter_context(context: ContextToken) -> Option<ContextToken> {
    global::dispatcher().map(|dispatcher| dispatcher.enter_context(context))
}

pub(crate) fn exit_context(previous: ContextToken) {
    if let Some(dispatcher) = global::dispatcher() {
        dispatcher.exit_context(previous);
    }
}

pub(crate) fn end_span(span: SpanToken, context: ContextToken) {
    if let Some(dispatcher) = global::dispatcher() {
        dispatcher.end_span(span, context);
    }
}

#[cfg(target_has_atomic = "ptr")]
#[allow(unsafe_code)]
mod global {
    use core::mem::MaybeUninit;
    use core::sync::atomic::{AtomicUsize, Ordering};

    use super::{Dispatch, InstallError};

    const UNINSTALLED: usize = 0;
    const INSTALLING: usize = 1;
    const INSTALLED: usize = 2;

    static STATE: AtomicUsize = AtomicUsize::new(UNINSTALLED);
    static mut DISPATCHER: MaybeUninit<&'static dyn Dispatch> = MaybeUninit::uninit();

    pub(super) fn install(dispatcher: &'static dyn Dispatch) -> Result<(), InstallError> {
        STATE
            .compare_exchange(
                UNINSTALLED,
                INSTALLING,
                Ordering::Acquire,
                Ordering::Relaxed,
            )
            .map_err(|_| InstallError::AlreadyInstalled)?;

        // SAFETY: the successful state transition gives this thread the only
        // write access. The value is written completely before the Release
        // store publishes INSTALLED, and it is never mutated afterward.
        unsafe {
            core::ptr::write(&raw mut DISPATCHER, MaybeUninit::new(dispatcher));
        }
        STATE.store(INSTALLED, Ordering::Release);
        Ok(())
    }

    pub(super) fn dispatcher() -> Option<&'static dyn Dispatch> {
        if STATE.load(Ordering::Acquire) != INSTALLED {
            return None;
        }

        // SAFETY: observing INSTALLED with Acquire synchronizes with the
        // installer Release store. The initialized value is immutable forever;
        // ptr::read copies the shared reference without creating a reference to
        // the mutable static itself.
        Some(unsafe { core::ptr::read(&raw const DISPATCHER).assume_init() })
    }
}

#[cfg(not(target_has_atomic = "ptr"))]
mod global {
    use super::{Dispatch, InstallError};

    pub(super) const fn install(_dispatcher: &'static dyn Dispatch) -> Result<(), InstallError> {
        Err(InstallError::UnsupportedTarget)
    }

    pub(super) const fn dispatcher() -> Option<&'static dyn Dispatch> {
        None
    }
}
