use crate::tests::front_end::checked_program_result;

fn reject_source(source: &str) -> Vec<diagnostics::Diagnostic> {
    // Stop at source checking: absence of a Terminal executable plan cannot
    // establish rejection of an unauthorized receiver access.
    let diagnostics = match checked_program_result(source) {
        Ok(_) => panic!("receiver access must fail source checking"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        !diagnostics.is_empty(),
        "rejection must report a diagnostic"
    );
    diagnostics
}

fn reject_assignment(source: &str, root: &str) {
    let diagnostics = reject_source(source);
    let expected =
        format!("assignment cannot write `{root}` because it is not mutable in this state");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(&expected)),
        "expected assignment writability diagnostic `{expected}`: {diagnostics:#?}"
    );
}

#[test]
fn shared_receiver_rejects_overlapping_exclusive_argument() {
    for caller in [
        "machine invoke(value: &mut Pair) -> u64 { value.inspect(&mut value) }",
        "machine invoke(input: u64) -> u64 {
            let value: Pair = Pair { left: input, right: 7 };
            value.inspect(&mut value)
        }",
    ] {
        let diagnostics = reject_source(&format!(
            "data Pair {{ left: u64; right: u64; }}
         machine Pair::inspect(&self, other: &mut Pair) -> u64 {{ self.right }}
         {caller}",
        ));
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(
                    "receives shared receiver overlapping another argument in the same call"
                )),
            "exact receiver/argument incompatibility: {diagnostics:#?}"
        );
        assert!(
            !diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("LET-bound"))
        );
    }
}

#[test]
fn projected_receiver_and_live_slice_require_compatible_access() {
    for (receiver, accepted) in [("&self", true), ("&mut self", false)] {
        let source = format!(
            "data Reader {{ bytes: [u8; 4]; }}
             data Container {{ reader: Reader; }}
             machine Reader::observe({receiver}, bytes: &[u8]) -> u8 {{
                 transition bytes.len > 0 {{
                     true -> (bytes[0])
                     false -> 0
                 }}
             }}
             machine Container::check(&mut self) -> u8 {{
                 let view: &[u8] = self.reader.bytes.as_slice();
                 self.reader.observe(view)
             }}"
        );
        if accepted {
            checked_program_result(&source).expect("shared receiver and shared slice may overlap");
        } else {
            let diagnostics = reject_source(&source);
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(
                        "receives mutable receiver while local borrow `view` is still active"
                    )),
                "an unused exclusive receiver still conflicts with its field's shared view: {diagnostics:#?}"
            );
        }
    }
}

#[test]
fn shared_self_direct_field_write_rejects() {
    reject_assignment(
        r#"
            data Pair { prefix: u8; value: u16; }

            machine Pair::replace(&self) {
                self.value = 17;
            }
        "#,
        "value",
    );
}

#[test]
fn projected_mutable_receiver_accepts_a_distinct_derived_slice_argument() {
    checked_program_result(
        "data Reader { marker: u8; }
         data Container { reader: Reader; bytes: [u8; 4]; }
         machine Reader::observe(&mut self, bytes: &[u8]) -> u8 {
             self.marker = 7;
             transition bytes.len > 0 {
                 true -> (bytes[0])
                 false -> 0
             }
         }
         machine Container::check(&mut self) -> u8 {
             let view: &[u8] = self.bytes.as_slice();
             self.reader.observe(view)
         }",
    )
    .expect("a derived argument retains its source place without receiver-lineage exemptions");
}

#[test]
fn projected_mutable_receiver_rejects_an_overlapping_derived_slice_argument() {
    let diagnostics = reject_source(
        "data Reader { bytes: [u8; 4]; }
         data Container { reader: Reader; }
         machine Reader::observe(&mut self, bytes: &[u8]) -> u8 {
             transition bytes.len > 0 {
                 true -> (bytes[0])
                 false -> 0
             }
         }
         machine Container::check(&mut self) -> u8 {
             let view: &[u8] = self.reader.bytes.as_slice();
             self.reader.observe(view)
         }",
    );
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("receives mutable receiver overlapping another argument in the same call")),
        "the explicit argument must be compared with its original storage: {diagnostics:#?}"
    );
}

#[test]
fn shared_self_nested_field_write_rejects() {
    reject_assignment(
        r#"
            data Inner { value: u16; }
            data Outer { inner: Inner; }

            machine Outer::replace(&self) {
                self.inner.value = 17;
            }
        "#,
        "inner",
    );
}

#[test]
fn shared_self_literal_index_write_rejects() {
    reject_assignment(
        r#"
            data Inner { values: [u16; 2]; }
            data Outer { inner: Inner; }

            machine Outer::replace(&self) {
                self.inner.values[1] = 17;
            }
        "#,
        "inner",
    );
}

#[test]
fn mutable_self_direct_field_write_checks() {
    checked_program_result(
        r#"
            data Pair { prefix: u8; value: u16; }

            machine Pair::replace(&mut self) {
                self.value = 17;
            }
        "#,
    )
    .expect("mutable self permits direct field stores");
}

#[test]
fn mutable_self_nested_field_write_checks() {
    checked_program_result(
        r#"
            data Inner { value: u16; }
            data Outer { inner: Inner; }

            machine Outer::replace(&mut self) {
                self.inner.value = 17;
            }
        "#,
    )
    .expect("mutable self permits nested field stores");
}

#[test]
fn mutable_self_literal_index_write_checks() {
    checked_program_result(
        r#"
            data Inner { values: [u16; 2]; }
            data Outer { inner: Inner; }

            machine Outer::replace(&mut self) {
                self.inner.values[1] = 17;
            }
        "#,
    )
    .expect("mutable self permits literal-index stores through nested fields");
}

#[test]
fn write_only_parameter_direct_field_write_checks() {
    checked_program_result(
        r#"
            data Pair { prefix: u8; value: u16; }

            machine replace(destination: &write Pair) {
                destination.value = 17;
            }
        "#,
    )
    .expect("write-only parameter permits a non-observing primitive field store");
}

#[test]
fn write_only_parameter_nested_field_write_checks() {
    checked_program_result(
        r#"
            data Inner { value: u16; }
            data Outer { inner: Inner; }

            machine replace(destination: &write Outer) {
                destination.inner.value = 17;
            }
        "#,
    )
    .expect("write-only parameter permits an invariant-free nested primitive field store");
}

#[test]
fn write_only_parameter_literal_index_write_checks() {
    checked_program_result(
        r#"
            data Inner { values: [u16; 2]; }
            data Outer { inner: Inner; }

            machine replace(destination: &write Outer) {
                destination.inner.values[1] = 17;
            }
        "#,
    )
    .expect("write-only parameter permits an in-bounds literal primitive element store");
}

#[test]
fn write_only_parameter_field_read_rejects() {
    let diagnostics = reject_source(
        r#"
            data Pair { prefix: u8; value: u16; }

            machine observe(destination: &write Pair) {
                let prior: u16 = destination.value;
            }
        "#,
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("reads field `value` from write-only parameter `destination`")
                && diagnostic.message.contains("never grants observation")
        }),
        "expected write-only parameter observation diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn write_only_primitive_reads_remain_forbidden_below_scalar_operators() {
    for source in [
        "machine observe(value: &write u64, mask: u64) -> u64 { value ^ mask }",
        "machine observe(value: &write bool) -> bool { !value }",
    ] {
        let diagnostics = reject_source(source);
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("reads write-only parameter `value`")
                && diagnostic.message.contains("never observation")),
            "nested scalar read retains source access rejection: {diagnostics:#?}"
        );
    }
}

#[test]
fn write_only_parameter_literal_index_read_rejects() {
    let diagnostics = reject_source(
        r#"
            data Inner { values: [u16; 2]; }
            data Outer { inner: Inner; }

            machine copy(destination: &write Outer) {
                destination.inner.values[0] = destination.inner.values[1];
            }
        "#,
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("reads through index projection of write-only parameter `destination`")
                && diagnostic.message.contains("never observation")
        }),
        "expected write-only parameter index observation diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn shared_self_field_cannot_supply_mutable_call_argument() {
    let diagnostics = reject_source(
        r#"
            data Pair { prefix: u8; value: u16; }

            machine replace(value: &mut u16) { value = 17; }

            machine Pair::forward(&self) {
                replace(&mut self.value);
            }
        "#,
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("mutable argument `value`")
                && diagnostic.message.contains("is not writable in this state")
        }),
        "expected mutable argument writability diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn mutable_self_field_can_supply_mutable_call_argument() {
    checked_program_result(
        r#"
            data Pair { prefix: u8; value: u16; }

            machine replace(value: &mut u16) { value = 17; }

            machine Pair::forward(&mut self) {
                replace(&mut self.value);
            }
        "#,
    )
    .expect("mutable self may lend a primitive field to a mutable call");
}

#[test]
fn shared_state_self_cannot_inherit_mutable_entry_authority() {
    reject_assignment(
        r#"
            data Pair { prefix: u8; value: u16; }

            machine Pair::replace(&mut self) {
                transition { _ -> store() }

                state store(&self) {
                    self.value = 17;
                }
            }
        "#,
        "value",
    );
}

#[test]
fn absent_state_self_cannot_inherit_mutable_entry_authority() {
    reject_assignment(
        r#"
            data Pair { prefix: u8; value: u16; }

            machine Pair::replace(&mut self) {
                transition { _ -> store() }

                state store() {
                    self.value = 17;
                }
            }
        "#,
        "value",
    );
}

#[test]
fn mutable_state_self_field_write_checks() {
    checked_program_result(
        r#"
            data Pair { prefix: u8; value: u16; }

            machine Pair::replace(&mut self) {
                transition { _ -> store() }

                state store(&mut self) {
                    self.value = 17;
                }
            }
        "#,
    )
    .expect("the current state's explicit mutable self permits a field store");
}

#[test]
fn local_record_receiver_cannot_acquire_unauthorized_mutation() {
    for body in [
        "let cell: Cell = Cell { value: 17 }; cell.replace();",
        "let mut cell: Cell = Cell { value: 17 }; let view: &Cell = &cell; view.replace();",
    ] {
        let source = format!(
            "data Cell {{ value: u64; }}
             machine Cell::replace(&mut self) {{ self.value = 29; }}
             machine observe() {{ {body} }}"
        );
        reject_source(&source);
    }
}

#[test]
fn local_record_receiver_cannot_be_reused_after_owned_self_transfer() {
    let diagnostics = reject_source(
        "data Cell { value: u64; }
         machine Cell::consume(self) {}
         machine observe() {
             let cell: Cell = Cell { value: 17 };
             cell.consume();
             cell.consume();
         }",
    );
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("already transferred or consumed")),
        "owned receiver transfer must retire the original local: {diagnostics:#?}"
    );
}

#[test]
fn owned_record_child_cannot_be_transferred_twice() {
    let diagnostics = reject_source("data Inner { value: u64; } data Outer { first: Inner; second: Inner; }
        machine wrap() -> Outer { let child: Inner = Inner { value: 7 }; Outer { first: child, second: child } }");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("already transferred or consumed")),
        "{diagnostics:#?}"
    );
}

fn machine_named(checked: &checked_trees::CheckedTrees, name: &str) -> symbols::SymbolHandle {
    checked
        .machines()
        .iter()
        .find(|machine| {
            machine.name.as_str() == name || machine.name.as_str().ends_with(&format!("::{name}"))
        })
        .unwrap_or_else(|| panic!("missing machine `{name}`"))
        .symbol
}

fn indexed_shared_receiver_source(caller_access: &str) -> String {
    format!(
        "data Cell {{ value: u64; }}
         data Rack {{ cells: [Cell; 2]; }}
         machine Cell::get(&self) -> u64 {{ self.value }}
         machine Rack::run({caller_access} self) -> u64 {{ self.cells[1].get() }}"
    )
}

#[test]
fn mutable_self_literal_indexed_element_can_supply_shared_receiver() {
    for caller_access in ["&mut", "&"] {
        let checked = checked_program_result(&indexed_shared_receiver_source(caller_access))
            .expect("shared indexed receiver must check");
        let run = machine_named(&checked, "run");
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(run)
            .unwrap_or_else(|| {
                panic!("`{caller_access}` caller keeps a Unit plan for a shared indexed receiver")
            });
        let calls = plan
            .operations
            .iter()
            .filter_map(|operation| match operation {
                checked_trees::CheckedUnitEffectOperationPlan::ScalarCall {
                    structural_arguments,
                    ..
                } => Some(structural_arguments),
                _ => None,
            })
            .flatten()
            .collect::<Vec<_>>();
        let [receiver] = calls.as_slice() else {
            panic!("the indexed receiver call passes exactly one structural argument: {calls:#?}")
        };
        assert!(
            matches!(
                receiver.access,
                checked_trees::CheckedStructuralAccess::SharedBorrow
            ) && receiver.path.iter().any(|segment| matches!(
                segment,
                checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(1)
            )),
            "the receiver argument lends the literal-indexed element shared: {receiver:#?}"
        );
    }
}

#[test]
fn mutable_self_mixed_field_index_path_can_supply_shared_receiver() {
    let checked = checked_program_result(
        "data Cell { value: u64; }
         data Rack { cells: [Cell; 2]; }
         data Shelf { rack: Rack; }
         machine Cell::get(&self) -> u64 { self.value }
         machine Shelf::run(&mut self) -> u64 { self.rack.cells[0].get() }",
    )
    .expect("shared mixed-path receiver must check");
    let run = machine_named(&checked, "run");
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(run)
            .is_some(),
        "a mixed field/index shared receiver keeps the caller's Unit plan"
    );
}

fn omission_stage(
    checked: &checked_trees::CheckedTrees,
    name: &str,
) -> checked_trees::CheckedUnitPlanOmissionStage {
    let machine = machine_named(checked, name);
    let plans = &checked.facts.flow.terminal_unit_effects;
    assert!(
        plans.for_machine(machine).is_none(),
        "expected no Unit plan for `{name}`"
    );
    plans
        .omission_for_machine(machine)
        .unwrap_or_else(|| panic!("expected an omission row for `{name}`"))
        .stage
}

#[test]
fn explicit_shared_literal_indexed_argument_names_its_caller_in_the_call_operation() {
    let checked = checked_program_result(
        "data Cell { value: u64; }
         data Rack { cells: [Cell; 2]; }
         machine take(view: &Cell) -> u64 { view.value }
         machine Rack::run(&mut self) -> u64 { take(&self.cells[0]) }",
    )
    .unwrap_or_else(|diagnostics| panic!("source still checks: {diagnostics:#?}"));
    let plans = &checked.facts.flow.terminal_unit_effects;
    assert!(
        plans.for_machine(machine_named(&checked, "take")).is_some(),
        "the callee keeps its own Unit plan"
    );
    assert!(
        plans.for_machine(machine_named(&checked, "run")).is_some(),
        "a literal index segment is an exact element, so the caller is planned: {:?}",
        plans.omission_for_machine(machine_named(&checked, "run"))
    );
}

#[test]
fn explicit_shared_dynamic_indexed_argument_still_omits_caller_in_call_operation() {
    let checked = checked_program_result(
        "data Cell { value: u64; }
         data Rack { cells: [Cell; 2]; }
         machine take(view: &Cell) -> u64 { view.value }
         machine Rack::run(&mut self, i: u64 [0..=1]) -> u64 { take(&self.cells[i]) }",
    )
    .unwrap_or_else(|diagnostics| panic!("source still checks: {diagnostics:#?}"));
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine_named(&checked, "take"))
            .is_some(),
        "the callee keeps its own Unit plan"
    );
    let stage = omission_stage(&checked, "run");
    assert!(
        matches!(
            stage,
            checked_trees::CheckedUnitPlanOmissionStage::LocalConstruction { phase, .. }
                if phase.contains("call operation")
        ),
        "a runtime index has no checked path segment, so the caller still stops in call operation: {stage:?}"
    );
}

#[test]
fn local_indexed_receiver_still_omits_in_call_statement_shape() {
    let checked = checked_program_result(
        "data Cell { value: u64; }
         machine Cell::get(&self) -> u64 { self.value }
         machine run() -> u64 {
             let cells: [Cell; 2] = [Cell { value: 1 }, Cell { value: 2 }];
             cells[1].get()
         }",
    )
    .expect("local indexed receiver still checks at the source stage");
    let stage = omission_stage(&checked, "run");
    assert!(
        matches!(
            stage,
            checked_trees::CheckedUnitPlanOmissionStage::LocalConstruction { phase, .. }
                if phase.contains("call statement shape")
        ),
        "a literal index on a local-rooted receiver still stops in call statement shape: {stage:?}"
    );
}

#[test]
fn dynamic_indexed_parameter_receiver_stops_at_receiver_reconciliation() {
    let checked = checked_program_result(
        "data Cell { value: u64; }
         machine Cell::get(&self) -> u64 { self.value }
         machine run(cells: &[Cell; 2], i: u64 [0..=1]) -> u64 { cells[i].get() }",
    )
    .expect("dynamic indexed parameter receiver still checks at the source stage");
    let stage = omission_stage(&checked, "run");
    assert!(
        matches!(
            stage,
            checked_trees::CheckedUnitPlanOmissionStage::ReceiverReconciliation
        ),
        "a runtime index through a parameter receiver still drops at receiver reconciliation: {stage:?}"
    );
}

#[test]
fn literal_indexed_parameter_receiver_can_supply_shared_receiver() {
    let checked = checked_program_result(
        "data Cell { value: u64; }
         machine Cell::get(&self) -> u64 { self.value }
         machine run(cells: &[Cell; 2]) -> u64 { cells[1].get() }",
    )
    .expect("literal indexed parameter receiver still checks");
    let run = machine_named(&checked, "run");
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(run)
            .is_some(),
        "a literal index on a parameter-rooted receiver keeps the Unit plan"
    );
}

#[test]
fn dynamic_indexed_element_still_cannot_supply_shared_receiver() {
    let checked = checked_program_result(
        "data Cell { value: u64; }
         data Rack { cells: [Cell; 2]; }
         machine Cell::get(&self) -> u64 { self.value }
         machine Rack::run(&mut self, i: u64 [0..=1]) -> u64 { self.cells[i].get() }",
    )
    .expect("dynamic indexed receiver still checks at the source stage");
    let run = machine_named(&checked, "run");
    let plans = &checked.facts.flow.terminal_unit_effects;
    assert!(
        plans.for_machine(run).is_none() && plans.omission_for_machine(run).is_some(),
        "a runtime index stays outside the admitted receiver projection"
    );
}
