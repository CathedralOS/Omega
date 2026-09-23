//! A caller that owns an aggregate actual, invokes a callee returning the
//! same structural type, and returns that result itself.

use abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan,
};
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, PlaceId, ScalarType, StructuralTypeId, ValueId,
};
use target_operations::TargetOperationPlan;
use terminal_psi::{
    CrashRouteBucket, SemanticFingerprint, StructuralAccess, StructuralArgument,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalPsiIdentity, VocabularyMarker,
};

pub(in crate::tests) fn fixture(
    native: target::NativeTarget,
    crash_routes: Vec<CrashRouteBucket>,
) -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    PsiOptimizationUnit,
) {
    let primitive = StructuralTypeId::new(1).unwrap();
    let array = StructuralTypeId::new(2).unwrap();
    let unsigned = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let caller_machine = MachineId::new(1).unwrap();
    let callee_machine = MachineId::new(2).unwrap();
    let result = |identity| terminal_psi::StructuralOperationResult {
        qualification_establishments: Vec::new(),
        place: PlaceId::new(identity).unwrap(),
        structural_type: array,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    };
    let declaration = terminal_psi::StructuralResultDeclaration {
        reference_sources: Vec::new(),
        place: PlaceId::new(99).unwrap(),
        structural_type: array,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let return_structural = |source| AbstractOperation::ReturnStructural {
        psi_edge: EdgeId::new(1).unwrap(),
        source: PlaceId::new(source).unwrap(),
        returned_claims: Vec::new(),
        trivial_affine_locals: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let call = |identity, source| AbstractOperation::CallStructural {
        psi_operation: OperationId::new(identity).unwrap(),
        result: result(identity),
        callee: callee_machine,
        arguments: Vec::new(),
        structural_arguments: vec![StructuralArgument {
            place: PlaceId::new(source).unwrap(),
            path: Vec::new(),
            access: StructuralAccess::Owned,
        }],
        claim_transfers: Vec::new(),
        returned_claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: crash_routes.clone(),
        selected_evidence: Vec::new(),
    };
    let block = |raw| AbstractBlockEntry {
        structural_parameters: Vec::new(),
        block: BlockId::new(raw).unwrap(),
        parameters: Vec::new(),
        operation_offset: 0,
    };
    let plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x50; 32]),
        },
        entry: caller_machine,
        structural_types: vec![
            StructuralTypeDeclaration {
                id: primitive,
                identity: "u64".into(),
                shape: StructuralTypeShape::PrimitiveScalar(unsigned),
            },
            StructuralTypeDeclaration {
                id: array,
                identity: "[u64; 2]".into(),
                shape: StructuralTypeShape::FixedArray {
                    element: primitive,
                    length: 2,
                },
            },
        ]
        .into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            AbstractFunction {
                machine: caller_machine,
                attachment: None,
                entry: BlockId::new(1).unwrap(),
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Structural(declaration.clone()),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![block(1)],
                operations: vec![
                    AbstractOperation::IntegerConstant {
                        psi_operation: OperationId::new(10).unwrap(),
                        result: ValueId::new(10).unwrap(),
                        scalar_type: unsigned,
                        value: IntegerValue::Unsigned(7),
                    },
                    AbstractOperation::EstablishScalarArray {
                        psi_operation: OperationId::new(11).unwrap(),
                        result: result(11),
                        elements: vec![ValueId::new(10).unwrap(), ValueId::new(10).unwrap()],
                    },
                    call(12, 11),
                    return_structural(12),
                ],
            },
            AbstractFunction {
                machine: callee_machine,
                attachment: None,
                entry: BlockId::new(2).unwrap(),
                parameters: Vec::new(),
                structural_parameters: vec![StructuralParameterDeclaration {
                    place: PlaceId::new(20).unwrap(),
                    position: 0,
                    is_self: false,
                    structural_type: array,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: StructuralAccess::Owned,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                }],
                result: AbstractFunctionResult::Structural(declaration),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![block(2)],
                operations: vec![return_structural(20)],
            },
        ],
    };
    let targeted = abstract_operations_to_target_operations::lower_to_target_operations(
        &plan,
        abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
    )
    .unwrap();
    let mut unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    for function in &mut unit.functions {
        function.verified_contract = Some(terminal_psi::MachineContract {
            id: semantic_vocabulary::ContractId::new(1).unwrap(),
            crash_routes: crash_routes.clone(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        });
    }
    unit.identity = optimization_unit::recompute_psi_optimization_unit_identity(&unit);
    (plan, targeted, unit)
}
