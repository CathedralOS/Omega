#![forbid(unsafe_code)]

/*
We define the target-neutral semantic judgments, operational analyses, and structural/scalar
invariants over `TerminalModule` (`terminal-semantics`), including control-flow graph queries,
owned and borrowed storage tracking, reference and placed-loan lifetimes, descriptor contents
replay, scalar and byte-sequence evaluation rules, content transformations, quotient opacity, and
contract substitution.
*/

/*
Why factor shared semantic judgments into `terminal-semantics` between the pure `terminal-psi` data
schema and the independent `terminal-verification` / `terminal-interpreter` / `Omega 00` consumers?
Both the receiving verifier and the native/interpreter realization stages must compute identical
facts about storage custody, descriptor origins, scalar leaf meanings, and call transfers without
duplicating subtle graph analyses or allowing producer-side assumptions to leak across the artifact
boundary.
*/

/*
Consumed by `terminal-verification` to reconstruct and check module obligations from decoded
`PSITERM\0` artifacts, by `terminal-interpreter` to execute Terminal Psi programs under the same
operational rules, and by `00_terminal-psi-to-abstract-operations` (`Omega 00`), `layout`,
`program-entry-plan`, `native-artifact`, and `image-emission` to lower verified Terminal Psi
modules to native code.
*/

pub mod access;
pub mod bytes;
pub mod calls;
pub mod content_transformations;
pub use content_transformations::{domain_preservation, quotients};
pub mod control_graph;
mod descriptor_replay;
pub mod descriptors;
pub mod plain_affine;
pub mod references;
pub mod scalars;
pub mod storage;
pub mod structural;
pub mod substitution;

pub use access::access_lifecycles::{
    AccessResultContract, access_result_contract, access_view_reference_slot,
    validate_access_lifecycle_plans, validate_access_view_content,
};
pub use access::access_ram_correspondence::{
    AccessRamCorrespondence, AccessRamFieldCorrespondence, access_ram_correspondence,
    access_ram_plan_correspondence, access_reference_correspondence,
};
pub use access::access_view_plans::{
    AccessViewPlanError, contains_access_carrier, contains_access_carrier_in_catalog,
    validate_access_view_catalog, validate_access_view_plans,
};
pub use access::accessor_scalar_boundaries::{
    AccessorScalarBoundaryDemand, access_region_projection, accessor_scalar_boundary_demand,
};
pub use calls::closed_conformances;
pub use calls::closed_reach;
pub use descriptors::descriptor_results;
pub(crate) use optimization::digest_encoder;
pub use references::resident_references::{
    resident_reference_plan, validate_resident_owner_fields,
};
pub use storage::activation_storage;
pub use storage::borrowed_storage;
pub use storage::calling_storage::{
    ForeignRecordStorage, callback_storage_has_slots, calling_abi_extent_dependencies,
    foreign_opaque_carrier, foreign_record_storage, foreign_storage_extent_dependencies,
};
pub use storage::run_custody::{EmptyRunReleaseContract, empty_run_release_contract};
pub use storage::storage_dependencies::{
    inline_storage_extent_dependencies, storage_operand_dependencies,
};
pub use storage::structural_locals::validate_vacant_structural_local;
pub use structural::nominal_cleanup;
pub use structural::owned_structure;
pub(crate) mod exactly_one {

    pub(crate) fn exactly_one<Row>(
        rows: impl Iterator<Item = Row>,
        keep: impl Fn(&Row) -> bool,
    ) -> Option<Row> {
        let mut matching = rows.filter(|row| keep(row));
        let row = matching.next()?;
        matching.next().is_none().then_some(row)
    }
}
pub use calls::invocation_inputs::validate_empty_erased_invocation_inputs;
pub use descriptors::membership_presentations::{
    present_structural_membership_contract, present_structural_memberships,
};
pub use descriptors::selected_memberships::{
    reference_carrier_membership_contract, reference_carrier_memberships,
    reference_source_memberships,
};
pub use domain_preservation::{
    domain_predicate_is_closed, domain_predicate_observes_no_storage, domain_write_is_disjoint,
    reference_preserves_ancestor_qualifications, scalar_field_store_preserves_content,
    scalar_field_store_preserves_domain_content, store_preserves_content,
    store_preserves_domain_content, store_preserves_qualifications,
};
pub use references::reference_completion::{
    ReferenceCompletionError, ReferenceCompletionOccurrence, borrowed_entry_reference_referent,
    cleanup_disposes_reference, validate_reference_result_completion,
};
pub use references::reference_origins::{
    BorrowedResultLeaf, BorrowedResultLeafKind, ReferenceCallContract, ReferenceCallOrigin,
    ReferenceCallResult, ReferenceCallSource, ReferenceLeaf, ReferenceOriginBinding,
    ReferenceResultMemberships, bind_public_reference_call_results, bind_reference_call_results,
    bind_reference_origin, borrowed_result_leaves, captured_reference_origin_type,
    collection_element_type, reference_call_preserved_memberships, reference_leaves,
    reference_origin_is_covered, reference_origin_paths_may_overlap, reference_origin_type,
    reference_result_memberships_weaken, reference_result_refines_regions,
    validate_boundary_reference_regions, validate_descriptor_result_regions,
    validate_dynamic_reference_regions,
};
pub use references::reference_storage_geometry::{
    RecordStorageResolver, ReferenceArrayWindow, ReferenceRecordField, ReferenceRepeatedField,
    ReferenceScalarStorage, ReferenceStorageGeometry, ReferenceStorageProjection,
    ReferenceStorageRegion, StorageGeometryError, UnselectedRecordStorage,
    reference_scalar_storage, reference_storage_dependencies, reference_storage_layout,
    reference_storage_projection, reference_storage_region, reference_storage_size,
};
pub use scalars::scalar_demand;
pub use scalars::scalar_ordering;
pub use structural::inline_carriers::{
    InlineCarrierError, inline_carrier_declaration, inline_field_presentation,
    validate_inline_carriers,
};
pub(crate) mod semantic_rows {

    use crate::scalars::scalar_leaf_schema::goal_free_scalar_leaf;
    use crate::{
        GoalFreeScalarLeafSchema, ScalarLeafCrashPolicy, ScalarLeafDenotation,
        ScalarLeafFrontierPolicy, ScalarLeafGoalShape, ScalarLeafOperandShape,
        ScalarLeafResultShape,
    };
    use semantic_vocabulary::{IntegerType, PropositionError, ScalarType, ValueId};
    use terminal_psi::OperationKind;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum OperationSemanticCustody {
        LeafDenotation,
        CallComposition,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct OperationSemanticRow {
        pub(crate) tag: OperationSemanticTag,
        pub(crate) identity: &'static str,
        pub(crate) custody: OperationSemanticCustody,
        pub(crate) goal_free_scalar_leaf: Option<GoalFreeScalarLeafSchema>,
    }

    impl OperationSemanticRow {
        pub const fn tag(self) -> OperationSemanticTag {
            self.tag
        }

        pub const fn name(self) -> &'static str {
            self.tag.name()
        }

        pub const fn identity(self) -> &'static str {
            self.identity
        }

        pub const fn custody(self) -> OperationSemanticCustody {
            self.custody
        }

        pub const fn goal_free_scalar_leaf(self) -> Option<GoalFreeScalarLeafSchema> {
            self.goal_free_scalar_leaf
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum OperationSemanticError {
        MissingRow(OperationSemanticTag),
        DuplicateRow(OperationSemanticTag),
        MissingScalarResult(OperationSemanticTag),
        UnknownValue(ValueId),
        ResultShapeMismatch(OperationSemanticTag),
        OperandShapeMismatch(OperationSemanticTag),
        DenotationShapeMismatch(OperationSemanticTag),
        MissingProofBearingScalarRow(OperationSemanticTag),
        DuplicateProofBearingScalarRow(OperationSemanticTag),
        UnexpectedProofBearingScalarRow(OperationSemanticTag),
        ProofBearingScalarSchemaMismatch(OperationSemanticTag),
        MissingStructuralEffectRow(OperationSemanticTag),
        DuplicateStructuralEffectRow(OperationSemanticTag),
        UnexpectedStructuralEffectRow(OperationSemanticTag),
        StructuralEffectSchemaMismatch(OperationSemanticTag),
        StructuralEffectResultShapeMismatch(OperationSemanticTag),
        StructuralEffectActionShapeMismatch(OperationSemanticTag),
        MissingCallCompositionRow(OperationSemanticTag),
        DuplicateCallCompositionRow(OperationSemanticTag),
        UnexpectedCallCompositionRow(OperationSemanticTag),
        CallCompositionSchemaMismatch(OperationSemanticTag),
        NonzeroDivisorRequiresFixedInteger(IntegerType),
        NonzeroDivisorTypeMismatch {
            declared: IntegerType,
            actual: ScalarType,
        },
        ExactDivisionRequiresFixedInteger(IntegerType),
        ExactDivisionOperandTypeMismatch {
            declared: IntegerType,
            left: ScalarType,
            right: ScalarType,
        },
        ExactShiftCountRequiresFixedValueInteger(IntegerType),
        ExactShiftCountRequiresFixedCountInteger(IntegerType),
        ExactShiftCountTypeMismatch {
            declared: IntegerType,
            actual: ScalarType,
        },
        ExactShiftLeftValueTypeMismatch {
            declared: IntegerType,
            actual: ScalarType,
        },
        ExactShiftLeftRequiresValueOrLiteralOperand,
        ExactShiftLeftRequiresValueOrLiteralCount,
        ExactArithmeticRequiresFixedInteger(IntegerType),
        ExactArithmeticExpressionTypeMismatch {
            declared: IntegerType,
            actual: ScalarType,
        },
        ExactArithmeticExpressionShapeMismatch,
        ExactArithmeticOperandTypeMismatch {
            declared: IntegerType,
            left: ScalarType,
            right: ScalarType,
        },
        ExactArithmeticRequiresValueOrLiteralOperand,
        ExactCastUnsupportedCarriers {
            source: IntegerType,
            target: IntegerType,
        },
        ExactCastOperandTypeMismatch {
            declared: IntegerType,
            actual: ScalarType,
        },
        ExactCastRequiresValueOrLiteralOperand,
        InvalidProposition(PropositionError),
    }

    macro_rules! operation_semantic_rows {
        ($( $variant:ident => ($identity:literal, $custody:ident, $schema:expr) ),+ $(,)?) => {
            #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
            pub enum OperationSemanticTag {
                $( $variant ),+
            }

            impl OperationSemanticTag {
                pub const ALL: [Self; operation_semantic_rows!(@count $( $variant )+)] = [
                    $( Self::$variant ),+
                ];

                pub const fn name(self) -> &'static str {
                    match self {
                        $( Self::$variant => stringify!($variant) ),+
                    }
                }

                pub const fn for_operation(operation: &OperationKind) -> Self {
                    match operation {
                        $( OperationKind::$variant { .. } => Self::$variant ),+
                    }
                }
            }

            impl OperationSemanticRow {
                pub const ALL: [Self; operation_semantic_rows!(@count $( $variant )+)] = [
                    $( Self {
                        tag: OperationSemanticTag::$variant,
                        identity: $identity,
                        custody: OperationSemanticCustody::$custody,
                        goal_free_scalar_leaf: $schema,
                    } ),+
                ];
            }
        };
        (@count $( $variant:ident )+) => {
            <[()]>::len(&[$(operation_semantic_rows!(@unit $variant)),+])
        };
        (@unit $variant:ident) => { () };
    }

    use ScalarLeafDenotation as Denotation;
    use ScalarLeafOperandShape as Operands;
    use ScalarLeafResultShape as ResultShape;

    operation_semantic_rows! {
        ReadScalarApplication => ("schema:operation:read-scalar-application", LeafDenotation, None),
        EstablishActivationClaim => ("schema:operation:establish-activation-claim", LeafDenotation, None),
        ReleaseActivationClaim => ("schema:operation:release-activation-claim", LeafDenotation, None),
        SplitActivationClaim => ("schema:operation:split-activation-claim", LeafDenotation, None),
        MergeActivationClaim => ("schema:operation:merge-activation-claim", LeafDenotation, None),
        StorageAddress => ("schema:operation:storage-address", LeafDenotation, None),
        EstablishAccessReference => ("schema:operation:establish-placed-reference", LeafDenotation, None),
        EstablishReference => ("schema:operation:establish-reference", LeafDenotation, None),
        ReleaseReference => ("schema:operation:release-reference", LeafDenotation, None),
        EstablishPrimitiveLocal => ("schema:operation:establish-primitive-local", LeafDenotation, None),
        PrimitiveScalarRead => ("schema:operation:primitive-scalar-read", LeafDenotation, None),
        StructuralCaseMembership => ("schema:operation:structural-case-membership", LeafDenotation, None),
        StructuralLeafCopy => ("schema:operation:structural-leaf-copy", LeafDenotation, None),
        MoveStructuralValue => ("schema:operation:move-structural-value", LeafDenotation, None),
        TransformContent => ("schema:operation:transform-content", LeafDenotation, None),
        StructuralCaseLeafCopy => ("schema:operation:structural-case-leaf-copy", LeafDenotation, None),
        WriteOnlyPrimitiveStore => ("schema:operation:write-only-primitive-store", LeafDenotation, None),
        StructuralScalarFieldStore => ("schema:operation:structural-scalar-field-store", LeafDenotation, None),
        MoveStructuralField => ("schema:operation:move-structural-field", LeafDenotation, None),
        StoreStructuralField => ("schema:operation:store-structural-field", LeafDenotation, None),
        StructuralByteSequenceFieldStore => ("schema:operation:structural-byte-sequence-field-store", LeafDenotation, None),
        StructuralByteSequenceFieldLength => ("schema:operation:structural-byte-sequence-field-length", LeafDenotation, None),
        StructuralByteSequenceFieldByteStore => ("schema:operation:structural-byte-sequence-field-byte-store", LeafDenotation, None),
        StructuralByteSequenceFieldRead => ("schema:operation:structural-byte-sequence-field-read", LeafDenotation, None),
        StructuralByteViewFieldStore => ("schema:operation:structural-byte-view-field-store", LeafDenotation, None),
        EstablishScalarCase => ("schema:operation:establish-scalar-case", LeafDenotation, None),
        EstablishStructuralCase => ("schema:operation:establish-structural-case", LeafDenotation, None),
        EstablishStructuralArray => ("schema:operation:establish-structural-array", LeafDenotation, None),
        EstablishScalarArray => ("schema:operation:establish-scalar-array", LeafDenotation, None),
        EstablishZeroedStructural => ("schema:operation:establish-zeroed-structural", LeafDenotation, None),
        EstablishStructuralLocal => ("schema:operation:establish-structural-local", LeafDenotation, None),
        EstablishByteSequenceConcatenation => ("schema:operation:establish-byte-sequence-concatenation", LeafDenotation, None),
        EstablishByteSequenceLiteral => ("schema:operation:establish-byte-sequence-literal", LeafDenotation, None),
        ByteSequenceLength => ("schema:operation:byte-sequence-length", LeafDenotation, None),
        ByteSequenceStore => ("schema:operation:byte-sequence-store", LeafDenotation, None),
        ByteSequenceWrite => ("schema:operation:byte-sequence-write", LeafDenotation, None),
        ByteSequenceRead => ("schema:operation:byte-sequence-read", LeafDenotation, None),
        ByteSequenceSubslice => ("schema:operation:byte-sequence-subslice", LeafDenotation, None),
        ByteSequencePredicate => ("schema:operation:byte-sequence-predicate", LeafDenotation, None),
        ByteSequenceEqual => ("schema:operation:byte-sequence-equal", LeafDenotation, None),
        EstablishElementView => ("schema:operation:establish-element-view", LeafDenotation, None),
        EstablishMutableElementView => ("schema:operation:establish-mutable-element-view", LeafDenotation, None),
        ElementViewLength => ("schema:operation:element-view-length", LeafDenotation, None),
        ElementViewRead => ("schema:operation:element-view-read", LeafDenotation, None),
        ElementViewWrite => ("schema:operation:element-view-write", LeafDenotation, None),
        ElementViewSubslice => ("schema:operation:element-view-subslice", LeafDenotation, None),
        EstablishTrivialAffineLocal => ("schema:operation:establish-trivial-affine-local", LeafDenotation, None),
        EstablishRecord => ("schema:operation:establish-record", LeafDenotation, None),
        EstablishNativeCodeLayout => ("schema:operation:establish-native-code-layout", LeafDenotation, None),
        Call => ("algebra:call:call", CallComposition, None),
        CallUnit => ("algebra:call:call-unit", CallComposition, None),
        CallStructuralScalar => ("algebra:call:call-structural-scalar", CallComposition, None),
        CallStructural => ("algebra:call:call-structural", CallComposition, None),
        CallStructuralWithScalarArguments => ("algebra:call:call-structural-with-scalar-arguments", CallComposition, None),
        BoundaryCall => ("algebra:call:boundary-call", CallComposition, None),
        SelectImplementation => ("schema:operation:select-implementation", LeafDenotation, None),
        CallDynamic => ("algebra:call:call-dynamic", CallComposition, None),
        PortWrite => ("schema:operation:port-write", LeafDenotation, None),
        RegisterAccess => ("schema:operation:register-access", LeafDenotation, None),
        IntegerConstant => ("schema:operation:integer-constant", LeafDenotation,
            goal_free_scalar_leaf(ResultShape::DeclaredInteger, Operands::IntegerLiteral, Denotation::IntegerConstant)),
        BooleanConstant => ("schema:operation:boolean-constant", LeafDenotation,
            goal_free_scalar_leaf(ResultShape::Boolean, Operands::BooleanLiteral, Denotation::BooleanConstant)),
        IeeeFloatConstant => ("schema:operation:ieee-float-constant", LeafDenotation, None),
        IeeeFloatCompare => ("schema:operation:ieee-float-compare", LeafDenotation, None),
        ScalarRepresentationView => ("schema:operation:scalar-representation-view", LeafDenotation, None),
        NearestIeeeFloatFusedMultiplyAdd => ("schema:operation:nearest-ieee-float-fused-multiply-add", LeafDenotation, None),
        BooleanStructuralField => ("schema:operation:boolean-structural-field", LeafDenotation, None),
        ScalarStructuralField => ("schema:operation:scalar-structural-field", LeafDenotation, None),
        BooleanNot => ("schema:operation:boolean-not", LeafDenotation,
            goal_free_scalar_leaf(ResultShape::Boolean, Operands::UnaryBoolean, Denotation::BooleanNot)),
        BooleanEqual => ("schema:operation:boolean-equal", LeafDenotation,
            goal_free_scalar_leaf(ResultShape::Boolean, Operands::BinaryBoolean, Denotation::BooleanEqual)),
        IntegerEqual => ("schema:operation:integer-equal", LeafDenotation,
            goal_free_scalar_leaf(ResultShape::Boolean, Operands::BinaryInteger, Denotation::IntegerEqual)),
        IntegerLessThan => ("schema:operation:integer-less-than", LeafDenotation,
            goal_free_scalar_leaf(ResultShape::Boolean, Operands::BinaryInteger, Denotation::IntegerLessThan)),
        IntegerLessOrEqual => ("schema:operation:integer-less-or-equal", LeafDenotation,
            goal_free_scalar_leaf(ResultShape::Boolean, Operands::BinaryInteger, Denotation::IntegerLessOrEqual)),
        IntegerBitwiseNot => ("schema:operation:integer-bitwise-not", LeafDenotation,
            goal_free_scalar_leaf(ResultShape::DeclaredInteger, Operands::UnaryInteger, Denotation::IntegerBitwiseNot)),
        IntegerWiden => ("schema:operation:integer-widen", LeafDenotation,
            goal_free_scalar_leaf(ResultShape::DeclaredInteger, Operands::WideningInteger, Denotation::IntegerWiden)),
        IntegerExactCast => ("schema:operation:integer-exact-cast", LeafDenotation, None),
        IntegerBitwiseAnd => ("schema:operation:integer-bitwise-and", LeafDenotation,
            goal_free_scalar_leaf(ResultShape::DeclaredInteger, Operands::BinaryInteger, Denotation::IntegerBitwiseAnd)),
        IntegerBitwiseOr => ("schema:operation:integer-bitwise-or", LeafDenotation,
            goal_free_scalar_leaf(ResultShape::DeclaredInteger, Operands::BinaryInteger, Denotation::IntegerBitwiseOr)),
        IntegerBitwiseXor => ("schema:operation:integer-bitwise-xor", LeafDenotation,
            goal_free_scalar_leaf(ResultShape::DeclaredInteger, Operands::BinaryInteger, Denotation::IntegerBitwiseXor)),
        WrappingIntegerShiftLeft => ("schema:operation:wrapping-integer-shift-left", LeafDenotation,
            goal_free_scalar_leaf(ResultShape::DeclaredInteger, Operands::IntegerShift, Denotation::WrappingIntegerShiftLeft)),
        WrappingIntegerShiftRight => ("schema:operation:wrapping-integer-shift-right", LeafDenotation,
            goal_free_scalar_leaf(ResultShape::DeclaredInteger, Operands::IntegerShift, Denotation::WrappingIntegerShiftRight)),
        ExactIntegerShiftLeft => ("schema:operation:exact-integer-shift-left", LeafDenotation, None),
        ExactIntegerShiftRight => ("schema:operation:exact-integer-shift-right", LeafDenotation, None),
        ExactIntegerAdd => ("schema:operation:exact-integer-add", LeafDenotation, None),
        ExactIntegerSubtract => ("schema:operation:exact-integer-subtract", LeafDenotation, None),
        ExactIntegerMultiply => ("schema:operation:exact-integer-multiply", LeafDenotation, None),
        ExactIntegerDivide => ("schema:operation:exact-integer-divide", LeafDenotation, None),
        ExactIntegerRemainder => ("schema:operation:exact-integer-remainder", LeafDenotation, None),
        WrappingIntegerDivide => ("schema:operation:wrapping-integer-divide", LeafDenotation, None),
        WrappingIntegerRemainder => ("schema:operation:wrapping-integer-remainder", LeafDenotation, None),
        SaturatingIntegerDivide => ("schema:operation:saturating-integer-divide", LeafDenotation, None),
        SaturatingIntegerRemainder => ("schema:operation:saturating-integer-remainder", LeafDenotation, None),
        WrappingIntegerAdd => ("schema:operation:wrapping-integer-add", LeafDenotation,
            goal_free_scalar_leaf(ResultShape::DeclaredInteger, Operands::BinaryInteger, Denotation::WrappingIntegerAdd)),
        SaturatingIntegerAdd => ("schema:operation:saturating-integer-add", LeafDenotation,
            goal_free_scalar_leaf(ResultShape::DeclaredInteger, Operands::BinaryInteger, Denotation::SaturatingIntegerAdd)),
        WrappingIntegerSubtract => ("schema:operation:wrapping-integer-subtract", LeafDenotation,
            goal_free_scalar_leaf(ResultShape::DeclaredInteger, Operands::BinaryInteger, Denotation::WrappingIntegerSubtract)),
        SaturatingIntegerSubtract => ("schema:operation:saturating-integer-subtract", LeafDenotation,
            goal_free_scalar_leaf(ResultShape::DeclaredInteger, Operands::BinaryInteger, Denotation::SaturatingIntegerSubtract)),
        WrappingIntegerMultiply => ("schema:operation:wrapping-integer-multiply", LeafDenotation,
            goal_free_scalar_leaf(ResultShape::DeclaredInteger, Operands::BinaryInteger, Denotation::WrappingIntegerMultiply)),
        SaturatingIntegerMultiply => ("schema:operation:saturating-integer-multiply", LeafDenotation,
            goal_free_scalar_leaf(ResultShape::DeclaredInteger, Operands::BinaryInteger, Denotation::SaturatingIntegerMultiply)),
        TrappingInteger => ("schema:operation:trapping-integer", LeafDenotation, None),
        FloatResultPolicy => ("schema:operation:float-result-policy", LeafDenotation, None),
        AtomicAccess => ("schema:operation:atomic-access", LeafDenotation, None),
        AtomicFence => ("schema:operation:atomic-fence", LeafDenotation, None),
        ActivateTaskRegister => ("schema:operation:activate-task-register", LeafDenotation, None),
        ReloadSegmentDescriptors => ("schema:operation:reload-segment-descriptors", LeafDenotation, None),
        ReadSegmentSelector => ("schema:operation:read-segment-selector", LeafDenotation, None),
        ReadTaskRegister => ("schema:operation:read-task-register", LeafDenotation, None),
        GlobalDescriptorTable => ("schema:operation:global-descriptor-table", LeafDenotation, None),
    }

    pub fn exact_operation_semantic_row_in(
        tag: OperationSemanticTag,
        rows: &[OperationSemanticRow],
    ) -> Result<&OperationSemanticRow, OperationSemanticError> {
        exactly_one_row_for_tag(
            rows,
            tag,
            |row| row.tag,
            OperationSemanticError::MissingRow,
            OperationSemanticError::DuplicateRow,
        )
    }

    pub fn operation_semantic_row(
        operation: &OperationKind,
    ) -> Result<&'static OperationSemanticRow, OperationSemanticError> {
        exact_operation_semantic_row_in(
            OperationSemanticTag::for_operation(operation),
            &OperationSemanticRow::ALL,
        )
    }

    pub fn is_unconditionally_total_scalar(operation: &OperationKind) -> bool {
        matches!(operation, OperationKind::IeeeFloatConstant { .. })
            || operation_semantic_row(operation).is_ok_and(|row| {
                row.goal_free_scalar_leaf().is_some_and(|schema| {
                    schema.goal() == ScalarLeafGoalShape::None
                        && schema.crash() == ScalarLeafCrashPolicy::Never
                        && schema.frontier() == ScalarLeafFrontierPolicy::PreserveLocal
                })
            })
    }

    pub(crate) fn exactly_one_row_for_tag<R>(
        rows: &[R],
        tag: OperationSemanticTag,
        row_tag: impl Fn(&R) -> OperationSemanticTag,
        missing: impl FnOnce(OperationSemanticTag) -> OperationSemanticError,
        duplicate: impl FnOnce(OperationSemanticTag) -> OperationSemanticError,
    ) -> Result<&R, OperationSemanticError> {
        let mut matches = rows.iter().filter(|row| row_tag(row) == tag);
        let row = matches.next().ok_or_else(|| missing(tag))?;
        if matches.next().is_some() {
            return Err(duplicate(tag));
        }
        Ok(row)
    }

    macro_rules! operation_semantic_cohort {
        (
            rows: $row:ty,
            tags: $tags:ident,
            is_tag: $is_tag:ident,
            exact_row: $exact_row:ident,
            reject_unexpected: $reject_unexpected:ident,
            missing: $missing:ident,
            duplicate: $duplicate:ident,
            unexpected: $unexpected:ident,
            $( $tag:ident ),+ $(,)?
        ) => {
            const $tags: [OperationSemanticTag;
                $crate::semantic_rows::operation_semantic_cohort!(@count $( $tag )+)] = [
                $( OperationSemanticTag::$tag ),+
            ];

            const fn $is_tag(tag: OperationSemanticTag) -> bool {
                matches!(tag, $( OperationSemanticTag::$tag )|+)
            }

            pub fn $exact_row(
                tag: OperationSemanticTag,
                rows: &[$row],
            ) -> Result<Option<&$row>, OperationSemanticError> {
                if !$is_tag(tag) {
                    return Ok(None);
                }
                $crate::semantic_rows::exactly_one_row_for_tag(
                    rows,
                    tag,
                    |row| row.tag,
                    OperationSemanticError::$missing,
                    OperationSemanticError::$duplicate,
                )
                .map(Some)
            }

            fn $reject_unexpected(rows: &[$row]) -> Result<(), OperationSemanticError> {
                for row in rows {
                    if !$is_tag(row.tag) {
                        return Err(OperationSemanticError::$unexpected(row.tag));
                    }
                }
                Ok(())
            }
        };
        (@count $( $variant:ident )+) => {
            <[()]>::len(&[ $( $crate::semantic_rows::operation_semantic_cohort!(@unit $variant) ),+ ])
        };
        (@unit $variant:ident) => { () };
    }

    pub(crate) use operation_semantic_cohort;
}
pub use structural::structural_store::{discardable_store_value, structural_store_value_admitted};

pub use access::access_view_referent::{
    AccessViewInputError, AccessViewReferentError, access_view_input_parameter,
    validate_access_view_referent,
};
pub use bytes::byte_carriers::{
    ByteCarrierLayout, ByteCarrierScalarLeaf, byte_carrier_scalar_layout,
};
pub use bytes::byte_sequence_concatenation::{
    byte_sequence_concatenation_capacity_goal, byte_sequence_concatenation_length_equation,
};
pub use bytes::byte_sequence_predicates::{
    MachineScalarIndex, ScalarConstantOrigin, byte_copied_field_predicates_at_transfer,
    byte_field_root, byte_literal_qualifications_hold, byte_literal_qualifications_hold_in,
    byte_owned_call_result, byte_owned_result_predicates, byte_parameter_predicates,
    byte_parameter_predicates_from_domains, byte_parameter_predicates_in, byte_predicate_carrier,
    byte_predicate_requirement, byte_qualification_predicates_in,
    byte_sequence_concatenation_predicates, byte_sequence_source_predicates, byte_view_call_result,
    indexed_byte_store_goal, machine_scalar_constant_origin, pure_byte_qualifications,
    scalar_array_qualifications_hold, scalar_constant_value, shared_byte_field_copy_predicates,
};
pub use calls::call_composition::{
    BorrowedReferentPresentation, CallArgumentRule, CallCompositionSchema,
    CallCompositionSemanticRow, CallCrashRule, CallEvidenceRule, CallFrontierRule, CallOutcomeRule,
    CallRequirementRule, CallResultRule, CallTargetRule, CallTransferRule, CallWorkPolicy,
    FixedCollectionWindow, borrowed_referent_matches, borrowed_referent_presentation,
    boundary_buffer_capacity, bounded_byte_owner_capacity,
    bounded_byte_owner_capacity_from_catalogs, bounded_byte_owner_capacity_in,
    call_composition_semantic_row, exact_call_composition_semantic_row_in, fixed_byte_array_extent,
    fixed_byte_array_loan, fixed_byte_array_window, fixed_byte_extent, fixed_byte_storage_window,
    fixed_collection_storage_window, fixed_element_array_extent, shared_boundary_buffer_capacity,
    stored_view_field_presentation, validate_call_composition_semantic_rows,
};
pub use references::reference_views::{
    borrowed_field_presentation, descriptor_geometry_matches, descriptor_reborrow_matches,
    descriptor_reborrow_shapes_match, descriptor_source_projection_accessible,
    descriptor_value_transfer_matches, is_copyable_carrier, is_shared_reference_carrier,
    reference_byte_field_compatible, reference_source_compatible,
    reference_source_projection_accessible, reference_source_projection_compatible,
    reference_source_storage_path, reference_storage_byte_width,
    reference_storage_export_byte_width, reference_storage_region_source_compatible,
    reference_storage_source_compatible, reference_storage_view_is_compatible,
    representation_view_declaration, scalar_reference_view_admits, scalar_reference_view_is_total,
    scalar_reference_view_value, shared_descriptor_copy_matches, stored_reference_access,
};
pub use scalars::proof_bearing_scalar::{
    CanonicalScalarGoal, ProofBearingIntegerPolicyBinding, ProofBearingScalarLeafElision,
    ProofBearingScalarLeafSchema, ProofBearingScalarLeafSemantics, ProofBearingScalarSemanticRow,
    elidable_proof_bearing_scalar_leaf, exact_proof_bearing_scalar_semantic_row_in,
    proof_bearing_integer_policy_binding, proof_bearing_scalar_leaf_semantics,
    proof_bearing_scalar_semantic_row, validate_proof_bearing_scalar_semantic_rows,
};
pub use scalars::scalar_array::{
    scalar_array_leaf_declaration, scalar_array_leaf_path, scalar_array_leaf_shape,
};
pub use scalars::scalar_field_requirements::{
    atomic_resident_requirement, scalar_field_accepts_literal, scalar_field_accepts_zero,
    scalar_field_entails, scalar_field_initialization_requirement, scalar_field_requirement,
    scalar_field_requires_evidence, scalar_field_requires_initialization_evidence,
};
pub use scalars::scalar_leaf_schema::{
    GoalFreeScalarLeafSchema, ScalarLeafCrashPolicy, ScalarLeafDenotation, ScalarLeafFactShape,
    ScalarLeafFrontierPolicy, ScalarLeafGoalShape, ScalarLeafOperandShape, ScalarLeafResultShape,
    ScalarLeafSchema, ScalarLeafWorkPolicy,
};
pub use scalars::scalar_leaf_semantics::{
    GoalFreeScalarLeafSemantics, ScalarLeafLiteral, SortedValueTypes, ValueTypes,
    constant_goal_free_scalar_leaf, goal_free_scalar_leaf_semantics,
};
pub use scalars::scalar_representation_view::{ScalarRepresentationViewPair, ScalarViewValue};
pub use semantic_rows::{
    OperationSemanticCustody, OperationSemanticError, OperationSemanticRow, OperationSemanticTag,
    exact_operation_semantic_row_in, is_unconditionally_total_scalar, operation_semantic_row,
};
pub use storage::arrival_storage::{
    StorageNames, arrival_parameter, borrowed_storage_parameter,
    borrowed_storage_reference_ceiling, descriptor_parameter_access, projection_before_edge,
    root_before_edge, storage_names, view_descriptor_parameter,
    view_descriptor_projected_qualifications,
};
pub use storage::collection_backing::{
    CollectionBacking, CollectionExtent, CollectionStorage, CollectionViewDescriptor,
    collection_backing, collection_view_descriptor,
};
pub use storage::storage_regions::{
    StorageRegion, canonical_field_storage_region, crosses_unaliased_reference,
    projected_storage_region,
};
pub use storage::zero_valid_storage::{scalar_declaration_accepts_zero, zero_valid_record_storage};
pub use structural::atomic_cell::atomic_cell_type;
pub use structural::partial_affine::partial_affine_residuals;
pub use structural::primitive_place::{
    Position, StructuralStorageSelection, borrowed_primitive_projection_declaration,
    byte_carrier_geometry_matches, canonical_primitive_path, canonical_primitive_read_path,
    canonical_static_projection, fixed_array_place_shape, inline_backing_container,
    primitive_place_type, primitive_projection_declaration, primitive_projection_restriction,
    primitive_projection_type, primitive_read_field, primitive_read_projection_type,
    primitive_read_requirement, projection_stays_in_inline_storage,
    projection_storage_dependencies, runtime_structural_copy_type, runtime_structural_path_tip,
    stored_descriptor_paths,
};
pub use structural::record_field::{
    CasePayloadCarrier, RecordFieldCarrier, SequenceFieldObservation, case_payload_carrier,
    observed_field_backing_access, observed_field_declaration, projected_field_declaration,
    record_field_carrier, runtime_structural_field, sequence_field_allows_observation,
    static_field_projection,
};
pub use structural::shared_mutation::{SharedMutation, shared_mutation};
pub use structural::static_path::{
    canonical_case_leaf_type, canonical_storage_dependencies, canonical_structural_path_tip,
    canonical_structural_storage_path, constant_runtime_index, interior_member_borrow,
    qualification_path_carrier, runtime_index_bound, runtime_index_selects,
    structural_memberships_disjoint_from_write, structural_paths_may_overlap,
    structural_paths_select_same_location,
};
pub use structural::structural_effect::{
    StructuralEffectAction, StructuralEffectCustody, StructuralEffectExternalEffect,
    StructuralEffectFrontierPolicy, StructuralEffectGoalShape, StructuralEffectLeafSchema,
    StructuralEffectObservation, StructuralEffectResultShape, StructuralEffectSemanticRow,
    StructuralEffectWorkPolicy, byte_equal_equation, byte_predicate_equation,
    element_establishment_length_equation, element_subslice_length_equation,
    element_view_leaf_scalar, exact_structural_effect_semantic_row_in, literal_length_equation,
    same_descriptor_coordinate, structural_effect_leaf_observation,
    structural_effect_leaf_observation_in, structural_effect_semantic_row,
    subslice_length_equation, validate_structural_effect_semantic_rows,
};
pub use structural::structural_window::{
    StructuralWindowProjection, structural_window_origins, structural_window_projection,
    structural_window_selects_child,
};
pub use structural::zeroed_structural::{
    EstablishedErasedStructuralField, ZeroedStructuralLeaf,
    zeroed_projected_byte_qualifications_hold, zeroed_structural_fields_satisfy,
    zeroed_structural_leaves, zeroed_structural_leaves_modulo_references,
    zeroed_structural_leaves_with_established_erased_fields, zeroed_structural_shape,
};

pub use descriptors::descriptor_lifetimes;

pub use storage::representation_regions::{
    ElementViewRegionGeometry, RepresentationElementLayout, RepresentationScalarLeaf,
    element_view_region_geometry, element_view_shape_region_geometry, representation_byte_extent,
    representation_element_layout, representation_region_goal, representation_region_length,
    whole_representation_count_goal,
};

pub use storage::element_representation::{
    ElementRepresentationLayout, ElementRepresentationLeaf, ElementRepresentationRelation,
    element_representation_layout, element_representation_relation,
};

pub use storage::collection_backing::{
    collection_view_backing, collection_view_preserves_coordinates,
};

pub use structural::structural_claims::{
    ResultClaimCatalogError, result_claim_paths_match_returns, structural_claim_paths_may_overlap,
    validate_result_claim_catalog,
};
pub use structural::structural_effect::{
    ElementViewLeaf, element_view_leaf, element_view_write_goal,
};

pub use scalars::scalar_observations::structural_read_scalar_type;

pub use access::access_ram_origins::{
    AccessRamFormation, AccessRamOrigin, AccessRamOriginAnalysis,
};

pub use descriptors::primitive_store_memberships::{
    primitive_scalar_memberships, primitive_store_byte_predicates,
    primitive_store_preserves_scalar_memberships, primitive_store_restriction,
    scalar_projection_entails_membership,
};

pub use calls::evidence_calls;
pub use calls::evidence_calls::validate_runtime_evidence_inputs;
pub use scalars::unchanged_scalar::{UnchangedScalarOrigin, unchanged_scalar_identity};

pub(crate) use structural::structural_declarations::{structural_declaration, structural_shape};
