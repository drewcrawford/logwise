// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{Detail, EventRef, Metadata, Privacy};

#[cfg(target_has_atomic = "ptr")]
use core::sync::atomic::{AtomicUsize, Ordering};

/// The field groups requested by currently active views.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Interest(usize);

impl Interest {
    pub const NONE: Self = Self(0);

    pub const CORE_SUPPORT: Self = Self(1 << 0);
    pub const CORE_LOCAL: Self = Self(1 << 1);
    pub const CORE_SECRET: Self = Self(1 << 2);
    pub const DETAIL_SUPPORT: Self = Self(1 << 3);
    pub const DETAIL_LOCAL: Self = Self(1 << 4);
    pub const DETAIL_SECRET: Self = Self(1 << 5);

    const ALL_BITS: usize = (1 << 6) - 1;

    pub const fn from_bits(bits: usize) -> Self {
        Self(bits & Self::ALL_BITS)
    }

    pub const fn bits(self) -> usize {
        self.0
    }

    pub const fn any(self) -> bool {
        self.0 != 0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

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
    fn emit(&self, event: EventRef<'_>);
}

/// Failure to install the process dispatcher.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstallError {
    AlreadyInstalled,
    /// This target cannot safely install a process-global dispatcher because it
    /// lacks pointer-width atomics. The facade remains a no-op on such targets.
    UnsupportedTarget,
}

#[cfg(target_has_atomic = "ptr")]
struct Cache {
    generation: AtomicUsize,
    interest: AtomicUsize,
}

#[cfg(target_has_atomic = "ptr")]
impl Cache {
    const fn new() -> Self {
        Self {
            generation: AtomicUsize::new(usize::MAX),
            interest: AtomicUsize::new(0),
        }
    }
}

#[cfg(not(target_has_atomic = "ptr"))]
struct Cache;

#[cfg(not(target_has_atomic = "ptr"))]
impl Cache {
    const fn new() -> Self {
        Self
    }
}

/// A static call site with a generation-keyed interest cache.
pub struct Callsite {
    metadata: &'static Metadata,
    cache: Cache,
}

impl Callsite {
    pub const fn new(metadata: &'static Metadata) -> Self {
        Self {
            metadata,
            cache: Cache::new(),
        }
    }

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
        if self.cache.generation.load(Ordering::Acquire) == generation {
            return Interest::from_bits(self.cache.interest.load(Ordering::Relaxed));
        }

        let interest = dispatcher.interest(self.metadata);
        self.cache
            .interest
            .store(interest.bits(), Ordering::Relaxed);
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
}

/// Installs the process dispatcher once.
///
/// Runtime configuration remains mutable behind the dispatcher; the global ABI
/// pointer itself is never replaced.
pub fn install_dispatcher(dispatcher: &'static dyn Dispatch) -> Result<(), InstallError> {
    global::install(dispatcher)
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
