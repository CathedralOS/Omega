//! Exact indexed mutation of the supplied mutable byte view.

use super::{LoweringError, lower_machine};
use crate::TerminalMachineSelection;
use checked_trees::{CheckedScalarExpression, CheckedUnitEffectOperationPlan};
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
pub(super) const PUT: &str = r#"
    machine put(out: &mut [u8], byte: u8) {
        transition out.len > 0 {
            true -> store(out, byte)
            false -> done()
        }
        state store(out: &mut [u8], byte: u8) { out[0] = byte; }
        state done() {}
    }
"#;

#[test]
fn byte_view_write_rejects_changed_source_operands_access_and_roster() {
    let checked = crate::front_end::checked_program(
        r#"
        machine put(out: &mut [u8], other: &mut [u8], byte: u8) {
            transition out.len > 0 {
                true -> store(out, other, byte)
                false -> done()
            }
            state store(out: &mut [u8], other: &mut [u8], byte: u8) {
                out[0] = byte;
                out[0] = byte;
            }
            state done() {}
        }
    "#,
    );
    let _artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("put"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("lawful two-write source publishes first")
    .into_artifact();
    let plan_index = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .position(|plan| {
            plan.states.iter().any(|state| {
                state.operations.iter().any(|operation| {
                    matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
                    )
                })
            })
        })
        .unwrap();
    let state_index = checked.facts.flow.terminal_unit_effects.composed_machines[plan_index]
        .states
        .iter()
        .position(|state| state.operations.len() == 2)
        .unwrap();
    for mutation in 0..7 {
        let mut changed = checked.clone();
        let state = &mut changed.facts.flow.terminal_unit_effects.composed_machines[plan_index]
            .states[state_index];
        match mutation {
            0 => {
                let CheckedUnitEffectOperationPlan::ByteSequenceWrite(write) =
                    &mut state.operations[0]
                else {
                    panic!("write")
                };
                write.destination = checked_trees::CheckedStoreRoot::Parameter { position: 1 };
            }
            1 => {
                let CheckedUnitEffectOperationPlan::ByteSequenceWrite(first) =
                    &mut state.operations[0]
                else {
                    panic!("write")
                };
                first.index = CheckedScalarExpression::IntegerLiteral {
                    literal: numerics::literals::IntegerLiteral::from_value(1).with_landing(
                        numerics::literals::IntegerLanding {
                            landed_type: numerics::literals::LandedIntegerType::U64,
                            domain: numerics::arithmetic::ArithmeticDomain::Exact,
                        },
                    ),
                };
            }
            2 => {
                state.structural_parameters[0].access =
                    checked_trees::CheckedStructuralAccess::SharedBorrow
            }
            3 => state.operations.push(state.operations[0].clone()),
            4 => {
                state.operations.remove(0);
            }
            5 => state.operations.swap(0, 1),
            6 => {
                let CheckedUnitEffectOperationPlan::ByteSequenceWrite(write) =
                    &mut state.operations[0]
                else {
                    panic!("write")
                };
                write.value = checked_trees::CheckedStoreValue::Pure(
                    CheckedScalarExpression::IntegerLiteral {
                        literal: numerics::literals::IntegerLiteral::from_value(0).with_landing(
                            numerics::literals::IntegerLanding {
                                landed_type: numerics::literals::LandedIntegerType::U8,
                                domain: numerics::arithmetic::ArithmeticDomain::Exact,
                            },
                        ),
                    },
                );
            }
            _ => unreachable!(),
        }
        assert!(
            lower_machine(&changed, TerminalMachineSelection::Name("put")).is_err(),
            "source custody mutation {mutation}"
        );
    }
}

/// Byte leaves share the scalar store vocabulary but lower no computation:
/// replay refuses a byte-view write whose value names one, in the leaf's own
/// wording, instead of treating it as a pure byte.
#[test]
fn byte_view_write_refuses_a_computation_value() {
    let mut checked = crate::front_end::checked_program(PUT);
    lower_machine(&checked, TerminalMachineSelection::Name("put"))
        .expect("untampered byte-view write lowers");
    let write = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter_mut()
        .flat_map(|plan| &mut plan.states)
        .flat_map(|state| &mut state.operations)
        .find_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::ByteSequenceWrite(write) => Some(write),
            _ => None,
        })
        .expect("byte-view write");
    write.value = checked_trees::CheckedStoreValue::Computation(Default::default());
    let error = lower_machine(&checked, TerminalMachineSelection::Name("put"))
        .expect_err("a byte-view write has no computation lowering");
    assert!(
        matches!(
            error,
            LoweringError::Unsupported("byte-view write kept a computation value")
        ),
        "{error:?}"
    );
}

/// An invalid local cannot replace an authored borrowed parameter, even though
/// exact mutable bounded-owned local destinations now have a lowering.
#[test]
fn byte_view_write_refuses_a_local_root() {
    let mut checked = crate::front_end::checked_program(PUT);
    let write = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter_mut()
        .flat_map(|plan| &mut plan.states)
        .flat_map(|state| &mut state.operations)
        .find_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::ByteSequenceWrite(write) => Some(write),
            _ => None,
        })
        .expect("byte-view write");
    write.destination = checked_trees::CheckedStoreRoot::Local {
        symbol: symbols::SymbolHandle::invalid(),
    };
    let error = lower_machine(&checked, TerminalMachineSelection::Name("put"))
        .expect_err("a byte view has no local root");
    assert!(
        matches!(
            error,
            LoweringError::Unsupported("byte write local differs from its authored collection")
        ),
        "{error:?}"
    );
}

#[test]
fn owned_byte_write_retains_utf8_without_freezing_incidental_no_nul() {
    let checked = crate::front_end::checked_program(
        r#"
        domain [u8; 8]::Utf8 requires valid_utf8(self);
        machine store() {
            transition { _ -> render() }
            state render() {
                let mut line: [u8; 8] in Utf8 = "ab";
                line[0] = 0;
                line[1] = 65;
            }
        }
    "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("store"))
        .expect("an ASCII byte, including NUL, preserves the declared Utf8 domain");
    let writes = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::ByteSequenceWrite { .. }
            )
        })
        .count();
    assert_eq!(writes, 2);
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independent replay preserves the actual declared invariant");
    let mut changed = checked.clone();
    let qualifications = changed
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter_mut()
        .flat_map(|plan| &mut plan.states)
        .flat_map(|state| &mut state.operations)
        .find_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::EstablishByteSequence { qualifications, .. } => {
                Some(qualifications)
            }
            _ => None,
        })
        .expect("owned literal declaration");
    qualifications.clear();
    assert!(matches!(
        lower_machine(&changed, TerminalMachineSelection::Name("store")),
        Err(LoweringError::Unsupported(
            "literal local changed its authored qualifications"
        ))
    ));
}

#[test]
fn owned_byte_write_guard_observes_live_length_after_mutation() {
    let checked = crate::front_end::checked_program(
        r#"
        domain [u8; 8]::Utf8 requires valid_utf8(self);
        machine store() {
            transition { _ -> render() }
            state render() {
                let original: [u8; 8] in Utf8 = "ab";
                let mut line: [u8; 8] in Utf8 = "ab";
                line[0] = 0;
                transition line.len == 2 && line[0] == 0 && original[0] == 97 {
                    true -> good()
                    false -> bad()
                }
            }
            state good() {}
            state bad() {}
        }
    "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("store"))
        .expect("a bounded owner's guard observes its live length after mutation");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("owned length guard replay");
}

#[test]
fn owned_byte_write_reads_dynamic_index_from_another_qualified_local() {
    let checked = crate::front_end::checked_program(
        r#"
        domain [u8; 8]::Utf8 requires valid_utf8(self);
        domain [u8; 10]::Utf8 requires valid_utf8(self);
        machine store(index: u64) {
            transition { _ -> render(index) }
            state render(index: u64) {
                let digits: [u8; 10] in Utf8 = "0123456789";
                let mut line: [u8; 8] in Utf8 = "ab";
                let position: u64 = index % 10;
                line[1] = digits[position];
            }
        }
    "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("store"))
        .expect("a checked byte-table observation feeds the qualified owned store");
    assert!(
        lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(
                operation.kind,
                terminal_psi::OperationKind::ByteSequenceRead { .. }
            ))
    );
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("live bound and byte class replay");
}

/// `put` writes through a bare `&mut [u8]`, so it may store bytes that break a
/// caller field's declared predicate. Terminal opens no invariant window on a
/// field, so lending a `Utf8` field to that view is refused at the exact call
/// argument (`wiki/spec/terminal-psi/structural_access.md`: "passing a
/// restricted field to a bare mutable byte view is rejected"), whatever the
/// field held before the call.
#[test]
fn a_restricted_field_cannot_be_lent_to_a_bare_mutable_byte_helper() {
    for initial in ["old", ""] {
        let checked = crate::front_end::checked_program(&format!(
            r#"
        {PUT}
        domain [u8; 3]::Utf8 requires valid_utf8(self);
        data Record {{ out: [u8; 3] in Utf8; other: [u8; 3] in Utf8; }}
        machine Record::run(&mut self) {{
            self.out = "{initial}";
            self.other = "QQ";
            put(&mut self.out, 65);
            put(&mut self.out, 0);
            self.out = "ok";
        }}
    "#
        ));
        let error = lower_machine(&checked, TerminalMachineSelection::Name("Record::run"))
            .expect_err("the restricted field cannot enter a bare mutable view");
        assert!(
            matches!(
                error,
                LoweringError::InvalidTerminalModule(
                    terminal_verifier::ModuleError::InvalidStructuralArgumentPath {
                        argument_index: 0,
                        ..
                    }
                )
            ),
            "{initial:?}: {error:?}"
        );
    }
}

#[test]
fn byte_view_write_resolves_owned_local_element_value() {
    // The dense assignment namespace an entry-body byte write resolves its
    // index and value through carries established structural locals: `arr[k]`
    // roots at `CheckedStorageRoot::OwnedLocal` and once died in bindings
    // evaluation ("owned local observation lost its established place"). The
    // machine's `requires` now reaches the write's reconstructed proof site,
    // so the entry body lowers its one byte write and verifies independently.
    let checked = crate::front_end::checked_program(
        r#"
        machine store(target: &mut [u8], i: u64, k: u64)
        requires i < target.len && k < 8
        {
            let arr: [u8; 8] = [9, 8, 7, 6, 5, 4, 3, 2];
            target[i] = arr[k];
        }
    "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("store"))
        .expect("the guarded entry-body write lowers through its requires premise");
    let writes = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::ByteSequenceWrite { .. }
            )
        })
        .count();
    assert_eq!(writes, 1);
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independent replay discharges the write's bound from the requires premise");
}
