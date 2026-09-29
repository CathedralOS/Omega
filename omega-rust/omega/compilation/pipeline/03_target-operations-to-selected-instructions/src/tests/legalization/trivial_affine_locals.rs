//! An empty-record affine local passed whole to an owned parameter: the
//! establishment realizes no storage, and calls and exit cleanup retain its
//! exact ownership disposition.
use crate::{
    legalize_target_operations, select_instructions, selection_constraints,
    validate_selected_instructions,
};
use abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractStructuralCallResult,
};
use abstract_operations_to_target_operations::TargetLoweringRequest;
use legalized_operations::LegalizedScalarInstructionKind;
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, MachineId, OperationId, PlaceId, StructuralPlaceKind,
    StructuralTypeId,
};
use target::NativeTarget;
use target_operations::{TargetStructuralArgumentSource, TargetUnitOperation};
use terminal_psi::{
    SemanticFingerprint, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalAffineCleanupAction, TerminalPsiIdentity, VocabularyMarker,
};

fn empty() -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        id: StructuralTypeId::new(1).unwrap(),
        identity: "test::Position".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    }
}

fn local() -> StructuralPlaceDeclaration {
    StructuralPlaceDeclaration {
        id: PlaceId::new(1).unwrap(),
        kind: StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal: 0,
            structural_type: empty().id,
            construction: None,
        },
    }
}

fn function(
    machine: u64,
    structural_parameters: Vec<StructuralParameterDeclaration>,
    operations: Vec<AbstractOperation>,
) -> AbstractFunction {
    AbstractFunction {
        machine: MachineId::new(machine).unwrap(),
        attachment: None,
        entry: BlockId::new(1).unwrap(),
        parameters: Vec::new(),
        structural_parameters,
        result: AbstractFunctionResult::Unit,
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries: vec![AbstractBlockEntry {
            structural_parameters: Vec::new(),
            block: BlockId::new(1).unwrap(),
            parameters: Vec::new(),
            operation_offset: 0,
        }],
        operations,
    }
}

/// `main` establishes `position` and moves it into `consume`, whose owned
/// affine parameter is discarded on return. Like a hosted entry over an
/// empty receiver, `main` has no structural parameter, so the local alone
/// admits it to the structural graph.
fn source() -> AbstractOperationPlan {
    let parameter = StructuralParameterDeclaration {
        place: PlaceId::new(5).unwrap(),
        position: 0,
        is_self: false,
        structural_type: empty().id,
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x51; 32]),
        },
        entry: MachineId::new(1).unwrap(),
        structural_types: vec![empty()].into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            function(
                1,
                Vec::new(),
                vec![
                    AbstractOperation::EstablishTrivialAffineLocal {
                        psi_operation: OperationId::new(1).unwrap(),
                        place: local(),
                        structural_type: empty(),
                    },
                    AbstractOperation::CallStructural {
                        psi_operation: OperationId::new(2).unwrap(),
                        callee: MachineId::new(2).unwrap(),
                        arguments: Vec::new(),
                        structural_arguments: vec![StructuralArgument {
                            place: local().id,
                            access: StructuralAccess::Owned,
                            path: Vec::new(),
                        }],
                        dynamic_arguments: Vec::new(),
                        claim_transfers: Vec::new(),
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                        result: AbstractStructuralCallResult::Unit,
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: EdgeId::new(1).unwrap(),
                        cleanup_actions: Vec::new(),
                    },
                ],
            ),
            function(
                2,
                vec![parameter],
                vec![AbstractOperation::ReturnUnit {
                    psi_edge: EdgeId::new(2).unwrap(),
                    cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(
                        PlaceId::new(5).unwrap(),
                    )],
                }],
            ),
        ],
    }
}

fn fixture(
    native: NativeTarget,
) -> (
    AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    let source = source();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    optimization_unit_semantics::validate_psi_optimization_unit(&unit)
        .expect("the empty local is an owned affine call source");
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &source,
        TargetLoweringRequest::new(native),
    )
    .expect("an empty local lowers as its establishment and a zero-byte argument");
    (source, target, unit)
}

#[test]
fn empty_local_transfers_as_a_zero_byte_owned_argument() {
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (source, target, unit) = fixture(native);
        let rows = &target.functions[0].graph.blocks[0].operations;
        assert!(matches!(
            rows.as_slice(),
            [
                TargetUnitOperation::EstablishTrivialAffineLocal { psi_operation, .. },
                TargetUnitOperation::Call { arguments, .. },
            ] if *psi_operation == OperationId::new(1).unwrap()
                && matches!(arguments.as_slice(), [argument]
                    if argument.shape.byte_size == 0
                        && argument.source == TargetStructuralArgumentSource::StructuralHome {
                            psi_operation: OperationId::new(1).unwrap(),
                        })
        ));
        let legal = legalize_target_operations(&target, &source, &unit)
            .expect("the empty local legalizes on both ISAs");
        assert!(
            legal.plan().scalar_functions[0]
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .any(|row| matches!(
                    row.kind,
                    LegalizedScalarInstructionKind::EstablishTrivialAffineLocal { .. }
                ))
        );
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = selection_constraints(&legal, &environment);
        let selected = select_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .expect("the empty local reaches selection");
        validate_selected_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .expect("selection replay accepts the storage-free transfer");
        assert!(
            selected.plan().functions[0].local_storage_slots.is_empty(),
            "an empty local owns no activation storage"
        );
    }
}

#[test]
fn empty_local_cleanup_replays_on_unit_and_structural_returns() {
    for structural_return in [false, true] {
        let mut source = source();
        let parameter = source.functions[1].structural_parameters[0].clone();
        source.functions.truncate(1);
        let function = &mut source.functions[0];
        function.operations.remove(1);
        function.operations[1] = AbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(1).unwrap(),
            cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(local().id)],
        };
        if structural_return {
            function.result =
                AbstractFunctionResult::Structural(terminal_psi::StructuralResultDeclaration {
                    place: PlaceId::new(6).unwrap(),
                    structural_type: empty().id,
                    multiplicity: StructuralMultiplicity::Affine,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    reference_sources: Vec::new(),
                });
            function.operations[1] = AbstractOperation::ReturnStructural {
                psi_edge: EdgeId::new(1).unwrap(),
                source: parameter.place,
                returned_claims: Vec::new(),
                trivial_affine_locals: Vec::new(),
                trivial_affine_discards: vec![local().id],
            };
            function.structural_parameters.push(parameter);
        }
        let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &source,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        optimization_unit_semantics::validate_psi_optimization_unit(&unit)
            .expect("the established empty local is discarded exactly once");
        let mut missing_cleanup = source.clone();
        match &mut missing_cleanup.functions[0].operations[1] {
            AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } => cleanup_actions.clear(),
            AbstractOperation::ReturnStructural {
                trivial_affine_discards,
                ..
            } => trivial_affine_discards.clear(),
            _ => unreachable!(),
        }
        let missing_cleanup = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &missing_cleanup,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        assert!(
            optimization_unit_semantics::validate_psi_optimization_unit(&missing_cleanup).is_err(),
            "an explicit local cannot disappear from the return frontier"
        );
        for native in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ] {
            let target = abstract_operations_to_target_operations::lower_to_target_operations(
                &source,
                TargetLoweringRequest::new(native),
            )
            .expect("a no-storage local still has cleanup custody");
            let legal = legalize_target_operations(&target, &source, &unit)
                .expect("cleanup rejoins the exact empty-local establishment");
            let environment =
                register_environment::baseline_target_register_environment(native).unwrap();
            let constraints = selection_constraints(&legal, &environment);
            let selected = select_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            validate_selected_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
                selected.plan().clone(),
            )
            .expect("the no-code discard survives independent selection replay");
            for mutation in ["lost", "unknown", "duplicate"] {
                let mut hostile = target.clone();
                let cleanup = match &mut hostile.functions[0].graph.blocks[0].terminator {
                    target_operations::TargetControlTerminator::Return {
                        cleanup_actions, ..
                    }
                    | target_operations::TargetControlTerminator::ReturnStructural {
                        cleanup_actions,
                        ..
                    } => cleanup_actions,
                    _ => unreachable!(),
                };
                match mutation {
                    "lost" => cleanup.clear(),
                    "unknown" => {
                        cleanup[0] =
                            TerminalAffineCleanupAction::DiscardRoot(PlaceId::new(99).unwrap());
                    }
                    "duplicate" => cleanup.push(cleanup[0].clone()),
                    _ => unreachable!(),
                }
                assert!(
                    legalize_target_operations(&hostile, &source, &unit).is_err(),
                    "{mutation} cleanup must not replay"
                );
            }
        }
    }
}
