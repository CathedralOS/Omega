use super::super::*;

mod indexed;
mod projections;

#[test]
fn indexed_write_only_receiver_call_checks_without_observing_the_element() {
    check_source(
        "data Record [copy] { value: u16; }
         machine Record::replace(&write self, replacement: u16) { self.value = replacement; }
         machine invoke(records: &write [Record; 2], replacement: u16) {
             records[0].replace(replacement);
         }",
    )
    .expect("fixed-array indexing selects a non-observing receiver address");
}

#[test]
fn projected_write_only_receiver_call_checks() {
    check_source(
        r#"
        data Record { value: u16; }
        data Container { record: Record; }
        machine Record::replace(&write self) { self.value = 17; }
        machine invoke(container: &write Container) { container.record.replace(); }
    "#,
    )
    .expect("a closed field projection can invoke an exact non-observing receiver");
}

fn check_source(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed)
}

fn caller_source(root: &str, body: &str, methods: &str) -> String {
    let signature = if root == "self" {
        "Record::exercise(&write self, replacement: u16)"
    } else {
        "exercise(destination: &write Record, replacement: u16)"
    };
    format!(
        "data Record {{ value: u16; }}
         {methods}
         machine {signature} {{ {body} }}"
    )
}

fn reject_source(source: &str, expected: &[&str]) {
    let diagnostics = match check_source(source) {
        Ok(_) => panic!("source must fail semantic checking: {source}"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics.iter().any(|diagnostic| {
            expected
                .iter()
                .all(|fragment| diagnostic.message.contains(fragment))
        }),
        "expected diagnostic containing {expected:?}: {diagnostics:#?}\nsource:\n{source}"
    );
}

#[test]
fn write_only_parameter_nonobserving_statement_call_checks() {
    let checked = check_source(&caller_source(
        "destination",
        "destination.replace(replacement);",
        "machine Record::replace(&write self, replacement: u16) {
             self.value = replacement;
         }",
    ))
    .expect("a direct write-only parameter may invoke a checked non-observing method");
    let caller = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "exercise")
        .unwrap();
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == caller.symbol)
        .expect("caller retains a Unit plan");
    let arguments = plan
        .operations
        .iter()
        .find_map(|operation| match operation {
            checked_trees::CheckedUnitEffectOperationPlan::CallUnit {
                structural_arguments,
                ..
            } => Some(structural_arguments),
            _ => None,
        })
        .expect("caller retains its method call");
    assert_eq!(
        arguments.len(),
        1,
        "the retained callee receiver needs one caller argument"
    );
}

#[test]
fn write_only_self_nonobserving_call_with_scalar_argument_checks() {
    check_source(&caller_source(
        "self",
        "self.replace(replacement);",
        "machine Record::replace(&write self, replacement: u16) {
             self.value = replacement;
         }",
    ))
    .expect("write-only self may forward an independently supplied scalar to a method");
}

#[test]
fn write_only_receiver_nonobserving_scalar_result_call_checks() {
    for root in ["destination", "self"] {
        let source = caller_source(
            root,
            &format!("let written: u16 = {root}.replace(replacement);"),
            "machine Record::replace(&write self, replacement: u16) -> u16 {
                 self.value = replacement;
                 replacement
             }",
        );
        check_source(&source).unwrap_or_else(|diagnostics| {
            panic!("a scalar result may depend on the written input: {diagnostics:#?}\n{source}")
        });
    }
}

fn reject_receiver_requirement(receiver: &str) {
    for root in ["destination", "self"] {
        // An empty body still requires the declared receiver authority.
        let source = caller_source(
            root,
            &format!("{root}.exercise_access();"),
            &format!("machine Record::exercise_access({receiver}) {{}}"),
        );
        reject_source(
            &source,
            &[&format!("calls through write-only parameter `{root}`")],
        );
        let source = caller_source(
            root,
            &format!("let result: u16 = {root}.exercise_access();"),
            &format!("machine Record::exercise_access({receiver}) -> u16 {{ 17 }}"),
        );
        reject_source(
            &source,
            &[
                &format!("reads write-only parameter `{root}`"),
                "never observation",
            ],
        );
    }
}

#[test]
fn write_only_receiver_cannot_satisfy_shared_receiver_requirement() {
    reject_receiver_requirement("&self");
}

#[test]
fn write_only_receiver_cannot_satisfy_mutable_receiver_requirement() {
    reject_receiver_requirement("&mut self");
}

#[test]
fn write_only_receiver_cannot_satisfy_owned_receiver_requirement() {
    reject_receiver_requirement("self");
}

#[test]
fn write_only_receiver_call_still_checks_argument_observation() {
    for root in ["destination", "self"] {
        for (body, result, returned) in [
            (format!("{root}.replace({root}.value);"), "", ""),
            (
                format!("let written: u16 = {root}.replace({root}.value);"),
                "-> u16",
                "replacement",
            ),
        ] {
            let source = caller_source(
                root,
                &body,
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
                    &format!("reads field `value` from write-only parameter `{root}`"),
                    "never grants observation",
                ],
            );
        }
    }
}

#[test]
fn write_only_receiver_cannot_overlap_an_explicit_exclusive_argument() {
    for root in ["destination", "self"] {
        for (body, result, returned) in [
            (format!("{root}.replace_both(&write {root});"), "", ""),
            (
                format!("let written: u16 = {root}.replace_both(&write {root});"),
                "-> u16",
                "17",
            ),
        ] {
            let source = caller_source(
                root,
                &body,
                &format!(
                    "machine Record::replace_both(&write self, other: &write Record) {result} {{
                         self.value = 17;
                         other.value = 23;
                         {returned}
                     }}"
                ),
            );
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
fn write_only_receiver_cannot_overlap_an_explicit_primitive_field_argument() {
    for root in ["destination", "self"] {
        for (body, result, returned) in [
            (format!("{root}.replace_both(&write {root}.value);"), "", ""),
            (
                format!("let written: u16 = {root}.replace_both(&write {root}.value);"),
                "-> u16",
                "17",
            ),
        ] {
            let source = caller_source(
                root,
                &body,
                &format!(
                    "machine Record::replace_both(&write self, other: &write u16) {result} {{
                         self.value = 17;
                         other = 23;
                         {returned}
                     }}"
                ),
            );
            // The primitive subloan is independently admitted. Rejection must
            // establish its overlap with the implicit whole-record receiver.
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
fn write_only_receiver_transitive_observing_helper_rejects() {
    for root in ["destination", "self"] {
        let source = caller_source(
            root,
            &format!("{root}.replace();"),
            "machine Record::replace(&write self) { self.helper(); }
             machine Record::helper(&write self) { self.observe(); }
             machine Record::observe(&write self) {
                 let prior: u16 = self.value;
             }",
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

#[test]
fn same_spelled_foreign_write_only_method_cannot_authorize_observing_call() {
    for root in ["destination", "self"] {
        for foreign_first in [true, false] {
            let foreign = "data Foreign { value: u16; }
                           machine Foreign::replace(&write self) { self.value = 17; }";
            let observing = "machine Record::replace(&self) {}";
            let methods = if foreign_first {
                format!("{foreign}\n{observing}")
            } else {
                format!("{observing}\n{foreign}")
            };
            let source = caller_source(root, &format!("{root}.replace();"), &methods);
            reject_source(
                &source,
                &[&format!("calls through write-only parameter `{root}`")],
            );
        }
    }
}

#[test]
fn same_spelled_foreign_observing_method_does_not_block_write_only_call() {
    for root in ["destination", "self"] {
        let source = caller_source(
            root,
            &format!("{root}.replace();"),
            "data Foreign { value: u16; }
             machine Foreign::replace(&self) { let prior: u16 = self.value; }
             machine Record::replace(&write self) { self.value = 17; }",
        );
        check_source(&source).unwrap_or_else(|diagnostics| {
            panic!("admission must use the exact attached method: {diagnostics:#?}\n{source}")
        });
    }
}

#[test]
fn write_only_receiver_and_distinct_projected_argument_check() {
    for (body, result, returned) in [
        ("destination.replace_both(&write other.value);", "", ""),
        (
            "let written: u16 = destination.replace_both(&write other.value);",
            "-> u16",
            "17",
        ),
    ] {
        let source = format!(
            "data Record {{ value: u16; }}
             machine Record::replace_both(&write self, other: &write u16) {result} {{
                 self.value = 17;
                 other = 23;
                 {returned}
             }}
             machine exercise(destination: &write Record, other: &write Record) {{ {body} }}"
        );
        check_source(&source)
            .expect("distinct receiver and argument retain independent exclusive loans");
    }
}

#[test]
fn write_only_receiver_cannot_overlap_a_live_local_loan_even_without_writes() {
    for root in ["destination", "self"] {
        let source = caller_source(
            root,
            &format!("let held: &write Record = &write {root}; {root}.noop(); held.value = 17;"),
            "machine Record::noop(&write self) {}",
        );
        reject_source(
            &source,
            &["write-only receiver", "local borrow `held` is still active"],
        );
    }
}

#[test]
fn shared_caller_cannot_supply_a_write_only_receiver() {
    for signature in ["Record::exercise(&self)", "exercise(destination: &Record)"] {
        let root = if signature.starts_with("Record::") {
            "self"
        } else {
            "destination"
        };
        let source = format!(
            "data Record {{ value: u16; }}
             machine Record::noop(&write self) {{}}
             machine {signature} {{ {root}.noop(); }}"
        );
        reject_source(&source, &["write-only receiver", "not writable"]);
    }
}

#[test]
fn mutable_caller_can_supply_a_write_only_receiver() {
    for signature in [
        "Record::exercise(&mut self)",
        "exercise(destination: &mut Record)",
    ] {
        let root = if signature.starts_with("Record::") {
            "self"
        } else {
            "destination"
        };
        let source = format!(
            "data Record {{ value: u16; }}
             machine Record::replace(&write self) {{ self.value = 17; }}
             machine {signature} {{ {root}.replace(); }}"
        );
        check_source(&source).expect("an exclusive caller may lend non-observing access");
    }
}
