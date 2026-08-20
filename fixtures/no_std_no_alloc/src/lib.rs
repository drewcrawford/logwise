// SPDX-License-Identifier: MIT OR Apache-2.0

//! Link-time proof that the facade is usable with neither `std` nor `alloc`.
//!
//! This fixture is a `no_std` crate that depends on `logwise` with default
//! features off and calls into it. It is not a test of behaviour — it is a
//! test that the code *builds and links* in that environment, which is the one
//! claim the facade makes that a normal test on the host cannot check.
//!
//! If someone gives the facade an allocation on a hot path, this stops
//! compiling. That is the entire point, and it is why the crate is
//! `publish = false` and excluded from the workspace.

#![no_std]

pub fn facade_is_linkable() {
    let answer = 42_u64;
    logwise::log!("answer={answer}");
    logwise::event!(
        "logwise.fixture.answer",
        answer = support(answer),
        detail rendered = local(logwise::ValueRef::display(&answer)),
    );
    let token = logwise::context::child(logwise::ContextToken::NONE, "fixture");
    let _entered = logwise::context::enter(token);
    let _span = logwise::span!("logwise.fixture.span", answer = support(answer));

    logwise::event!(
        #[cfg(any())]
        "LOGWISE_STATICALLY_EXCLUDED_SENTINEL_3FC0A730",
        answer = support(answer),
    );
}
