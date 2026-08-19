// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use logwise::{Class, Detail, Interest, Kind, Privacy, Severity};
use logwise_runtime::{ActivationResult, DetailLevel, EventSink, Filter, ProjectedEvent, Target};

#[derive(Debug)]
struct SeenEvent {
    name: &'static str,
    fields: Vec<(&'static str, String)>,
    message: Option<String>,
    omitted: usize,
}

#[derive(Default)]
struct RetainingSink {
    seen: Mutex<Vec<SeenEvent>>,
}

impl EventSink for RetainingSink {
    fn emit(&self, event: ProjectedEvent<'_>) {
        let fields = event
            .fields
            .iter()
            .map(|field| (field.name, format!("{:?}", field.value)))
            .collect();
        self.seen.lock().unwrap().push(SeenEvent {
            name: event.metadata.event_name,
            fields,
            message: event.message.map(|message| message.to_string()),
            omitted: event.omitted_fields,
        });
    }
}

#[derive(Default)]
struct EphemeralSink {
    support: AtomicUsize,
    local: AtomicUsize,
    secret: AtomicUsize,
}

impl EventSink for EphemeralSink {
    fn emit(&self, event: ProjectedEvent<'_>) {
        for field in event.fields {
            match event
                .metadata
                .fields
                .iter()
                .find(|metadata| metadata.name == field.name)
                .unwrap()
                .privacy
            {
                Privacy::SupportSafe => self.support.fetch_add(1, Ordering::Relaxed),
                Privacy::LocalOnly => self.local.fetch_add(1, Ordering::Relaxed),
                Privacy::Secret => self.secret.fetch_add(1, Ordering::Relaxed),
            };
        }
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_lite::wasm_lite_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn simultaneous_views_are_projected_before_sinks() {
    let runtime = logwise_runtime::init().expect("install runtime");
    runtime.set_interest(Interest::NONE);

    let remote = Arc::new(RetainingSink::default());
    let local = Arc::new(RetainingSink::default());
    let ephemeral = Arc::new(EphemeralSink::default());

    let remote_id = runtime.add_remote_sink(
        remote.clone(),
        Filter::new()
            .domain("integration.projection")
            .event("integration.projection")
            .class(Class::Forensic)
            .minimum_severity(Severity::Warn),
        DetailLevel::Full,
    );
    const DOMAIN: logwise::Domain = logwise::domain!("integration.projection.subsystem");
    let remote_only_evaluations = AtomicUsize::new(0);
    logwise::event!(
        domain: DOMAIN,
        class: forensic,
        severity: warn,
        name: "integration.projection.remote_only",
        public = support({ remote_only_evaluations.fetch_add(1, Ordering::Relaxed); 1_u64 }),
        private = local({ remote_only_evaluations.fetch_add(10, Ordering::Relaxed); "local" }),
        detail route = support({ remote_only_evaluations.fetch_add(1, Ordering::Relaxed); "route" }),
        detail debug = local({ remote_only_evaluations.fetch_add(100, Ordering::Relaxed); "debug" }),
    );
    assert_eq!(remote_only_evaluations.load(Ordering::Relaxed), 2);
    assert_eq!(remote.seen.lock().unwrap()[0].fields.len(), 2);
    remote.seen.lock().unwrap().clear();

    runtime.add_local_sink(local.clone(), Filter::new(), DetailLevel::Full);
    runtime.add_ephemeral_sink(ephemeral.clone(), Filter::new(), DetailLevel::Full);

    let evaluations = AtomicUsize::new(0);
    logwise::event!(
        domain: DOMAIN,
        class: forensic,
        severity: warn,
        name: "integration.projection.event",
        public = support({ evaluations.fetch_add(1, Ordering::Relaxed); 7_u64 }),
        private = local({ evaluations.fetch_add(1, Ordering::Relaxed); "local" }),
        password = secret({ evaluations.fetch_add(1, Ordering::Relaxed); "secret" }),
        detail route = support({ evaluations.fetch_add(1, Ordering::Relaxed); "route" }),
        detail debug = local({ evaluations.fetch_add(1, Ordering::Relaxed); "debug" }),
        detail key = secret({ evaluations.fetch_add(1, Ordering::Relaxed); "key" }),
    );
    assert_eq!(evaluations.load(Ordering::Relaxed), 6);

    let remote_seen = remote.seen.lock().unwrap();
    assert_eq!(remote_seen.len(), 1);
    assert_eq!(
        remote_seen[0].fields,
        vec![("public", "7".into()), ("route", "\"route\"".into())]
    );
    assert_eq!(remote_seen[0].omitted, 4);
    assert!(remote_seen[0].message.is_none());
    drop(remote_seen);

    let local_seen = local.seen.lock().unwrap();
    assert_eq!(local_seen.len(), 1);
    assert_eq!(local_seen[0].fields.len(), 4);
    assert!(
        local_seen[0]
            .fields
            .iter()
            .all(|(name, _)| *name != "password" && *name != "key")
    );
    assert_eq!(local_seen[0].omitted, 2);
    drop(local_seen);

    assert_eq!(ephemeral.support.load(Ordering::Relaxed), 2);
    assert_eq!(ephemeral.local.load(Ordering::Relaxed), 2);
    assert_eq!(ephemeral.secret.load(Ordering::Relaxed), 2);

    logwise::log!("private text {}", "never remote");
    assert_eq!(remote.seen.lock().unwrap().len(), 1);
    let local_seen = local.seen.lock().unwrap();
    assert_eq!(local_seen.len(), 2);
    assert_eq!(
        local_seen[1].message.as_deref(),
        Some("private text never remote")
    );
    drop(local_seen);

    assert!(runtime.remove_sink(remote_id));
    logwise::event!(
        domain: DOMAIN,
        class: forensic,
        severity: warn,
        name: "integration.projection.after_remove",
        public = support(8_u64),
    );
    assert_eq!(remote.seen.lock().unwrap().len(), 1);

    assert_eq!(
        runtime.activate(
            Filter::new().event("integration.projection.event"),
            Interest::DETAIL_LOCAL,
            core::time::Duration::from_secs(60),
        ),
        ActivationResult::Enabled
    );
    assert_eq!(
        runtime.activate(
            Filter::new()
                .domain("integration.projection")
                .event("integration.projection.not_compiled"),
            Interest::DETAIL_LOCAL,
            core::time::Duration::from_secs(60),
        ),
        ActivationResult::NotCompiled
    );
    assert_eq!(
        runtime.activate(
            Filter::new().domain("unknown.domain"),
            Interest::DETAIL_LOCAL,
            core::time::Duration::from_secs(60),
        ),
        ActivationResult::UnknownSelector
    );
    let other_target = if cfg!(target_arch = "wasm32") {
        Target::Native
    } else {
        Target::Wasm
    };
    assert_eq!(
        runtime.activate(
            Filter::new().target(other_target),
            Interest::DETAIL_LOCAL,
            core::time::Duration::from_secs(60),
        ),
        ActivationResult::UnavailableTarget
    );

    let catalog = runtime.catalog();
    let projected = catalog
        .iter()
        .find(|metadata| metadata.event_name == "integration.projection.event")
        .expect("catalogued call site");
    assert_eq!(projected.kind, Kind::Event);
    assert_eq!(projected.fields[3].detail, Detail::Detail);
    assert_eq!(
        remote.seen.lock().unwrap()[0].name,
        "integration.projection.event"
    );
}
