// SPDX-License-Identifier: MIT OR Apache-2.0

use logwise::{
    Class, ContextToken, Detail, EventRef, FieldMetadata, FieldRef, Kind, Metadata, Privacy,
    Severity, ValueRef,
};
use logwise_runtime_wasm::{ABI_VERSION, HostStatus, Identity, Transport};

static FIELD: FieldMetadata = FieldMetadata::new("task_id", Privacy::SupportSafe, Detail::Core);
static FIELDS: &[FieldMetadata] = &[FIELD];
static METADATA: Metadata = Metadata {
    event_name: "wasm_wire.task",
    package: "logwise_integration_tests",
    target: "wasm_wire",
    module: "wasm_wire",
    domain: None,
    severity: Severity::Info,
    class: Class::Forensic,
    kind: Kind::Event,
    location: None,
    fields: FIELDS,
};

#[cfg_attr(target_arch = "wasm32", wasm_lite::wasm_lite_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn versioned_wire_encoder_runs_without_host_glue() {
    let field = Some(FieldRef::new(&FIELD, ValueRef::U64(487)));
    let fields = [field];
    let event = EventRef::structured(&METADATA, ContextToken::from_parts(9, 1), &fields);
    let transport = Transport::new(64);
    let mut output = [0_u8; 512];
    let mut scratch = [0_u8; 64];
    let encoded = transport
        .encode(
            event,
            &[],
            Identity {
                worker: 3,
                test: Some("wire-test"),
            },
            &mut output,
            &mut scratch,
        )
        .unwrap();
    assert_eq!(&encoded.bytes[..4], b"LW1\0");
    assert_eq!(
        u16::from_le_bytes(encoded.bytes[4..6].try_into().unwrap()),
        ABI_VERSION
    );
    assert_eq!(
        u32::from_le_bytes(encoded.bytes[8..12].try_into().unwrap()) as usize,
        encoded.bytes.len()
    );
    assert!(
        encoded
            .bytes
            .windows(METADATA.event_name.len())
            .any(|window| window == METADATA.event_name.as_bytes())
    );

    let status = transport
        .emit(event, &[], Identity::default(), &mut output, &mut scratch)
        .unwrap();
    assert_eq!(status, HostStatus::Unavailable);
    assert_eq!(transport.dropped(), 1);
}
