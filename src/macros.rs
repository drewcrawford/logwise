// SPDX-License-Identifier: MIT OR Apache-2.0

/// Creates a hierarchical domain override.
#[macro_export]
macro_rules! domain {
    ($name:literal $(,)?) => {
        $crate::Domain::new($name)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __logwise_class {
    (operational) => {
        $crate::Class::Operational
    };
    (diagnostic) => {
        $crate::Class::Diagnostic
    };
    (forensic) => {
        $crate::Class::Forensic
    };
    (performance) => {
        $crate::Class::Performance
    };
    (metric) => {
        $crate::Class::Metric
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __logwise_severity {
    (trace) => {
        $crate::Severity::Trace
    };
    (debug) => {
        $crate::Severity::Debug
    };
    (info) => {
        $crate::Severity::Info
    };
    (warn) => {
        $crate::Severity::Warn
    };
    (error) => {
        $crate::Severity::Error
    };
    (critical) => {
        $crate::Severity::Critical
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __logwise_privacy {
    (support) => {
        $crate::Privacy::SupportSafe
    };
    (local) => {
        $crate::Privacy::LocalOnly
    };
    (secret) => {
        $crate::Privacy::Secret
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __logwise_field_metadata {
    (@accum [$($output:expr,)*]) => { &[$($output,)*] };
    (@accum [$($output:expr,)*] ,) => { &[$($output,)*] };

    (@accum [$($output:expr,)*] detail $name:ident = $privacy:ident($value:expr), $($rest:tt)*) => {
        $crate::__logwise_field_metadata!(@accum [
            $($output,)*
            $crate::FieldMetadata::new(
                stringify!($name),
                $crate::__logwise_privacy!($privacy),
                $crate::Detail::Detail,
            ),
        ] $($rest)*)
    };
    (@accum [$($output:expr,)*] detail $name:ident = $privacy:ident($value:expr)) => {
        &[$($output,)* $crate::FieldMetadata::new(
            stringify!($name),
            $crate::__logwise_privacy!($privacy),
            $crate::Detail::Detail,
        )]
    };
    (@accum [$($output:expr,)*] detail $name:ident = $value:expr, $($rest:tt)*) => {
        $crate::__logwise_field_metadata!(@accum [
            $($output,)*
            $crate::FieldMetadata::new(
                stringify!($name),
                $crate::Privacy::LocalOnly,
                $crate::Detail::Detail,
            ),
        ] $($rest)*)
    };
    (@accum [$($output:expr,)*] detail $name:ident = $value:expr) => {
        &[$($output,)* $crate::FieldMetadata::new(
            stringify!($name),
            $crate::Privacy::LocalOnly,
            $crate::Detail::Detail,
        )]
    };
    (@accum [$($output:expr,)*] $name:ident = $privacy:ident($value:expr), $($rest:tt)*) => {
        $crate::__logwise_field_metadata!(@accum [
            $($output,)*
            $crate::FieldMetadata::new(
                stringify!($name),
                $crate::__logwise_privacy!($privacy),
                $crate::Detail::Core,
            ),
        ] $($rest)*)
    };
    (@accum [$($output:expr,)*] $name:ident = $privacy:ident($value:expr)) => {
        &[$($output,)* $crate::FieldMetadata::new(
            stringify!($name),
            $crate::__logwise_privacy!($privacy),
            $crate::Detail::Core,
        )]
    };
    (@accum [$($output:expr,)*] $name:ident = $value:expr, $($rest:tt)*) => {
        $crate::__logwise_field_metadata!(@accum [
            $($output,)*
            $crate::FieldMetadata::new(
                stringify!($name),
                $crate::Privacy::LocalOnly,
                $crate::Detail::Core,
            ),
        ] $($rest)*)
    };
    (@accum [$($output:expr,)*] $name:ident = $value:expr) => {
        &[$($output,)* $crate::FieldMetadata::new(
            stringify!($name),
            $crate::Privacy::LocalOnly,
            $crate::Detail::Core,
        )]
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __logwise_field_values {
    (@accum [$($output:expr,)*] $metadata:ident $interest:ident;) => { [$($output,)*] };
    (@accum [$($output:expr,)*] $metadata:ident $interest:ident; ,) => { [$($output,)*] };

    (@accum [$($output:expr,)*] $metadata:ident $interest:ident;
        detail $name:ident = $privacy:ident($value:expr), $($rest:tt)*) => {
        $crate::__logwise_field_values!(@accum [
            $($output,)*
            {
                let __logwise_field_metadata = $metadata.next().expect("field metadata");
                if $interest.wants(
                    $crate::__logwise_privacy!($privacy),
                    $crate::Detail::Detail,
                ) {
                    Some($crate::FieldRef::new(
                        __logwise_field_metadata,
                        $crate::ValueRef::from($value),
                    ))
                } else {
                    None
                }
            },
        ] $metadata $interest; $($rest)*)
    };
    (@accum [$($output:expr,)*] $metadata:ident $interest:ident;
        detail $name:ident = $privacy:ident($value:expr)) => {
        [$($output,)* {
            let __logwise_field_metadata = $metadata.next().expect("field metadata");
            if $interest.wants(
                $crate::__logwise_privacy!($privacy),
                $crate::Detail::Detail,
            ) {
                Some($crate::FieldRef::new(
                    __logwise_field_metadata,
                    $crate::ValueRef::from($value),
                ))
            } else {
                None
            }
        }]
    };
    (@accum [$($output:expr,)*] $metadata:ident $interest:ident;
        detail $name:ident = $value:expr, $($rest:tt)*) => {
        $crate::__logwise_field_values!(@accum [
            $($output,)*
            {
                let __logwise_field_metadata = $metadata.next().expect("field metadata");
                if $interest.wants($crate::Privacy::LocalOnly, $crate::Detail::Detail) {
                    Some($crate::FieldRef::new(
                        __logwise_field_metadata,
                        $crate::ValueRef::from($value),
                    ))
                } else {
                    None
                }
            },
        ] $metadata $interest; $($rest)*)
    };
    (@accum [$($output:expr,)*] $metadata:ident $interest:ident;
        detail $name:ident = $value:expr) => {
        [$($output,)* {
            let __logwise_field_metadata = $metadata.next().expect("field metadata");
            if $interest.wants($crate::Privacy::LocalOnly, $crate::Detail::Detail) {
                Some($crate::FieldRef::new(
                    __logwise_field_metadata,
                    $crate::ValueRef::from($value),
                ))
            } else {
                None
            }
        }]
    };
    (@accum [$($output:expr,)*] $metadata:ident $interest:ident;
        $name:ident = $privacy:ident($value:expr), $($rest:tt)*) => {
        $crate::__logwise_field_values!(@accum [
            $($output,)*
            {
                let __logwise_field_metadata = $metadata.next().expect("field metadata");
                if $interest.wants(
                    $crate::__logwise_privacy!($privacy),
                    $crate::Detail::Core,
                ) {
                    Some($crate::FieldRef::new(
                        __logwise_field_metadata,
                        $crate::ValueRef::from($value),
                    ))
                } else {
                    None
                }
            },
        ] $metadata $interest; $($rest)*)
    };
    (@accum [$($output:expr,)*] $metadata:ident $interest:ident;
        $name:ident = $privacy:ident($value:expr)) => {
        [$($output,)* {
            let __logwise_field_metadata = $metadata.next().expect("field metadata");
            if $interest.wants(
                $crate::__logwise_privacy!($privacy),
                $crate::Detail::Core,
            ) {
                Some($crate::FieldRef::new(
                    __logwise_field_metadata,
                    $crate::ValueRef::from($value),
                ))
            } else {
                None
            }
        }]
    };
    (@accum [$($output:expr,)*] $metadata:ident $interest:ident;
        $name:ident = $value:expr, $($rest:tt)*) => {
        $crate::__logwise_field_values!(@accum [
            $($output,)*
            {
                let __logwise_field_metadata = $metadata.next().expect("field metadata");
                if $interest.wants($crate::Privacy::LocalOnly, $crate::Detail::Core) {
                    Some($crate::FieldRef::new(
                        __logwise_field_metadata,
                        $crate::ValueRef::from($value),
                    ))
                } else {
                    None
                }
            },
        ] $metadata $interest; $($rest)*)
    };
    (@accum [$($output:expr,)*] $metadata:ident $interest:ident;
        $name:ident = $value:expr) => {
        [$($output,)* {
            let __logwise_field_metadata = $metadata.next().expect("field metadata");
            if $interest.wants($crate::Privacy::LocalOnly, $crate::Detail::Core) {
                Some($crate::FieldRef::new(
                    __logwise_field_metadata,
                    $crate::ValueRef::from($value),
                ))
            } else {
                None
            }
        }]
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __logwise_structured {
    ($domain:expr; $class:ident, $severity:ident, $kind:ident, $name:literal) => {
        $crate::__logwise_structured!($domain; $class, $severity, $kind, $name,)
    };
    ($domain:expr; $class:ident, $severity:ident, $kind:ident, $name:literal, $($fields:tt)*) => {{
        static __LOGWISE_FIELDS: &[$crate::FieldMetadata] =
            $crate::__logwise_field_metadata!(@accum [] $($fields)*);
        static __LOGWISE_METADATA: $crate::Metadata = $crate::Metadata {
            event_name: $name,
            package: env!("CARGO_PKG_NAME"),
            target: env!("CARGO_CRATE_NAME"),
            module: module_path!(),
            domain: $domain,
            severity: $crate::__logwise_severity!($severity),
            class: $crate::__logwise_class!($class),
            kind: $crate::Kind::$kind,
            location: Some($crate::Location::new(file!(), line!(), column!())),
            fields: __LOGWISE_FIELDS,
        };
        static __LOGWISE_CALLSITE: $crate::Callsite =
            $crate::Callsite::new(&__LOGWISE_METADATA);

        let __logwise_cached_interest = __LOGWISE_CALLSITE.interest();
        if __logwise_cached_interest.any() {
            let __logwise_context = $crate::context::capture();
            let __logwise_interest = __LOGWISE_CALLSITE.contextual_interest(
                __logwise_cached_interest,
                __logwise_context,
            );
            if __logwise_interest.any() {
                let mut __logwise_metadata = __LOGWISE_FIELDS.iter();
                let __logwise_fields = $crate::__logwise_field_values!(
                    @accum [] __logwise_metadata __logwise_interest; $($fields)*
                );
                __LOGWISE_CALLSITE.emit($crate::EventRef::structured(
                    &__LOGWISE_METADATA,
                    __logwise_context,
                    &__logwise_fields,
                ));
            }
        }
    }};
}

/// Emits a private, schema-unstable ad-hoc diagnostic using Rust formatting.
#[macro_export]
macro_rules! log {
    ($(#[$site_attr:meta])+ $($args:tt)+) => {{
        $(#[$site_attr])*
        { $crate::log!($($args)+) }
    }};
    ($($args:tt)+) => {{
        static __LOGWISE_FIELDS: &[$crate::FieldMetadata] = &[];
        static __LOGWISE_METADATA: $crate::Metadata = $crate::Metadata {
            event_name: "logwise.adhoc",
            package: env!("CARGO_PKG_NAME"),
            target: env!("CARGO_CRATE_NAME"),
            module: module_path!(),
            domain: None,
            severity: $crate::Severity::Debug,
            class: $crate::Class::Diagnostic,
            kind: $crate::Kind::AdHocText,
            location: Some($crate::Location::new(file!(), line!(), column!())),
            fields: __LOGWISE_FIELDS,
        };
        static __LOGWISE_CALLSITE: $crate::Callsite =
            $crate::Callsite::new(&__LOGWISE_METADATA);

        let __logwise_cached_interest = __LOGWISE_CALLSITE.interest();
        if __logwise_cached_interest.any() {
            let __logwise_context = $crate::context::capture();
            let __logwise_interest = __LOGWISE_CALLSITE.contextual_interest(
                __logwise_cached_interest,
                __logwise_context,
            );
            if __logwise_interest.wants($crate::Privacy::LocalOnly, $crate::Detail::Core) {
                __LOGWISE_CALLSITE.emit($crate::EventRef::text(
                    &__LOGWISE_METADATA,
                    __logwise_context,
                    core::format_args!($($args)+),
                ));
            }
        }
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! __logwise_span {
    ($timing:ident, $threshold:expr; $class:ident, $severity:ident, $name:literal) => {
        $crate::__logwise_span!($timing, $threshold; $class, $severity, $name,)
    };
    ($timing:ident, $threshold:expr; $class:ident, $severity:ident, $name:literal, $($fields:tt)*) => {{
        static __LOGWISE_FIELDS: &[$crate::FieldMetadata] =
            $crate::__logwise_field_metadata!(@accum [] $($fields)*);
        static __LOGWISE_METADATA: $crate::Metadata = $crate::Metadata {
            event_name: $name,
            package: env!("CARGO_PKG_NAME"),
            target: env!("CARGO_CRATE_NAME"),
            module: module_path!(),
            domain: None,
            severity: $crate::__logwise_severity!($severity),
            class: $crate::__logwise_class!($class),
            kind: $crate::Kind::Span,
            location: Some($crate::Location::new(file!(), line!(), column!())),
            fields: __LOGWISE_FIELDS,
        };
        static __LOGWISE_CALLSITE: $crate::Callsite =
            $crate::Callsite::new(&__LOGWISE_METADATA);

        let __logwise_cached_interest = __LOGWISE_CALLSITE.interest();
        if __logwise_cached_interest.any() {
            let __logwise_context = $crate::context::capture();
            let __logwise_interest = __LOGWISE_CALLSITE.contextual_interest(
                __logwise_cached_interest,
                __logwise_context,
            );
            if __logwise_interest.any() {
                let mut __logwise_metadata = __LOGWISE_FIELDS.iter();
                let __logwise_fields = $crate::__logwise_field_values!(
                    @accum [] __logwise_metadata __logwise_interest; $($fields)*
                );
                __LOGWISE_CALLSITE.start_span($crate::SpanRef {
                    event: $crate::EventRef::structured(
                        &__LOGWISE_METADATA,
                        __logwise_context,
                        &__logwise_fields,
                    ),
                    timing: $crate::SpanTiming::$timing,
                    warning_threshold: $threshold,
                })
            } else {
                $crate::SpanGuard::disabled()
            }
        } else {
            $crate::SpanGuard::disabled()
        }
    }};
}

/// Emits a stable structured event.
#[macro_export]
macro_rules! event {
    ($(#[$site_attr:meta])+ $($event:tt)+) => {{
        $(#[$site_attr])*
        { $crate::event!($($event)+) }
    }};
    (class: $class:ident, severity: $severity:ident, name: $name:literal) => {
        $crate::__logwise_structured!(None; $class, $severity, Event, $name)
    };
    (class: $class:ident, severity: $severity:ident, name: $name:literal, $($fields:tt)*) => {
        $crate::__logwise_structured!(None; $class, $severity, Event, $name, $($fields)*)
    };
    (domain: $domain:expr, class: $class:ident, severity: $severity:ident, name: $name:literal) => {
        $crate::__logwise_structured!(Some($domain); $class, $severity, Event, $name)
    };
    (domain: $domain:expr, class: $class:ident, severity: $severity:ident, name: $name:literal, $($fields:tt)*) => {
        $crate::__logwise_structured!(Some($domain); $class, $severity, Event, $name, $($fields)*)
    };
    (domain: $domain:expr, name: $name:literal) => {
        $crate::__logwise_structured!(Some($domain); operational, info, Event, $name)
    };
    (domain: $domain:expr, name: $name:literal, $($fields:tt)*) => {
        $crate::__logwise_structured!(Some($domain); operational, info, Event, $name, $($fields)*)
    };
    ($name:literal) => {
        $crate::__logwise_structured!(None; operational, info, Event, $name)
    };
    ($name:literal, $($fields:tt)*) => {
        $crate::__logwise_structured!(None; operational, info, Event, $name, $($fields)*)
    };
}

/// Emits a stable forensic event with debug severity.
#[macro_export]
macro_rules! forensic {
    ($(#[$site_attr:meta])+ $($event:tt)+) => {{
        $(#[$site_attr])*
        { $crate::forensic!($($event)+) }
    }};
    ($name:literal) => {
        $crate::__logwise_structured!(None; forensic, debug, Event, $name)
    };
    ($name:literal, $($fields:tt)*) => {
        $crate::__logwise_structured!(None; forensic, debug, Event, $name, $($fields)*)
    };
}

/// Emits a structured span observation.
#[macro_export]
macro_rules! span {
    ($(#[$site_attr:meta])+ $($event:tt)+) => {{
        $(#[$site_attr])*
        { $crate::span!($($event)+) }
    }};
    (class: $class:ident, severity: $severity:ident, name: $name:literal) => {
        $crate::__logwise_span!(WallTime, None; $class, $severity, $name)
    };
    (class: $class:ident, severity: $severity:ident, name: $name:literal, $($fields:tt)*) => {
        $crate::__logwise_span!(WallTime, None; $class, $severity, $name, $($fields)*)
    };
    ($name:literal) => {
        $crate::__logwise_span!(WallTime, None; operational, info, $name)
    };
    ($name:literal, $($fields:tt)*) => {
        $crate::__logwise_span!(WallTime, None; operational, info, $name, $($fields)*)
    };
}

/// Measures time spent actively polling or executing work.
#[macro_export]
macro_rules! active_span {
    ($name:literal) => {
        $crate::__logwise_span!(ActiveTime, None; performance, debug, $name)
    };
    ($name:literal, $($fields:tt)*) => {
        $crate::__logwise_span!(ActiveTime, None; performance, debug, $name, $($fields)*)
    };
}

/// Measures time from a wake signal to the next poll.
#[macro_export]
macro_rules! wake_latency_span {
    ($name:literal) => {
        $crate::__logwise_span!(WakeLatency, None; performance, debug, $name)
    };
    ($name:literal, $($fields:tt)*) => {
        $crate::__logwise_span!(WakeLatency, None; performance, debug, $name, $($fields)*)
    };
}

/// Starts a performance span that records a threshold violation on completion.
#[macro_export]
macro_rules! perfwarn {
    (threshold: $threshold:expr, name: $name:literal) => {
        $crate::__logwise_span!(WallTime, Some($threshold); performance, warn, $name)
    };
    (threshold: $threshold:expr, name: $name:literal, $($fields:tt)*) => {
        $crate::__logwise_span!(WallTime, Some($threshold); performance, warn, $name, $($fields)*)
    };
}

/// Starts an unthresholded performance profile span.
#[macro_export]
macro_rules! profile {
    ($name:literal) => {
        $crate::__logwise_span!(WallTime, None; performance, debug, $name)
    };
    ($name:literal, $($fields:tt)*) => {
        $crate::__logwise_span!(WallTime, None; performance, debug, $name, $($fields)*)
    };
}

/// Emits a structured counter observation.
#[macro_export]
macro_rules! counter {
    ($(#[$site_attr:meta])+ $($event:tt)+) => {{
        $(#[$site_attr])*
        { $crate::counter!($($event)+) }
    }};
    ($name:literal) => {
        $crate::__logwise_structured!(None; metric, info, Counter, $name)
    };
    ($name:literal, $($fields:tt)*) => {
        $crate::__logwise_structured!(None; metric, info, Counter, $name, $($fields)*)
    };
}

/// Emits a structured measurement observation.
#[macro_export]
macro_rules! measurement {
    ($(#[$site_attr:meta])+ $($event:tt)+) => {{
        $(#[$site_attr])*
        { $crate::measurement!($($event)+) }
    }};
    ($name:literal) => {
        $crate::__logwise_structured!(None; metric, info, Measurement, $name)
    };
    ($name:literal, $($fields:tt)*) => {
        $crate::__logwise_structured!(None; metric, info, Measurement, $name, $($fields)*)
    };
}

/// Deprecated migration alias for `log!`.
#[deprecated(note = "use logwise::log!; dispatch is synchronous")]
#[macro_export]
macro_rules! mandatory_sync {
    ($($args:tt)*) => { $crate::log!($($args)*) };
}

/// Deprecated migration alias for `log!`.
#[deprecated(note = "use logwise::log!; dispatch is synchronous")]
#[macro_export]
macro_rules! mandatory_async {
    ($($args:tt)*) => { $crate::log!($($args)*) };
}
