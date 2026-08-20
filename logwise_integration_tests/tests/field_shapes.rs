// SPDX-License-Identifier: MIT OR Apache-2.0

//! A field value that happens to be a one-argument call is a value, not a
//! privacy tag.
//!
//! `name = privacy(value)` and `name = value` are told apart by the leading
//! keyword only, so `size = len(buffer)` must reach `ValueRef::from` intact
//! rather than being read as a privacy tag named `len`.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use logwise::{Detail, Dispatch, EventRef, Interest, Metadata, Privacy, install_dispatcher};

#[derive(Debug, Eq, PartialEq)]
struct Seen {
    name: &'static str,
    privacy: Privacy,
    detail: Detail,
    value: String,
}

struct Capture {
    materialized: Mutex<Vec<Seen>>,
    declared: Mutex<Vec<(&'static str, Privacy, Detail)>>,
    evaluations: AtomicUsize,
}

impl Dispatch for Capture {
    fn generation(&self) -> usize {
        0
    }

    fn interest(&self, _metadata: &'static Metadata) -> Interest {
        Interest::CORE_SUPPORT
            .union(Interest::CORE_LOCAL)
            .union(Interest::CORE_SECRET)
            .union(Interest::DETAIL_SUPPORT)
            .union(Interest::DETAIL_LOCAL)
            .union(Interest::DETAIL_SECRET)
    }

    fn emit(&self, event: EventRef<'_>) {
        *self.declared.lock().unwrap() = event
            .metadata
            .fields
            .iter()
            .map(|field| (field.name, field.privacy, field.detail))
            .collect();
        *self.materialized.lock().unwrap() = event
            .fields
            .iter()
            .flatten()
            .map(|field| Seen {
                name: field.metadata.name,
                privacy: field.metadata.privacy,
                detail: field.metadata.detail,
                value: format!("{:?}", field.value),
            })
            .collect();
    }
}

static CAPTURE: Capture = Capture {
    materialized: Mutex::new(Vec::new()),
    declared: Mutex::new(Vec::new()),
    evaluations: AtomicUsize::new(0),
};

/// A plain identifier applied to exactly one argument -- the shape a privacy
/// tag also has.
fn width(value: u64) -> u64 {
    CAPTURE.evaluations.fetch_add(1, Ordering::Relaxed);
    value * 2
}

#[cfg_attr(target_arch = "wasm32", wasm_lite::wasm_lite_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn call_expressions_are_values_not_privacy_tags() {
    install_dispatcher(&CAPTURE).expect("install capture dispatcher");

    logwise::event!(
        "integration.fields.calls",
        implicit = width(1),
        tagged = support(width(2)),
        detail deferred = secret(width(3)),
        detail bare = width(4),
    );

    assert_eq!(CAPTURE.evaluations.load(Ordering::Relaxed), 4);
    assert_eq!(
        *CAPTURE.declared.lock().unwrap(),
        vec![
            ("implicit", Privacy::LocalOnly, Detail::Core),
            ("tagged", Privacy::SupportSafe, Detail::Core),
            ("deferred", Privacy::Secret, Detail::Detail),
            ("bare", Privacy::LocalOnly, Detail::Detail),
        ],
        "an untagged call must keep the default policy, and a tagged one its own"
    );
    let materialized = CAPTURE.materialized.lock().unwrap();
    assert_eq!(
        materialized
            .iter()
            .map(|seen| seen.name)
            .collect::<Vec<_>>(),
        vec!["implicit", "tagged", "deferred", "bare"]
    );
    assert_eq!(
        materialized
            .iter()
            .map(|seen| seen.value.as_str())
            .collect::<Vec<_>>(),
        vec!["2", "4", "6", "8"],
        "each call must be evaluated once, in order, as the field's value"
    );
}
