//! Field selection preserves non-observation and exact exclusive receiver access.

use super::reject_source;
use crate::tests::front_end::checked_program_result;

const RECEIVERS: [&str; 4] = [
    "container.record",
    "container.inner.record",
    "self.record",
    "self.inner.record",
];

fn source(receiver: &str, body: &str, methods: &str) -> String {
    let signature = if !receiver.starts_with("container.") {
        "Container::exercise(&write self, replacement: u16)"
    } else {
        "exercise(container: &write Container, replacement: u16)"
    };
    format!(
        "data Record {{ value: u16; }}
         data Inner {{ record: Record; }}
         data Container {{ record: Record; inner: Inner; }}
         {methods}
         machine {signature} {{ {body} }}"
    )
}

#[test]
fn bare_attached_field_write_only_receiver_calls_check() {
    for receiver in ["record", "inner.record"] {
        for (binding, result, returned) in [
            ("", "", ""),
            ("let written: u16 = ", "-> u16", "replacement"),
        ] {
            let source = source(
                receiver,
                &format!("{binding}{receiver}.replace(replacement);"),
                &format!(
                    "machine Record::replace(&write self, replacement: u16) {result} {{
                         self.value = replacement;
                         {returned}
                     }}"
                ),
            );
            checked_program_result(&source).unwrap_or_else(|diagnostics| {
                panic!("bare attached fields retain their self root: {diagnostics:#?}\n{source}")
            });
        }
    }
}

#[test]
fn projected_write_only_receiver_cannot_overlap_live_whole_container_loan() {
    for receiver in RECEIVERS {
        let root = receiver.split('.').next().unwrap();
        for (binding, result, returned) in [("", "", ""), ("let written: u16 = ", "-> u16", "17")] {
            let source = source(
                receiver,
                &format!(
                    "let held: &write Container = &write {root};
                     {binding}{receiver}.noop();
                     held.record.value = 17;"
                ),
                &format!("machine Record::noop(&write self) {result} {{ {returned} }}"),
            );
            // Borrow the supported whole root, not an unsupported local projection.
            reject_source(
                &source,
                &["write-only receiver", "local borrow `held` is still active"],
            );
        }
    }
}
