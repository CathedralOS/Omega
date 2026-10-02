//! Unit-local refinements survive completed-value lowering and source-free replay.
//! The earlier integer local stays live across the float establishment boundary;
//! neither a new invariant nor a helper result may discard that prefix.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IeeeFloatValue, IntegerSign, IntegerType, IntegerValue};
use terminal_codec::{
    decode_module, decode_proof_section_for, encode_module, encode_proof_section,
};
use terminal_interpreter::{
    AcceptTerminalEffects, TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalScalarValue, TerminalStructuralInputs, TerminalStructuralPrimitiveValue,
    TerminalStructuralValue,
};
use terminal_psi::OperationKind;

fn source(initializer: &str, named_state: bool) -> String {
    let body = format!(
        "let earlier: i32 = 42;
         let value: f32 in Finite = {initializer};
         marker = earlier;
         output = value;"
    );
    let body = if named_state {
        format!(
            "transition {{ _ -> complete(output, marker) }}
             state complete(output: &mut f32, marker: &mut i32) {{ {body} }}"
        )
    } else {
        body
    };
    format!(
        "machine finite_value() -> f32 in Finite {{ 0.5 }}
         machine observe(output: &mut f32, marker: &mut i32) {{ {body} }}"
    )
}

fn lower(source: &str) -> lowered_psi::LoweredPsi {
    checked_trees_to_lowered_psi::lower_machine(
        &crate::front_end::checked_program(source),
        TerminalMachineSelection::Name("observe"),
    )
    .unwrap_or_else(|error| panic!("{source}: {error:#?}"))
}

fn verify_and_execute(lowered: &lowered_psi::LoweredPsi) {
    let semantic_bytes = encode_module(&lowered.semantic_module).unwrap();
    let proof_bytes =
        encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap();
    let module = decode_module(&semantic_bytes).expect("reload semantics without source");
    let proof =
        decode_proof_section_for(&module, &proof_bytes).expect("reload proof without source");
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("completed local refinement is independently proved");
    assert!(
        !module.scalar_block_invariants.is_empty(),
        "a Unit local's Finite qualification requires an establishment proof"
    );
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert_eq!(entry.structural_parameters.len(), 2);
    let arguments = entry
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| TerminalStructuralValue {
            opaque_identity: index as u64 + 1,
            structural_type: parameter.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let integer = |value| TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
        value: IntegerValue::Signed(value),
    };
    let initial = [
        TerminalStructuralPrimitiveValue {
            argument_index: 0,
            value: TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(0.0f32.to_bits())),
        },
        TerminalStructuralPrimitiveValue {
            argument_index: 1,
            value: integer(0),
        },
    ];
    let mut execution = TerminalExecution::start_artifact(
        &semantic_bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &arguments,
            primitive_values: &initial,
            ..Default::default()
        },
    )
    .expect("reloaded Unit entry starts");
    let mut meter = terminal_fuel::TerminalFuelMeter::with_allowance(1000);
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit),
    );
    let values = execution.structural_primitive_values();
    assert_eq!(values.len(), 2);
    assert_eq!(
        values
            .iter()
            .find(|value| value.argument_index == 0)
            .unwrap()
            .value,
        TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(0.5f32.to_bits())),
    );
    assert_eq!(
        values
            .iter()
            .find(|value| value.argument_index == 1)
            .unwrap()
            .value,
        integer(42),
        "the scalar prefix remains live after local completion",
    );
}

#[test]
fn finite_value_contracts_unit_locals_preserve_prefix_and_reload() {
    for initializer in ["0.5", "finite_value()"] {
        verify_and_execute(&lower(&source(initializer, false)));
    }
}

#[test]
fn finite_value_contracts_named_state_unit_locals_preserve_prefix_and_reload() {
    for initializer in ["0.5", "finite_value()"] {
        verify_and_execute(&lower(&source(initializer, true)));
    }
}

#[test]
fn finite_value_contracts_unit_local_replay_rejects_nonfinite_completion() {
    for named_state in [false, true] {
        let lowered = lower(&source("0.5", named_state));
        verify_and_execute(&lowered);
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut module = lowered.semantic_module.clone();
            let mut changed = 0;
            for operation in module
                .machines
                .iter_mut()
                .flat_map(|machine| &mut machine.blocks)
                .flat_map(|block| &mut block.operations)
            {
                if let OperationKind::IeeeFloatConstant { value: constant } = &mut operation.kind
                    && *constant == IeeeFloatValue::Binary32(0.5f32.to_bits())
                {
                    *constant = IeeeFloatValue::Binary32(value.to_bits());
                    changed += 1;
                }
            }
            assert_eq!(changed, 1, "replace exactly the local's completed value");
            let semantic_bytes = encode_module(&module).unwrap();
            let proof_bytes = encode_proof_section(&module, &lowered.proof_bundle).unwrap();
            let reloaded = decode_module(&semantic_bytes).unwrap();
            let proof = decode_proof_section_for(&reloaded, &proof_bytes).unwrap();
            assert!(
                terminal_verifier::verify_module(&reloaded, &proof, &AdmissionProfile::default())
                    .is_err(),
                "{value:?} cannot reuse a finite local's arrival evidence (named state: {named_state})",
            );
        }
    }
}

#[test]
fn finite_value_contracts_unit_local_cannot_erase_custom_domain_predicate() {
    let source = "domain f32::AboveOne(value) requires value > 1.0;
                  machine observe(output: &mut f32) {
                      let value: f32 in AboveOne = 0.5;
                      output = value;
                  }";
    if let Ok(checked) = crate::front_end::checked_program_result(source) {
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &checked,
                TerminalMachineSelection::Name("observe"),
            )
            .is_err(),
            "an empty Unit predicate roster cannot make a false domain vacuous",
        );
    }
}
