#![no_std]

pub fn facade_is_linkable() {
    let answer = 42_u64;
    logwise::log!("answer={answer}");
    logwise::event!(
        "logwise.fixture.answer",
        answer = support(answer),
        detail rendered = local(logwise::ValueRef::display(&answer)),
    );

    logwise::event!(
        #[cfg(any())]
        "LOGWISE_STATICALLY_EXCLUDED_SENTINEL_3FC0A730",
        answer = support(answer),
    );
}
