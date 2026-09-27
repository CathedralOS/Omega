use super::{
    AdmissionProfile, CheckedUnitEffectOperationPlan, FuelChargeSite, OperationKind,
    OperationResult, TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalFuelMeter, TerminalScalarValue, TerminalStructuralValue, Terminator, checked,
    decode_module, decode_proof_bundle, encode_module, encode_proof_section, lower_machine,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_interpreter::AcceptTerminalEffects;
use terminal_interpreter::TerminalStructuralInputs;

fn assert_chain(
    source: &str,
    name: &str,
    scalar_arguments: &[TerminalScalarValue],
    statements: &[usize],
    fuel: usize,
) {
    let checked = checked(source);
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| {
            plan.operations
                .iter()
                .filter(|operation| {
                    matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::StructuralCall { .. }
                    )
                })
                .count()
                == 2
        })
        .expect("checked result sequence");
    let scalar_bindings = plan
        .operations
        .iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, .. }
            | CheckedUnitEffectOperationPlan::ScalarCall { result, .. } => {
                Some(result.binding_ordinal)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        scalar_bindings,
        (0..scalar_bindings.len() as u32).collect::<Vec<_>>()
    );
    assert_eq!(
        plan.operations
            .iter()
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::StructuralCall { result, .. } =>
                    Some(result.binding_ordinal),
                _ => None,
            })
            .collect::<Vec<_>>(),
        [0, 1]
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name(name))
        .unwrap_or_else(|error| panic!("result chain lowers: {error:?}\n{source}"));
    let module = &lowered.semantic_module;
    let caller = &module.machines[0];
    let structural_calls = caller.blocks[0]
        .operations
        .iter()
        .filter(|operation| {
            matches!(
                operation.kind,
                OperationKind::CallStructuralWithScalarArguments { .. }
            )
        })
        .collect::<Vec<_>>();
    let [first, second] = structural_calls.as_slice() else {
        panic!("two structural producers")
    };
    let OperationResult::Structural(first_result) = &first.result else {
        unreachable!()
    };
    let OperationResult::Structural(second_result) = &second.result else {
        unreachable!()
    };
    assert_ne!(first_result.place, second_result.place);
    let OperationKind::CallStructuralWithScalarArguments {
        structural_arguments,
        ..
    } = &second.kind
    else {
        unreachable!()
    };
    assert_eq!(structural_arguments[0].place, first_result.place);
    assert!(structural_arguments[0].path.is_empty());
    let last = caller.blocks[0].operations.last().unwrap();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &last.kind
    else {
        panic!("Unit consumer")
    };
    assert_eq!(structural_arguments[0].place, second_result.place);
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &caller.blocks[0].terminator
    else {
        unreachable!()
    };
    assert!(trivial_affine_discards.is_empty());
    assert_eq!(
        lowered
            .source_call_occurrences
            .iter()
            .map(|occurrence| occurrence.statement_index)
            .collect::<Vec<_>>(),
        statements
    );
    assert!(
        lowered
            .source_call_occurrences
            .iter()
            .all(|occurrence| occurrence.call_ordinal == 0)
    );
    assert_execution(module, &lowered.proof_bundle, scalar_arguments, fuel);
}

pub(super) fn assert_execution(
    module: &terminal_psi::TerminalModule,
    proof_bundle: &terminal_psi::ProofBundle,
    scalar_arguments: &[TerminalScalarValue],
    fuel: usize,
) {
    let semantic = encode_module(module).unwrap();
    let proof = encode_proof_section(module, proof_bundle).unwrap();
    let decoded = decode_module(&semantic).unwrap();
    let decoded_proof = decode_proof_bundle(&proof).unwrap();
    let verified =
        terminal_verifier::verify_module(&decoded, &decoded_proof, &AdmissionProfile::default())
            .unwrap();
    let certificate =
        terminal_fixed_fuel::derive_fixed_entry_fuel(&verified, decoded.entry).unwrap();
    terminal_fixed_fuel::validate_fixed_entry_fuel(&verified, &certificate).unwrap();
    assert_eq!(certificate.ceiling_units(), fuel as u64);
    let caller = &module.machines[0];
    let structural_arguments = caller
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| TerminalStructuralValue {
            opaque_identity: 0xcafe + index as u64,
            structural_type: parameter.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        scalar_arguments,
        TerminalStructuralInputs {
            arguments: &structural_arguments,
            ..Default::default()
        },
    )
    .unwrap();
    let mut meter = TerminalFuelMeter::with_allowance(0);
    for _ in 0..fuel {
        let paused = execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap();
        assert!(matches!(
            paused,
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        let frontier = execution
            .live_affine_frontier()
            .cloned()
            .collect::<Vec<_>>();
        let units = meter.usage().total_units();
        assert_eq!(
            execution
                .resume(&mut meter, &mut AcceptTerminalEffects)
                .unwrap(),
            paused
        );
        assert_eq!(meter.usage().total_units(), units);
        assert_eq!(
            execution
                .live_affine_frontier()
                .cloned()
                .collect::<Vec<_>>(),
            frontier
        );
        meter.replenish(1).unwrap();
    }
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(meter.usage().total_units(), fuel as u64);
    for operation in &caller.blocks[0].operations {
        assert_eq!(
            meter
                .usage()
                .at(FuelChargeSite::Operation(operation.id))
                .unwrap()
                .units(),
            1
        );
    }
    assert!(execution.live_affine_frontier().next().is_none());
    assert!(execution.live_claim_frontier().next().is_none());
    assert!(execution.effects().is_empty());
}

#[test]
fn free_callers_chain_nested_arrays_and_generic_results() {
    for (declarations, identity) in [
        (
            "data Inner { number: u64; } data Outer { inner: Inner; count: u32; }",
            "Outer",
        ),
        ("data Entry { number: u64; }", "[Entry; 3]"),
        (
            "data Entry { number: u64; } data Buffer<T> { entries: [T; 3]; }",
            "Buffer<Entry>",
        ),
    ] {
        let source = format!(
            "{declarations}
            machine forward(value: {identity}) -> {identity} {{ value }}
            machine Main::consume(value: {identity}) {{}}
            machine caller(value: {identity}) {{
                let first: {identity} = forward(value);
                let second: {identity} = forward(first);
                Main::consume(second);
            }}"
        );
        assert_chain(&source, "caller", &[], &[0, 1, 2], 7);
    }
}
