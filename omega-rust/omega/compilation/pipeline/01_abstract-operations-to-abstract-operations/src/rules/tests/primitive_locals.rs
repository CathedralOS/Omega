//! Memory observations remain ordered through scalar optimization and loop reentry.
use super::super::{
    DeadScalarLiteralEliminationRule, DeadUnconditionallyTotalScalarEliminationRule,
};
use crate::rules::tests::fixtures::id;
use abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation as O,
    AbstractOperationPlan, AbstractResult,
};
use optimization_unit::{
    NodeLocation, ProvenanceDisposition, ProvenanceRewrite, PsiOptimizationUnit,
    PsiRealizationSite, PsiRewriteCandidate, reconstruct_psi_optimization_unit_seed,
};
use optimization_unit_semantics::{
    OptimizationUnitValidationError, validate_dead_scalar_node_candidate,
    validate_psi_optimization_unit,
};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, PlaceId, ScalarType, StructuralTypeId, ValueId,
};
use terminal_psi::{
    SemanticFingerprint, StructuralMultiplicity, StructuralOperationResult,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalPsiIdentity, VocabularyMarker,
};

fn local_plan(reentry: bool) -> AbstractOperationPlan {
    let machine = id(1, MachineId::new);
    let block = id(2, BlockId::new);
    let place = id(3, PlaceId::new);
    let structural_type = id(4, StructuralTypeId::new);
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let value = AbstractResult {
        value: id(5, ValueId::new),
        scalar_type,
    };
    let read = |operation, result| O::PrimitiveScalarRead {
        psi_operation: id(operation, OperationId::new),
        result: AbstractResult {
            value: id(result, ValueId::new),
            scalar_type,
        },
        source: place,
        path: Vec::new(),
    };
    let mut operations = vec![
        O::IntegerConstant {
            psi_operation: id(6, OperationId::new),
            result: value.value,
            scalar_type,
            value: IntegerValue::Unsigned(7),
        },
        O::EstablishPrimitiveLocal {
            psi_operation: id(7, OperationId::new),
            result: StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            },
            value,
        },
        read(8, 9),
        O::PrimitiveLocalStore {
            psi_operation: id(10, OperationId::new),
            destination: place,
            value,
        },
        read(11, 12),
    ];
    operations.push(if reentry {
        O::Jump {
            psi_edge: id(13, EdgeId::new),
            target: block,
            bindings: Vec::new(),
            structural_bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        }
    } else {
        O::ReturnUnit {
            psi_edge: id(13, EdgeId::new),
            cleanup_actions: Vec::new(),
        }
    });
    AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([84; 32]),
        },
        entry: machine,
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "test::primitive-local".into(),
            shape: StructuralTypeShape::PrimitiveScalar(scalar_type),
        }]
        .into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry: block,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations,
        }],
    }
}

fn local_unit(reentry: bool) -> PsiOptimizationUnit {
    reconstruct_psi_optimization_unit_seed(
        &local_plan(reentry),
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}

#[test]
fn independent_validator_rejects_forged_deletion_of_initialized_reads() {
    let unit = local_unit(false);
    assert_eq!(validate_psi_optimization_unit(&unit), Ok(()));
    for position in [2, 4] {
        let node = &unit.functions[0].blocks[0].nodes[position];
        let O::PrimitiveScalarRead {
            psi_operation,
            result,
            ..
        } = node.operation
        else {
            panic!("read")
        };
        let location = NodeLocation {
            machine: unit.entry,
            block: unit.functions[0].entry,
            node: position as u32,
        };
        let receiver = NodeLocation {
            node: location.node + 1,
            ..location
        };
        let provenance = vec![ProvenanceRewrite {
            input: PsiRealizationSite::Node(location),
            sources: node.provenance.clone(),
            fuel: node.fuel.clone(),
            disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(receiver)),
        }];
        for contract in [
            DeadScalarLiteralEliminationRule::contract(),
            DeadUnconditionallyTotalScalarEliminationRule::contract(),
        ] {
            let forged = PsiRewriteCandidate::new_dead_scalar_node(
                unit.identity,
                contract,
                vec![location.block],
                provenance.clone(),
                -1,
                optimization_unit::DeadScalarNodeRewrite {
                    location,
                    source_operation: psi_operation,
                    result: result.value,
                    scalar_type: result.scalar_type,
                },
            )
            .unwrap();
            assert_eq!(
                validate_dead_scalar_node_candidate(&unit, &forged),
                Err(OptimizationUnitValidationError::CandidatePatchMismatch)
            );
        }
    }
}
