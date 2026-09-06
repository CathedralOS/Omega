//! Field selection preserves non-observation and exact exclusive receiver access.

use super::{check_source, reject_source};

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
fn projected_write_only_receiver_scalar_result_depends_on_written_input() {
    for receiver in RECEIVERS {
        let source = source(
            receiver,
            &format!("let written: u16 = {receiver}.replace(replacement);"),
            "machine Record::replace(&write self, replacement: u16) -> u16 {
                 self.value = replacement;
                 replacement
             }",
        );
        check_source(&source).unwrap_or_else(|diagnostics| {
            panic!("an independent scalar result checks: {diagnostics:#?}\n{source}")
        });
    }
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
            check_source(&source).unwrap_or_else(|diagnostics| {
                panic!("bare attached fields retain their self root: {diagnostics:#?}\n{source}")
            });
        }
    }
}

#[test]
fn projected_write_only_receiver_callee_cannot_read_even_after_replacement() {
    for receiver in RECEIVERS {
        for replacement in ["", "self.value = 17;"] {
            let source = source(
                receiver,
                &format!("let written: u16 = {receiver}.replace();"),
                &format!(
                    "machine Record::replace(&write self) -> u16 {{
                         {replacement}
                         self.value
                     }}"
                ),
            );
            reject_source(
                &source,
                &[
                    "reads field `value` from write-only parameter `self`",
                    "never grants observation",
                ],
            );
        }
    }
}

#[test]
fn projected_write_only_receiver_explicit_scalar_argument_cannot_read() {
    for receiver in RECEIVERS {
        for (binding, result, returned) in [
            ("", "", ""),
            ("let written: u16 = ", "-> u16", "replacement"),
        ] {
            let source = source(
                receiver,
                &format!("{binding}{receiver}.replace({receiver}.value);"),
                &format!(
                    "machine Record::replace(&write self, replacement: u16) {result} {{
                         self.value = replacement;
                         {returned}
                     }}"
                ),
            );
            reject_source(
                &source,
                &[
                    "reads field `value` from write-only parameter",
                    "never grants observation",
                ],
            );
        }
    }
}

#[test]
fn projected_write_only_receiver_cannot_widen_to_shared_or_mutable() {
    for receiver in RECEIVERS {
        for access in ["&self", "&mut self"] {
            // Even an empty callee requires its declared readable access.
            let statement = source(
                receiver,
                &format!("{receiver}.replace();"),
                &format!("machine Record::replace({access}) {{}}"),
            );
            reject_source(&statement, &["calls through write-only parameter"]);
            let expression = source(
                receiver,
                &format!("let written: u16 = {receiver}.replace();"),
                &format!("machine Record::replace({access}) -> u16 {{ 17 }}"),
            );
            reject_source(&expression, &["write-only parameter", "observation"]);
        }
    }
}

#[test]
fn projected_write_only_receiver_cannot_overlap_explicit_primitive_subloan() {
    for receiver in RECEIVERS {
        for (binding, result, returned) in [("", "", ""), ("let written: u16 = ", "-> u16", "17")] {
            let source = source(
                receiver,
                &format!("{binding}{receiver}.replace(&write {receiver}.value);"),
                &format!(
                    "machine Record::replace(&write self, other: &write u16) {result} {{
                         {returned}
                     }}"
                ),
            );
            // The primitive field subloan is admitted independently; the call
            // must reject overlapping exclusive arguments even without stores.
            reject_source(
                &source,
                &[
                    "receives write-only",
                    "overlapping another argument in the same call",
                ],
            );
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

#[test]
fn same_spelled_foreign_write_only_method_cannot_authorize_projected_call() {
    for receiver in RECEIVERS {
        for foreign_first in [false, true] {
            let foreign = "data Foreign { value: u16; }
                           machine Foreign::replace(&write self) { self.value = 17; }";
            let observing = "machine Record::replace(&self) {}";
            let methods = if foreign_first {
                format!("{foreign}\n{observing}")
            } else {
                format!("{observing}\n{foreign}")
            };
            let source = source(receiver, &format!("{receiver}.replace();"), &methods);
            reject_source(&source, &["calls through write-only parameter"]);
        }
    }
}

#[test]
fn same_spelled_foreign_observing_method_does_not_block_projected_call() {
    for receiver in RECEIVERS {
        for foreign_first in [false, true] {
            let foreign = "data Foreign { value: u16; }
                           machine Foreign::replace(&self) { let prior: u16 = self.value; }";
            let replacing = "machine Record::replace(&write self) { self.value = 17; }";
            let methods = if foreign_first {
                format!("{foreign}\n{replacing}")
            } else {
                format!("{replacing}\n{foreign}")
            };
            let source = source(receiver, &format!("{receiver}.replace();"), &methods);
            check_source(&source).unwrap_or_else(|diagnostics| {
                panic!("only the exact attached target controls access: {diagnostics:#?}\n{source}")
            });
        }
    }
}
