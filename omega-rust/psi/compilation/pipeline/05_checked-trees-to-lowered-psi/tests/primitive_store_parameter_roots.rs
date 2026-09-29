//! A primitive store names its borrowed destination by authored parameter
//! position. A scalar formal before the destination moves that position away
//! from the destination's dense structural index, so every consumer must
//! resolve the authored position to the same parameter the dense index named.
//! A consumer that read the position as a dense index would write the other
//! borrowed parameter or find none.

use checked_trees::{CheckedStoreRoot, CheckedTrees, CheckedUnitEffectOperationPlan};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_interpreter::{
    AcceptTerminalEffects, TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalScalarValue, TerminalStructuralInputs, TerminalStructuralPrimitiveValue,
    TerminalStructuralValue,
};
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
};

fn unsigned(value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

/// Every primitive store's checked root, across ordinary and composed plans.
fn store_roots(checked: &CheckedTrees) -> Vec<CheckedStoreRoot> {
    let effects = &checked.facts.flow.terminal_unit_effects;
    effects
        .machines
        .iter()
        .flat_map(|plan| &plan.operations)
        .chain(
            effects
                .composed_machines
                .iter()
                .flat_map(|plan| &plan.states)
                .flat_map(|state| &state.operations),
        )
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { destination, .. } => {
                Some(*destination)
            }
            _ => None,
        })
        .collect()
}

/// Publish `enter`, run it with `first = 11`, `second = 22` and the given
/// scalar arguments, and return both borrowed referents afterwards.
fn run(checked: &CheckedTrees, arguments: &[TerminalScalarValue]) -> [TerminalScalarValue; 2] {
    let artifact = terminal_production::TerminalProductionRequest::new(
        checked,
        TerminalMachineSelection::Name("enter"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("the store publishes past its preceding formals")
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    let profile = proof_admission::AdmissionProfile::default();
    terminal_verifier::verify_module(&module, &proof, &profile).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert_eq!(entry.structural_parameters.len(), 2);
    let structural = entry
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| TerminalStructuralValue {
            opaque_identity: 70 + index as u64,
            structural_type: parameter.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let initial = [
        TerminalStructuralPrimitiveValue {
            argument_index: 0,
            value: unsigned(11),
        },
        TerminalStructuralPrimitiveValue {
            argument_index: 1,
            value: unsigned(22),
        },
    ];
    let mut execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &profile,
        arguments,
        TerminalStructuralInputs {
            arguments: &structural,
            primitive_values: &initial,
            ..Default::default()
        },
    )
    .unwrap();
    let mut meter = terminal_fuel::TerminalFuelMeter::with_allowance(0);
    for _ in 0..256 {
        match execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap()
        {
            TerminalExecutionStatus::Complete(result) => {
                assert_eq!(result, TerminalExecutionResult::Unit);
                let values = execution.structural_primitive_values();
                assert_eq!(values.len(), 2);
                assert_eq!(values[0].argument_index, 0);
                assert_eq!(values[1].argument_index, 1);
                return [values[0].value, values[1].value];
            }
            TerminalExecutionStatus::SponsorExhausted(_) => meter.replenish(1).unwrap(),
            other => panic!("unexpected execution status: {other:?}"),
        }
    }
    panic!("the store must complete within its fuel budget");
}

/// The ordinary Unit route: replay and the operation frame both resolve the
/// authored root, and execution writes exactly that parameter.
#[test]
fn a_primitive_store_after_a_scalar_formal_writes_its_authored_parameter() {
    for (target, position, expected) in [("first", 1, [7u128, 22]), ("second", 2, [11, 7])] {
        let checked = crate::front_end::checked_program(&format!(
            "machine enter(marker: u64, first: &mut u64, second: &mut u64) {{ {target} = marker; }}"
        ));
        assert_eq!(
            store_roots(&checked),
            [CheckedStoreRoot::Parameter { position }],
            "{target}: the checked root is the authored position; omissions: {:?}",
            checked.facts.flow.terminal_unit_effects.omissions
        );
        assert_eq!(
            run(&checked, &[unsigned(7)]),
            expected.map(unsigned),
            "{target}"
        );
    }
}

/// The state graph replays a primitive store's authored root before it emits
/// anything. Terminal has no block-parameter form for a `&mut` primitive, so
/// emission then refuses the state's parameters; this pins the replay: the
/// untampered root resolves to its own parameter and stops at that emission
/// wall, while the sibling's authored position fails replay against the
/// authored target.
#[test]
fn a_state_graph_primitive_store_replays_its_authored_parameter_after_a_scalar_formal() {
    for (target, position, sibling) in [("first", 1, 2), ("second", 2, 1)] {
        let mut checked = crate::front_end::checked_program(&format!(
            r#"
            machine enter(marker: u64, first: &mut u64, second: &mut u64) {{
                transition marker > 0 {{
                    true -> store(marker, first, second)
                    false -> done()
                }}
                state store(marker: u64, first: &mut u64, second: &mut u64) {{
                    {target} = marker;
                }}
                state done() {{}}
            }}
            "#
        ));
        assert_eq!(
            store_roots(&checked),
            [CheckedStoreRoot::Parameter { position }],
            "{target}: the checked root is the authored position; omissions: {:?}",
            checked.facts.flow.terminal_unit_effects.omissions
        );
        let error = checked_trees_to_lowered_psi::lower_machine(
            &checked,
            TerminalMachineSelection::Name("enter"),
        )
        .map(|_| ())
        .expect_err("a state's `&mut` primitive parameter stops emission");
        assert!(
            matches!(
                error,
                checked_trees_to_lowered_psi::LoweringError::Unsupported(
                    "a state's mutable primitive reference parameter has no Terminal block form"
                )
            ),
            "{target}: replay admits the authored root: {error:?}"
        );
        for state in &mut checked.facts.flow.terminal_unit_effects.composed_machines[0].states {
            for operation in &mut state.operations {
                if let CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                    destination, ..
                } = operation
                {
                    *destination = CheckedStoreRoot::Parameter { position: sibling };
                }
            }
        }
        let error = checked_trees_to_lowered_psi::lower_machine(
            &checked,
            TerminalMachineSelection::Name("enter"),
        )
        .map(|_| ())
        .expect_err("the sibling's root is not the authored target");
        assert!(
            matches!(
                error,
                checked_trees_to_lowered_psi::LoweringError::Unsupported(
                    "primitive store destination differs from its authored parameter"
                )
            ),
            "{target}: replay rejects the sibling root: {error:?}"
        );
    }
}
