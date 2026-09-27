use super::bind_thunk_machine_interface;

fn scalar_type(bits: u16) -> semantic_vocabulary::ScalarType {
    semantic_vocabulary::ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, bits)
            .expect("fixed-width integer type"),
    )
}

fn scalar_parameter(id: u64, bits: u16) -> terminal_psi::ValueDeclaration {
    terminal_psi::ValueDeclaration {
        id: semantic_vocabulary::ValueId::new(id).expect("nonzero value id"),
        scalar_type: scalar_type(bits),
        qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
    }
}

fn machine(
    parameters: Vec<terminal_psi::ValueDeclaration>,
    structural_parameters: Vec<terminal_psi::StructuralParameterDeclaration>,
    result: terminal_psi::TerminalMachineResult,
) -> terminal_psi::TerminalMachine {
    terminal_psi::TerminalMachine {
        id: semantic_vocabulary::MachineId::new(1).expect("nonzero machine id"),
        attachment: None,
        parameters,
        structural_parameters,
        ranked_scc: None,
        result,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        declared_service_reach: Vec::new(),
        closed_reach_application: None,
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: semantic_vocabulary::BlockId::new(1).expect("nonzero block id"),
        blocks: Vec::new(),
        contract: terminal_psi::MachineContract {
            id: semantic_vocabulary::ContractId::new(1).expect("nonzero contract id"),
            crash_routes: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    }
}

fn materialized(
    signature: calling_conventions::CallSignature,
) -> provider_planning::calling_policy_plans::MaterializedBoundarySignature {
    provider_planning::calling_policy_plans::materialized_boundary_signature_from_abi(&signature)
        .expect("materialized signature")
}

#[test]
fn thunk_interface_binds_a_multi_parameter_scalar_signature() {
    let signature = calling_conventions::CallSignature {
        parameters: vec![
            calling_conventions::ValueShape::integer(8, 8),
            calling_conventions::ValueShape::integer(1, 1),
        ],
        result: Some(calling_conventions::ValueShape::integer(2, 2)),
    };
    let machine = machine(
        vec![scalar_parameter(1, 64), scalar_parameter(2, 8)],
        Vec::new(),
        terminal_psi::TerminalMachineResult::Scalar(scalar_parameter(3, 16)),
    );
    bind_thunk_machine_interface(&machine, &materialized(signature))
        .expect("multi-parameter scalar signature binds");
}

#[test]
fn thunk_interface_binds_a_parameterless_unit_signature() {
    let signature = calling_conventions::CallSignature {
        parameters: Vec::new(),
        result: None,
    };
    let machine = machine(
        Vec::new(),
        Vec::new(),
        terminal_psi::TerminalMachineResult::Unit,
    );
    bind_thunk_machine_interface(&machine, &materialized(signature))
        .expect("parameterless Unit signature binds");
}

#[test]
fn thunk_interface_rejects_a_drifted_scalar_shape() {
    let signature = calling_conventions::CallSignature {
        parameters: vec![calling_conventions::ValueShape::integer(8, 8)],
        result: None,
    };
    let machine = machine(
        vec![scalar_parameter(1, 32)],
        Vec::new(),
        terminal_psi::TerminalMachineResult::Unit,
    );
    assert!(bind_thunk_machine_interface(&machine, &materialized(signature)).is_err());
}

#[test]
fn thunk_interface_rejects_extra_and_missing_machine_parameters() {
    let signature = || {
        materialized(calling_conventions::CallSignature {
            parameters: vec![calling_conventions::ValueShape::integer(8, 8)],
            result: None,
        })
    };
    let extra = machine(
        vec![scalar_parameter(1, 64), scalar_parameter(2, 64)],
        Vec::new(),
        terminal_psi::TerminalMachineResult::Unit,
    );
    assert!(bind_thunk_machine_interface(&extra, &signature()).is_err());
    let missing = machine(
        Vec::new(),
        Vec::new(),
        terminal_psi::TerminalMachineResult::Unit,
    );
    assert!(bind_thunk_machine_interface(&missing, &signature()).is_err());
}

#[test]
fn thunk_interface_binds_float_and_boolean_shapes() {
    let signature = calling_conventions::CallSignature {
        parameters: vec![
            calling_conventions::ValueShape::float(8),
            calling_conventions::ValueShape::integer(1, 1),
        ],
        result: Some(calling_conventions::ValueShape::float(4)),
    };
    let machine = machine(
        vec![
            terminal_psi::ValueDeclaration {
                id: semantic_vocabulary::ValueId::new(1).expect("nonzero value id"),
                scalar_type: semantic_vocabulary::ScalarType::IeeeFloat(
                    semantic_vocabulary::IeeeFloatFormat::Binary64,
                ),
                qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
            },
            terminal_psi::ValueDeclaration {
                id: semantic_vocabulary::ValueId::new(2).expect("nonzero value id"),
                scalar_type: semantic_vocabulary::ScalarType::Boolean,
                qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
            },
        ],
        Vec::new(),
        terminal_psi::TerminalMachineResult::Scalar(terminal_psi::ValueDeclaration {
            id: semantic_vocabulary::ValueId::new(3).expect("nonzero value id"),
            scalar_type: semantic_vocabulary::ScalarType::IeeeFloat(
                semantic_vocabulary::IeeeFloatFormat::Binary32,
            ),
            qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
        }),
    );
    bind_thunk_machine_interface(&machine, &materialized(signature))
        .expect("float and boolean shapes bind");
}

#[test]
fn thunk_interface_rejects_result_presence_drift() {
    let unit_requirement = materialized(calling_conventions::CallSignature {
        parameters: Vec::new(),
        result: None,
    });
    let scalar_machine = machine(
        Vec::new(),
        Vec::new(),
        terminal_psi::TerminalMachineResult::Scalar(scalar_parameter(1, 64)),
    );
    assert!(bind_thunk_machine_interface(&scalar_machine, &unit_requirement).is_err());

    let scalar_requirement = materialized(calling_conventions::CallSignature {
        parameters: Vec::new(),
        result: Some(calling_conventions::ValueShape::integer(8, 8)),
    });
    let unit_machine = machine(
        Vec::new(),
        Vec::new(),
        terminal_psi::TerminalMachineResult::Unit,
    );
    assert!(bind_thunk_machine_interface(&unit_machine, &scalar_requirement).is_err());
}
