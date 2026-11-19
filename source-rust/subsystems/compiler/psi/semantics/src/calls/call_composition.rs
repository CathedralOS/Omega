pub(crate) mod bounded_byte_owner;
pub(crate) mod fixed_byte_view;
pub use bounded_byte_owner::{
    bounded_byte_owner_capacity, bounded_byte_owner_capacity_from_catalogs,
    bounded_byte_owner_capacity_in,
};
pub use fixed_byte_view::{
    BorrowedReferentPresentation, FixedCollectionWindow, borrowed_referent_matches,
    borrowed_referent_presentation, fixed_byte_array_extent, fixed_byte_array_loan,
    fixed_byte_array_window, fixed_byte_extent, fixed_byte_storage_window,
    fixed_collection_storage_window, fixed_element_array_extent,
};

use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{
    ByteSequenceCarrier, OperationKind, StructuralAccess, StructuralArgument,
    StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
};

use crate::OperationSemanticError;
use crate::runtime_structural_field;
use crate::operations::OperationSemanticTag;

pub fn boundary_buffer_capacity<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root_type: StructuralTypeId,
    argument: &StructuralArgument,
    expected: &StructuralParameterDeclaration,
) -> Option<u64> {
    inline_byte_field_capacity(
        types,
        root_type,
        argument,
        expected,
        StructuralAccess::MutableBorrow,
    )
}

pub fn shared_boundary_buffer_capacity<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root_type: StructuralTypeId,
    argument: &StructuralArgument,
    expected: &StructuralParameterDeclaration,
) -> Option<u64> {
    inline_byte_field_capacity(
        types,
        root_type,
        argument,
        expected,
        StructuralAccess::SharedBorrow,
    )
}

pub fn stored_view_field_presentation<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root_type: StructuralTypeId,
    argument: &StructuralArgument,
    expected: &StructuralParameterDeclaration,
) -> Option<terminal_psi::ByteSequencePredicateSet> {
    if argument.access != StructuralAccess::SharedBorrow
        || expected.access != StructuralAccess::SharedBorrow
        || !expected.projected_qualifications.is_empty()
        || !types.clone().any(|declaration| {
            declaration.id == expected.structural_type
                && declaration.shape.is_shared_readable_byte_view()
        })
    {
        return None;
    }
    let field = runtime_structural_field(types, root_type, &argument.path)?;
    (field.field_type.byte_sequence_carrier()?
        == ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::SharedBorrow),
        })
    .then(|| field.field_type.byte_predicates())
}

fn inline_byte_field_capacity<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root_type: StructuralTypeId,
    argument: &StructuralArgument,
    expected: &StructuralParameterDeclaration,
    access: StructuralAccess,
) -> Option<u64> {
    if argument.access != access
        || expected.access != access
        || !expected.qualifications.is_empty()
        || !expected.projected_qualifications.is_empty()
        || !types.clone().any(|declaration| {
            declaration.id == expected.structural_type
                && matches!(
                    declaration.shape,
                    StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { .. })
                )
        })
    {
        return None;
    }
    if access == StructuralAccess::SharedBorrow && argument.path.is_empty() {
        return types
            .clone()
            .find_map(|declaration| match declaration.shape {
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                    capacity,
                }) if declaration.id == root_type => Some(capacity),
                _ => None,
            });
    }
    let field = runtime_structural_field(types, root_type, &argument.path)?;
    if access != StructuralAccess::SharedBorrow
        && !field.field_type.byte_predicates().is_empty()
    {
        return None;
    }
    match field.field_type.byte_sequence_carrier()? {
        ByteSequenceCarrier::BoundedOwned { capacity } => Some(capacity),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CallTargetRule {
    ExactModuleMachine,
    ExactBoundaryMachine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CallResultRule {
    ScalarCalleeResult,
    UnitCalleeResult,
    StructuralCalleeResult,
    BoundaryDeclaredResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CallArgumentRule {
    ScalarPositionalArguments,
    StructuralPositionalArguments,
    ScalarAndStructuralPositionalArguments,
    BoundaryScalarAndStructuralPositionalArguments,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CallRequirementRule {
    EnumerateScalarRequires,
    EnumerateStructuralRequires,
    ValidateBoundaryRequirements,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CallTransferRule {
    NoStructuralTransfer,
    ExactClaimTransfers,
    ExactClaimAndStructuralResultTransfers,
    ExactCompletionReceipts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CallOutcomeRule {
    ImportScalarEnsures,
    ImportStructuralEnsures,
    BoundaryDeclaredCompletion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CallCrashRule {
    ComposeScalarCrashRoutes,
    ComposeStructuralCrashRoutes,
    BoundaryHasNoCallRoutes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CallEvidenceRule {
    SameUnitOrCertifiedScalarEvidence,
    SameUnitOrCertifiedStructuralEvidence,
    ExactBoundaryDeclarationEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CallWorkPolicy {
    ConsumeOne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CallFrontierRule {
    KeepsScalarFrontier,
    TransfersStructuralFrontier,
    SettlesBoundaryFrontier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CallCompositionSchema {
    target: CallTargetRule,
    result: CallResultRule,
    arguments: CallArgumentRule,
    requirements: CallRequirementRule,
    transfers: CallTransferRule,
    outcomes: CallOutcomeRule,
    crash: CallCrashRule,
    evidence: CallEvidenceRule,
    work: CallWorkPolicy,
    frontier: CallFrontierRule,
}

impl CallCompositionSchema {
    pub const fn target(self) -> CallTargetRule {
        self.target
    }

    pub const fn result(self) -> CallResultRule {
        self.result
    }

    pub const fn arguments(self) -> CallArgumentRule {
        self.arguments
    }

    pub const fn requirements(self) -> CallRequirementRule {
        self.requirements
    }

    pub const fn transfers(self) -> CallTransferRule {
        self.transfers
    }

    pub const fn outcomes(self) -> CallOutcomeRule {
        self.outcomes
    }

    pub const fn crash(self) -> CallCrashRule {
        self.crash
    }

    pub const fn evidence(self) -> CallEvidenceRule {
        self.evidence
    }

    pub const fn work(self) -> CallWorkPolicy {
        self.work
    }

    pub const fn frontier(self) -> CallFrontierRule {
        self.frontier
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallCompositionSemanticRow {
    tag: OperationSemanticTag,
    schema: CallCompositionSchema,
}

impl CallCompositionSemanticRow {
    pub const fn tag(self) -> OperationSemanticTag {
        self.tag
    }

    pub const fn schema(self) -> CallCompositionSchema {
        self.schema
    }
}

macro_rules! call_composition_rows {
    ($( $tag:ident => ($target:ident, $result:ident, $arguments:ident,
        $requirements:ident, $transfers:ident, $outcomes:ident,
        $crash:ident, $evidence:ident, $frontier:ident) ),+ $(,)?) => {
        impl CallCompositionSemanticRow {
            pub const ALL: [Self; call_composition_rows!(@count $( $tag )+)] = [
                $( Self {
                    tag: OperationSemanticTag::$tag,
                    schema: CallCompositionSchema {
                        target: CallTargetRule::$target,
                        result: CallResultRule::$result,
                        arguments: CallArgumentRule::$arguments,
                        requirements: CallRequirementRule::$requirements,
                        transfers: CallTransferRule::$transfers,
                        outcomes: CallOutcomeRule::$outcomes,
                        crash: CallCrashRule::$crash,
                        evidence: CallEvidenceRule::$evidence,
                        work: CallWorkPolicy::ConsumeOne,
                        frontier: CallFrontierRule::$frontier,
                    },
                } ),+
            ];
        }

        crate::operations::operation_semantic_cohort! {
            rows: CallCompositionSemanticRow,
            tags: CALL_COMPOSITION_TAGS,
            is_tag: is_call_composition_tag,
            exact_row: exact_call_composition_semantic_row_in,
            reject_unexpected: reject_unexpected_call_composition_rows,
            missing: MissingCallCompositionRow,
            duplicate: DuplicateCallCompositionRow,
            unexpected: UnexpectedCallCompositionRow,
            $( $tag ),+
        }
    };
    (@count $( $variant:ident )+) => {
        <[()]>::len(&[$(call_composition_rows!(@unit $variant)),+])
    };
    (@unit $variant:ident) => { () };
}

call_composition_rows! {
    Call => (ExactModuleMachine, ScalarCalleeResult, ScalarPositionalArguments, EnumerateScalarRequires, NoStructuralTransfer, ImportScalarEnsures, ComposeScalarCrashRoutes, SameUnitOrCertifiedScalarEvidence, KeepsScalarFrontier),
    CallUnit => (ExactModuleMachine, UnitCalleeResult, ScalarAndStructuralPositionalArguments, EnumerateStructuralRequires, ExactClaimTransfers, ImportStructuralEnsures, ComposeStructuralCrashRoutes, SameUnitOrCertifiedStructuralEvidence, TransfersStructuralFrontier),
    CallStructuralScalar => (ExactModuleMachine, ScalarCalleeResult, StructuralPositionalArguments, EnumerateStructuralRequires, ExactClaimTransfers, ImportStructuralEnsures, ComposeStructuralCrashRoutes, SameUnitOrCertifiedStructuralEvidence, TransfersStructuralFrontier),
    CallStructural => (ExactModuleMachine, StructuralCalleeResult, StructuralPositionalArguments, EnumerateStructuralRequires, ExactClaimAndStructuralResultTransfers, ImportStructuralEnsures, ComposeStructuralCrashRoutes, SameUnitOrCertifiedStructuralEvidence, TransfersStructuralFrontier),
    CallStructuralWithScalarArguments => (ExactModuleMachine, StructuralCalleeResult, ScalarAndStructuralPositionalArguments, EnumerateStructuralRequires, ExactClaimAndStructuralResultTransfers, ImportStructuralEnsures, ComposeStructuralCrashRoutes, SameUnitOrCertifiedStructuralEvidence, TransfersStructuralFrontier),
    BoundaryCall => (ExactBoundaryMachine, BoundaryDeclaredResult, BoundaryScalarAndStructuralPositionalArguments, ValidateBoundaryRequirements, ExactCompletionReceipts, BoundaryDeclaredCompletion, BoundaryHasNoCallRoutes, ExactBoundaryDeclarationEvidence, SettlesBoundaryFrontier),
}

fn canonical_schema(tag: OperationSemanticTag) -> Option<CallCompositionSchema> {
    CallCompositionSemanticRow::ALL
        .iter()
        .find(|row| row.tag == tag)
        .map(|row| row.schema)
}

pub fn validate_call_composition_semantic_rows(
    rows: &[CallCompositionSemanticRow],
) -> Result<(), OperationSemanticError> {
    reject_unexpected_call_composition_rows(rows)?;
    for tag in CALL_COMPOSITION_TAGS {
        let row = exact_call_composition_semantic_row_in(tag, rows)?
            .expect("the requested tag belongs to call composition");
        if Some(row.schema) != canonical_schema(tag) {
            return Err(OperationSemanticError::CallCompositionSchemaMismatch(tag));
        }
    }
    Ok(())
}

pub fn call_composition_semantic_row(
    operation: &OperationKind,
) -> Result<Option<&'static CallCompositionSemanticRow>, OperationSemanticError> {
    validate_call_composition_semantic_rows(&CallCompositionSemanticRow::ALL)?;
    exact_call_composition_semantic_row_in(
        OperationSemanticTag::for_operation(operation),
        &CallCompositionSemanticRow::ALL,
    )
}
