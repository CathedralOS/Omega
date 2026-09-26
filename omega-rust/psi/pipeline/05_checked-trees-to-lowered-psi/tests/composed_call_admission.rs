//! A composed state's calls take the same admission an ordinary body's do.
//!
//! Each program here makes, inside a state of a multi-state machine, a call
//! shape the ordinary body already lowered and the old per-state admission
//! copy refused. Each one lowers, passes independent verification after both
//! codec roundtrips, and executes on every selected state.

use checked_trees::{CheckedStructuralAccess, CheckedUnitEffectOperationPlan};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_section};
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use terminal_interpreter::{
    TerminalEffect, TerminalExecutionResult, TerminalScalarValue, TerminalStructuralValue,
    interpret_terminal_artifact_measured,
};

/// Lower `root`, verify the decoded artifact independently, run it once per
/// entry argument and return each run's observed `Sink` values. `shape`
/// names the call a state of `root`'s composed plan must carry.
fn run(
    source: &str,
    root: &str,
    shape: fn(&CheckedUnitEffectOperationPlan) -> bool,
    arguments: &[bool],
) -> Vec<Vec<u128>> {
    let checked = crate::front_end::checked_program(source);
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .find(|plan| checked.typed.symbols.display_path(plan.machine, "::") == root)
        .unwrap_or_else(|| panic!("{root} is a composed state graph"));
    assert!(
        plan.states
            .iter()
            .flat_map(|state| &state.operations)
            .any(shape),
        "a state of {root} carries the call shape under test"
    );
    let lowered =
        checked_trees_to_lowered_psi::lower_machine(&checked, TerminalMachineSelection::Name(root))
            .expect("a composed state's call lowers through the shared admission");
    let semantic = encode_module(&lowered.semantic_module).unwrap();
    let evidence = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap();
    let module = decode_module(&semantic).unwrap();
    let proof = decode_proof_bundle(&evidence).unwrap();
    assert_eq!(module, lowered.semantic_module);
    assert_eq!(proof, lowered.proof_bundle);
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("independent verification after both codec roundtrips");
    // A receiver the entry borrows arrives as an opaque structural input.
    let receivers = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap()
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| TerminalStructuralValue {
            opaque_identity: 100 + index as u64,
            structural_type: parameter.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    arguments
        .iter()
        .map(|selected| {
            let execution = interpret_terminal_artifact_measured(
                &semantic,
                &evidence,
                &AdmissionProfile::default(),
                &[TerminalScalarValue::Boolean(*selected)],
                TerminalStructuralInputs {
                    arguments: &receivers,
                    ..Default::default()
                },
                &mut AcceptTerminalEffects,
            )
            .unwrap();
            assert_eq!(execution.value(), TerminalExecutionResult::Unit);
            execution
                .effects()
                .iter()
                .map(|effect| {
                    let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
                        panic!("observable sink effect");
                    };
                    let [TerminalScalarValue::Integer { value, .. }] = arguments.as_slice() else {
                        panic!("one sink value");
                    };
                    let semantic_vocabulary::IntegerValue::Unsigned(value) = value else {
                        panic!("unsigned sink value");
                    };
                    *value
                })
                .collect()
        })
        .collect()
}

/// A state lends a call result it bound to a local to a Unit call. The old
/// state copy routed every result operand of a Unit call to its linear-chain
/// check and refused this borrow.
#[test]
fn state_lends_a_bound_call_result_to_a_unit_call() {
    let source = r#"
        boundary trait Sink { machine finish(value: u16); }
        data Pair { left: u16; right: u16; }
        machine make(left: u16, right: u16) -> Pair { Pair { left: left, right: right } }
        machine show(pair: &Pair) reaches Sink {
            Sink::finish(pair.left);
            Sink::finish(pair.right);
        }
        data Main {}
        machine Main::main(selected: bool) reaches Sink {
            transition selected { true -> yes() _ -> no() }
            state yes() {
                let pair: Pair = make(1u16, 2u16);
                show(&pair);
            }
            state no() {
                let pair: Pair = make(3u16, 4u16);
                show(&pair);
            }
        }
    "#;
    let lends_result = |operation: &CheckedUnitEffectOperationPlan| {
        matches!(operation, CheckedUnitEffectOperationPlan::CallUnit { structural_arguments, .. }
        if structural_arguments.iter().any(|argument| {
            argument.source_structural_result_binding_ordinal().is_some()
                && argument.access == CheckedStructuralAccess::SharedBorrow
        }))
    };
    assert_eq!(
        run(source, "Main::main", lends_result, &[true, false]),
        [vec![1, 2], vec![3, 4]]
    );
}

/// A state moves an owned value its incoming edge transferred into a Unit
/// call that takes it whole. The old state copy admitted only borrowed Unit
/// operands.
#[test]
fn state_moves_an_owned_parameter_into_a_unit_call() {
    let source = r#"
        boundary trait Sink { machine finish(value: u16); }
        data Token { value: u16; }
        machine spend(token: Token) reaches Sink { Sink::finish(token.value); }
        data Main {}
        machine Main::main(selected: bool) reaches Sink {
            let token: Token = Token { value: 5u16 };
            transition selected { true -> yes(token) _ -> no(token) }
            state yes(token: Token) { spend(token); }
            state no(token: Token) { Sink::finish(9u16); }
        }
    "#;
    let moves_parameter = |operation: &CheckedUnitEffectOperationPlan| {
        matches!(operation, CheckedUnitEffectOperationPlan::CallUnit { structural_arguments, .. }
        if structural_arguments.iter().any(|argument| {
            argument.source_parameter_index().is_some()
                && argument.access == CheckedStructuralAccess::Owned
                && argument.path.is_empty()
        }))
    };
    assert_eq!(
        run(source, "Main::main", moves_parameter, &[true, false]),
        [vec![5], vec![9]]
    );
}
