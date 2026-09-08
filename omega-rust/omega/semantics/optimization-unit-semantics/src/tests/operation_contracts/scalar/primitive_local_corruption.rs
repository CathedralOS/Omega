//! Hostile current-IR roots, SSA observations, and occurrence metadata.

use crate::tests::*;
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use abstract_operations::{AbstractOperation as O, AbstractStructuralBinding};
use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration};

#[test]
fn local_cannot_be_laundered_through_owned_edge_parameter() {
    let mut plan = primitive_local_plan();
    let function = &mut plan.functions[0];
    let target = id(81, BlockId::new);
    let parameter = StructuralParameterDeclaration {
        place: id(82, PlaceId::new),
        position: 0,
        is_self: false,
        structural_type: id(55, StructuralTypeId::new),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    *function.operations.last_mut().unwrap() = O::Jump {
        psi_edge: id(58, EdgeId::new),
        target,
        bindings: Vec::new(),
        structural_bindings: vec![AbstractStructuralBinding {
            parameter: parameter.place,
            argument: terminal_psi::StructuralArgument {
                place: id(54, PlaceId::new),
                path: Vec::new(),
                access: StructuralAccess::Owned,
            },
        }],
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    function.block_entries.push(AbstractBlockEntry {
        block: target,
        parameters: Vec::new(),
        structural_parameters: vec![parameter],
        operation_offset: function.operations.len(),
    });
    function.operations.push(O::ReturnUnit {
        psi_edge: id(83, EdgeId::new),
        cleanup_actions: Vec::new(),
    });
    let unit = reconstruct_psi_optimization_unit_seed(
        &plan,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap();
    assert_eq!(
        validate_psi_optimization_unit(&unit),
        Err(OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch)
    );
}

#[test]
fn local_result_place_and_producer_must_rejoin_exact_catalog() {
    for forge_place in [false, true] {
        let mut unit = primitive_local_unit();
        let O::EstablishPrimitiveLocal {
            psi_operation,
            result,
            ..
        } = &mut unit.functions[0].blocks[0].nodes[1].operation
        else {
            panic!("local")
        };
        if forge_place {
            result.place = id(90, PlaceId::new);
        } else {
            *psi_operation = id(91, OperationId::new);
        }
        refresh_function_derivatives(&mut unit, 0);
        assert!(matches!(
            validate_psi_optimization_unit(&unit),
            Err(OptimizationUnitValidationError::StructuralCatalogMismatch { .. })
        ));
    }
}

#[test]
fn reads_cannot_reuse_initializer_or_pre_call_observation_identity() {
    for replacement in [53, 60] {
        let plan = primitive_local_call_plan();
        let mut unit = reconstruct_psi_optimization_unit_seed(
            &plan,
            terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
        )
        .unwrap();
        let O::PrimitiveScalarRead { result, .. } =
            &mut unit.functions[0].blocks[0].nodes[5].operation
        else {
            panic!("post-call read")
        };
        result.value = id(replacement, ValueId::new);
        refresh_function_derivatives(&mut unit, 0);
        assert!(validate_psi_optimization_unit(&unit).is_err());
    }
}

#[test]
fn primitive_occurrences_reject_forged_provenance_and_fuel() {
    for position in 1..5 {
        for forge_fuel in [false, true] {
            let mut unit = primitive_local_unit();
            let node = &mut unit.functions[0].blocks[0].nodes[position];
            if forge_fuel {
                node.fuel[0].units = 0;
            } else {
                node.provenance[0] =
                    optimization_unit::PsiProvenance::Operation(id(99, OperationId::new));
                node.fuel[0].site = node.provenance[0];
            }
            refresh_identity(&mut unit);
            assert!(validate_psi_optimization_unit(&unit).is_err());
        }
    }
}
