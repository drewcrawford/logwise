// SPDX-License-Identifier: MIT OR Apache-2.0

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use logwise::{ContextToken, Dispatch, Interest};
use logwise_runtime::{DetailLevel, Filter, FlightRecorder, RecorderView};
use some_executor::{
    current_executor::current_executor,
    task::{Configuration, Task},
};

#[cfg(not(target_arch = "wasm32"))]
use std::thread;
#[cfg(target_arch = "wasm32")]
use wasm_lite_std as thread;

struct PollTwice {
    contexts: Arc<Mutex<Vec<ContextToken>>>,
    first: bool,
}

impl Future for PollTwice {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.contexts
            .lock()
            .unwrap()
            .push(logwise::context::capture());
        if self.first {
            self.first = false;
            cx.waker().wake_by_ref();
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_lite::wasm_lite_test(worker))]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn task_owned_context_migrates_and_restores_poll_threads() {
    let runtime = logwise_runtime::init().expect("install runtime");
    runtime.set_interest(Interest::NONE);
    let recorder = Arc::new(FlightRecorder::with_shards(64, 256, 2));
    let sink = runtime.add_local_sink(
        recorder.clone(),
        Filter::new().event("some_executor.task"),
        DetailLevel::Full,
    );

    let parent = logwise::context::child(ContextToken::NONE, "integration.parent");
    let contexts = Arc::new(Mutex::new(Vec::new()));
    let mut executor = current_executor();
    let (spawned, observer) = {
        let _parent = logwise::context::enter(parent);
        let task = Task::without_notifications(
            "migrating".to_string(),
            Configuration::default(),
            PollTwice {
                contexts: contexts.clone(),
                first: true,
            },
        );
        task.spawn(&mut executor)
    };
    assert!(logwise::context::capture().is_none());

    let mut spawned = thread::spawn(move || {
        assert!(logwise::context::capture().is_none());
        let mut spawned = Box::pin(spawned);
        let mut poll_context = Context::from_waker(Waker::noop());
        assert!(Future::poll(spawned.as_mut(), &mut poll_context).is_pending());
        assert!(logwise::context::capture().is_none());
        spawned
    })
    .join()
    .expect("migrated poll thread");

    let mut poll_context = Context::from_waker(Waker::noop());
    assert!(Future::poll(spawned.as_mut(), &mut poll_context).is_ready());
    assert!(logwise::context::capture().is_none());
    drop(spawned);
    drop(observer);

    let read = recorder.tail(64, RecorderView::Local);
    let spawned = read
        .records
        .iter()
        .find(|record| record.event.metadata.event_name == "some_executor.task.spawned")
        .expect("spawn event");
    let child = spawned.event.context;
    assert_eq!(
        runtime.context(child).expect("task context").parent,
        Some(parent)
    );
    assert_eq!(*contexts.lock().unwrap(), vec![child, child]);

    let names: Vec<_> = read
        .records
        .iter()
        .map(|record| record.event.metadata.event_name)
        .collect();
    for expected in [
        "some_executor.task.spawned",
        "some_executor.task.first_poll",
        "some_executor.task.woken",
        "some_executor.task.completed",
        "some_executor.task.dropped",
        "some_executor.task.wall_lifetime",
        "some_executor.task.active_poll_time",
        "some_executor.task.wake_latency",
    ] {
        assert!(names.contains(&expected), "missing {expected}: {names:?}");
    }
    assert!(
        read.records
            .iter()
            .all(|record| record.event.context == child)
    );

    assert!(runtime.remove_sink(sink));
    runtime.activate_context(parent, Interest::DETAIL_LOCAL, Duration::from_secs(60));
    assert_eq!(
        runtime.contextual_interest(spawned.event.metadata, child),
        Interest::DETAIL_LOCAL
    );
    let unrelated = logwise::context::child(ContextToken::NONE, "integration.unrelated");
    assert_eq!(
        runtime.contextual_interest(spawned.event.metadata, unrelated),
        Interest::NONE
    );
    runtime.activate_context(unrelated, Interest::CORE_LOCAL, Duration::ZERO);
    assert!(!runtime.context_is_active(unrelated));

    let token = logwise::context::child(ContextToken::NONE, "integration.raw_executor");
    let observed = Arc::new(Mutex::new(ContextToken::NONE));
    let inside = observed.clone();
    {
        let _entered = logwise::context::enter(token);
        wasm_lite_std::block_on(async move {
            *inside.lock().unwrap() = logwise::context::capture();
        });
    }
    assert_eq!(*observed.lock().unwrap(), token);
    assert!(logwise::context::capture().is_none());
}
