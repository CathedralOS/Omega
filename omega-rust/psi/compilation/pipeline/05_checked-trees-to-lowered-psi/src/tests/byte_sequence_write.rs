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
                write.destination_parameter_position = 1;
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
                write.value = checked_trees::CheckedByteSequenceStoreValue::Pure(
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
    // roots at `CheckedStorageRoot::OwnedLocal` and previously died in
    // bindings evaluation ("owned local observation lost its established
    // place"). Resolution now completes and the write mints; the remaining
    // decline is the preexisting bound-obligation wall — an entry body has no
    // dominating guard and a `requires` premise does not reach the write's
    // reconstructed proof site — tracked as its own item. Flip this to the
    // lowered operation once that wall lands.
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
    assert!(matches!(
        lower_machine(&checked, TerminalMachineSelection::Name("store")),
        Err(LoweringError::OperationProofUnavailable(_))
    ));
}
