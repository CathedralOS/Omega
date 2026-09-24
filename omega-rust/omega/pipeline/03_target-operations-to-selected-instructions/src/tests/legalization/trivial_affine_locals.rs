//! An empty-record affine local passed whole to an owned parameter: the
//! establishment realizes no storage, and the call names it as the zero-byte
//! argument's producer.
use crate::{
    legalize_target_operations, select_instructions, selection_constraints,
    validate_selected_instructions,
};
use abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan,
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
                    AbstractOperation::CallUnit {
                        psi_operation: OperationId::new(2).unwrap(),
                        callee: MachineId::new(2).unwrap(),
                        arguments: Vec::new(),
                        structural_arguments: vec![StructuralArgument {
                            place: local().id,
                            access: StructuralAccess::Owned,
                            path: Vec::new(),
                        }],
                        claim_transfers: Vec::new(),
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
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

/// The argument must name the one establishment of its place: another
/// producer, or an establishment spelling another place, rejects.
#[test]
fn empty_local_argument_rejects_producer_and_place_substitution() {
    let (source, target, unit) = fixture(NativeTarget::linux_x64());
    for mutation in 0..2 {
        let mut changed = target.clone();
        match (
            mutation,
            &mut changed.functions[0].graph.blocks[0].operations[..],
        ) {
            (0, [_, TargetUnitOperation::Call { arguments, .. }]) => {
                arguments[0].source = TargetStructuralArgumentSource::StructuralHome {
                    psi_operation: OperationId::new(2).unwrap(),
                }
            }
            (
                1,
                [
                    TargetUnitOperation::EstablishTrivialAffineLocal { place, .. },
                    _,
                ],
            ) => place.id = PlaceId::new(7).unwrap(),
            _ => unreachable!(),
        }
        assert!(
            legalize_target_operations(&changed, &source, &unit).is_err(),
            "mutation {mutation}"
        );
    }
}
