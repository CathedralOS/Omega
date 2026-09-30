//! A composed state's mutable primitive local initialized by a call.
//!
//! `let mut total: u8 = echo(seed);` retains its initializer as the
//! computation rooted at the declaration's `StorageInitializer` coordinate.
//! The composed state establishes the local through the same frame emitter an
//! ordinary body uses; later reads name its storage. The controls substitute
//! or drop that initializer and require lowering to refuse the plan.

use checked_trees::{
    CheckedCallScalarArgument, CheckedScalarExpression, CheckedUnitEffectOperationPlan,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use terminal_codec::{encode_module, encode_proof_section};
use terminal_interpreter::{
    AcceptTerminalEffects, TerminalEffect, TerminalExecutionResult, TerminalScalarValue,
    TerminalStructuralInputs, interpret_terminal_artifact_measured,
};

const SOURCE: &str = r#"
    boundary trait Output { machine byte(value: u8) reaches Output; }
    machine echo(value: u8) -> u8 { value }
    data Root {}
    machine Root::enter() reaches Output {
        let mut seed: u8 = 7;
        let mut total: u8 = echo(seed);
        let first: u8 = echo(total);
        transition first == 7 && total == 7 {
            true -> good()
            false -> bad()
        }
        state good() { Output::byte(70); }
        state bad() { Output::byte(71); }
    }
"#;

fn lower(checked: &checked_trees::CheckedTrees) -> Result<lowered_psi::LoweredPsi, String> {
    checked_trees_to_lowered_psi::lower_machine(
        checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .map_err(|error| format!("{error:?}"))
}

fn emitted_bytes(lowered: &lowered_psi::LoweredPsi) -> Vec<u128> {
    let execution = interpret_terminal_artifact_measured(
        &encode_module(&lowered.semantic_module).unwrap(),
        &encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
        &mut AcceptTerminalEffects,
    )
    .unwrap();
    assert_eq!(execution.value(), TerminalExecutionResult::Unit);
    execution
        .effects()
        .iter()
        .map(|effect| {
            let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
                panic!("byte output");
            };
            let [TerminalScalarValue::Integer { value, .. }] = arguments.as_slice() else {
                panic!("one byte");
            };
            let semantic_vocabulary::IntegerValue::Unsigned(value) = value else {
                panic!("unsigned byte");
            };
            *value
        })
        .collect()
}

/// The one composed state operation roster holding the call-initialized
/// establishment, and that establishment's index in it.
fn call_initialized_local(
    checked: &mut checked_trees::CheckedTrees,
) -> (&mut Vec<CheckedUnitEffectOperationPlan>, usize) {
    let is_call_initialized = |operation: &CheckedUnitEffectOperationPlan| {
        matches!(
            operation,
            CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal {
                value: CheckedCallScalarArgument::Computation(_),
                ..
            }
        )
    };
    let mut rosters = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter_mut()
        .flat_map(|machine| machine.states.iter_mut())
        .map(|state| &mut state.operations)
        .filter(|operations| operations.iter().any(is_call_initialized));
    let operations = rosters.next().expect("call-initialized mutable local");
    assert!(
        rosters.next().is_none(),
        "one call-initialized mutable local"
    );
    let index = operations
        .iter()
        .position(is_call_initialized)
        .expect("selected establishment");
    (operations, index)
}

#[test]
fn composed_state_establishes_a_call_initialized_mutable_local() {
    let mut checked = crate::front_end::checked_program(SOURCE);
    let _ = call_initialized_local(&mut checked);
    let lowered = lower(&checked).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(emitted_bytes(&lowered), [70]);
}

#[test]
fn composed_call_initializer_substitution_or_omission_is_rejected() {
    let original = crate::front_end::checked_program(SOURCE);
    for (mutation, refusal) in [
        "substituted its initializer computation",
        "lost its exact initializer",
        "dropped or added a body effect",
    ]
    .into_iter()
    .enumerate()
    {
        let mut changed = original.clone();
        let computations = changed.facts.values.scalar_computations.clone();
        let (operations, index) = call_initialized_local(&mut changed);
        match mutation {
            // A different node of the same computation arena is not the
            // root checking retained at the declaration.
            0 => {
                let CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal {
                    value: CheckedCallScalarArgument::Computation(root),
                    ..
                } = &mut operations[index]
                else {
                    unreachable!("selected establishment");
                };
                *root = computations
                    .nodes
                    .iter()
                    .map(|(handle, _)| handle)
                    .find(|handle| handle != root)
                    .expect("another computation node");
            }
            // A pure literal naming the call's result value has no source
            // binding at the declaration: the call would silently vanish.
            1 => {
                let CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { value, .. } =
                    &mut operations[index]
                else {
                    unreachable!("selected establishment");
                };
                *value = CheckedCallScalarArgument::Pure(CheckedScalarExpression::IntegerLiteral {
                    literal: numerics::literals::IntegerLiteral::from_parts(
                        false,
                        numerics::literals::IntegerRadix::Decimal,
                        "7",
                    )
                    .unwrap(),
                });
            }
            // Dropping the establishment leaves the authored `let` and every
            // later read of its storage without an owner.
            2 => {
                operations.remove(index);
            }
            _ => unreachable!(),
        }
        let error = lower(&changed).expect_err("a changed call initializer must be refused");
        assert!(
            error.contains(refusal),
            "call initializer mutation {mutation} refused for another reason: {error}"
        );
    }
}
