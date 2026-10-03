//! Structural-call argument arity and structural-access corruption rejection.

use crate::tests::fixtures::scalar_units::structural_call_unit;
use crate::tests::support::{id, refresh_identity, refresh_node_derivatives};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use abstract_operations::{AbstractOperation, AbstractStructuralCallResult};
use semantic_vocabulary::BoundaryMachineId;

#[test]
fn local_byte_windows_keep_their_producer_bounds_and_access() {
    let source = r#"
        boundary trait Output { machine count(value: u64) reaches Output; }
        machine consume(bytes: &[u8]) reaches Output { Output::count(bytes.len); }
        machine enter() reaches Output {
            let bytes: [u8; 3];
            consume(&bytes[1..3]);
        }
    "#;
    let baseline = crate::tests::structural_cases::source_machine_unit(source, "enter", true);
    validate_psi_optimization_unit(&baseline).expect("a completed local lends its bounded window");
    for widen_access in [false, true] {
        let mut changed = baseline.clone();
        let (function_index, block_index, node_index) = changed
            .functions
            .iter_mut()
            .enumerate()
            .find_map(|(function_index, function)| {
                function
                    .blocks
                    .iter_mut()
                    .enumerate()
                    .find_map(|(block_index, block)| {
                        block
                            .nodes
                            .iter_mut()
                            .enumerate()
                            .find_map(|(node_index, node)| {
                                let AbstractOperation::CallStructural {
                                    structural_arguments,
                                    ..
                                } = &mut node.operation
                                else {
                                    return None;
                                };
                                let argument =
                                    structural_arguments.iter_mut().find(|argument| {
                                        argument.path.iter().any(|segment| {
                                            matches!(segment,
                                terminal_psi::StructuralPathSegment::FixedByteRange { .. })
                                        })
                                    })?;
                                if widen_access {
                                    argument.access = terminal_psi::StructuralAccess::MutableBorrow;
                                } else {
                                    *argument.path.last_mut().unwrap() =
                                        terminal_psi::StructuralPathSegment::FixedByteRange {
                                            start: 1,
                                            end: 4,
                                        };
                                }
                                Some((function_index, block_index, node_index))
                            })
                    })
            })
            .expect("the abstract call keeps the window path");
        refresh_node_derivatives(&mut changed, function_index, block_index, node_index);
        assert!(
            validate_psi_optimization_unit(&changed).is_err(),
            "window geometry cannot widen bounds or access"
        );
    }
}

#[test]
fn rejects_structural_call_argument_arity_and_access_corruption() {
    let baseline = structural_call_unit();
    validate_psi_optimization_unit(&baseline)
        .expect("matching structural argument access should validate");

    let mut access = baseline.clone();
    let AbstractOperation::CallStructural {
        structural_arguments,
        result: AbstractStructuralCallResult::Unit,
        ..
    } = &mut access.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with a structural Unit call")
    };
    structural_arguments[0].access = terminal_psi::StructuralAccess::SharedBorrow;
    refresh_node_derivatives(&mut access, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&access),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));

    let mut arity = baseline;
    let AbstractOperation::CallStructural {
        structural_arguments,
        result: AbstractStructuralCallResult::Unit,
        ..
    } = &mut arity.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with a structural Unit call")
    };
    structural_arguments.clear();
    refresh_node_derivatives(&mut arity, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&arity),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));

    let mut boundary = structural_call_unit();
    let boundary_id = id(341, BoundaryMachineId::new);
    boundary
        .boundary_machines
        .push(terminal_psi::BoundaryMachineDeclaration {
            unconditional_termination: false,
            completion: terminal_psi::BoundaryCompletion::NormalReturn,
            kind: terminal_psi::BoundaryRequirementKind::Trait,
            scalar_ensures: Vec::new(),
            scalar_requires: Vec::new(),
            fixed_service_reach: Vec::new(),
            id: boundary_id,
            identity: "validation::structural-boundary".into(),
            attachment: None,
            parameter_order: vec![terminal_psi::BoundaryParameterKind::Structural],
            scalar_parameters: Vec::new(),
            structural_parameters: vec![boundary.functions[1].structural_parameters[0].clone()],
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
            crash_routes: Vec::new(),
        });
    let (psi_operation, structural_arguments) =
        match &boundary.functions[0].blocks[0].nodes[0].operation {
            AbstractOperation::CallStructural {
                psi_operation,
                structural_arguments,
                result: AbstractStructuralCallResult::Unit,
                ..
            } => (*psi_operation, structural_arguments.clone()),
            _ => panic!("fixture begins with a structural Unit call"),
        };
    boundary.functions[0].blocks[0].nodes[0].operation = AbstractOperation::BoundaryCall {
        psi_operation,
        result: abstract_operations::AbstractBoundaryResult::Unit,
        boundary: boundary_id,
        arguments: Vec::new(),
        structural_arguments,
        completion_claim_sources: Vec::new(),
        completion_receipts: Vec::new(),
    };
    refresh_node_derivatives(&mut boundary, 0, 0, 0);
    validate_psi_optimization_unit(&boundary)
        .expect("matching boundary structural access should validate");

    boundary.boundary_machines[0].structural_parameters[0].access =
        terminal_psi::StructuralAccess::SharedBorrow;
    refresh_identity(&mut boundary);
    assert!(matches!(
        validate_psi_optimization_unit(&boundary),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));
}
