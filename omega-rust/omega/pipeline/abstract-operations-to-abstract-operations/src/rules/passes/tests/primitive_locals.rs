//! Memory observations remain ordered through scalar optimization and loop reentry.

use super::*;
use terminal_psi::{
    StructuralMultiplicity, StructuralOperationResult, StructuralTypeDeclaration,
    StructuralTypeShape,
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
        }],
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
fn primitive_observations_are_not_constants_or_pure_common_subexpressions() {
    for reentry in [false, true] {
        let unit = local_unit(reentry);
        if reentry {
            assert_eq!(optimization_unit_semantics::validate_psi_optimization_unit_with_admitted_cycle_machines(&unit, &[unit.entry]), Ok(()));
        } else {
            assert_eq!(validate_psi_optimization_unit(&unit), Ok(()));
        }
        let before = unit.clone();
        let AnalysisProduct::ScalarConstants(constants) =
            compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap()
        else {
            panic!("constants")
        };
        for observation in [9, 12] {
            assert!(
                constants
                    .facts
                    .iter()
                    .all(|fact| fact.value != id(observation, ValueId::new))
            );
        }
        let AnalysisProduct::EffectSummaries(effects) =
            compute_analysis(&unit, AnalysisKind::EffectSummaries).unwrap()
        else {
            panic!("effects")
        };
        for effect in &effects.nodes[1..5] {
            assert_eq!(effect.class, crate::EffectClass::StructuralState);
        }
        let contract = SameBlockTotalScalarCseRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            SameBlockTotalScalarCseRule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );
        let contract = DeadUnconditionallyTotalScalarEliminationRule::contract();
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            DeadUnconditionallyTotalScalarEliminationRule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );
        assert_eq!(unit, before);
    }
}

#[test]
fn independent_scalar_rewrite_preserves_local_memory_sequence() {
    let mut unit = local_unit(false);
    // A dead literal after the memory operations lets a real rewrite commit
    // without making any memory operation a deletion candidate.
    let mut literal = unit.functions[0].blocks[0].nodes.last().unwrap().clone();
    literal.operation = O::BooleanConstant {
        psi_operation: id(20, OperationId::new),
        result: id(21, ValueId::new),
        value: true,
    };
    let mut plan = AbstractOperationPlan {
        psi: unit.psi,
        entry: unit.entry,
        structural_types: unit.structural_types.clone(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine: unit.entry,
            attachment: None,
            entry: unit.functions[0].entry,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block: unit.functions[0].entry,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: unit.functions[0].blocks[0]
                .nodes
                .iter()
                .map(|node| node.operation.clone())
                .collect(),
        }],
    };
    plan.functions[0].operations.insert(5, literal.operation);
    unit = reconstruct_psi_optimization_unit_seed(&plan, unit.fuel_schedule).unwrap();
    assert_eq!(validate_psi_optimization_unit(&unit), Ok(()));
    let contract = DeadScalarLiteralEliminationRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let candidates = DeadScalarLiteralEliminationRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap();
    assert_eq!(candidates.len(), 1);
    let accepted = validate_dead_scalar_node_candidate(&unit, &candidates[0]).unwrap();
    assert_eq!(
        &unit.functions[0].blocks[0].nodes[..5],
        &accepted.unit().functions[0].blocks[0].nodes[..5]
    );
    assert_eq!(validate_psi_optimization_unit(accepted.unit()), Ok(()));
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

#[test]
fn calls_and_reentry_do_not_turn_local_reads_into_scalar_constants() {
    for reentry in [false, true] {
        let mut plan = local_plan(reentry);
        let mut callee = plan.functions[0].clone();
        callee.machine = id(30, MachineId::new);
        callee.entry = id(31, BlockId::new);
        callee.block_entries[0].block = callee.entry;
        let scalar_type = plan.structural_types[0].shape.clone();
        let StructuralTypeShape::PrimitiveScalar(scalar_type) = scalar_type else {
            panic!("primitive")
        };
        let parameter = terminal_psi::StructuralParameterDeclaration {
            place: id(32, PlaceId::new),
            position: 0,
            is_self: false,
            structural_type: plan.structural_types[0].id,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: terminal_psi::StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        };
        callee.structural_parameters = vec![parameter.clone()];
        callee.operations = vec![
            O::IntegerConstant {
                psi_operation: id(33, OperationId::new),
                result: id(34, ValueId::new),
                scalar_type,
                value: IntegerValue::Unsigned(0),
            },
            O::WriteOnlyPrimitiveStore {
                psi_operation: id(35, OperationId::new),
                destination: parameter,
                value: AbstractResult {
                    value: id(34, ValueId::new),
                    scalar_type,
                },
            },
            O::ReturnUnit {
                psi_edge: id(36, EdgeId::new),
                cleanup_actions: Vec::new(),
            },
        ];
        plan.functions[0].operations.insert(
            4,
            O::CallUnit {
                psi_operation: id(37, OperationId::new),
                callee: callee.machine,
                arguments: Vec::new(),
                structural_arguments: vec![terminal_psi::StructuralArgument {
                    place: id(3, PlaceId::new),
                    path: Vec::new(),
                    access: terminal_psi::StructuralAccess::MutableBorrow,
                }],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        );
        plan.functions.push(callee);
        let unit =
            reconstruct_psi_optimization_unit_seed(&plan, FuelScheduleIdentity::new(1).unwrap())
                .unwrap();
        assert_eq!(optimization_unit_semantics::validate_psi_optimization_unit_with_admitted_cycle_machines(
            &unit, if reentry { std::slice::from_ref(&unit.entry) } else { &[] }), Ok(()));
        let AnalysisProduct::ScalarConstants(constants) =
            compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap()
        else {
            panic!("constants")
        };
        assert!(
            constants
                .facts
                .iter()
                .all(|fact| ![id(9, ValueId::new), id(12, ValueId::new)].contains(&fact.value))
        );
        let contract = SameBlockTotalScalarCseRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            SameBlockTotalScalarCseRule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );
    }
}
