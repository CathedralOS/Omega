use super::{artifact, execute, integer, source};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use terminal_codec::{decode_module, decode_proof_bundle};
use terminal_interpreter::TerminalExecutionStatus;

#[test]
fn scalar_wrapper_signature_and_parameter_range_custody_reject_mutations() {
    let source = source()
        .replace(
            "Scalar::measure() ->",
            "Scalar::measure(value: i32 [1..=100]) ->",
        )
        .replace("Host::measure(70)", "Host::measure(value)")
        .replace("Scalar::measure();", "Scalar::measure(70);");
    let original = crate::front_end::checked_program(&source);
    for mutation in 0..4 {
        let mut checked = original.clone();
        let plan = &mut checked.facts.flow.terminal_boundary_scalar_returns.machines[0];
        match mutation {
            0 => plan.scalar_parameters.clear(),
            1 => plan.scalar_parameters[0].source_position = 1,
            2 => plan.scalar_parameters[0].primitive_type = typed_trees::types::PrimitiveType::Bool,
            3 => {
                let machine = plan.machine;
                let contract = checked
                    .facts
                    .contract_plans
                    .machines
                    .iter_mut()
                    .find(|contract| contract.machine == machine)
                    .unwrap();
                contract.closed_scalar_values = checked_trees::ClosedScalarValueContractPlan::new(
                    Vec::new(),
                    contract.closed_scalar_values.ensures().to_vec(),
                    contract.closed_scalar_values.has_crash_clauses(),
                    contract.closed_scalar_values.has_outcome_specific_clauses(),
                );
            }
            _ => unreachable!(),
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &checked,
                TerminalMachineSelection::Name("Scalar::measure")
            )
            .is_err(),
            "mutation {mutation}"
        );
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &checked,
                TerminalMachineSelection::Name("Main::main")
            )
            .is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn wrapper_boundary_and_direct_boundary_share_one_exact_declaration() {
    let source = source().replace(
        "Host::finish(result);",
        "let direct: i32 = Host::measure(21);\n Host::finish(result);\n Host::finish(direct);",
    );
    let artifact = artifact(&crate::front_end::checked_program(&source));
    assert_eq!(
        decode_module(&artifact.0).unwrap().boundary_machines.len(),
        2
    );
    let (_, observed) = execute(&artifact);
    assert_eq!(
        observed.arguments,
        [
            vec![integer(70)],
            vec![integer(21)],
            vec![integer(70)],
            vec![integer(21)]
        ]
    );
}

#[test]
fn scalar_wrapper_registration_and_result_drift_reject() {
    for mutation in 0..4 {
        let mut checked = crate::front_end::checked_program(&source());
        let plans = &mut checked.facts.flow.terminal_boundary_scalar_returns;
        match mutation {
            0 => plans.machines.push(plans.machines[0].clone()),
            1 => plans.machines[0].result_type = typed_trees::types::PrimitiveType::Bool,
            2 => plans.boundary_machines.clear(),
            3 => plans.machines[0].attachment_type_identity = "named(name(Main))".into(),
            _ => unreachable!(),
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &checked,
                TerminalMachineSelection::Name("Main::main")
            )
            .is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn selected_wrapper_type_duplicates_and_cross_owner_conflicts_reject() {
    for duplicate in [false, true] {
        let mut checked = crate::front_end::checked_program(&source());
        let declaration = checked
            .facts
            .flow
            .terminal_boundary_scalar_returns
            .structural_types
            .iter()
            .find(|plan| plan.identity == "named(name(Scalar))")
            .unwrap()
            .clone();
        if duplicate {
            checked
                .facts
                .flow
                .terminal_boundary_scalar_returns
                .structural_types
                .push(declaration);
        } else {
            let mut conflicting = declaration;
            conflicting.shape = checked_trees::CheckedUnitStructuralTypeShape::PrimitiveScalar(
                typed_trees::types::PrimitiveType::Bool,
            );
            checked
                .facts
                .flow
                .terminal_unit_effects
                .structural_types
                .push(conflicting);
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &checked,
                TerminalMachineSelection::Name("Main::main")
            )
            .is_err()
        );
    }
}

#[test]
fn wrapper_operand_crash_preserves_call_ceiling_and_prevents_boundary_effects() {
    let source = format!(
        "machine abort() -> i32 crashes Abort {{ crash Abort; }}\n{}",
        source()
            .replace("Host::measure(70)", "Host::measure(abort())")
            .replace("reaches Host {", "reaches Host crashes Abort {")
            .replace("reaches Host\n{", "reaches Host\ncrashes Abort\n{")
    );
    for source in [
        source.clone(),
        source.replace(
            "let result: i32 = Host::measure(abort());\n            result",
            "Host::measure(abort())",
        ),
    ] {
        let artifact = artifact(&crate::front_end::checked_program(&source));
        let (status, observed) = execute(&artifact);
        let TerminalExecutionStatus::Crashed(crash) = status else {
            panic!("the nested helper must crash before invoking the boundary")
        };
        assert_eq!(crash.cause, terminal_psi::CrashCause::Abort);
        assert!(observed.arguments.is_empty());
        let mut module = decode_module(&artifact.0).unwrap();
        let proof = decode_proof_bundle(&artifact.1).unwrap();
        let entry = module
            .machines
            .iter_mut()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        for operation in entry
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
        {
            if let terminal_psi::OperationKind::Call {
                crash_continuations,
                ..
            }
            | terminal_psi::OperationKind::CallStructuralScalar {
                crash_continuations,
                ..
            } = &mut operation.kind
            {
                crash_continuations.clear();
            }
        }
        assert!(
            terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
                .is_err()
        );
    }
}
