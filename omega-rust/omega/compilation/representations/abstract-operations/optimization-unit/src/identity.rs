/*
We compute the canonical cryptographic identities (`OptimizationUnitIdentity`,
`scalar_refinement_identity_bytes`, `access_view_catalog_identity_bytes`) for
`PsiOptimizationUnit` and its structural, proposition, and runtime-evidence
catalogs. Every declaration, fact, proof question, frontier snapshot, and
function body is serialized into `CanonicalBytes` in a deterministic order so
any structural or semantic drift produces a distinct identity hash.
*/

use abstract_operations::{AbstractFunctionResult, CompletionClaimSource, ValueBinding};
use optimization_core::OptimizationUnitIdentity;
use semantic_vocabulary::{
    ByteSequenceStructuralField, CanonicalStructuralPathSegment, ContentAlgebra,
    ContentAlgebraKind, ContentConservation, ContentPlaceSegment, ContentPlaceVersion,
    ContentProjectionExpression, ContentProjectionScalar, ContentStructuralPlace, ContentTerm,
    IeeeFloatComparisonKind, IeeeFloatFormat, IeeeFloatStructuralField, IntegerMathTerm,
    IntegerSign, IntegerType, IntegerValue, Proposition, PsiSemanticId, ScalarTerm, ScalarType,
    StructuralCaseSubject, StructuralPlaceKind,
};
use terminal_psi::{
    BindingRelevance, BoundaryContentGuarantee, BoundaryMachineDeclaration, ByteSequenceCarrier,
    ClaimContentProjection, ContentConservationGuarantee, CrashCause, CrashPredicateTerm,
    CrashRouteBucket, CrashRouteGuard, EntryClaim, EvidenceContractLane, EvidenceContractLaneKind,
    EvidenceInterfaceIdentity, MachineContract, OutcomeSpecificCallEvidence,
    ProgramLocalRootIntroductionSchema, ProviderCandidateConformance,
    RetainedBorrowContentProjection, RetainedBorrowCustody, RetainedBorrowPlace,
    RetainedBorrowPlaceRoot, ServiceDeclaration, StructuralAccess, StructuralArgument,
    StructuralDomainDeclaration, StructuralDomainRequirement, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralPlaceDeclaration,
    StructuralResultDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalAffineCleanupAction, TerminalRootServiceReach,
};

use crate::{
    AcceptedObligationFact, EffectLink, OptimizationEdge, OptimizationFact, OptimizationNode,
    OwnershipEvent, OwnershipFrontierFact, OwnershipFrontierSite, OwnershipFrontierSnapshot,
    ProofQuestion, ProofQuestionAdmissionKind, ProofQuestionClass, ProofQuestionOwner,
    PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance, ValueDefinition,
    ValueDefinitionSite, ValueUse, WorkSettlement,
};

use unit_encoding::CanonicalBytes;

pub fn scalar_refinement_identity_bytes(
    refinement: &semantic_vocabulary::ScalarRefinement,
) -> Vec<u8> {
    let mut bytes = CanonicalBytes::collect();
    carrier_encoding::encode_scalar_type(&mut bytes, refinement.carrier());
    proposition_encoding::encode_proposition(&mut bytes, refinement.predicate());
    bytes.finish()
}

pub(crate) fn scalar_receiving_identity_bytes(
    receiving: &crate::SourceCheckedScalarReceiving,
) -> Vec<u8> {
    let mut bytes = CanonicalBytes::collect();
    bytes.id(receiving.value);
    structural_encoding::encode_scalar_restriction(&mut bytes, &receiving.restriction);
    bytes.finish()
}

pub use unit_encoding::{
    access_view_catalog_identity_bytes, recompute_psi_optimization_unit_identity,
};

pub(crate) mod carrier_encoding {
    use super::{
        CanonicalBytes, IntegerSign, IntegerType, IntegerValue, PsiSemanticId, ScalarType,
        StructuralAccess, StructuralArgument, StructuralMultiplicity,
        StructuralParameterDeclaration, StructuralPathSegment, ValueBinding,
        structural_encoding::encode_structural_operation_result,
    };
    pub(super) fn encode_binding(bytes: &mut CanonicalBytes, binding: &ValueBinding) {
        bytes.id(binding.parameter);
        bytes.id(binding.argument);
        encode_scalar_type(bytes, binding.scalar_type);
    }

    pub(super) fn encode_optional_id<T: PsiSemanticId>(
        bytes: &mut CanonicalBytes,
        value: Option<&T>,
    ) {
        bytes.boolean(value.is_some());
        if let Some(value) = value {
            bytes.id(*value);
        }
    }

    pub(super) fn encode_optional<T>(
        bytes: &mut CanonicalBytes,
        value: Option<&T>,
        encode: impl Fn(&mut CanonicalBytes, &T),
    ) {
        bytes.boolean(value.is_some());
        if let Some(value) = value {
            encode(bytes, value);
        }
    }

    pub(super) fn encode_ids<T: PsiSemanticId>(bytes: &mut CanonicalBytes, ids: &[T]) {
        bytes.len(ids.len());
        for id in ids {
            bytes.id(*id);
        }
    }

    pub(super) fn encode_abstract_result(
        bytes: &mut CanonicalBytes,
        result: abstract_operations::AbstractResult,
    ) {
        bytes.id(result.value);
        encode_scalar_type(bytes, result.scalar_type);
    }

    pub(super) fn encode_scalar_type(bytes: &mut CanonicalBytes, scalar_type: ScalarType) {
        crate::encode_scalar_type(
            bytes,
            scalar_type,
            |bytes, tag| bytes.u8(tag),
            encode_integer_type,
        )
    }

    pub(super) fn encode_integer_type(bytes: &mut CanonicalBytes, integer_type: IntegerType) {
        bytes.u8(match integer_type.sign() {
            IntegerSign::Unsigned => 1,
            IntegerSign::Signed => 2,
        });
        bytes.u16(integer_type.bits());
    }

    pub(super) fn encode_integer_value(bytes: &mut CanonicalBytes, value: IntegerValue) {
        match value {
            IntegerValue::Unsigned(value) => {
                bytes.u8(1);
                bytes.u128(value);
            }
            IntegerValue::Signed(value) => {
                bytes.u8(2);
                bytes.bytes(&value.to_le_bytes());
            }
        }
    }

    pub(super) fn encode_structural_parameter(
        bytes: &mut CanonicalBytes,
        parameter: &StructuralParameterDeclaration,
    ) {
        bytes.id(parameter.place);
        bytes.u32(parameter.position);
        bytes.id(parameter.structural_type);
        encode_multiplicity(bytes, parameter.multiplicity);
        encode_access(bytes, parameter.access);
        encode_ids(bytes, &parameter.qualifications);
        bytes.slice(
            &parameter.projected_qualifications,
            |bytes, qualification| {
                bytes.slice(&qualification.path, encode_structural_path_segment);
                bytes.id(qualification.domain);
            },
        );
        bytes.slice(
            &parameter.additional_input_qualifications,
            |bytes, qualification| {
                bytes.slice(&qualification.path, encode_structural_path_segment);
                bytes.id(qualification.domain);
            },
        );
    }

    pub(super) fn encode_structural_binding(
        bytes: &mut CanonicalBytes,
        binding: &abstract_operations::AbstractStructuralBinding,
    ) {
        bytes.id(binding.parameter);
        encode_structural_argument(bytes, &binding.argument);
    }

    pub(super) fn encode_structural_argument(
        bytes: &mut CanonicalBytes,
        argument: &StructuralArgument,
    ) {
        bytes.id(argument.place);
        bytes.slice(&argument.path, encode_structural_path_segment);
        encode_access(bytes, argument.access);
    }

    pub(super) fn encode_structural_path_segment(
        bytes: &mut CanonicalBytes,
        segment: &StructuralPathSegment,
    ) {
        match segment {
            StructuralPathSegment::AllElements => bytes.u8(10),
            StructuralPathSegment::Case(case) => {
                bytes.u8(7);
                bytes.u64(case.get());
            }
            StructuralPathSegment::Field(identity) => {
                bytes.u8(1);
                bytes.u32(identity.0);
            }
            StructuralPathSegment::FixedIndex(index) => {
                bytes.u8(2);
                bytes.u64(*index);
            }
            StructuralPathSegment::Referent => bytes.u8(3),
            StructuralPathSegment::FixedRange { start, end } => {
                bytes.u8(4);
                bytes.u64(*start);
                bytes.u64(*end);
            }
            StructuralPathSegment::RuntimeIndex { index, obligation } => {
                bytes.u8(5);
                bytes.u64(index.get());
                bytes.u64(obligation.get());
            }
            StructuralPathSegment::RepresentationView {
                structural_type,
                stored_scalar,
            } => {
                bytes.u8(8);
                bytes.id(*structural_type);
                bytes.boolean(stored_scalar.is_some());
                if let Some(scalar) = stored_scalar {
                    encode_scalar_type(bytes, *scalar);
                }
            }
        }
    }

    pub(super) fn encode_optional_access(
        bytes: &mut CanonicalBytes,
        access: Option<StructuralAccess>,
    ) {
        match access {
            Some(access) => encode_access(bytes, access),
            None => bytes.u8(0),
        }
    }

    pub(super) fn encode_access(bytes: &mut CanonicalBytes, access: StructuralAccess) {
        bytes.u8(match access {
            StructuralAccess::Owned => 1,
            StructuralAccess::SharedBorrow => 2,
            StructuralAccess::MutableBorrow => 3,
            StructuralAccess::WriteOnlyBorrow => 4,
        });
    }

    pub(super) fn encode_multiplicity(
        bytes: &mut CanonicalBytes,
        multiplicity: StructuralMultiplicity,
    ) {
        bytes.u8(match multiplicity {
            StructuralMultiplicity::Unrestricted => 1,
            StructuralMultiplicity::Affine => 2,
            StructuralMultiplicity::Linear => 3,
        });
    }

    pub(super) fn encode_window_root(
        bytes: &mut CanonicalBytes,
        root: &abstract_operations::StructuralWindowRoot,
    ) {
        match root {
            abstract_operations::StructuralWindowRoot::Parameter(parameter) => {
                bytes.u8(0);
                encode_structural_parameter(bytes, parameter);
            }
            abstract_operations::StructuralWindowRoot::OwnedResult { producer, result } => {
                bytes.u8(1);
                bytes.id(*producer);
                encode_structural_operation_result(bytes, result);
            }
            abstract_operations::StructuralWindowRoot::OwnedBlockParameter { block, parameter } => {
                bytes.u8(2);
                bytes.id(*block);
                encode_structural_parameter(bytes, parameter);
            }
        }
    }
}

pub(crate) mod proposition_encoding {
    use super::carrier_encoding::{
        encode_integer_type, encode_integer_value, encode_optional, encode_scalar_type,
    };
    use super::structural_encoding::{encode_content_algebra, encode_content_place};
    use super::{
        ByteSequenceStructuralField, CanonicalBytes, CanonicalStructuralPathSegment, ContentTerm,
        IeeeFloatComparisonKind, IeeeFloatFormat, IeeeFloatStructuralField, IntegerMathTerm,
        IntegerType, Proposition, ScalarTerm, StructuralCaseSubject,
    };
    pub(super) fn encode_proposition(bytes: &mut CanonicalBytes, proposition: &Proposition) {
        match proposition {
            Proposition::Truth => bytes.u8(1),
            Proposition::Falsehood => bytes.u8(2),
            Proposition::Atom(id) => {
                bytes.u8(3);
                bytes.id(*id);
            }
            Proposition::Equal(left, right) => {
                bytes.u8(4);
                encode_scalar_term(bytes, left);
                encode_scalar_term(bytes, right);
            }
            Proposition::LessThan(left, right) => {
                bytes.u8(5);
                encode_scalar_term(bytes, left);
                encode_scalar_term(bytes, right);
            }
            Proposition::LessOrEqual(left, right) => {
                bytes.u8(6);
                encode_scalar_term(bytes, left);
                encode_scalar_term(bytes, right);
            }
            semantic_vocabulary::integer_math_comparison_operands!(left, right) => {
                bytes.u8(match proposition {
                    Proposition::IntegerMathEqual(_, _) => 14,
                    Proposition::IntegerMathLessThan(_, _) => 15,
                    Proposition::IntegerMathLessOrEqual(_, _) => 16,
                    _ => unreachable!(),
                });
                encode_integer_math_term(bytes, left);
                encode_integer_math_term(bytes, right);
            }
            Proposition::Conjunction(values) => {
                bytes.u8(7);
                bytes.slice(values, encode_proposition);
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                bytes.u8(8);
                encode_proposition(bytes, premise);
                encode_proposition(bytes, conclusion);
            }
            Proposition::ContentConservation(value) => {
                bytes.u8(9);
                encode_content_algebra(bytes, value.algebra());
                encode_content_term(bytes, value.left());
                encode_content_term(bytes, value.right());
            }
            Proposition::Disjunction(values) => {
                bytes.u8(10);
                bytes.slice(values, encode_proposition);
            }
            Proposition::IeeeFloatComparison {
                kind,
                format,
                left,
                right,
            } => {
                bytes.u8(11);
                encode_float_comparison(bytes, *kind);
                encode_float_format(bytes, *format);
                encode_float_field(bytes, left);
                encode_float_field(bytes, right);
            }
            Proposition::ByteSequenceEqual { left, right } => {
                bytes.u8(12);
                encode_byte_field(bytes, left);
                encode_byte_field(bytes, right);
            }
            Proposition::StructuralCaseMembership { subject, case } => {
                bytes.u8(13);
                encode_case_subject(bytes, subject);
                bytes.id(*case);
            }
            Proposition::StructuralIdentity { left, right } => {
                bytes.u8(18);
                for subject in [left, right] {
                    bytes.id(subject.structural_type());
                    bytes.id(subject.root());
                    encode_canonical_path(bytes, subject.path());
                }
            }
            Proposition::ScalarIeeeFloatComparison {
                kind,
                format,
                left,
                right,
            } => {
                bytes.u8(17);
                encode_float_comparison(bytes, *kind);
                encode_float_format(bytes, *format);
                encode_scalar_term(bytes, left);
                encode_scalar_term(bytes, right);
            }
        }
    }

    fn encode_integer_math_term(bytes: &mut CanonicalBytes, term: &IntegerMathTerm) {
        match term {
            IntegerMathTerm::Embed(term) => {
                bytes.u8(7);
                encode_scalar_term(bytes, term);
            }
            IntegerMathTerm::IntegerLiteral(literal) => {
                bytes.u8(1);
                bytes.boolean(literal.negative());
                bytes.u128(literal.magnitude());
            }
            IntegerMathTerm::MathValue { source_type, value } => {
                bytes.u8(2);
                encode_integer_type(bytes, *source_type);
                bytes.id(*value);
            }
            IntegerMathTerm::Add(left, right)
            | IntegerMathTerm::Subtract(left, right)
            | IntegerMathTerm::Multiply(left, right) => {
                bytes.u8(match term {
                    IntegerMathTerm::Add(_, _) => 3,
                    IntegerMathTerm::Subtract(_, _) => 4,
                    IntegerMathTerm::Multiply(_, _) => 5,
                    _ => unreachable!(),
                });
                encode_integer_math_term(bytes, left);
                encode_integer_math_term(bytes, right);
            }
            IntegerMathTerm::ShiftLeft { value, count } => {
                bytes.u8(6);
                encode_integer_math_term(bytes, value);
                encode_integer_math_term(bytes, count);
            }
        }
    }

    pub(super) fn encode_content_term(bytes: &mut CanonicalBytes, term: &ContentTerm) {
        match term {
            ContentTerm::Projection {
                projection,
                subject,
            } => {
                bytes.u8(1);
                bytes.id(projection.domain);
                bytes.u64(projection.projection_report_fingerprint);
                encode_content_place(bytes, subject);
            }
            ContentTerm::Separate(terms) => {
                bytes.u8(2);
                bytes.slice(terms, encode_content_term);
            }
        }
    }

    pub(super) fn encode_scalar_term(bytes: &mut CanonicalBytes, term: &ScalarTerm) {
        use ScalarTerm as S;
        match term {
            S::Application {
                application,
                scalar_type,
            } => {
                bytes.u8(41);
                bytes.id(*application);
                encode_scalar_type(bytes, *scalar_type);
            }
            S::IeeeFloatField { root, path, format } => {
                bytes.u8(38);
                bytes.id(*root);
                encode_canonical_path(bytes, path);
                encode_float_format(bytes, *format);
            }
            S::IeeeFloatLiteral(value) => {
                bytes.u8(37);
                match value {
                    semantic_vocabulary::IeeeFloatValue::Binary32(bits) => {
                        bytes.u8(1);
                        bytes.u32(*bits);
                    }
                    semantic_vocabulary::IeeeFloatValue::Binary64(bits) => {
                        bytes.u8(2);
                        bytes.u64(*bits);
                    }
                }
            }
            S::Value { id, scalar_type } => {
                bytes.u8(1);
                bytes.id(*id);
                encode_scalar_type(bytes, *scalar_type);
            }
            S::Boolean(value) => {
                bytes.u8(2);
                bytes.boolean(*value);
            }
            S::Integer { scalar_type, value } => {
                bytes.u8(3);
                encode_integer_type(bytes, *scalar_type);
                encode_integer_value(bytes, *value);
            }
            S::BooleanField { root, path } => {
                bytes.u8(4);
                bytes.id(*root);
                encode_canonical_path(bytes, path);
            }
            S::IntegerField {
                root,
                path,
                scalar_type,
            } => {
                bytes.u8(5);
                bytes.id(*root);
                encode_canonical_path(bytes, path);
                encode_integer_type(bytes, *scalar_type);
            }
            S::BooleanNot { operand } => encode_scalar_unary(bytes, 6, None, operand),
            S::BooleanEqual { left, right } => encode_scalar_binary(bytes, 7, None, left, right),
            S::IntegerEqual {
                scalar_type,
                left,
                right,
            } => encode_scalar_binary(bytes, 8, Some(*scalar_type), left, right),
            S::IntegerLessThan {
                scalar_type,
                left,
                right,
            } => encode_scalar_binary(bytes, 9, Some(*scalar_type), left, right),
            S::IntegerLessOrEqual {
                scalar_type,
                left,
                right,
            } => encode_scalar_binary(bytes, 10, Some(*scalar_type), left, right),
            S::IntegerBitwiseNot {
                scalar_type,
                operand,
            } => encode_scalar_unary(bytes, 11, Some(*scalar_type), operand),
            S::IntegerWiden {
                source_type,
                target_type,
                operand,
            } => encode_scalar_cast(bytes, 12, *source_type, *target_type, operand),
            S::IntegerExactCast {
                source_type,
                target_type,
                operand,
            } => encode_scalar_cast(bytes, 13, *source_type, *target_type, operand),
            S::IntegerBitwiseAnd {
                scalar_type,
                left,
                right,
            } => encode_scalar_binary(bytes, 14, Some(*scalar_type), left, right),
            S::IntegerBitwiseOr {
                scalar_type,
                left,
                right,
            } => encode_scalar_binary(bytes, 15, Some(*scalar_type), left, right),
            S::IntegerBitwiseXor {
                scalar_type,
                left,
                right,
            } => encode_scalar_binary(bytes, 16, Some(*scalar_type), left, right),
            S::WrappingIntegerShiftLeft {
                value_type,
                count_type,
                value,
                count,
            } => encode_scalar_shift(bytes, 17, *value_type, *count_type, value, count),
            S::WrappingIntegerShiftRight {
                value_type,
                count_type,
                value,
                count,
            } => encode_scalar_shift(bytes, 18, *value_type, *count_type, value, count),
            S::ExactIntegerShiftRight {
                value_type,
                count_type,
                value,
                count,
            } => encode_scalar_shift(bytes, 19, *value_type, *count_type, value, count),
            S::ExactIntegerShiftLeft {
                value_type,
                count_type,
                value,
                count,
            } => encode_scalar_shift(bytes, 20, *value_type, *count_type, value, count),
            S::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } => encode_scalar_binary(bytes, 21, Some(*scalar_type), left, right),
            S::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } => encode_scalar_binary(bytes, 22, Some(*scalar_type), left, right),
            S::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } => encode_scalar_binary(bytes, 23, Some(*scalar_type), left, right),
            S::ExactIntegerDivide {
                scalar_type,
                left,
                right,
            } => encode_scalar_binary(bytes, 24, Some(*scalar_type), left, right),
            S::ExactIntegerRemainder {
                scalar_type,
                left,
                right,
            } => encode_scalar_binary(bytes, 25, Some(*scalar_type), left, right),
            S::WrappingIntegerDivide {
                scalar_type,
                left,
                right,
            } => encode_scalar_binary(bytes, 26, Some(*scalar_type), left, right),
            S::WrappingIntegerRemainder {
                scalar_type,
                left,
                right,
            } => encode_scalar_binary(bytes, 27, Some(*scalar_type), left, right),
            S::SaturatingIntegerDivide {
                scalar_type,
                left,
                right,
            } => encode_scalar_binary(bytes, 28, Some(*scalar_type), left, right),
            S::SaturatingIntegerRemainder {
                scalar_type,
                left,
                right,
            } => encode_scalar_binary(bytes, 29, Some(*scalar_type), left, right),
            S::WrappingIntegerAdd {
                scalar_type,
                left,
                right,
            } => encode_scalar_binary(bytes, 30, Some(*scalar_type), left, right),
            S::SaturatingIntegerAdd {
                scalar_type,
                left,
                right,
            } => encode_scalar_binary(bytes, 31, Some(*scalar_type), left, right),
            S::WrappingIntegerSubtract {
                scalar_type,
                left,
                right,
            } => encode_scalar_binary(bytes, 32, Some(*scalar_type), left, right),
            S::SaturatingIntegerSubtract {
                scalar_type,
                left,
                right,
            } => encode_scalar_binary(bytes, 33, Some(*scalar_type), left, right),
            S::WrappingIntegerMultiply {
                scalar_type,
                left,
                right,
            } => encode_scalar_binary(bytes, 34, Some(*scalar_type), left, right),
            S::SaturatingIntegerMultiply {
                scalar_type,
                left,
                right,
            } => encode_scalar_binary(bytes, 35, Some(*scalar_type), left, right),
            S::ByteSequencePredicate {
                predicate,
                root,
                path,
            } => {
                bytes.u8(39);
                bytes.u8(predicate.mask_bit());
                bytes.id(*root);
                encode_canonical_path(bytes, path);
            }
            S::StructuralRead {
                root,
                path,
                scalar_type,
            } => {
                bytes.u8(40);
                bytes.id(*root);
                encode_scalar_type(bytes, *scalar_type);
                bytes.len(path.len());
                for projection in path {
                    match projection {
                        semantic_vocabulary::ScalarReadProjection::Referent => bytes.u8(3),
                        semantic_vocabulary::ScalarReadProjection::Extent => bytes.u8(4),
                        semantic_vocabulary::ScalarReadProjection::Static(segment) => {
                            bytes.u8(1);
                            encode_canonical_path(bytes, std::slice::from_ref(segment));
                        }
                        semantic_vocabulary::ScalarReadProjection::Index(index) => {
                            bytes.u8(2);
                            encode_scalar_term(bytes, index);
                        }
                    }
                }
            }
            S::ViewExtent {
                root,
                path,
                scalar_type,
            } => {
                bytes.u8(36);
                bytes.id(*root);
                encode_canonical_path(bytes, path);
                encode_integer_type(bytes, *scalar_type);
            }
        }
    }

    pub(super) fn encode_scalar_unary(
        bytes: &mut CanonicalBytes,
        tag: u8,
        scalar_type: Option<IntegerType>,
        operand: &ScalarTerm,
    ) {
        bytes.u8(tag);
        encode_optional(bytes, scalar_type.as_ref(), |bytes, value| {
            encode_integer_type(bytes, *value)
        });
        encode_scalar_term(bytes, operand);
    }
    pub(super) fn encode_scalar_binary(
        bytes: &mut CanonicalBytes,
        tag: u8,
        scalar_type: Option<IntegerType>,
        left: &ScalarTerm,
        right: &ScalarTerm,
    ) {
        bytes.u8(tag);
        encode_optional(bytes, scalar_type.as_ref(), |bytes, value| {
            encode_integer_type(bytes, *value)
        });
        encode_scalar_term(bytes, left);
        encode_scalar_term(bytes, right);
    }
    pub(super) fn encode_scalar_cast(
        bytes: &mut CanonicalBytes,
        tag: u8,
        source: IntegerType,
        target: IntegerType,
        operand: &ScalarTerm,
    ) {
        bytes.u8(tag);
        encode_integer_type(bytes, source);
        encode_integer_type(bytes, target);
        encode_scalar_term(bytes, operand);
    }
    pub(super) fn encode_scalar_shift(
        bytes: &mut CanonicalBytes,
        tag: u8,
        value_type: IntegerType,
        count_type: IntegerType,
        value: &ScalarTerm,
        count: &ScalarTerm,
    ) {
        bytes.u8(tag);
        encode_integer_type(bytes, value_type);
        encode_integer_type(bytes, count_type);
        encode_scalar_term(bytes, value);
        encode_scalar_term(bytes, count);
    }

    pub(super) fn encode_canonical_path(
        bytes: &mut CanonicalBytes,
        path: &[CanonicalStructuralPathSegment],
    ) {
        bytes.len(path.len());
        for segment in path {
            match segment {
                CanonicalStructuralPathSegment::Referent => bytes.u8(4),
                CanonicalStructuralPathSegment::Field(value) => {
                    bytes.u8(1);
                    bytes.id(*value);
                }
                CanonicalStructuralPathSegment::FixedIndex(value) => {
                    bytes.u8(2);
                    bytes.u64(*value);
                }
                CanonicalStructuralPathSegment::Case(value) => {
                    bytes.u8(3);
                    bytes.id(*value);
                }
            }
        }
    }
    pub(super) fn encode_float_format(bytes: &mut CanonicalBytes, value: IeeeFloatFormat) {
        bytes.u8(match value {
            IeeeFloatFormat::Binary32 => 1,
            IeeeFloatFormat::Binary64 => 2,
        });
    }
    pub(super) fn encode_float_comparison(
        bytes: &mut CanonicalBytes,
        value: IeeeFloatComparisonKind,
    ) {
        bytes.u8(match value {
            IeeeFloatComparisonKind::Equal => 1,
            IeeeFloatComparisonKind::NotEqual => 2,
            IeeeFloatComparisonKind::LessThan => 3,
            IeeeFloatComparisonKind::LessOrEqual => 4,
            IeeeFloatComparisonKind::NotLessThan => 5,
            IeeeFloatComparisonKind::NotLessOrEqual => 6,
        });
    }
    macro_rules! encode_rooted_field {
        ($name:ident, $t:ty) => {
            pub(super) fn $name(bytes: &mut CanonicalBytes, value: &$t) {
                bytes.id(value.root());
                encode_canonical_path(bytes, value.path());
            }
        };
    }

    encode_rooted_field!(encode_float_field, IeeeFloatStructuralField);
    encode_rooted_field!(encode_byte_field, ByteSequenceStructuralField);
    encode_rooted_field!(encode_case_subject, StructuralCaseSubject);
}

pub(crate) mod runtime_evidence_encoding {
    use super::CanonicalBytes;
    use super::carrier_encoding::{encode_optional, encode_scalar_type};
    use super::structural_encoding::{encode_evidence_contract_lane, encode_evidence_interface};
    use crate::RuntimeEvidenceCatalog;

    pub(super) fn encode_catalog(bytes: &mut CanonicalBytes, catalog: &RuntimeEvidenceCatalog) {
        bytes.u16(catalog.psi().vocabulary_marker.get());
        bytes.bytes(catalog.psi().program_fingerprint.as_bytes());
        bytes.slice(catalog.declarations(), |bytes, (machine, declaration)| {
            bytes.id(*machine);
            encode_optional(bytes, declaration.as_ref(), |bytes, declaration| {
                bytes.u32(declaration.machine.identity().0);
                bytes.u32(declaration.state.identity().0);
                bytes.u32(declaration.semantic_declaration.0);
                bytes.slice(&declaration.parameter_types, |bytes, identity| {
                    bytes.u32(identity.0)
                });
                bytes.u32(declaration.callable.0);
                bytes.u32(declaration.result_type.0);
                bytes.bytes(&declaration.contract);
                encode_optional(
                    bytes,
                    declaration.specialization.as_ref(),
                    |bytes, specialization| {
                        bytes.u32(specialization.template.identity().0);
                        bytes.u32(specialization.template_callable.0);
                        bytes.u32(specialization.template_semantic_declaration.0);
                        bytes.bytes(&specialization.commitment);
                    },
                );
            });
        });
        bytes.slice(catalog.applications(), |bytes, application| {
            bytes.id(application.id);
            bytes.id(application.declaration);
            bytes.slice(&application.binder_arguments, |bytes, argument| {
                use terminal_psi::PropositionBinderArgumentKind as K;
                bytes.u8(match argument.kind {
                    K::Type => 1,
                    K::Const => 2,
                    K::Machine => 3,
                });
                bytes.u32(argument.identity.0);
                encode_optional(
                    bytes,
                    argument.evidence_projection.as_ref(),
                    |bytes, projection| {
                        bytes.id(projection.term);
                        bytes.u32(projection.declaring_trait_identity.0);
                        bytes.slice(&projection.declaring_trait_arguments, |bytes, argument| {
                            bytes.string(argument)
                        });
                        bytes.u32(projection.requirement_identity.0);
                    },
                );
            });
            bytes.slice(&application.arguments, |bytes, argument| {
                bytes.string(argument)
            });
            encode_optional(
                bytes,
                application.evidence_interface.as_ref(),
                encode_evidence_interface,
            );
        });
        bytes.slice(catalog.terms(), |bytes, term| {
            bytes.id(term.id);
            bytes.id(term.proposition);
            encode_evidence_interface(bytes, &term.interface);
        });
        bytes.slice(catalog.lanes(), encode_evidence_contract_lane);
        bytes.slice(catalog.calls(), |bytes, call| {
            bytes.id(call.caller);
            bytes.u32(call.ordinal);
            bytes.u32(call.target_machine_identity.0);
            encode_optional(
                bytes,
                call.static_requirement_dispatch.as_ref(),
                |bytes, dispatch| {
                    bytes.u64(dispatch.conformance_application_report_fingerprint);
                    bytes.bytes(&dispatch.conformance_application_commitment.as_bytes());
                    bytes.u32(dispatch.public_requirement_identity.0);
                    bytes.u32(dispatch.declaring_trait_identity.0);
                    bytes.u32(dispatch.requirement_identity.0);
                    bytes.u32(dispatch.realization_identity.0);
                    bytes.u32(dispatch.realization_callable_identity.0);
                    bytes.id(dispatch.realization);
                },
            );
            encode_optional(
                bytes,
                call.runtime_result.as_ref(),
                |bytes, result| match result {
                    terminal_psi::ProofOutputRuntimeResult::Unit => bytes.u8(1),
                    terminal_psi::ProofOutputRuntimeResult::NoNormal => bytes.u8(3),
                    terminal_psi::ProofOutputRuntimeResult::Scalar(scalar_type) => {
                        bytes.u8(2);
                        encode_scalar_type(bytes, *scalar_type);
                    }
                },
            );
            encode_optional(bytes, call.runtime_call.as_ref(), |bytes, runtime| {
                let mut site = Vec::new();
                runtime.site.encode_identity(&mut site);
                bytes.slice(&site, |bytes, value| bytes.u8(*value));
                bytes.id(runtime.callee);
            });
            bytes.slice(&call.evidence_arguments, |bytes, argument| {
                bytes.u32(argument.input_position);
                bytes.id(argument.callee_proposition);
                bytes.id(argument.source);
                bytes.id(argument.instantiated_proposition);
            });
            bytes.slice(&call.outputs, |bytes, output| {
                bytes.u32(output.output_position);
                bytes.string(&output.output_field);
                bytes.id(output.callee_proposition);
                encode_optional(bytes, output.callee_output.as_ref(), |bytes, term| {
                    bytes.id(*term)
                });
                bytes.id(output.instantiated_proposition);
                encode_optional(
                    bytes,
                    output.forwarded_input_position.as_ref(),
                    |bytes, position| bytes.u32(*position),
                );
                encode_optional(bytes, output.output.as_ref(), |bytes, term| bytes.id(*term));
            });
        });
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use semantic_vocabulary::{EvidenceTermId, MachineId, OperationId, PropositionId};
        use terminal_psi::{
            ProofOutputCall, ProofOutputEvidenceArgument, ProofOutputRuntimeCall,
            ProofOutputRuntimeResult, SemanticFingerprint, TerminalIdentityIndex, TerminalModule,
            TerminalPsiIdentity, VocabularyMarker,
        };

        #[test]
        fn invocation_subject_provenance_is_part_of_optimizer_content() {
            let machine = MachineId::new(1).unwrap();
            let mut module = TerminalModule::for_entry(machine);
            module.proof_output_calls.push(ProofOutputCall {
                caller: machine,
                ordinal: 0,
                target_machine_identity: TerminalIdentityIndex(1),
                static_requirement_dispatch: None,
                runtime_result: Some(ProofOutputRuntimeResult::Unit),
                runtime_call: Some(ProofOutputRuntimeCall {
                    site: OperationId::new(1).unwrap().into(),
                    callee: machine,
                }),
                evidence_arguments: vec![ProofOutputEvidenceArgument {
                    input_position: 0,
                    callee_proposition: PropositionId::new(1).unwrap(),
                    source: EvidenceTermId::new(1).unwrap(),
                    instantiated_proposition: PropositionId::new(2).unwrap(),
                }],
                outputs: Vec::new(),
            });
            let psi = TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
            };
            let encoded = |module: &TerminalModule| {
                let mut bytes = CanonicalBytes::collect();
                encode_catalog(
                    &mut bytes,
                    &RuntimeEvidenceCatalog::from_verified_module(psi, module),
                );
                bytes.finish()
            };
            let before = encoded(&module);
            module.proof_output_calls[0].evidence_arguments[0].source =
                EvidenceTermId::new(2).unwrap();
            assert_ne!(before, encoded(&module));
            module.proof_output_calls[0]
                .runtime_call
                .as_mut()
                .unwrap()
                .site = OperationId::new(2).unwrap().into();
            assert_ne!(before, encoded(&module));
        }
    }
}

pub(crate) mod structural_encoding {
    use super::carrier_encoding::{
        encode_access, encode_ids, encode_integer_type, encode_integer_value, encode_multiplicity,
        encode_optional, encode_optional_access, encode_scalar_type, encode_structural_parameter,
        encode_structural_path_segment,
    };
    use super::proposition_encoding::{
        encode_content_term, encode_float_format, encode_proposition,
    };
    use super::{
        BindingRelevance, BoundaryContentGuarantee, BoundaryMachineDeclaration,
        ByteSequenceCarrier, CanonicalBytes, ClaimContentProjection, CompletionClaimSource,
        ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentConservationGuarantee,
        ContentPlaceSegment, ContentPlaceVersion, ContentProjectionExpression,
        ContentProjectionScalar, ContentStructuralPlace, EntryClaim,
        ProgramLocalRootIntroductionSchema, ProviderCandidateConformance,
        RetainedBorrowContentProjection, RetainedBorrowCustody, RetainedBorrowPlace,
        RetainedBorrowPlaceRoot, StructuralDomainDeclaration, StructuralDomainRequirement,
        StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
        StructuralOperationResult, StructuralPlaceDeclaration, StructuralPlaceKind,
        StructuralTypeDeclaration, StructuralTypeShape, TerminalAffineCleanupAction,
    };
    use terminal_psi::BoundaryMachineResult;

    mod contract {
        use super::super::carrier_encoding::{
            encode_ids, encode_optional, encode_structural_path_segment,
        };
        use super::super::{
            CanonicalBytes, CrashCause, CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard,
            EvidenceContractLane, EvidenceContractLaneKind, EvidenceInterfaceIdentity,
            MachineContract, OutcomeSpecificCallEvidence,
        };

        use super::super::proposition_encoding::encode_proposition;

        pub(in super::super) fn encode_crash_cause(bytes: &mut CanonicalBytes, cause: CrashCause) {
            bytes.u8(match cause {
                CrashCause::Trap => 1,
                CrashCause::Abort => 2,
            });
        }

        pub(in super::super) fn encode_crash_predicate(
            bytes: &mut CanonicalBytes,
            predicate: &CrashPredicateTerm,
        ) {
            encode_proposition(bytes, predicate.proposition());
        }

        pub(in super::super) fn encode_crash_route_bucket(
            bytes: &mut CanonicalBytes,
            bucket: &CrashRouteBucket,
        ) {
            encode_crash_cause(bytes, bucket.cause);
            bytes.slice(
                &bucket.alternatives,
                |bytes, alternative| match alternative {
                    CrashRouteGuard::Truth => bytes.u8(1),
                    CrashRouteGuard::Predicate(predicate) => {
                        bytes.u8(2);
                        encode_crash_predicate(bytes, predicate);
                    }
                },
            );
        }

        pub(in super::super) fn encode_evidence_interface(
            bytes: &mut CanonicalBytes,
            interface: &EvidenceInterfaceIdentity,
        ) {
            bytes.u32(interface.trait_identity.0);
            bytes.slice(&interface.arguments, |bytes, argument| {
                bytes.string(argument)
            });
            bytes.slice(&interface.requirements, |bytes, requirement| {
                bytes.u32(requirement.declaring_trait_identity.0);
                bytes.slice(&requirement.declaring_trait_arguments, |bytes, argument| {
                    bytes.string(argument)
                });
                bytes.u32(requirement.requirement_identity.0);
            });
        }

        pub(in super::super) fn encode_outcome_specific_call_evidence(
            bytes: &mut CanonicalBytes,
            evidence: &OutcomeSpecificCallEvidence,
        ) {
            bytes.id(evidence.guard.result_type);
            bytes.id(evidence.guard.result_case);
            bytes.u32(evidence.position);
            bytes.id(evidence.callee_obligation);
            bytes.id(evidence.callee_term);
            bytes.string(&evidence.output_field);
            bytes.id(evidence.callee_proposition);
            bytes.id(evidence.instantiated_proposition);
            bytes.id(evidence.output);
            encode_optional(
                bytes,
                evidence.result_substitution.as_ref(),
                |bytes, substitution| {
                    bytes.u32(substitution.argument_position);
                    bytes.id(substitution.callee_result);
                    bytes.id(substitution.caller_result);
                },
            );
            bytes.slice(&evidence.argument_substitutions, |bytes, substitution| {
                bytes.u32(substitution.argument_position);
                bytes.u32(substitution.parameter_position);
                encode_optional(
                    bytes,
                    substitution.structural_path.as_ref(),
                    |bytes, path| {
                        bytes.slice(path, encode_structural_path_segment);
                    },
                );
            });
            bytes.id(evidence.validity.result);
            encode_ids(bytes, &evidence.validity.proposition_dependencies);
            encode_evidence_interface(bytes, &evidence.validity.evidence_interface);
            encode_ids(bytes, &evidence.validity.interface_dependencies);
            bytes.u32(evidence.expected_use_count);
            bytes.slice(&evidence.uses, |bytes, usage| {
                bytes.id(usage.target);
                bytes.u32(usage.input_position);
                bytes.id(usage.target_requirement);
                bytes.id(usage.target_term);
                bytes.id(usage.source);
                bytes.id(usage.instantiated_proposition);
                bytes.id(usage.target_parameter);
                bytes.id(usage.caller_result);
            });
        }

        pub(in super::super) fn encode_machine_contract(
            bytes: &mut CanonicalBytes,
            contract: &MachineContract,
        ) {
            bytes.id(contract.id);
            bytes.slice(&contract.crash_routes, encode_crash_route_bucket);
            bytes.slice(&contract.requires, encode_proposition);
            bytes.slice(&contract.ensures, |bytes, clause| {
                bytes.id(clause.obligation);
                encode_proposition(bytes, &clause.proposition);
            });
            bytes.slice(&contract.outcome_specific_ensures, |bytes, row| {
                bytes.id(row.guard.result_type);
                bytes.id(row.guard.result_case);
                bytes.u32(row.position);
                bytes.id(row.obligation);
                encode_proposition(bytes, &row.proposition);
                encode_optional(bytes, row.evidence.as_ref(), |bytes, evidence| {
                    bytes.id(evidence.term);
                    bytes.string(&evidence.output_field);
                    bytes.slice(&evidence.argument_subjects, |bytes, subject| {
                        bytes.u32(subject.argument_position);
                        bytes.u32(subject.parameter_position);
                        encode_optional(bytes, subject.structural_path.as_ref(), |bytes, path| {
                            bytes.slice(path, encode_structural_path_segment);
                        });
                    });
                });
            });
        }

        pub(in super::super) fn encode_evidence_contract_lane(
            bytes: &mut CanonicalBytes,
            lane: &EvidenceContractLane,
        ) {
            bytes.id(lane.machine);
            bytes.u8(match lane.kind {
                EvidenceContractLaneKind::Requires => 1,
                EvidenceContractLaneKind::Ensures => 2,
            });
            bytes.u32(lane.position);
            bytes.id(lane.term);
            encode_optional(bytes, lane.output_field.as_ref(), |bytes, output| {
                bytes.string(output)
            });
            bytes.slice(&lane.argument_subjects, |bytes, subject| {
                use terminal_psi::EvidenceArgumentSubject as S;
                match subject {
                    S::Parameter(parameter) => {
                        bytes.u8(1);
                        bytes.u32(parameter.argument_position);
                        bytes.u32(parameter.parameter_position);
                        encode_optional(
                            bytes,
                            parameter.structural_path.as_ref(),
                            |bytes, path| bytes.slice(path, encode_structural_path_segment),
                        );
                    }
                    S::Result { argument_position } => {
                        bytes.u8(2);
                        bytes.u32(*argument_position);
                    }
                    S::Closed { argument_position } => {
                        bytes.u8(3);
                        bytes.u32(*argument_position);
                    }
                    S::Unknown { argument_position } => {
                        bytes.u8(4);
                        bytes.u32(*argument_position);
                    }
                }
            });
        }
    }

    pub(super) use contract::{
        encode_crash_cause, encode_crash_predicate, encode_crash_route_bucket,
        encode_evidence_contract_lane, encode_evidence_interface, encode_machine_contract,
        encode_outcome_specific_call_evidence,
    };

    pub(super) fn encode_place_declaration(
        bytes: &mut CanonicalBytes,
        place: StructuralPlaceDeclaration,
    ) {
        bytes.id(place.id);
        match place.kind {
            StructuralPlaceKind::BlockParameter { block, position } => {
                bytes.u8(8);
                bytes.id(block);
                bytes.u32(position);
            }
            StructuralPlaceKind::Parameter { position } => {
                bytes.u8(1);
                bytes.u32(position);
            }
            StructuralPlaceKind::Result => bytes.u8(2),
            StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            } => {
                bytes.u8(3);
                bytes.id(producer);
                bytes.id(structural_type);
            }
            StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal,
                structural_type,
            } => {
                bytes.u8(4);
                bytes.u32(declaration_ordinal);
                bytes.id(structural_type);
            }
            StructuralPlaceKind::ProviderAttachment {
                attachment,
                field,
                boundary,
            } => {
                bytes.u8(5);
                bytes.id(attachment);
                bytes.id(field);
                bytes.id(boundary);
            }
            StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                structural_type,
                construction,
            } => {
                bytes.u8(if construction.is_some() { 7 } else { 6 });
                bytes.u32(declaration_ordinal);
                bytes.id(structural_type);
                if let Some(construction) = construction {
                    bytes.id(construction.root_structural_type);
                    bytes.u64(construction.index);
                }
            }
        }
    }

    pub(super) fn encode_boundary_machine(
        bytes: &mut CanonicalBytes,
        declaration: &BoundaryMachineDeclaration,
    ) {
        bytes.id(declaration.id);
        bytes.u32(declaration.identity.0);
        bytes.u32(declaration.owner.identity().0);
        bytes.u8(match declaration.role {
            terminal_psi::BoundaryDeclarationRole::Callable => 0,
            terminal_psi::BoundaryDeclarationRole::ContractEvidence => 1,
        });
        bytes.u8(match declaration.kind {
            terminal_psi::BoundaryRequirementKind::Machine => 0,
            terminal_psi::BoundaryRequirementKind::Trait => 1,
            terminal_psi::BoundaryRequirementKind::Operator => 2,
        });
        bytes.u8(match declaration.completion {
            terminal_psi::BoundaryCompletion::NormalReturn => 0,
            terminal_psi::BoundaryCompletion::External(
                terminal_psi::ExternalCompletionKind::ProcessExit,
            ) => 1,
        });
        bytes.boolean(declaration.unconditional_termination);
        encode_optional(
            bytes,
            declaration.attachment.as_ref(),
            |bytes, attachment| bytes.id(*attachment),
        );
        bytes.slice(&declaration.scalar_parameters, |bytes, parameter| {
            encode_scalar_type(bytes, *parameter)
        });
        bytes.slice(&declaration.parameter_order, |bytes, kind| {
            bytes.u8(match kind {
                terminal_psi::BoundaryParameterKind::Scalar => 0,
                terminal_psi::BoundaryParameterKind::Structural => 1,
            });
        });
        bytes.slice(
            &declaration.structural_parameters,
            encode_structural_parameter,
        );
        encode_boundary_result(bytes, &declaration.result);
        bytes.slice(&declaration.requires, encode_domain_requirement);
        bytes.slice(
            &declaration.normal_qualifications,
            encode_domain_requirement,
        );
        bytes.slice(&declaration.reference_outputs, |bytes, output| {
            match output.destination {
                terminal_psi::BoundaryReferenceDestination::Parameter(position) => {
                    bytes.u8(0);
                    bytes.u32(position);
                }
            }
            bytes.slice(&output.path, encode_structural_path_segment);
            encode_access(bytes, output.access);
            bytes.slice(&output.dependencies, |bytes, dependency| match dependency {
                terminal_psi::BoundaryLifetimeDependency::Referent {
                    argument_index,
                    access,
                } => {
                    bytes.u8(0);
                    bytes.u32(*argument_index);
                    encode_access(bytes, *access);
                }
                terminal_psi::BoundaryLifetimeDependency::Stored {
                    argument_index,
                    path,
                    access,
                } => {
                    bytes.u8(1);
                    bytes.u32(*argument_index);
                    encode_access(bytes, *access);
                    bytes.slice(path, encode_structural_path_segment);
                }
            });
        });
        bytes.slice(
            &declaration.program_local_root_introductions,
            encode_program_local_root_introduction,
        );
        bytes.slice(
            &declaration.content_guarantees,
            encode_boundary_content_guarantee,
        );
        encode_ids(bytes, &declaration.fixed_service_reach);
        encode_optional(
            bytes,
            declaration.owning_service.as_ref(),
            |bytes, service| bytes.id(*service),
        );
        encode_ids(bytes, &declaration.published_service_ceiling);
    }

    fn encode_boundary_content_guarantee(
        bytes: &mut CanonicalBytes,
        guarantee: &BoundaryContentGuarantee,
    ) {
        match guarantee {
            BoundaryContentGuarantee::Conservation(guarantee) => {
                bytes.u8(1);
                encode_content_conservation_guarantee(bytes, guarantee);
            }
            BoundaryContentGuarantee::RetainedBorrow(custody) => {
                bytes.u8(2);
                encode_retained_borrow_custody(bytes, custody);
            }
        }
    }

    fn encode_retained_borrow_place(bytes: &mut CanonicalBytes, place: &RetainedBorrowPlace) {
        bytes.u8(match place.version {
            ContentPlaceVersion::Entry => 1,
            ContentPlaceVersion::Current => 2,
        });
        match &place.root {
            RetainedBorrowPlaceRoot::Parameter { position, identity } => {
                bytes.u8(1);
                bytes.u32(*position);
                bytes.u32(identity.0);
            }
            RetainedBorrowPlaceRoot::Result => bytes.u8(2),
        }
        bytes.slice(&place.segments, |bytes, segment| match segment {
            ContentPlaceSegment::Case(identity) => {
                bytes.u8(1);
                bytes.string(identity);
            }
            ContentPlaceSegment::Field(identity) => {
                bytes.u8(2);
                bytes.string(identity);
            }
            ContentPlaceSegment::FixedIndex(index) => {
                bytes.u8(3);
                bytes.u64(*index);
            }
        });
    }

    fn encode_retained_borrow_projection(
        bytes: &mut CanonicalBytes,
        projection: &RetainedBorrowContentProjection,
    ) {
        bytes.id(projection.semantic_domain);
        bytes.u32(projection.carrier_identity.0);
        bytes.id(projection.projection.identity.domain);
        bytes.u64(projection.projection.identity.projection_report_fingerprint);
        encode_content_algebra(bytes, &projection.projection.algebra);
        encode_content_projection_expression(bytes, &projection.projection.expression);
    }

    fn encode_retained_borrow_custody(bytes: &mut CanonicalBytes, custody: &RetainedBorrowCustody) {
        bytes.u32(custody.callable_identity.0);
        encode_retained_borrow_place(bytes, &custody.source);
        encode_retained_borrow_place(bytes, &custody.result);
        super::carrier_encoding::encode_access(bytes, custody.access);
        bytes.u32(custody.callable_lifetime_parameter_count);
        bytes.u32(custody.callable_lifetime_parameter_ordinal);
        bytes.u32(custody.result_nominal_identity.0);
        encode_multiplicity(bytes, custody.result_multiplicity);
        bytes.u32(custody.result_lifetime_argument_count);
        bytes.u32(custody.result_lifetime_argument_ordinal);
        bytes.boolean(custody.result_lifetime_slot_is_erased);
        bytes.id(custody.retained_semantic_domain);
        encode_retained_borrow_projection(bytes, &custody.source_projection);
        encode_retained_borrow_projection(bytes, &custody.result_projection);
    }

    pub(super) fn encode_domain_requirement(
        bytes: &mut CanonicalBytes,
        requirement: &StructuralDomainRequirement,
    ) {
        bytes.u32(requirement.argument_index);
        bytes.slice(&requirement.path, encode_structural_path_segment);
        bytes.id(requirement.domain);
    }

    pub(super) fn encode_program_local_root_introduction(
        bytes: &mut CanonicalBytes,
        schema: &ProgramLocalRootIntroductionSchema,
    ) {
        bytes.u32(schema.argument_index);
        bytes.u32(schema.source_parameter_position);
        bytes.id(schema.qualification);
        bytes.id(schema.carrier);
        bytes.id(schema.projection.domain);
        bytes.u64(schema.projection.projection_report_fingerprint);
        encode_content_algebra(bytes, &schema.algebra);
        encode_content_projection_expression(bytes, &schema.capacity);
        bytes.u64(schema.compatibility_report_identity);
    }

    pub(super) fn encode_content_projection_expression(
        bytes: &mut CanonicalBytes,
        expression: &ContentProjectionExpression,
    ) {
        match expression {
            ContentProjectionExpression::IntervalSet(members) => {
                bytes.u8(1);
                bytes.len(members.len());
                for (start, end) in members {
                    encode_content_projection_scalar(bytes, start);
                    encode_content_projection_scalar(bytes, end);
                }
            }
            ContentProjectionExpression::CountedQuantity(magnitude) => {
                bytes.u8(2);
                encode_content_projection_scalar(bytes, magnitude);
            }
        }
    }

    pub(super) fn encode_content_projection_scalar(
        bytes: &mut CanonicalBytes,
        scalar: &ContentProjectionScalar,
    ) {
        let mut pending = vec![scalar];
        while let Some(scalar) = pending.pop() {
            match scalar {
                ContentProjectionScalar::SubjectField(path)
                | ContentProjectionScalar::RuntimeScalarEmbedding(path) => {
                    bytes.u8(
                        if matches!(scalar, ContentProjectionScalar::SubjectField(_)) {
                            1
                        } else {
                            2
                        },
                    );
                    bytes.slice(path, |bytes, segment| bytes.string(segment));
                }
                ContentProjectionScalar::Natural(value) => {
                    bytes.u8(3);
                    bytes.string(value);
                }
                ContentProjectionScalar::Successor(inner) => {
                    bytes.u8(4);
                    pending.push(inner);
                }
                ContentProjectionScalar::Add(left, right)
                | ContentProjectionScalar::Subtract(left, right)
                | ContentProjectionScalar::Multiply(left, right) => {
                    bytes.u8(match scalar {
                        ContentProjectionScalar::Add(_, _) => 5,
                        ContentProjectionScalar::Subtract(_, _) => 6,
                        ContentProjectionScalar::Multiply(_, _) => 7,
                        _ => unreachable!(),
                    });
                    pending.push(right);
                    pending.push(left);
                }
            }
        }
    }

    pub(super) fn encode_content_conservation_guarantee(
        bytes: &mut CanonicalBytes,
        guarantee: &ContentConservationGuarantee,
    ) {
        bytes.u64(guarantee.report_fingerprint);
        bytes.slice(&guarantee.structural_places, |bytes, place| {
            encode_place_declaration(bytes, *place)
        });
        encode_content_conservation(bytes, &guarantee.conservation);
    }

    pub(super) fn encode_content_transformation(
        bytes: &mut CanonicalBytes,
        derived: &terminal_psi::ContentTransformationDerivation,
    ) {
        encode_content_conservation(bytes, &derived.conservation);
        bytes.slice(&derived.lineage_roots, |bytes, root| bytes.id(*root));
        match &derived.custody {
            terminal_psi::ContentTransformationCustody::Derived => bytes.u8(0),
            terminal_psi::ContentTransformationCustody::Boundary {
                requirements,
                addresses,
            } => {
                bytes.u8(1);
                bytes.slice(requirements, |bytes, requirement| {
                    bytes.id(requirement.boundary);
                    encode_content_conservation_guarantee(bytes, &requirement.guarantee);
                    bytes.slice(&requirement.substitutions, |bytes, substitution| {
                        encode_content_term(bytes, &substitution.source);
                        encode_content_term(bytes, &substitution.target);
                    });
                });
                bytes.slice(addresses, |bytes, address| {
                    for endpoint in [&address.input, &address.output] {
                        bytes.id(endpoint.claim);
                        encode_content_place(bytes, &endpoint.subject);
                    }
                });
            }
        }
        bytes.slice(&derived.ancestry, |bytes, step| {
            bytes.id(step.producer_operation);
            bytes.slice(&step.inputs, |bytes, claim| bytes.id(*claim));
            bytes.slice(&step.outputs, |bytes, claim| bytes.id(*claim));
        });
        for bindings in [&derived.inputs, &derived.outputs] {
            bytes.slice(bindings, |bytes, binding| {
                bytes.id(binding.claim);
                encode_content_term(
                    bytes,
                    &semantic_vocabulary::ContentTerm::Projection {
                        projection: binding.projection,
                        subject: binding.subject.clone(),
                    },
                );
            });
        }
    }

    pub(super) fn encode_content_conservation(
        bytes: &mut CanonicalBytes,
        conservation: &ContentConservation,
    ) {
        encode_content_algebra(bytes, conservation.algebra());
        encode_content_term(bytes, conservation.left());
        encode_content_term(bytes, conservation.right());
    }

    pub(super) fn encode_provider_candidate(
        bytes: &mut CanonicalBytes,
        candidate: &ProviderCandidateConformance,
    ) {
        bytes.id(candidate.boundary);
        bytes.u32(candidate.requirement_identity.0);
        bytes.u32(candidate.source_requirement_identity.0);
        bytes.u32(candidate.declaration_application.0);
        bytes.u32(candidate.receiving_application.0);
        bytes.slice(&candidate.receiving_region_arguments, |bytes, region| {
            bytes.u32(region.0);
        });
        bytes.u32(candidate.provider_identity.0);
        bytes.u32(candidate.provider_declaration.identity().0);
        bytes.u32(candidate.candidate_identity.0);
        bytes.u32(candidate.source_candidate_identity.0);
        encode_optional(
            bytes,
            candidate.source_candidate_package_identity.as_ref(),
            |bytes, package| {
                bytes.bytes(&package.digest());
            },
        );
        bytes.id(candidate.candidate);
        bytes.slice(&candidate.signature.parameters, |bytes, parameter| {
            bytes.u32(parameter.position);
            bytes.id(parameter.structural_type);
            encode_multiplicity(bytes, parameter.multiplicity);
            encode_ids(bytes, &parameter.qualifications);
            bytes.slice(
                &parameter.projected_qualifications,
                |bytes, qualification| {
                    bytes.slice(&qualification.path, encode_structural_path_segment);
                    bytes.id(qualification.domain);
                },
            );
        });
        bytes.slice(
            &candidate.refinement.positional_parameters,
            |bytes, parameter| {
                bytes.u32(parameter.boundary_index);
                bytes.u32(parameter.candidate_index);
            },
        );
        bytes.slice(
            &candidate.refinement.required_domains,
            encode_domain_requirement,
        );
        encode_ids(bytes, &candidate.refinement.realized_service_ceiling);
    }

    pub(super) fn encode_structural_type(
        bytes: &mut CanonicalBytes,
        declaration: &StructuralTypeDeclaration,
    ) {
        bytes.u8(u8::from(declaration.quotient_carrier.is_some()));
        if let Some(carrier) = declaration.quotient_carrier {
            bytes.id(carrier);
        }
        bytes.id(declaration.id);
        bytes.u32(declaration.identity.0);
        bytes.u8(u8::from(declaration.no_code_affine_cleanup));
        bytes.u8(u8::from(declaration.owns_linear_claim));
        bytes.u8(u8::from(declaration.descriptor_cleanup));
        bytes.u8(u8::from(declaration.copyable));
        match &declaration.shape {
            StructuralTypeShape::Reference { referent, access } => {
                bytes.u8(7);
                bytes.id(*referent);
                encode_access(bytes, *access);
            }
            StructuralTypeShape::PrimitiveScalar(scalar_type) => {
                bytes.u8(if declaration.scalar_restriction.is_some() {
                    12
                } else {
                    6
                });
                encode_scalar_type(bytes, *scalar_type);
                if let Some(restriction) = &declaration.scalar_restriction {
                    encode_scalar_restriction(bytes, restriction);
                }
            }
            StructuralTypeShape::AtomicCell(cell) => {
                bytes.u8(16);
                encode_scalar_restriction(bytes, &(*cell).into());
            }
            StructuralTypeShape::ByteSequence(carrier) => {
                bytes.u8(1);
                encode_byte_carrier(bytes, *carrier);
            }
            StructuralTypeShape::ElementView { element, access } => {
                bytes.u8(8);
                bytes.id(*element);
                encode_optional_access(bytes, *access);
            }
            StructuralTypeShape::Record { fields, storage } => {
                match storage {
                    terminal_psi::StructuralRecordStorage::AccessView(storage) => {
                        bytes.u8(14);
                        bytes.u64(storage.byte_size);
                        bytes.u64(storage.byte_alignment);
                    }
                    terminal_psi::StructuralRecordStorage::Accessor(storage) => {
                        bytes.u8(15);
                        bytes.u64(storage.byte_size);
                        bytes.u64(storage.byte_alignment);
                    }
                    terminal_psi::StructuralRecordStorage::Packed => bytes.u8(2),
                    terminal_psi::StructuralRecordStorage::Authored(storage) => {
                        bytes.u8(10);
                        bytes.u64(storage.byte_size);
                        bytes.u64(storage.byte_alignment);
                    }
                    terminal_psi::StructuralRecordStorage::TargetLayout(application) => {
                        bytes.u8(13);
                        bytes.u32(application.0);
                    }
                }
                bytes.slice(fields, encode_structural_field);
                if matches!(
                    storage,
                    terminal_psi::StructuralRecordStorage::Authored(_)
                        | terminal_psi::StructuralRecordStorage::AccessView(_)
                        | terminal_psi::StructuralRecordStorage::Accessor(_)
                ) {
                    for field in fields {
                        encode_authored_field_storage(bytes, field.storage.as_ref());
                    }
                }
            }
            StructuralTypeShape::FixedArray { element, length } => {
                bytes.u8(3);
                bytes.id(*element);
                match length {
                    terminal_psi::ArrayExtent::Literal(length) => {
                        bytes.u8(0);
                        bytes.u64(*length);
                    }
                    terminal_psi::ArrayExtent::Application(application) => {
                        bytes.u8(1);
                        bytes.id(*application);
                    }
                }
            }
            StructuralTypeShape::Sum { cases } => {
                bytes.u8(4);
                bytes.len(cases.len());
                for case in cases {
                    bytes.id(case.id);
                    bytes.u32(case.identity.0);
                    bytes.slice(&case.fields, encode_structural_field);
                }
            }
            StructuralTypeShape::Mixed { fields, cases } => {
                bytes.u8(5);
                bytes.slice(fields, encode_structural_field);
                bytes.len(cases.len());
                for case in cases {
                    bytes.id(case.id);
                    bytes.u32(case.identity.0);
                    bytes.slice(&case.fields, encode_structural_field);
                }
            }
        }
    }

    fn encode_authored_field_storage(
        bytes: &mut CanonicalBytes,
        storage: Option<&semantic_vocabulary::AuthoredFieldStorage>,
    ) {
        use semantic_vocabulary::AuthoredFieldStorage;
        match storage {
            None => bytes.u8(0),
            Some(AuthoredFieldStorage::Scalar {
                byte_offset,
                stored_scalar,
            }) => {
                bytes.u8(if stored_scalar.is_some() { 2 } else { 1 });
                bytes.u64(*byte_offset);
                if let Some(scalar) = stored_scalar {
                    encode_scalar_type(bytes, *scalar);
                }
            }
            Some(AuthoredFieldStorage::Repeated {
                byte_offset,
                element_stride,
            }) => {
                bytes.u8(3);
                bytes.u64(*byte_offset);
                bytes.u64(*element_stride);
            }
            Some(AuthoredFieldStorage::Direct { byte_offset }) => {
                bytes.u8(4);
                bytes.u64(*byte_offset);
            }
            Some(AuthoredFieldStorage::Fragmented(recipe)) => {
                bytes.u8(6);
                bytes.len(recipe.fragments().len());
                for fragment in recipe.fragments() {
                    bytes.u64(fragment.container_byte_offset);
                    bytes.u64(fragment.container_width_bits);
                    bytes.u64(fragment.destination_lsb);
                    bytes.u64(fragment.source_lsb);
                    bytes.u64(fragment.width);
                }
            }
        }
    }

    pub(super) fn encode_structural_domain(
        bytes: &mut CanonicalBytes,
        declaration: &StructuralDomainDeclaration,
    ) {
        bytes.id(declaration.id);
        bytes.id(declaration.semantic_domain);
        bytes.u32(declaration.identity.0);
        bytes.id(declaration.carrier);
        bytes.boolean(declaration.complete_predicate_theory);
        encode_optional(
            bytes,
            declaration.required_memberships.as_ref(),
            |bytes, required| encode_ids(bytes, required),
        );
        bytes.slice(&declaration.establishment_routes, |bytes, route| {
            bytes.u8(match route {
                terminal_psi::StructuralEstablishmentRoute::CheckedRequirement { .. } => 1,
                terminal_psi::StructuralEstablishmentRoute::ExactMachine { .. } => 2,
                terminal_psi::StructuralEstablishmentRoute::BoundaryRequirement { .. } => 3,
            });
            bytes.u32(route.declaration().identity().0);
            bytes.u32(route.identity().0);
        });
        encode_optional(bytes, declaration.predicate.as_ref(), |bytes, predicate| {
            bytes.id(predicate.subject);
            encode_proposition(bytes, &predicate.proposition);
        });
        bytes.u8(declaration
            .byte_predicate
            .map_or(0, |predicate| predicate.mask_bit()));
        encode_optional(
            bytes,
            declaration.content_projection.as_ref(),
            |bytes, projection| {
                bytes.id(projection.identity.domain);
                bytes.u64(projection.identity.projection_report_fingerprint);
                encode_content_algebra(bytes, &projection.algebra);
                encode_content_projection_expression(bytes, &projection.expression);
            },
        );
    }

    pub(super) fn encode_structural_field(
        bytes: &mut CanonicalBytes,
        field: &StructuralFieldDeclaration,
    ) {
        bytes.id(field.id);
        bytes.u32(field.identity.0);
        bytes.u8(u8::from(field.inline_carrier.is_some()));
        if let Some(carrier) = field.inline_carrier {
            bytes.id(carrier);
        }
        bytes.u8(match field.relevance {
            BindingRelevance::Relevant => 1,
            BindingRelevance::Erased => 2,
        });
        encode_scalar_restriction(bytes, &field.field_type);
    }

    pub(super) fn encode_byte_carrier(bytes: &mut CanonicalBytes, carrier: ByteSequenceCarrier) {
        match carrier {
            ByteSequenceCarrier::BorrowedView { access } => {
                bytes.u8(1);
                encode_optional_access(bytes, access);
            }
            ByteSequenceCarrier::BoundedView { access, capacity } => {
                bytes.u8(3);
                encode_optional_access(bytes, access);
                bytes.u64(capacity);
            }
            ByteSequenceCarrier::BoundedOwned { capacity } => {
                bytes.u8(2);
                bytes.u64(capacity);
            }
        }
    }

    pub(super) fn encode_structural_operation_result(
        bytes: &mut CanonicalBytes,
        result: &StructuralOperationResult,
    ) {
        bytes.id(result.place);
        bytes.id(result.structural_type);
        encode_multiplicity(bytes, result.multiplicity);
        encode_ids(bytes, &result.qualifications);
        encode_projected_qualification_roster(bytes, &result.projected_qualifications);
        encode_projected_qualification_roster(bytes, &result.additional_input_qualifications);
        bytes.len(result.claims.len());
        for claim in &result.claims {
            bytes.id(claim.claim);
            bytes.slice(&claim.path, encode_structural_path_segment);
        }
    }

    pub(super) fn encode_projected_qualification_roster(
        bytes: &mut CanonicalBytes,
        qualifications: &[terminal_psi::StructuralPathQualification],
    ) {
        bytes.slice(qualifications, |bytes, qualification| {
            bytes.slice(&qualification.path, encode_structural_path_segment);
            bytes.id(qualification.domain);
        });
    }

    pub(super) fn encode_completion_claim_source(
        bytes: &mut CanonicalBytes,
        source: &CompletionClaimSource,
    ) {
        bytes.id(source.claim);
        encode_optional(bytes, source.entry.as_ref(), encode_entry_claim);
        encode_optional(bytes, source.content.as_ref(), encode_content_entry_claim);
    }

    pub(super) fn encode_entry_claim(bytes: &mut CanonicalBytes, claim: &EntryClaim) {
        bytes.id(claim.claim);
        bytes.id(claim.input);
        bytes.slice(&claim.path, encode_structural_path_segment);
    }

    pub(super) fn encode_content_entry_claim(
        bytes: &mut CanonicalBytes,
        claim: &terminal_psi::ContentEntryClaim,
    ) {
        bytes.id(claim.claim);
        encode_content_place(bytes, &claim.input);
        bytes.slice(&claim.projections, encode_claim_projection);
    }

    pub(super) fn encode_claim_projection(
        bytes: &mut CanonicalBytes,
        projection: &ClaimContentProjection,
    ) {
        bytes.id(projection.projection.domain);
        bytes.u64(projection.projection.projection_report_fingerprint);
        encode_content_algebra(bytes, &projection.algebra);
    }

    pub(super) fn encode_content_algebra(bytes: &mut CanonicalBytes, algebra: &ContentAlgebra) {
        bytes.u8(match algebra.kind {
            ContentAlgebraKind::IntervalSet => 1,
            ContentAlgebraKind::CountedQuantity => 2,
        });
        bytes.string(&algebra.parameter);
    }

    pub(super) fn encode_content_place(bytes: &mut CanonicalBytes, place: &ContentStructuralPlace) {
        bytes.u8(match place.version {
            ContentPlaceVersion::Entry => 1,
            ContentPlaceVersion::Current => 2,
        });
        bytes.id(place.root);
        bytes.len(place.segments.len());
        for segment in &place.segments {
            match segment {
                ContentPlaceSegment::Field(value) => {
                    bytes.u8(1);
                    bytes.string(value);
                }
                ContentPlaceSegment::FixedIndex(value) => {
                    bytes.u8(2);
                    bytes.u64(*value);
                }
                ContentPlaceSegment::Case(value) => {
                    bytes.u8(3);
                    bytes.string(value);
                }
            }
        }
    }

    pub(super) fn encode_residual_cleanup(
        bytes: &mut CanonicalBytes,
        discard: &terminal_psi::StructuralAffineDiscard,
    ) {
        bytes.id(discard.place);
        bytes.slice(&discard.path, encode_structural_path_segment);
        bytes.id(discard.structural_type);
    }

    pub(super) fn encode_cleanup(bytes: &mut CanonicalBytes, action: &TerminalAffineCleanupAction) {
        match action {
            TerminalAffineCleanupAction::RetireHome(place) => {
                bytes.u8(4);
                bytes.id(*place);
            }
            TerminalAffineCleanupAction::DiscardRoot(place) => {
                bytes.u8(1);
                bytes.id(*place);
            }
            TerminalAffineCleanupAction::DiscardResidual(discard) => {
                bytes.u8(2);
                encode_residual_cleanup(bytes, discard);
            }
            TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                bytes.u8(if cleanup.retains_descendants { 5 } else { 3 });
                bytes.id(cleanup.place);
                bytes.slice(&cleanup.path, encode_structural_path_segment);
                bytes.id(cleanup.structural_type);
                bytes.id(cleanup.cleanup_machine);
                encode_ids(bytes, &cleanup.consumed_claims);
                encode_optional(bytes, cleanup.cleanup_receiver.as_ref(), |bytes, place| {
                    bytes.id(*place)
                });
                encode_ids(bytes, &cleanup.requirement_obligations);
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use semantic_vocabulary::{MachineId, PlaceId, StructuralTypeId};
        use terminal_psi::{
            NominalAffineCleanup, StructuralPathSegment, TerminalAffineCleanupAction,
        };

        #[test]
        fn exact_inline_carrier_changes_field_identity() {
            let encodings = [None, Some(2), Some(3)].map(|identity| {
                let field = terminal_psi::StructuralFieldDeclaration {
                    id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                    identity: terminal_psi::TerminalIdentityIndex(1),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    storage: None,
                    field_type: terminal_psi::StructuralFieldType::Scalar(
                        semantic_vocabulary::ScalarType::Boolean,
                    ),
                    inline_carrier: identity
                        .map(|identity| StructuralTypeId::new(identity).unwrap()),
                };
                let mut encoder = CanonicalBytes::collect();
                encode_structural_field(&mut encoder, &field);
                encoder.finish()
            });
            for (position, encoding) in encodings.iter().enumerate() {
                assert!(!encodings[..position].contains(encoding));
            }
        }

        #[test]
        fn logical_copy_permission_changes_encoded_identity() {
            let mut encodings = Vec::new();
            for copyable in [false, true] {
                let declaration = StructuralTypeDeclaration {
                    requires: Vec::new(),
                    quotient_carrier: None,
                    owns_linear_claim: false,
                    no_code_affine_cleanup: false,
                    descriptor_cleanup: true,
                    copyable,
                    scalar_restriction: None,
                    id: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
                    identity: terminal_psi::TerminalIdentityIndex(1),
                    shape: StructuralTypeShape::Record {
                        fields: Vec::new(),
                        storage: terminal_psi::StructuralRecordStorage::Packed,
                    },
                };
                let mut encoder = CanonicalBytes::collect();
                encode_structural_type(&mut encoder, &declaration);
                let bytes = encoder.finish();
                assert!(!encodings.contains(&bytes));
                encodings.push(bytes);
            }
        }

        #[test]
        fn nominal_cleanup_identity_distinguishes_paths_and_descendant_custody() {
            let mut previous_encoding = None;
            for (path, retains_descendants) in [
                (Vec::new(), false),
                (Vec::new(), true),
                (
                    vec![StructuralPathSegment::Field(
                        terminal_psi::TerminalIdentityIndex(1),
                    )],
                    false,
                ),
                (
                    vec![
                        StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(1)),
                        StructuralPathSegment::FixedIndex(1),
                    ],
                    false,
                ),
            ] {
                let action = TerminalAffineCleanupAction::InvokeNominal(NominalAffineCleanup {
                    retains_descendants,
                    place: PlaceId::new(1).unwrap(),
                    path,
                    structural_type: StructuralTypeId::new(2).unwrap(),
                    cleanup_machine: MachineId::new(3).unwrap(),
                    consumed_claims: Vec::new(),
                    cleanup_receiver: None,
                    requirement_obligations: Vec::new(),
                });
                let mut bytes = CanonicalBytes::collect();
                encode_cleanup(&mut bytes, &action);
                let encoded = bytes.finish();
                if let Some(previous) = previous_encoding {
                    assert_ne!(encoded, previous);
                }
                previous_encoding = Some(encoded);
            }
        }
    }

    pub(super) fn encode_scalar_restriction(
        bytes: &mut CanonicalBytes,
        field_type: &StructuralFieldType,
    ) {
        match field_type {
            StructuralFieldType::RefinedScalar(refinement) => {
                bytes.u8(9);
                encode_scalar_type(bytes, refinement.carrier());
                encode_proposition(bytes, refinement.predicate());
            }
            StructuralFieldType::AtomicBoundedInteger(bounds) => {
                bytes.u8(10);
                encode_integer_type(bytes, bounds.integer_type());
                encode_integer_value(bytes, bounds.minimum());
                encode_integer_value(bytes, bounds.maximum());
            }
            StructuralFieldType::Atomic(value) => {
                bytes.u8(8);
                encode_scalar_type(bytes, *value);
            }
            StructuralFieldType::BoundedInteger(bounds) => {
                bytes.u8(6);
                encode_integer_type(bytes, bounds.integer_type());
                encode_integer_value(bytes, bounds.minimum());
                encode_integer_value(bytes, bounds.maximum());
            }
            StructuralFieldType::Scalar(value) => {
                bytes.u8(1);
                encode_scalar_type(bytes, *value);
            }
            StructuralFieldType::IeeeFloat(value) => {
                bytes.u8(2);
                encode_float_format(bytes, *value);
            }
            StructuralFieldType::PredicateByteSequence {
                carrier,
                predicates,
            } => {
                bytes.u8(7);
                encode_byte_carrier(bytes, *carrier);
                bytes.u8(predicates.bits());
            }
            StructuralFieldType::ByteSequence(value) => {
                bytes.u8(3);
                encode_byte_carrier(bytes, *value);
            }
            StructuralFieldType::Structural(value) => {
                bytes.u8(4);
                bytes.id(*value);
            }
            StructuralFieldType::Erased { type_identity } => {
                bytes.u8(5);
                bytes.u32(type_identity.0);
            }
        }
    }

    pub(super) fn encode_boundary_result(
        bytes: &mut CanonicalBytes,
        result: &BoundaryMachineResult,
    ) {
        match result {
            BoundaryMachineResult::Unit => bytes.u8(0),
            BoundaryMachineResult::Scalar(result) => {
                bytes.u8(1);
                encode_scalar_type(bytes, result.scalar_type);
                bytes.u64(result.qualifications.get());
            }
            BoundaryMachineResult::Structural(result) => {
                bytes.u8(2);
                bytes.id(result.structural_type);
                bytes.u8(match result.multiplicity {
                    StructuralMultiplicity::Unrestricted => 1,
                    StructuralMultiplicity::Affine => 2,
                    StructuralMultiplicity::Linear => 3,
                });
                encode_ids(bytes, &result.qualifications);
                encode_projected_qualification_roster(bytes, &result.projected_qualifications);
            }
        }
    }

    #[cfg(test)]
    mod normal_qualification_tests {
        use crate::{PsiOptimizationUnit, recompute_psi_optimization_unit_identity};
        use optimization_core::OptimizationUnitIdentity;
        use semantic_vocabulary::{
            BoundaryMachineId, MachineId, PlaceId, StructuralDomainId, StructuralTypeId,
            WorkScheduleIdentity,
        };
        use std::sync::Arc;
        use terminal_psi::{
            BoundaryCompletion, BoundaryMachineDeclaration, BoundaryMachineResult,
            BoundaryParameterKind, BoundaryRequirementKind, NominalDeclarationIdentity,
            SemanticFingerprint, StructuralAccess, StructuralDomainRequirement,
            StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
            TerminalIdentityIndex, TerminalIdentityTableBuilder, TerminalPsiIdentity,
            VocabularyMarker,
        };

        fn unit() -> PsiOptimizationUnit {
            let mut identities = TerminalIdentityTableBuilder::default();
            for position in 1..=4 {
                identities.intern(&format!("test::normal-preservation::{position}"));
            }
            let boundary = BoundaryMachineDeclaration {
                id: BoundaryMachineId::new(1).unwrap(),
                declaration: None,
                role: terminal_psi::BoundaryDeclarationRole::Callable,
                kind: BoundaryRequirementKind::Machine,
                completion: BoundaryCompletion::NormalReturn,
                numeric_intrinsic: None,
                unconditional_termination: false,
                identity: TerminalIdentityIndex(1),
                owner: NominalDeclarationIdentity::new(TerminalIdentityIndex(2)),
                attachment: None,
                parameter_order: vec![BoundaryParameterKind::Structural],
                scalar_parameters: Vec::new(),
                crash_routes: Vec::new(),
                structural_parameters: vec![StructuralParameterDeclaration {
                    place: PlaceId::new(1).unwrap(),
                    position: 0,
                    structural_type: StructuralTypeId::new(1).unwrap(),
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::MutableBorrow,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    additional_input_qualifications: Vec::new(),
                }],
                result: BoundaryMachineResult::Unit,
                requires: vec![requirement(Vec::new())],
                normal_qualifications: Vec::new(),
                scalar_domain_requirements: Vec::new(),
                scalar_requires: Vec::new(),
                scalar_ensures: Vec::new(),
                result_reference_regions: Vec::new(),
                reference_outputs: Vec::new(),
                program_local_root_introductions: Vec::new(),
                content_guarantees: Vec::new(),
                fixed_service_reach: Vec::new(),
                owning_service: None,
                published_service_ceiling: Vec::new(),
            };
            PsiOptimizationUnit {
                identity: OptimizationUnitIdentity::from_canonical_bytes(b"uncomputed test seed"),
                psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
                },
                work_schedule: WorkScheduleIdentity::new(1).unwrap(),
                entry: MachineId::new(1).unwrap(),
                structural_types: Vec::new().into(),
                structural_domains: Arc::new([]),
                services: Arc::new([]),
                root_service_reach: Default::default(),
                boundary_machines: vec![boundary],
                provider_candidates: Vec::new(),
                dynamic_dispatch: Default::default(),
                accepted_obligation_facts: Vec::new(),
                proof_questions: Vec::new(),
                ownership_frontier_facts: Vec::new(),
                pruned_machines: Vec::new(),
                identities: identities.build(),
                runtime_evidence: None,
                functions: Vec::new(),
            }
        }

        fn requirement(path: Vec<StructuralPathSegment>) -> StructuralDomainRequirement {
            StructuralDomainRequirement {
                argument_index: 0,
                path,
                domain: StructuralDomainId::new(1).unwrap(),
            }
        }

        #[test]
        fn optimization_unit_identity_commits_normal_boundary_rows_and_exact_scope() {
            let mut unit = unit();
            let absent = recompute_psi_optimization_unit_identity(&unit);
            unit.boundary_machines[0]
                .normal_qualifications
                .push(requirement(Vec::new()));
            let root = recompute_psi_optimization_unit_identity(&unit);
            assert_ne!(
                root, absent,
                "normal-only guarantees must enter the unit identity"
            );
            unit.boundary_machines[0].normal_qualifications[0].path =
                vec![StructuralPathSegment::Field(TerminalIdentityIndex(3))];
            let field = recompute_psi_optimization_unit_identity(&unit);
            unit.boundary_machines[0].normal_qualifications[0].path = vec![
                StructuralPathSegment::Referent,
                StructuralPathSegment::AllElements,
            ];
            let elements = recompute_psi_optimization_unit_identity(&unit);
            assert_ne!(field, root);
            assert_ne!(elements, field);
            assert_ne!(elements, root);
        }

        #[test]
        fn optimization_unit_identity_commits_required_boundary_scope() {
            let mut unit = unit();
            let root = recompute_psi_optimization_unit_identity(&unit);
            unit.boundary_machines[0].requires[0].path =
                vec![StructuralPathSegment::Field(TerminalIdentityIndex(3))];
            let field = recompute_psi_optimization_unit_identity(&unit);
            assert_ne!(field, root);
            unit.boundary_machines[0].requires[0].path[0] =
                StructuralPathSegment::Field(TerminalIdentityIndex(4));
            assert_ne!(recompute_psi_optimization_unit_identity(&unit), field);
        }
    }
}

pub(crate) mod unit_encoding {
    use super::carrier_encoding::{
        encode_abstract_result, encode_access, encode_binding, encode_ids, encode_integer_value,
        encode_multiplicity, encode_optional, encode_scalar_type, encode_structural_binding,
        encode_structural_parameter, encode_structural_path_segment,
    };
    use super::operation_encoding::encode_operation;
    use super::structural_encoding::{
        encode_boundary_machine, encode_boundary_result, encode_cleanup,
        encode_content_entry_claim, encode_entry_claim, encode_evidence_contract_lane,
        encode_machine_contract, encode_place_declaration, encode_projected_qualification_roster,
        encode_provider_candidate, encode_structural_domain, encode_structural_type,
    };
    use super::{
        AbstractFunctionResult, AcceptedObligationFact, EffectLink, OptimizationEdge,
        OptimizationFact, OptimizationNode, OptimizationUnitIdentity, OwnershipEvent,
        OwnershipFrontierFact, OwnershipFrontierSite, OwnershipFrontierSnapshot, ProofQuestion,
        ProofQuestionAdmissionKind, ProofQuestionClass, ProofQuestionOwner,
        PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance, PsiSemanticId,
        ServiceDeclaration, StructuralResultDeclaration, TerminalRootServiceReach, ValueDefinition,
        ValueDefinitionSite, ValueUse, WorkSettlement,
    };

    const UNIT_IDENTITY_DOMAIN: &[u8] = b"omega.psi-optimization-unit-content.v54\0";
    pub fn recompute_psi_optimization_unit_identity(
        unit: &PsiOptimizationUnit,
    ) -> OptimizationUnitIdentity {
        let mut length = CanonicalBytes::length();
        encode_unit(&mut length, unit);
        let mut digest = CanonicalBytes::digest(OptimizationUnitIdentity::canonical_encoder(
            length.finish_length(),
        ));
        encode_unit(&mut digest, unit);
        OptimizationUnitIdentity::from_canonical_encoder(digest.finish_digest())
    }

    fn encode_unit(bytes: &mut CanonicalBytes, unit: &PsiOptimizationUnit) {
        bytes.bytes(UNIT_IDENTITY_DOMAIN);
        bytes.u16(unit.psi.vocabulary_marker.get());
        bytes.bytes(unit.psi.program_fingerprint.as_bytes());
        bytes.u32(unit.work_schedule.marker());
        bytes.id(unit.entry);
        bytes.slice(&unit.structural_types, encode_structural_type);
        bytes.slice(unit.structural_types.referents(), |bytes, referent| {
            bytes.id(referent.id);
            bytes.u32(referent.identity.0);
            bytes.u8(u8::from(referent.foreign_descriptor));
            bytes.u8(u8::from(referent.abstract_opaque));
        });
        bytes.slice(
            unit.structural_types.quotient_identity_bytes(),
            |bytes, value| bytes.u8(*value),
        );
        encode_access_view_catalog(bytes, &unit.structural_types);
        bytes.slice(unit.structural_domains.as_ref(), encode_structural_domain);
        bytes.slice(unit.services.as_ref(), encode_service_declaration);
        encode_root_service_reach(bytes, &unit.root_service_reach);
        bytes.slice(&unit.boundary_machines, encode_boundary_machine);
        bytes.slice(&unit.provider_candidates, encode_provider_candidate);
        bytes.slice(&unit.accepted_obligation_facts, encode_accepted_fact);
        bytes.slice(&unit.proof_questions, encode_proof_question);
        bytes.slice(
            &unit.ownership_frontier_facts,
            encode_ownership_frontier_fact,
        );
        bytes.slice(&unit.pruned_machines, |bytes, custody| {
            bytes.id(custody.machine);
            bytes.u32(custody.source_ordinal);
        });
        encode_optional(
            bytes,
            unit.runtime_evidence.as_deref(),
            super::runtime_evidence_encoding::encode_catalog,
        );
        bytes.slice(&unit.functions, encode_function);
        if !unit.dynamic_dispatch.interfaces.is_empty() || !unit.dynamic_dispatch.tables.is_empty()
        {
            bytes.bytes(b"omega.psi-optimization-dynamic-dispatch.v3\0");
            bytes.slice(&unit.dynamic_dispatch.interfaces, |bytes, interface| {
                bytes.u32(interface.id);
                bytes.u32(interface.identity.0);
                bytes.slice(&interface.slots, |bytes, slot| {
                    bytes.u32(slot.slot);
                    bytes.u32(slot.declaring_trait_identity.0);
                    bytes.u32(slot.requirement_identity.0);
                    bytes.slice(&slot.family_tuple, |bytes, value| bytes.string(value));
                    bytes.slice(&slot.scalar_parameters, |bytes, scalar_type| {
                        encode_scalar_type(bytes, *scalar_type);
                    });
                    bytes.slice(&slot.structural_parameters, |bytes, parameter| {
                        bytes.u32(parameter.position);
                        bytes.id(parameter.structural_type);
                        super::carrier_encoding::encode_access(bytes, parameter.access);
                        encode_multiplicity(bytes, parameter.multiplicity);
                    });
                    bytes.slice(&slot.entry_claim_paths, |bytes, claim| {
                        bytes.u32(claim.argument_index);
                        bytes.slice(&claim.path, encode_structural_path_segment);
                    });
                    encode_boundary_result(bytes, &slot.result);
                    bytes.slice(&slot.result_claim_paths, |bytes, path| {
                        bytes.slice(path, encode_structural_path_segment);
                    });
                    bytes.slice(&slot.result_reference_regions, |bytes, region| {
                        bytes.slice(&region.path, encode_structural_path_segment);
                        match region.origin {
                            terminal_psi::TerminalDynamicReferenceOrigin::Parameter {
                                argument_index,
                            } => {
                                bytes.u8(1);
                                bytes.u32(argument_index);
                            }
                            terminal_psi::TerminalDynamicReferenceOrigin::Invocation {
                                carrier_type,
                            } => {
                                bytes.u8(2);
                                bytes.id(carrier_type);
                            }
                        }
                        bytes.slice(&region.source_path, encode_structural_path_segment);
                        encode_access(bytes, region.access);
                    });
                });
            });
            bytes.slice(&unit.dynamic_dispatch.tables, |bytes, table| {
                bytes.u32(table.id);
                bytes.u32(table.interface);
                bytes.u32(table.implementation_identity.0);
                bytes.slice(&table.rows, |bytes, machine| {
                    encode_optional(bytes, machine.as_ref(), |bytes, machine| bytes.id(*machine));
                });
            });
        }
    }

    fn encode_service_declaration(bytes: &mut CanonicalBytes, service: &ServiceDeclaration) {
        bytes.id(service.id);
        bytes.u32(service.identity.0);
        encode_ids(bytes, &service.parents);
    }

    fn encode_root_service_reach(bytes: &mut CanonicalBytes, reach: &TerminalRootServiceReach) {
        encode_ids(bytes, &reach.concrete);
        bytes.slice(&reach.installation_dependencies, |bytes, dependency| {
            bytes.u32(dependency.requirement_identity.0);
            encode_ids(bytes, &dependency.upper_bound);
        });
    }

    pub(super) enum CanonicalBytes {
        Length(usize),
        Digest(optimization_core::CanonicalIdentityEncoder),
        Collect(Vec<u8>),
    }

    impl CanonicalBytes {
        fn length() -> Self {
            Self::Length(0)
        }

        fn digest(encoder: optimization_core::CanonicalIdentityEncoder) -> Self {
            Self::Digest(encoder)
        }

        pub(super) fn collect() -> Self {
            Self::Collect(Vec::new())
        }

        pub(super) fn finish(self) -> Vec<u8> {
            match self {
                Self::Collect(bytes) => bytes,
                _ => unreachable!("collect target precedes byte inspection"),
            }
        }

        fn finish_length(self) -> u64 {
            match self {
                Self::Length(length) => {
                    u64::try_from(length).expect("canonical optimization-unit length fits u64")
                }
                Self::Digest(_) => unreachable!("digest pass follows the completed length pass"),
                Self::Collect(_) => unreachable!("length target precedes length inspection"),
            }
        }

        fn finish_digest(self) -> optimization_core::CanonicalIdentityEncoder {
            match self {
                Self::Digest(encoder) => encoder,
                Self::Length(_) => unreachable!("length pass precedes the digest pass"),
                Self::Collect(_) => unreachable!("digest target precedes digest inspection"),
            }
        }

        pub(super) fn bytes(&mut self, bytes: &[u8]) {
            match self {
                Self::Length(length) => *length += bytes.len(),
                Self::Digest(encoder) => encoder.update(bytes),
                Self::Collect(buffer) => buffer.extend_from_slice(bytes),
            }
        }

        pub(super) fn u8(&mut self, value: u8) {
            self.bytes(&[value]);
        }

        pub(super) fn boolean(&mut self, value: bool) {
            self.u8(u8::from(value));
        }

        pub(super) fn u16(&mut self, value: u16) {
            self.bytes(&value.to_le_bytes());
        }

        pub(super) fn u32(&mut self, value: u32) {
            self.bytes(&value.to_le_bytes());
        }

        pub(super) fn u64(&mut self, value: u64) {
            self.bytes(&value.to_le_bytes());
        }

        pub(super) fn u128(&mut self, value: u128) {
            self.bytes(&value.to_le_bytes());
        }

        pub(super) fn id(&mut self, value: impl PsiSemanticId) {
            self.u64(value.get());
        }

        pub(super) fn len(&mut self, len: usize) {
            self.u64(u64::try_from(len).expect("canonical optimization-unit length fits u64"));
        }

        pub(super) fn string(&mut self, value: &str) {
            self.len(value.len());
            self.bytes(value.as_bytes());
        }

        pub(super) fn slice<T>(&mut self, values: &[T], encode: impl Fn(&mut Self, &T)) {
            self.len(values.len());
            for value in values {
                encode(self, value);
            }
        }
    }

    fn encode_accepted_fact(bytes: &mut CanonicalBytes, fact: &AcceptedObligationFact) {
        bytes.bytes(&fact.identity.bytes());
        bytes.u16(fact.psi.vocabulary_marker.get());
        bytes.bytes(fact.psi.program_fingerprint.as_bytes());
        bytes.bytes(&fact.proof_bundle_fingerprint);
        bytes.id(fact.machine);
        match fact.owner {
            PsiProvenance::Operation(operation) => {
                bytes.u8(0);
                bytes.id(operation);
            }
            PsiProvenance::Edge(edge) => {
                bytes.u8(1);
                bytes.id(edge);
            }
        }
        bytes.id(fact.obligation);
        bytes.len(fact.proposition.len());
        bytes.bytes(&fact.proposition);
        if let Some(receiving) = &fact.scalar_receiving {
            bytes.u8(1);
            let canonical = super::scalar_receiving_identity_bytes(receiving);
            bytes.len(canonical.len());
            bytes.bytes(&canonical);
        } else {
            bytes.u8(0);
        }
    }

    fn encode_proof_question(bytes: &mut CanonicalBytes, question: &ProofQuestion) {
        bytes.bytes(&question.identity.bytes());
        bytes.u16(question.terminal_psi.vocabulary_marker.get());
        bytes.bytes(question.terminal_psi.program_fingerprint.as_bytes());
        bytes.bytes(&question.proof_bundle_fingerprint);
        match question.owner {
            ProofQuestionOwner::ScalarBlockInvariant {
                machine,
                header,
                edge,
            } => {
                bytes.u8(5);
                bytes.id(machine);
                bytes.id(header);
                bytes.id(edge);
            }
            ProofQuestionOwner::Operation { machine, operation } => {
                bytes.u8(1);
                bytes.id(machine);
                bytes.id(operation);
            }
            ProofQuestionOwner::CallRequires {
                machine,
                operation,
                requirement_position,
            } => {
                bytes.u8(2);
                bytes.id(machine);
                bytes.id(operation);
                bytes.u32(requirement_position);
            }
            ProofQuestionOwner::NominalCleanupRequires {
                machine,
                edge,
                cleanup_position,
                requirement_position,
            } => {
                bytes.u8(3);
                bytes.id(machine);
                bytes.id(edge);
                bytes.u32(cleanup_position);
                bytes.u32(requirement_position);
            }
            ProofQuestionOwner::ContractEnsures {
                machine,
                contract,
                clause_position,
            } => {
                bytes.u8(4);
                bytes.id(machine);
                bytes.id(contract);
                bytes.u32(clause_position);
            }
        }
        bytes.id(question.obligation);
        match question.class {
            ProofQuestionClass::Derivable => bytes.u8(1),
            ProofQuestionClass::AdmissionAuthorized {
                site,
                kind,
                authority_identity,
            } => {
                bytes.u8(2);
                bytes.id(site);
                bytes.u8(match kind {
                    ProofQuestionAdmissionKind::ForeignBoundaryGuarantee => 1,
                    ProofQuestionAdmissionKind::ProviderFact => 2,
                    ProofQuestionAdmissionKind::CheckedAssemblyClaim => 3,
                });
                bytes.id(authority_identity);
            }
        }
        bytes.len(question.proposition.len());
        bytes.bytes(&question.proposition);
        bytes.slice(&question.requirements, |bytes, proposition| {
            bytes.len(proposition.len());
            bytes.bytes(proposition);
        });
        bytes.slice(&question.semantic_axioms, |bytes, proposition| {
            bytes.len(proposition.len());
            bytes.bytes(proposition);
        });
        bytes.boolean(question.canonical_certificate);
    }

    fn encode_ownership_frontier_fact(bytes: &mut CanonicalBytes, fact: &OwnershipFrontierFact) {
        bytes.bytes(&fact.identity.bytes());
        bytes.u16(fact.psi.vocabulary_marker.get());
        bytes.bytes(fact.psi.program_fingerprint.as_bytes());
        bytes.id(fact.machine);
        match fact.site {
            OwnershipFrontierSite::BlockEntry(id) => {
                bytes.u8(1);
                bytes.id(id);
            }
            OwnershipFrontierSite::OperationEntry(id) => {
                bytes.u8(2);
                bytes.id(id);
            }
            OwnershipFrontierSite::OperationExit(id) => {
                bytes.u8(3);
                bytes.id(id);
            }
            OwnershipFrontierSite::EdgeEntry(id) => {
                bytes.u8(4);
                bytes.id(id);
            }
            OwnershipFrontierSite::EdgeExit(id) => {
                bytes.u8(5);
                bytes.id(id);
            }
        }
        encode_ownership_frontier_snapshot(bytes, &fact.snapshot);
    }

    fn encode_ownership_frontier_snapshot(
        bytes: &mut CanonicalBytes,
        snapshot: &OwnershipFrontierSnapshot,
    ) {
        bytes.slice(&snapshot.claims, |bytes, claim| {
            bytes.id(claim.claim);
            encode_optional(bytes, claim.input.as_ref(), |bytes, input| bytes.id(*input));
            bytes.slice(&claim.path, encode_structural_path_segment);
            encode_optional(bytes, claim.multiplicity.as_ref(), |bytes, multiplicity| {
                encode_multiplicity(bytes, *multiplicity)
            });
        });
        bytes.slice(&snapshot.owned_places, |bytes, place| {
            bytes.id(place.place);
            encode_multiplicity(bytes, place.multiplicity);
        });
        bytes.slice(&snapshot.partial_custody, |bytes, partial| {
            bytes.id(partial.place);
            bytes.slice(&partial.moved_paths, |bytes, path| {
                bytes.slice(path, encode_structural_path_segment)
            });
        });
    }

    fn encode_function(bytes: &mut CanonicalBytes, function: &PsiOptimizationFunction) {
        bytes.id(function.machine);
        encode_optional(bytes, function.attachment.as_ref(), |bytes, attachment| {
            bytes.id(*attachment)
        });
        encode_optional(
            bytes,
            function.nominal_cleanup_subject.as_ref(),
            |bytes, attachment| bytes.id(*attachment),
        );
        bytes.id(function.entry);
        bytes.slice(&function.parameters, encode_definition);
        bytes.slice(&function.structural_parameters, encode_structural_parameter);
        bytes.slice(&function.structural_places, |bytes, place| {
            encode_place_declaration(bytes, *place)
        });
        encode_function_result(bytes, &function.result);
        bytes.len(function.declared_places.len());
        for place in &function.declared_places {
            bytes.id(*place);
        }
        bytes.slice(&function.entry_claim_declarations, encode_entry_claim);
        bytes.slice(&function.block_entry_claims, encode_entry_claim);
        bytes.slice(&function.content_entry_claims, encode_content_entry_claim);
        encode_optional(
            bytes,
            function.verified_contract.as_ref(),
            encode_machine_contract,
        );
        bytes.slice(
            &function.evidence_contract_lanes,
            encode_evidence_contract_lane,
        );
        bytes.len(function.entry_claims.len());
        for claim in &function.entry_claims {
            bytes.id(*claim);
        }
        encode_ids(bytes, &function.published_service_ceiling);
        encode_ids(bytes, &function.declared_service_reach);
        bytes.slice(&function.facts, encode_fact);
        bytes.len(function.blocks.len());
        for block in &function.blocks {
            bytes.id(block.id);
            bytes.slice(&block.parameters, encode_definition);
            bytes.slice(&block.structural_parameters, encode_structural_parameter);
            bytes.slice(&block.nodes, encode_node);
        }
    }

    fn encode_function_result(bytes: &mut CanonicalBytes, result: &AbstractFunctionResult) {
        match result {
            AbstractFunctionResult::Unit => bytes.u8(1),
            AbstractFunctionResult::Scalar(result) => {
                bytes.u8(2);
                encode_abstract_result(bytes, *result);
            }
            AbstractFunctionResult::Structural(result) => {
                bytes.u8(3);
                encode_structural_result(bytes, result);
            }
        }
    }

    fn encode_structural_result(bytes: &mut CanonicalBytes, result: &StructuralResultDeclaration) {
        bytes.id(result.place);
        bytes.id(result.structural_type);
        encode_multiplicity(bytes, result.multiplicity);
        encode_ids(bytes, &result.qualifications);
        encode_projected_qualification_roster(bytes, &result.projected_qualifications);
        bytes.slice(&result.reference_sources, |bytes, reference| {
            bytes.slice(&reference.path, encode_structural_path_segment);
            bytes.slice(&reference.sources, |bytes, source| {
                match source {
                    terminal_psi::StructuralReferenceOrigin::Parameter(source) => {
                        bytes.u8(1);
                        bytes.id(source.place);
                    }
                    terminal_psi::StructuralReferenceOrigin::Invocation {
                        carrier_type, ..
                    } => {
                        bytes.u8(2);
                        bytes.id(*carrier_type);
                    }
                }
                bytes.u8(match source.extent() {
                    semantic_vocabulary::ReferenceOriginExtent::Projected => 1,
                    semantic_vocabulary::ReferenceOriginExtent::WithinRegion => 2,
                });
                bytes.slice(source.path(), |bytes, projection| match projection {
                    terminal_psi::ReferenceOriginProjection::Exact(segment) => {
                        encode_structural_path_segment(bytes, segment)
                    }
                    terminal_psi::ReferenceOriginProjection::Element => bytes.u8(9),
                });
                encode_access(bytes, source.access());
            });
        });
    }

    fn encode_node(bytes: &mut CanonicalBytes, node: &OptimizationNode) {
        encode_operation(bytes, &node.operation);
        bytes.slice(&node.provenance, |bytes, provenance| {
            encode_provenance(bytes, *provenance)
        });
        bytes.slice(&node.work, encode_work);
        encode_effect(bytes, node.effect);
        bytes.slice(&node.definitions, encode_definition);
        bytes.slice(&node.uses, encode_use);
        bytes.slice(&node.successors, encode_edge);
        bytes.slice(&node.ownership, encode_ownership);
    }

    fn encode_provenance(bytes: &mut CanonicalBytes, provenance: PsiProvenance) {
        match provenance {
            PsiProvenance::Operation(operation) => {
                bytes.u8(1);
                bytes.id(operation);
            }
            PsiProvenance::Edge(edge) => {
                bytes.u8(2);
                bytes.id(edge);
            }
        }
    }

    fn encode_work(bytes: &mut CanonicalBytes, work: &WorkSettlement) {
        encode_provenance(bytes, work.site);
        bytes.u64(work.units);
    }

    fn encode_effect(bytes: &mut CanonicalBytes, effect: EffectLink) {
        bytes.u64(effect.input);
        bytes.u64(effect.output);
    }

    fn encode_definition(bytes: &mut CanonicalBytes, definition: &ValueDefinition) {
        bytes.id(definition.value);
        encode_scalar_type(bytes, definition.scalar_type);
        match definition.site {
            ValueDefinitionSite::FunctionParameter(position) => {
                bytes.u8(1);
                bytes.u32(position);
            }
            ValueDefinitionSite::BlockParameter { block, position } => {
                bytes.u8(2);
                bytes.id(block);
                bytes.u32(position);
            }
            ValueDefinitionSite::Node { block, node } => {
                bytes.u8(3);
                bytes.id(block);
                bytes.u32(node);
            }
        }
    }

    fn encode_use(bytes: &mut CanonicalBytes, value_use: &ValueUse) {
        bytes.id(value_use.value);
        bytes.id(value_use.block);
        bytes.u32(value_use.node);
    }

    fn encode_edge(bytes: &mut CanonicalBytes, edge: &OptimizationEdge) {
        bytes.id(edge.psi_edge);
        bytes.id(edge.target);
        bytes.slice(&edge.bindings, encode_binding);
        bytes.slice(&edge.structural_bindings, encode_structural_binding);
        bytes.slice(&edge.cleanup_actions, encode_cleanup);
        bytes.slice(&edge.provenance, |bytes, provenance| {
            encode_provenance(bytes, *provenance)
        });
        bytes.slice(&edge.work, encode_work);
    }

    fn encode_ownership(bytes: &mut CanonicalBytes, event: &OwnershipEvent) {
        match event {
            OwnershipEvent::ClaimTransfer(claims) => {
                bytes.u8(1);
                encode_ids(bytes, claims);
            }
            OwnershipEvent::ClaimCompletion(receipts) => {
                bytes.u8(2);
                bytes.slice(receipts, |bytes, receipt| {
                    bytes.id(receipt.claim);
                    bytes.u32(receipt.argument_index);
                    bytes.slice(&receipt.path, encode_structural_path_segment);
                });
            }
            OwnershipEvent::Cleanup(actions) => {
                bytes.u8(3);
                bytes.slice(actions, encode_cleanup);
            }
            OwnershipEvent::StructuralReturn(claims) => {
                bytes.u8(4);
                encode_ids(bytes, claims);
            }
            OwnershipEvent::CrashFrontier(claims) => {
                bytes.u8(5);
                encode_ids(bytes, claims);
            }
            OwnershipEvent::ExternalCompletionFrontier(claims) => {
                bytes.u8(6);
                encode_ids(bytes, claims);
            }
        }
    }

    fn encode_fact(bytes: &mut CanonicalBytes, fact: &OptimizationFact) {
        match fact {
            OptimizationFact::OperationObligationReference {
                obligation,
                support,
            } => {
                bytes.u8(1);
                bytes.id(*obligation);
                bytes.id(*support);
            }
            OptimizationFact::BooleanConstant {
                value,
                constant,
                support,
            } => {
                bytes.u8(2);
                bytes.id(*value);
                bytes.boolean(*constant);
                bytes.id(*support);
            }
            OptimizationFact::IntegerConstant {
                value,
                constant,
                support,
            } => {
                bytes.u8(3);
                bytes.id(*value);
                encode_integer_value(bytes, *constant);
                bytes.id(*support);
            }
        }
    }

    fn encode_access_view_plans(
        bytes: &mut CanonicalBytes,
        plans: &[terminal_psi::TerminalAccessViewPlan],
    ) {
        if !plans.is_empty() {
            bytes.bytes(b"placed-interpretations-v1");
            bytes.slice(plans, |bytes, plan| {
                bytes.id(plan.carrier);
                bytes.id(plan.schema);
                for identity in [
                    plan.view_identity,
                    plan.policy_identity,
                    plan.policy_plan_machine_identity,
                    plan.schema_identity,
                ] {
                    bytes.u32(identity.0);
                }
                bytes.bytes(&plan.placement.content_interpretation().commitment());
                bytes.slice(&plan.fields, |bytes, field| {
                    bytes.u32(field.access_slot);
                    bytes.u32(field.view_field.0);
                    bytes.u32(field.schema_field.0);
                    bytes.id(field.accessor);
                    bytes.slice(&field.operations, |bytes, operation| {
                        bytes.id(operation.boundary);
                        bytes.u8(match operation.kind {
                            terminal_psi::TerminalAccessorKind::Read => 0,
                            terminal_psi::TerminalAccessorKind::Take => 1,
                            terminal_psi::TerminalAccessorKind::Write => 2,
                        });
                    });
                });
            });
        }
    }

    fn encode_access_view_catalog(
        bytes: &mut CanonicalBytes,
        catalog: &abstract_operations::StructuralTypeCatalog,
    ) {
        encode_access_view_plans(bytes, catalog.access_views());
        if !catalog.access_lifecycles().is_empty() {
            bytes.bytes(b"placement-lifecycle-demands-v1");
            bytes.bytes(catalog.access_lifecycle_identity_bytes());
        }
        if !catalog.access_view_inputs().is_empty() {
            bytes.bytes(b"placed-formal-demands-v1");
            bytes.slice(catalog.access_view_inputs(), |bytes, input| {
                bytes.id(input.machine);
                bytes.u32(input.position);
                bytes.u32(input.runtime_position);
                for identity in [
                    input.carrier_identity,
                    input.source_machine_identity,
                    input.source_state_identity,
                    input.source_parameter_identity,
                    input.view_identity,
                    input.policy_identity,
                    input.policy_plan_machine_identity,
                    input.schema_identity,
                ] {
                    bytes.u32(identity.0);
                }
                encode_access(bytes, input.access);
                bytes.u8(u8::from(input.binding_is_const));
                bytes.u8(u8::from(input.binding_is_mutable));
                bytes.u64(input.placement_report_fingerprint);
                bytes.bytes(&input.placement_commitment);
            });
        }
    }

    pub fn access_view_catalog_identity_bytes(
        catalog: &abstract_operations::StructuralTypeCatalog,
    ) -> Vec<u8> {
        let mut bytes = CanonicalBytes::collect();
        encode_access_view_catalog(&mut bytes, catalog);
        bytes.finish()
    }
}

pub(crate) mod operation_encoding {
    use super::CanonicalBytes;
    use abstract_operations::AbstractOperation;
    pub(super) fn encode_operation(bytes: &mut CanonicalBytes, operation: &AbstractOperation) {
        use AbstractOperation as O;
        match operation {
            O::EstablishActivationClaim { .. }
            | O::SplitActivationClaim { .. }
            | O::MergeActivationClaim { .. }
            | O::ReleaseActivationClaim { .. }
            | O::StorageAddress { .. }
            | O::ByteSequenceSubslice { .. }
            | O::EstablishElementView { .. }
            | O::ElementViewSubslice { .. }
            | O::EstablishPrimitiveLocal { .. }
            | O::PrimitiveLocalStore { .. }
            | abstract_operations::primitive_storage_observation_pattern!(..)
            | O::WriteOnlyPrimitiveStore { .. }
            | O::StructuralScalarFieldStore { .. }
            | O::EstablishScalarArray { .. }
            | abstract_operations::structural_array_or_case_pattern!(..)
            | abstract_operations::establish_byte_literal_triple_pattern!(..)
            | O::EstablishZeroedStructural { .. }
            | O::EstablishStructuralLocal { .. }
            | O::EstablishReference { .. }
            | O::EstablishAccessReference { .. }
            | O::ReleaseReference { .. }
            | abstract_operations::move_or_store_structural_field_pattern!()
            | O::StructuralLeafCopy { .. }
            | O::MoveStructuralValue { .. }
            | O::TransformContent { .. }
            | O::EstablishNativeCodeLayout { .. }
            | O::EstablishRecord { .. } => structural::encode(bytes, operation),

            O::AtomicEvent { .. } => atomic::encode(bytes, operation),

            O::CallStructural { .. }
            | O::SelectImplementation { .. }
            | O::BoundaryCall { .. }
            | O::PortWrite { .. }
            | O::ActivateTaskRegister { .. }
            | O::ReadTaskRegister { .. }
            | O::ReloadSegmentDescriptors { .. }
            | O::ReadSegmentSelector { .. }
            | O::GlobalDescriptorTable { .. }
            | O::RegisterAccess { .. }
            | O::Call { .. } => calls_and_effects::encode(bytes, operation),

            O::IeeeFloatConstant { .. }
            | O::FloatResultPolicy { .. }
            | O::NearestIeeeFloatFusedMultiplyAdd { .. }
            | O::IeeeFloatCompare { .. } => ieee_float::encode(bytes, operation),

            O::IntegerConstant { .. }
            | O::BooleanConstant { .. }
            | O::StructuralScalarFieldRead { .. }
            | abstract_operations::byte_read_predicate_equal_pattern!(..)
            | O::ByteSequenceWrite { .. }
            | abstract_operations::byte_sequence_store_pair_pattern!(..)
            | O::StructuralByteViewFieldStore { .. }
            | O::StructuralByteSequenceFieldByteStore { .. }
            | O::ByteSequenceLength { .. }
            | O::ElementViewLength { .. }
            | abstract_operations::element_view_read_write_pair_pattern!(..)
            | abstract_operations::byte_len_or_field_read_pattern!(..)
            | abstract_operations::boolean_not_or_equal_pair_pattern!(..)
            | abstract_operations::integer_equal_less_pair_pattern!(..)
            | abstract_operations::bitwise_not_or_less_pair_pattern!(..)
            | abstract_operations::integer_widen_or_exact_cast_pattern!(..)
            | abstract_operations::bitwise_and_or_view_pair_pattern!(..)
            | abstract_operations::bitwise_or_xor_pair_pattern!(..)
            | abstract_operations::integer_dispatch_triple_pattern!(..) => {
                scalar::encode(bytes, operation)
            }

            abstract_operations::jump_or_conditional_pattern!()
            | O::StructuralCase { .. }
            | abstract_operations::return_or_return_unit_pattern!()
            | O::ReturnStructural { .. }
            | O::InvokeNoNormal { .. }
            | O::ExternalCompletion { .. }
            | O::Crash { .. } => control::encode(bytes, operation),
        }
    }

    fn encode_optional_field(
        bytes: &mut CanonicalBytes,
        field: Option<semantic_vocabulary::StructuralFieldId>,
    ) {
        bytes.u64(field.map_or(0, semantic_vocabulary::StructuralFieldId::get));
    }

    mod atomic {
        use crate::identity::carrier_encoding::{
            encode_abstract_result, encode_optional_id, encode_structural_path_segment,
        };
        use crate::identity::structural_encoding::encode_structural_operation_result;

        use super::CanonicalBytes;
        use abstract_operations::AbstractOperation;
        use abstract_operations::atomic::AbstractAtomicLocation;
        use abstract_operations::{
            AbstractAtomicFenceOrdering, AbstractAtomicReadModifyWrite, AtomicResidencyEdge,
            AtomicResidencySource,
        };
        use language_core::atomic::{
            AtomicCompareExchangeAttemptAxis, AtomicCompareExchangeObservationAxis,
            AtomicCompareExchangeOutcomeIdentity, MemoryOrdering,
        };

        pub(super) fn encode(bytes: &mut CanonicalBytes, operation: &AbstractOperation) {
            let AbstractOperation::AtomicEvent {
                psi_operation,
                event,
                resident_obligation,
                reads_from,
                modification_after,
            } = operation
            else {
                unreachable!("operation family routing admitted a non-atomic operation")
            };
            use abstract_operations::AbstractAtomicEvent as E;
            bytes.u8(72);
            bytes.id(*psi_operation);
            encode_optional_id(bytes, resident_obligation.as_ref());
            match event {
                E::Load {
                    location,
                    ordering,
                    result,
                } => {
                    bytes.u8(1);
                    encode_location(bytes, location);
                    encode_memory_ordering(bytes, *ordering);
                    encode_abstract_result(bytes, *result);
                }
                E::Store {
                    location,
                    ordering,
                    value,
                } => {
                    bytes.u8(2);
                    encode_location(bytes, location);
                    encode_memory_ordering(bytes, *ordering);
                    bytes.id(*value);
                }
                E::ReadModifyWrite {
                    location,
                    operation,
                    ordering,
                    operand,
                    prior,
                } => {
                    bytes.u8(3);
                    encode_location(bytes, location);
                    bytes.u8(match operation {
                        AbstractAtomicReadModifyWrite::FetchAdd => 1,
                        AbstractAtomicReadModifyWrite::FetchSub => 2,
                        AbstractAtomicReadModifyWrite::FetchXor => 3,
                        AbstractAtomicReadModifyWrite::FetchOr => 4,
                        AbstractAtomicReadModifyWrite::FetchAnd => 5,
                    });
                    encode_memory_ordering(bytes, *ordering);
                    bytes.id(*operand);
                    encode_abstract_result(bytes, *prior);
                }
                E::Swap {
                    location,
                    ordering,
                    value,
                    prior,
                } => {
                    bytes.u8(4);
                    encode_location(bytes, location);
                    encode_memory_ordering(bytes, *ordering);
                    bytes.id(*value);
                    encode_abstract_result(bytes, *prior);
                }
                E::CompareExchange {
                    location,
                    success,
                    failure,
                    expected,
                    replacement,
                    observed,
                } => {
                    bytes.u8(5);
                    encode_location(bytes, location);
                    encode_memory_ordering(bytes, *success);
                    encode_memory_ordering(bytes, *failure);
                    bytes.id(*expected);
                    bytes.id(*replacement);
                    encode_abstract_result(bytes, *observed);
                }
                E::CompareExchangeOnce {
                    location,
                    success,
                    failure,
                    expected,
                    replacement,
                    outcome,
                    custody,
                } => {
                    bytes.u8(6);
                    encode_location(bytes, location);
                    encode_memory_ordering(bytes, *success);
                    encode_memory_ordering(bytes, *failure);
                    bytes.id(*expected);
                    bytes.id(*replacement);
                    encode_structural_operation_result(bytes, outcome);
                    bytes.u8(match custody.attempt {
                        AtomicCompareExchangeAttemptAxis::Decisive => 1,
                        AtomicCompareExchangeAttemptAxis::SingleAttempt => 2,
                    });
                    bytes.u8(match custody.observation {
                        AtomicCompareExchangeObservationAxis::Observing => 1,
                        AtomicCompareExchangeObservationAxis::NonObserving => 2,
                    });
                    bytes.u8(match custody.outcome_identity {
                        AtomicCompareExchangeOutcomeIdentity::AtomicCompareExchangeOutcome => 1,
                        AtomicCompareExchangeOutcomeIdentity::AtomicCompareExchangeOnceOutcome => 2,
                        AtomicCompareExchangeOutcomeIdentity::AtomicTryExchangeOutcome => 3,
                        AtomicCompareExchangeOutcomeIdentity::AtomicTryExchangeOnceOutcome => 4,
                    });
                }
                E::Fence { ordering } => {
                    bytes.u8(7);
                    bytes.u8(match ordering {
                        AbstractAtomicFenceOrdering::Receive => 1,
                        AbstractAtomicFenceOrdering::Publish => 2,
                        AbstractAtomicFenceOrdering::ReceivePublish => 3,
                    });
                }
            }
            encode_residency_edge(bytes, reads_from);
            encode_residency_edge(bytes, modification_after);
        }

        fn encode_residency_edge(bytes: &mut CanonicalBytes, edge: &Option<AtomicResidencyEdge>) {
            match edge {
                None => bytes.u8(1),
                Some(AtomicResidencyEdge::InitialResidency) => bytes.u8(2),
                Some(AtomicResidencyEdge::Write { operation }) => {
                    bytes.u8(3);
                    bytes.id(*operation);
                }
                Some(AtomicResidencyEdge::AfterEffect { operation }) => {
                    bytes.u8(4);
                    bytes.id(*operation);
                }
                Some(AtomicResidencyEdge::PathAlternatives { sources }) => {
                    bytes.u8(5);
                    bytes.len(sources.len());
                    for source in sources {
                        match source {
                            AtomicResidencySource::Initial => bytes.u8(1),
                            AtomicResidencySource::Write { operation } => {
                                bytes.u8(2);
                                bytes.id(*operation);
                            }
                            AtomicResidencySource::AfterEffect { operation } => {
                                bytes.u8(3);
                                bytes.id(*operation);
                            }
                        }
                    }
                }
            }
        }

        fn encode_memory_ordering(bytes: &mut CanonicalBytes, ordering: MemoryOrdering) {
            bytes.u8(match ordering {
                MemoryOrdering::NoOrdering => 1,
                MemoryOrdering::Receive => 2,
                MemoryOrdering::Publish => 3,
                MemoryOrdering::ReceivePublish => 4,
                MemoryOrdering::GlobalOrder => 5,
            });
        }

        fn encode_location(bytes: &mut CanonicalBytes, location: &AbstractAtomicLocation) {
            bytes.id(location.root);
            bytes.slice(&location.path, encode_structural_path_segment);
        }
    }

    mod calls_and_effects {
        use crate::identity::carrier_encoding::{
            encode_abstract_result, encode_ids, encode_scalar_type, encode_structural_argument,
            encode_structural_path_segment,
        };
        use crate::identity::structural_encoding::{
            encode_completion_claim_source, encode_crash_route_bucket,
            encode_outcome_specific_call_evidence, encode_structural_operation_result,
        };

        use super::CanonicalBytes;
        use abstract_operations::AbstractOperation;
        pub(super) fn encode(bytes: &mut CanonicalBytes, operation: &AbstractOperation) {
            use AbstractOperation as O;
            match operation {
                O::CallStructural {
                    psi_operation,
                    callee,
                    arguments,
                    structural_arguments,
                    claim_transfers,
                    result,
                    requirement_obligations,
                    crash_continuations,
                } => {
                    use abstract_operations::AbstractStructuralCallResult as R;
                    match result {
                        R::Unit | R::Scalar(_) => {
                            bytes.u8(if matches!(result, R::Unit) { 3 } else { 4 });
                            bytes.id(*psi_operation);
                            if let Some(result) = result.scalar() {
                                encode_abstract_result(bytes, result);
                            }
                            encode_target(bytes, *callee);
                            encode_ids(bytes, arguments);
                            bytes.slice(structural_arguments, encode_structural_argument);
                            encode_ordinary_call_tail(
                                bytes,
                                claim_transfers,
                                requirement_obligations,
                                crash_continuations,
                            );
                        }
                        R::Structural {
                            result,
                            selected_evidence,
                        } => {
                            bytes.u8(5);
                            bytes.id(*psi_operation);
                            encode_structural_operation_result(bytes, result);
                            encode_target(bytes, *callee);
                            encode_ids(bytes, arguments);
                            bytes.slice(structural_arguments, encode_structural_argument);
                            bytes.slice(claim_transfers, encode_claim_transfer);
                            encode_ids(bytes, requirement_obligations);
                            bytes.slice(crash_continuations, encode_crash_route_bucket);
                            bytes.slice(selected_evidence, encode_outcome_specific_call_evidence);
                        }
                    }
                }
                O::SelectImplementation {
                    psi_operation,
                    result,
                    table,
                } => {
                    bytes.u8(95);
                    bytes.id(*psi_operation);
                    encode_abstract_result(bytes, *result);
                    bytes.u32(*table);
                }
                O::BoundaryCall {
                    psi_operation,
                    result,
                    boundary,
                    arguments,
                    structural_arguments,
                    completion_claim_sources,
                    completion_receipts,
                } => {
                    bytes.u8(6);
                    bytes.id(*psi_operation);
                    match result {
                        abstract_operations::AbstractBoundaryResult::Unit => bytes.u8(0),
                        abstract_operations::AbstractBoundaryResult::Scalar(result) => {
                            bytes.u8(1);
                            encode_abstract_result(bytes, *result);
                        }
                        abstract_operations::AbstractBoundaryResult::Structural(result) => {
                            bytes.u8(2);
                            encode_structural_operation_result(bytes, result);
                        }
                    }
                    bytes.id(*boundary);
                    encode_ids(bytes, arguments);
                    bytes.slice(structural_arguments, encode_structural_argument);
                    bytes.slice(completion_claim_sources, encode_completion_claim_source);
                    bytes.slice(completion_receipts, |bytes, receipt| {
                        bytes.id(receipt.claim);
                        bytes.u32(receipt.argument_index);
                        bytes.slice(&receipt.path, encode_structural_path_segment);
                    });
                }
                O::PortWrite {
                    psi_operation,
                    service,
                    port,
                    value,
                } => {
                    bytes.u8(7);
                    bytes.id(*psi_operation);
                    bytes.id(*service);
                    bytes.u16(*port);
                    bytes.u8(*value);
                }
                O::ActivateTaskRegister {
                    psi_operation,
                    service,
                    result,
                    entries,
                    gdt,
                    task_state,
                    selector,
                    scratch,
                } => {
                    bytes.u8(244);
                    bytes.id(*psi_operation);
                    bytes.id(*service);
                    encode_abstract_result(bytes, *result);
                    bytes.u16(*entries);
                    encode_structural_argument(bytes, gdt);
                    encode_structural_argument(bytes, task_state);
                    bytes.id(*selector);
                    encode_structural_argument(bytes, scratch);
                }
                O::ReadTaskRegister {
                    psi_operation,
                    service,
                    result,
                } => {
                    bytes.u8(245);
                    bytes.id(*psi_operation);
                    bytes.id(*service);
                    encode_abstract_result(bytes, *result);
                }
                O::ReloadSegmentDescriptors {
                    psi_operation,
                    service,
                    result,
                    entries,
                    gdt,
                    kind,
                    selector,
                    scratch,
                } => {
                    bytes.u8(246);
                    bytes.id(*psi_operation);
                    bytes.id(*service);
                    encode_abstract_result(bytes, *result);
                    bytes.u16(*entries);
                    encode_structural_argument(bytes, gdt);
                    bytes.u8(kind.code());
                    bytes.id(*selector);
                    encode_structural_argument(bytes, scratch);
                }
                O::ReadSegmentSelector {
                    register,
                    psi_operation,
                    service,
                    result,
                } => {
                    bytes.u8(247);
                    bytes.u8(register.code());
                    bytes.id(*psi_operation);
                    bytes.id(*service);
                    encode_abstract_result(bytes, *result);
                }
                O::GlobalDescriptorTable {
                    psi_operation,
                    service,
                    operation,
                    source,
                } => {
                    bytes.u8(243);
                    bytes.id(*psi_operation);
                    bytes.id(*service);
                    bytes.u8(match operation {
                        terminal_psi::GlobalDescriptorTableOperation::Store => 0,
                        terminal_psi::GlobalDescriptorTableOperation::Load => 1,
                    });
                    encode_structural_argument(bytes, source);
                }
                O::RegisterAccess {
                    psi_operation,
                    service,
                    event,
                    result,
                } => {
                    bytes.u8(242);
                    bytes.id(*psi_operation);
                    bytes.u64(service.map_or(0, |service| service.get()));
                    bytes.u16(event.operation().catalog_tag());
                    encode_ids(bytes, &event.inputs().copied().collect::<Vec<_>>());
                    bytes.u8(u8::from(result.is_some()));
                    if let Some(result) = result {
                        encode_abstract_result(bytes, *result);
                    }
                }
                O::Call {
                    psi_operation,
                    result,
                    scalar_type,
                    callee,
                    arguments,
                    requirement_obligations,
                    crash_continuations,
                } => {
                    bytes.u8(8);
                    bytes.id(*psi_operation);
                    bytes.id(*result);
                    encode_scalar_type(bytes, *scalar_type);
                    bytes.id(*callee);
                    encode_ids(bytes, arguments);
                    encode_ids(bytes, requirement_obligations);
                    bytes.slice(crash_continuations, encode_crash_route_bucket);
                }
                _ => unreachable!("operation family routing admitted a non-call operation"),
            }
        }

        pub(super) fn encode_claim_transfer(
            bytes: &mut CanonicalBytes,
            transfer: &terminal_psi::ClaimTransfer,
        ) {
            bytes.id(transfer.claim);
            bytes.u32(transfer.argument_index);
            bytes.slice(&transfer.path, encode_structural_path_segment);
        }

        fn encode_ordinary_call_tail(
            bytes: &mut CanonicalBytes,
            claim_transfers: &[terminal_psi::ClaimTransfer],
            requirement_obligations: &[semantic_vocabulary::ObligationId],
            crash_continuations: &[terminal_psi::CrashRouteBucket],
        ) {
            bytes.slice(claim_transfers, encode_claim_transfer);
            encode_ids(bytes, requirement_obligations);
            bytes.slice(crash_continuations, encode_crash_route_bucket);
        }

        fn encode_target(
            bytes: &mut CanonicalBytes,
            target: abstract_operations::AbstractCallTarget,
        ) {
            match target {
                abstract_operations::AbstractCallTarget::Machine(machine) => bytes.id(machine),
                abstract_operations::AbstractCallTarget::Dynamic {
                    selector,
                    interface,
                    slot,
                } => {
                    bytes.u64(0);
                    bytes.id(selector);
                    bytes.u32(interface);
                    bytes.u32(slot);
                }
            }
        }
    }

    mod control {
        use super::super::carrier_encoding::{
            encode_binding, encode_ids, encode_scalar_type, encode_structural_argument,
            encode_structural_binding, encode_structural_path_segment,
        };
        use super::super::structural_encoding::{
            encode_cleanup, encode_crash_cause, encode_crash_predicate, encode_crash_route_bucket,
            encode_place_declaration, encode_structural_type,
        };
        use super::CanonicalBytes;
        use super::calls_and_effects::encode_claim_transfer;
        use abstract_operations::AbstractOperation;
        use abstract_operations::AbstractSuccessor;
        pub(super) fn encode(bytes: &mut CanonicalBytes, operation: &AbstractOperation) {
            use AbstractOperation as O;
            match operation {
                O::InvokeNoNormal {
                    psi_edge,
                    callee,
                    arguments,
                    structural_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                } => {
                    bytes.u8(253);
                    bytes.id(*psi_edge);
                    bytes.id(*callee);
                    encode_ids(bytes, arguments);
                    bytes.slice(structural_arguments, encode_structural_argument);
                    bytes.slice(claim_transfers, encode_claim_transfer);
                    encode_ids(bytes, requirement_obligations);
                    bytes.slice(crash_continuations, encode_crash_route_bucket);
                }

                O::Jump {
                    psi_edge,
                    target,
                    bindings,
                    structural_bindings,
                    cleanup_actions,
                } => {
                    bytes.u8(42);
                    bytes.id(*psi_edge);
                    bytes.id(*target);
                    bytes.slice(bindings, encode_binding);
                    bytes.slice(structural_bindings, encode_structural_binding);
                    bytes.slice(cleanup_actions, encode_cleanup);
                }
                O::Conditional {
                    condition,
                    when_true,
                    when_false,
                } => {
                    bytes.u8(43);
                    bytes.id(*condition);
                    encode_successor(bytes, when_true);
                    encode_successor(bytes, when_false);
                }
                O::StructuralCase {
                    source,
                    path,
                    cases,
                } => {
                    bytes.u8(48);
                    bytes.id(*source);
                    bytes.slice(path, encode_structural_path_segment);
                    bytes.len(cases.len());
                    for case in cases {
                        bytes.id(case.psi_edge);
                        bytes.id(case.target);
                        bytes.id(case.case);
                        bytes.len(case.payloads.len());
                        for payload in &case.payloads {
                            bytes.id(payload.parameter);
                            bytes.id(payload.field);
                            encode_scalar_type(bytes, payload.scalar_type);
                        }
                        bytes.slice(&case.structural_bindings, encode_structural_binding);
                        bytes.slice(&case.cleanup_actions, encode_cleanup);
                    }
                }
                O::Return {
                    psi_edge,
                    result,
                    value,
                    scalar_type,
                    cleanup_actions,
                } => {
                    bytes.u8(44);
                    bytes.id(*psi_edge);
                    bytes.id(*result);
                    bytes.id(*value);
                    encode_scalar_type(bytes, *scalar_type);
                    bytes.slice(cleanup_actions, encode_cleanup);
                }
                O::ReturnUnit {
                    psi_edge,
                    cleanup_actions,
                } => {
                    bytes.u8(45);
                    bytes.id(*psi_edge);
                    bytes.slice(cleanup_actions, encode_cleanup);
                }
                O::ReturnStructural {
                    psi_edge,
                    source,
                    returned_claims,
                    trivial_affine_locals,
                    cleanup_actions,
                } => {
                    bytes.u8(46);
                    bytes.id(*psi_edge);
                    bytes.id(*source);
                    bytes.slice(returned_claims, |bytes, binding| {
                        bytes.id(binding.claim);
                        bytes.slice(&binding.path, encode_structural_path_segment);
                    });
                    bytes.len(trivial_affine_locals.len());
                    for (operation, place, structural_type) in trivial_affine_locals {
                        bytes.id(*operation);
                        encode_place_declaration(bytes, *place);
                        encode_structural_type(bytes, structural_type);
                    }
                    bytes.slice(cleanup_actions, encode_cleanup);
                }
                O::ExternalCompletion {
                    psi_edge,
                    boundary,
                    kind,
                    arguments,
                    structural_arguments,
                    requirement_obligations,
                    frontier_lower_bound,
                } => {
                    bytes.u8(101);
                    bytes.id(*psi_edge);
                    bytes.id(*boundary);
                    match kind {
                        semantic_vocabulary::ExternalCompletionKind::ProcessExit => bytes.u8(1),
                    }
                    encode_ids(bytes, arguments);
                    bytes.slice(structural_arguments, encode_structural_argument);
                    encode_ids(bytes, requirement_obligations);
                    encode_ids(bytes, frontier_lower_bound);
                }
                O::Crash {
                    psi_edge,
                    cause,
                    site_guard,
                    frontier_lower_bound,
                } => {
                    bytes.u8(47);
                    bytes.id(*psi_edge);
                    encode_crash_cause(bytes, *cause);
                    bytes.slice(site_guard, encode_crash_predicate);
                    encode_ids(bytes, frontier_lower_bound);
                }
                _ => unreachable!("operation family routing admitted a non-control operation"),
            }
        }

        fn encode_successor(bytes: &mut CanonicalBytes, successor: &AbstractSuccessor) {
            bytes.id(successor.psi_edge);
            bytes.id(successor.target);
            bytes.slice(&successor.bindings, encode_binding);
            bytes.slice(&successor.structural_bindings, encode_structural_binding);
            bytes.slice(&successor.cleanup_actions, encode_cleanup);
        }
    }

    mod ieee_float {
        use super::CanonicalBytes;
        use abstract_operations::AbstractOperation;
        use semantic_vocabulary::IeeeFloatFormat;

        fn encode_operation_format(bytes: &mut CanonicalBytes, format: &IeeeFloatFormat) {
            bytes.u8(match format {
                IeeeFloatFormat::Binary32 => 0,
                IeeeFloatFormat::Binary64 => 1,
            });
        }

        pub(super) fn encode(bytes: &mut CanonicalBytes, operation: &AbstractOperation) {
            use AbstractOperation as O;
            match operation {
                O::FloatResultPolicy {
                    psi_operation,
                    result,
                    format,
                    policy,
                } => {
                    bytes.u8(97);
                    bytes.id(*psi_operation);
                    bytes.id(*result);
                    encode_operation_format(bytes, format);
                    match policy {
                        terminal_psi::FloatResultPolicy::Trapping { .. } => bytes.u8(0),
                        terminal_psi::FloatResultPolicy::Saturating { .. } => bytes.u8(1),
                    };
                    for operand in policy.operands() {
                        bytes.id(operand);
                    }
                }
                O::IeeeFloatConstant {
                    psi_operation,
                    result,
                    value,
                } => {
                    bytes.u8(42);
                    bytes.id(*psi_operation);
                    bytes.id(*result);
                    match value {
                        semantic_vocabulary::IeeeFloatValue::Binary32(bits) => {
                            bytes.u8(0);
                            bytes.u32(*bits);
                        }
                        semantic_vocabulary::IeeeFloatValue::Binary64(bits) => {
                            bytes.u8(1);
                            bytes.u64(*bits);
                        }
                    }
                }
                O::IeeeFloatCompare {
                    psi_operation,
                    result,
                    comparison,
                    format,
                    left,
                    right,
                } => {
                    bytes.u8(70);
                    bytes.id(*psi_operation);
                    bytes.id(*result);
                    bytes.u8(match comparison {
                        semantic_vocabulary::IeeeFloatComparisonOperation::Equal => 0,
                        semantic_vocabulary::IeeeFloatComparisonOperation::NotEqual => 1,
                        semantic_vocabulary::IeeeFloatComparisonOperation::Less => 2,
                        semantic_vocabulary::IeeeFloatComparisonOperation::LessOrEqual => 3,
                        semantic_vocabulary::IeeeFloatComparisonOperation::Greater => 4,
                        semantic_vocabulary::IeeeFloatComparisonOperation::GreaterOrEqual => 5,
                    });
                    encode_operation_format(bytes, format);
                    bytes.id(*left);
                    bytes.id(*right);
                }
                O::NearestIeeeFloatFusedMultiplyAdd {
                    psi_operation,
                    result,
                    format,
                    left,
                    right,
                    addend,
                } => {
                    bytes.u8(43);
                    bytes.id(*psi_operation);
                    bytes.id(*result);
                    encode_operation_format(bytes, format);
                    bytes.id(*left);
                    bytes.id(*right);
                    bytes.id(*addend);
                }
                _ => unreachable!("operation family routing admitted a non-IEEE operation"),
            }
        }
    }

    mod scalar {
        use super::scalar_shapes::{
            encode_cast, encode_shift, encode_trapping, encode_typed_binary, encode_typed_unary,
            encode_untyped_binary, encode_untyped_unary,
        };
        use crate::identity::carrier_encoding::{
            encode_abstract_result, encode_integer_value, encode_optional_id, encode_scalar_type,
            encode_structural_argument, encode_structural_path_segment,
        };
        use crate::identity::proposition_encoding::encode_canonical_path;

        use super::CanonicalBytes;
        use abstract_operations::AbstractOperation;
        use abstract_operations::{
            IntegerArithmetic, IntegerArithmeticOperator, IntegerOverflowPolicy,
            IntegerShiftDirection, IntegerShiftPolicy,
        };
        use semantic_vocabulary::ScalarType;

        pub(super) fn encode(bytes: &mut CanonicalBytes, operation: &AbstractOperation) {
            use AbstractOperation as O;
            match operation {
                O::StructuralByteSequenceFieldByteStore {
                    psi_operation,
                    destination,
                    path,
                    field,
                    index,
                    value,
                    length,
                    obligation,
                } => {
                    bytes.u8(78);
                    bytes.id(*psi_operation);
                    bytes.id(*destination);
                    bytes.slice(path, encode_structural_path_segment);
                    bytes.id(*field);
                    bytes.id(*index);
                    bytes.id(*value);
                    bytes.id(*length);
                    bytes.id(*obligation);
                }
                O::ByteSequenceStore {
                    psi_operation,
                    destination,
                    source,
                    length,
                    obligation,
                } => {
                    bytes.u8(241);
                    bytes.id(*psi_operation);
                    bytes.id(*destination);
                    bytes.id(*source);
                    bytes.id(*length);
                    bytes.id(*obligation);
                }
                O::StructuralByteSequenceFieldStore {
                    psi_operation,
                    destination,
                    path,
                    field,
                    source,
                    length,
                    obligation,
                } => {
                    bytes.u8(77);
                    bytes.id(*psi_operation);
                    bytes.id(*destination);
                    bytes.slice(path, encode_structural_path_segment);
                    bytes.id(*field);
                    bytes.id(*source);
                    bytes.id(*length);
                    bytes.id(*obligation);
                }
                O::StructuralByteViewFieldStore {
                    psi_operation,
                    destination,
                    path,
                    field,
                    source,
                    predicate_obligation,
                } => {
                    bytes.u8(98);
                    bytes.id(*psi_operation);
                    bytes.id(*destination);
                    bytes.slice(path, encode_structural_path_segment);
                    super::encode_optional_field(bytes, *field);
                    bytes.id(*source);
                    encode_optional_id(bytes, predicate_obligation.as_ref());
                }
                O::ByteSequenceWrite {
                    psi_operation,
                    destination,
                    index,
                    value,
                    length,
                    obligation,
                } => {
                    bytes.u8(68);
                    bytes.id(*psi_operation);
                    bytes.id(*destination);
                    bytes.id(*index);
                    bytes.id(*value);
                    bytes.id(*length);
                    bytes.id(*obligation);
                }
                O::ByteSequenceRead {
                    psi_operation,
                    result,
                    source,
                    index,
                    length,
                    obligation,
                } => {
                    bytes.u8(63);
                    bytes.id(*psi_operation);
                    encode_abstract_result(bytes, *result);
                    bytes.id(*source);
                    bytes.id(*index);
                    bytes.id(*length);
                    bytes.id(*obligation);
                }
                O::ByteSequencePredicate {
                    psi_operation,
                    result,
                    operand,
                    predicate,
                } => {
                    bytes.u8(100);
                    bytes.id(*psi_operation);
                    encode_abstract_result(bytes, *result);
                    encode_byte_sequence_operand(bytes, operand);
                    bytes.u8(predicate.mask_bit());
                }
                O::ByteSequenceEqual {
                    psi_operation,
                    result,
                    left,
                    right,
                } => {
                    bytes.u8(104);
                    bytes.id(*psi_operation);
                    encode_abstract_result(bytes, *result);
                    encode_byte_sequence_operand(bytes, left);
                    encode_byte_sequence_operand(bytes, right);
                }
                O::StructuralByteSequenceFieldRead {
                    psi_operation,
                    result,
                    source,
                    path,
                    field,
                    index,
                    length,
                    obligation,
                } => {
                    bytes.u8(89);
                    bytes.id(*psi_operation);
                    encode_abstract_result(bytes, *result);
                    bytes.id(*source);
                    bytes.slice(path, encode_structural_path_segment);
                    bytes.id(*field);
                    bytes.id(*index);
                    bytes.id(*length);
                    bytes.id(*obligation);
                }
                O::StructuralByteSequenceFieldLength {
                    psi_operation,
                    result,
                    source,
                    path,
                    field,
                } => {
                    bytes.u8(76);
                    bytes.id(*psi_operation);
                    encode_abstract_result(bytes, *result);
                    bytes.id(*source);
                    bytes.slice(path, encode_structural_path_segment);
                    bytes.id(*field);
                }
                O::ByteSequenceLength {
                    psi_operation,
                    result,
                    source,
                } => {
                    bytes.u8(62);
                    bytes.id(*psi_operation);
                    encode_abstract_result(bytes, *result);
                    encode_structural_argument(bytes, source);
                }
                O::ElementViewLength {
                    psi_operation,
                    result,
                    source,
                } => {
                    bytes.u8(86);
                    bytes.id(*psi_operation);
                    encode_abstract_result(bytes, *result);
                    encode_structural_argument(bytes, source);
                }
                O::ElementViewRead {
                    psi_operation,
                    result,
                    source,
                    index,
                    length,
                    obligation,
                    path,
                } => {
                    bytes.u8(87);
                    bytes.id(*psi_operation);
                    encode_abstract_result(bytes, *result);
                    bytes.id(*source);
                    bytes.id(*index);
                    bytes.id(*length);
                    bytes.id(*obligation);
                    bytes.slice(path, encode_structural_path_segment);
                }
                O::ElementViewWrite {
                    path,
                    psi_operation,
                    destination,
                    index,
                    value,
                    length,
                    obligation,
                } => {
                    bytes.u8(88);
                    bytes.slice(path, encode_structural_path_segment);
                    bytes.id(*psi_operation);
                    encode_structural_argument(bytes, destination);
                    bytes.id(*index);
                    bytes.id(*value);
                    bytes.id(*length);
                    bytes.id(*obligation);
                }
                O::IntegerConstant {
                    psi_operation,
                    result,
                    scalar_type,
                    value,
                } => {
                    bytes.u8(9);
                    bytes.id(*psi_operation);
                    bytes.id(*result);
                    encode_scalar_type(bytes, *scalar_type);
                    encode_integer_value(bytes, *value);
                }
                O::BooleanConstant {
                    psi_operation,
                    result,
                    value,
                } => {
                    bytes.u8(10);
                    bytes.id(*psi_operation);
                    bytes.id(*result);
                    bytes.boolean(*value);
                }
                O::StructuralScalarFieldRead {
                    psi_operation,
                    result,
                    source,
                    path,
                    field,
                } => {
                    if result.scalar_type == ScalarType::Boolean {
                        bytes.u8(11);
                        bytes.id(*psi_operation);
                        bytes.id(result.value);
                    } else {
                        bytes.u8(51);
                        bytes.id(*psi_operation);
                        encode_abstract_result(bytes, *result);
                    }
                    bytes.id(*source);
                    encode_canonical_path(bytes, path);
                    bytes.id(*field);
                }
                O::BooleanNot {
                    psi_operation,
                    result,
                    operand,
                } => encode_untyped_unary(bytes, 12, *psi_operation, *result, *operand),
                O::BooleanEqual {
                    psi_operation,
                    result,
                    left,
                    right,
                } => encode_untyped_binary(bytes, 13, *psi_operation, *result, *left, *right),
                O::IntegerEqual {
                    psi_operation,
                    result,
                    left,
                    right,
                } => encode_untyped_binary(bytes, 14, *psi_operation, *result, *left, *right),
                O::IntegerLessThan {
                    psi_operation,
                    result,
                    left,
                    right,
                } => encode_untyped_binary(bytes, 15, *psi_operation, *result, *left, *right),
                O::IntegerLessOrEqual {
                    psi_operation,
                    result,
                    left,
                    right,
                } => encode_untyped_binary(bytes, 16, *psi_operation, *result, *left, *right),
                O::IntegerBitwiseNot {
                    psi_operation,
                    result,
                    scalar_type,
                    operand,
                } => encode_typed_unary(bytes, 17, *psi_operation, *result, *scalar_type, *operand),
                O::IntegerWiden {
                    psi_operation,
                    result,
                    source_type,
                    target_type,
                    operand,
                } => encode_cast(
                    bytes,
                    18,
                    *psi_operation,
                    None,
                    *result,
                    *source_type,
                    *target_type,
                    *operand,
                ),
                O::ScalarRepresentationView {
                    psi_operation,
                    result,
                    source_type,
                    target_type,
                    operand,
                } => {
                    bytes.u8(99);
                    bytes.id(*psi_operation);
                    bytes.id(*result);
                    encode_scalar_type(bytes, *source_type);
                    encode_scalar_type(bytes, *target_type);
                    bytes.id(*operand);
                }
                O::IntegerExactCast {
                    psi_operation,
                    obligation,
                    result,
                    source_type,
                    target_type,
                    operand,
                } => encode_cast(
                    bytes,
                    19,
                    *psi_operation,
                    Some(*obligation),
                    *result,
                    *source_type,
                    *target_type,
                    *operand,
                ),
                O::IntegerBitwiseAnd {
                    psi_operation,
                    result,
                    scalar_type,
                    left,
                    right,
                } => encode_typed_binary(
                    bytes,
                    20,
                    *psi_operation,
                    None,
                    *result,
                    *scalar_type,
                    *left,
                    *right,
                ),
                O::IntegerBitwiseOr {
                    psi_operation,
                    result,
                    scalar_type,
                    left,
                    right,
                } => encode_typed_binary(
                    bytes,
                    21,
                    *psi_operation,
                    None,
                    *result,
                    *scalar_type,
                    *left,
                    *right,
                ),
                O::IntegerBitwiseXor {
                    psi_operation,
                    result,
                    scalar_type,
                    left,
                    right,
                } => encode_typed_binary(
                    bytes,
                    22,
                    *psi_operation,
                    None,
                    *result,
                    *scalar_type,
                    *left,
                    *right,
                ),
                O::IntegerShift {
                    psi_operation,
                    result,
                    value_type,
                    count_type,
                    policy,
                    direction,
                    value,
                    count,
                } => encode_shift(
                    bytes,
                    integer_shift_tag(*policy, *direction),
                    *psi_operation,
                    policy.obligation(),
                    *result,
                    *value_type,
                    *count_type,
                    *value,
                    *count,
                ),
                O::IntegerArithmetic {
                    psi_operation,
                    result,
                    scalar_type,
                    arithmetic,
                    left,
                    right,
                } => encode_typed_binary(
                    bytes,
                    integer_arithmetic_tag(*arithmetic),
                    *psi_operation,
                    arithmetic.obligation(),
                    *result,
                    *scalar_type,
                    *left,
                    *right,
                ),
                O::TrappingInteger {
                    psi_operation,
                    result,
                    scalar_type,
                    operand_type,
                    operation,
                } => encode_trapping(
                    bytes,
                    90,
                    *psi_operation,
                    *result,
                    *scalar_type,
                    *operand_type,
                    *operation,
                ),
                _ => unreachable!("operation family routing admitted a non-scalar operation"),
            }
        }

        fn integer_shift_tag(policy: IntegerShiftPolicy, direction: IntegerShiftDirection) -> u8 {
            match (policy, direction) {
                (IntegerShiftPolicy::Wrapping, IntegerShiftDirection::Left) => 23,
                (IntegerShiftPolicy::Wrapping, IntegerShiftDirection::Right) => 24,
                (IntegerShiftPolicy::Exact { .. }, IntegerShiftDirection::Left) => 25,
                (IntegerShiftPolicy::Exact { .. }, IntegerShiftDirection::Right) => 26,
            }
        }

        fn integer_arithmetic_tag(arithmetic: IntegerArithmetic) -> u8 {
            use IntegerArithmeticOperator as Operator;
            use IntegerOverflowPolicy as Policy;
            match (arithmetic.policy(), arithmetic.operator()) {
                (Policy::Wrapping, Operator::Add) => 27,
                (Policy::Exact, Operator::Add) => 28,
                (Policy::Saturating, Operator::Add) => 29,
                (Policy::Wrapping, Operator::Subtract) => 30,
                (Policy::Exact, Operator::Subtract) => 31,
                (Policy::Saturating, Operator::Subtract) => 32,
                (Policy::Wrapping, Operator::Multiply) => 33,
                (Policy::Exact, Operator::Multiply) => 34,
                (Policy::Exact, Operator::Divide) => 35,
                (Policy::Exact, Operator::Remainder) => 36,
                (Policy::Wrapping, Operator::Divide) => 37,
                (Policy::Wrapping, Operator::Remainder) => 38,
                (Policy::Saturating, Operator::Divide) => 39,
                (Policy::Saturating, Operator::Remainder) => 40,
                (Policy::Saturating, Operator::Multiply) => 41,
            }
        }

        pub(super) fn encode_byte_sequence_operand(
            bytes: &mut CanonicalBytes,
            operand: &terminal_psi::control_flow::ByteSequenceOperand,
        ) {
            match operand {
                terminal_psi::control_flow::ByteSequenceOperand::Whole { source, length } => {
                    bytes.u8(0);
                    encode_structural_argument(bytes, source);
                    bytes.id(*length);
                }
                terminal_psi::control_flow::ByteSequenceOperand::Field {
                    source,
                    field,
                    length,
                } => {
                    bytes.u8(1);
                    encode_structural_argument(bytes, source);
                    bytes.id(*field);
                    bytes.id(*length);
                }
            }
        }
    }

    mod scalar_shapes {
        use super::super::IntegerType;
        use super::CanonicalBytes;
        use crate::identity::carrier_encoding::{encode_integer_type, encode_optional};
        pub(super) fn encode_untyped_unary(
            bytes: &mut CanonicalBytes,
            tag: u8,
            operation: semantic_vocabulary::OperationId,
            result: semantic_vocabulary::ValueId,
            operand: semantic_vocabulary::ValueId,
        ) {
            bytes.u8(tag);
            bytes.id(operation);
            bytes.id(result);
            bytes.id(operand);
        }

        pub(super) fn encode_untyped_binary(
            bytes: &mut CanonicalBytes,
            tag: u8,
            operation: semantic_vocabulary::OperationId,
            result: semantic_vocabulary::ValueId,
            left: semantic_vocabulary::ValueId,
            right: semantic_vocabulary::ValueId,
        ) {
            bytes.u8(tag);
            bytes.id(operation);
            bytes.id(result);
            bytes.id(left);
            bytes.id(right);
        }

        pub(super) fn encode_typed_unary(
            bytes: &mut CanonicalBytes,
            tag: u8,
            operation: semantic_vocabulary::OperationId,
            result: semantic_vocabulary::ValueId,
            scalar_type: IntegerType,
            operand: semantic_vocabulary::ValueId,
        ) {
            bytes.u8(tag);
            bytes.id(operation);
            bytes.id(result);
            encode_integer_type(bytes, scalar_type);
            bytes.id(operand);
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_cast(
            bytes: &mut CanonicalBytes,
            tag: u8,
            operation: semantic_vocabulary::OperationId,
            obligation: Option<semantic_vocabulary::ObligationId>,
            result: semantic_vocabulary::ValueId,
            source_type: IntegerType,
            target_type: IntegerType,
            operand: semantic_vocabulary::ValueId,
        ) {
            bytes.u8(tag);
            bytes.id(operation);
            encode_optional(bytes, obligation.as_ref(), |bytes, value| bytes.id(*value));
            bytes.id(result);
            encode_integer_type(bytes, source_type);
            encode_integer_type(bytes, target_type);
            bytes.id(operand);
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_typed_binary(
            bytes: &mut CanonicalBytes,
            tag: u8,
            operation: semantic_vocabulary::OperationId,
            obligation: Option<semantic_vocabulary::ObligationId>,
            result: semantic_vocabulary::ValueId,
            scalar_type: IntegerType,
            left: semantic_vocabulary::ValueId,
            right: semantic_vocabulary::ValueId,
        ) {
            bytes.u8(tag);
            bytes.id(operation);
            encode_optional(bytes, obligation.as_ref(), |bytes, value| bytes.id(*value));
            bytes.id(result);
            encode_integer_type(bytes, scalar_type);
            bytes.id(left);
            bytes.id(right);
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_shift(
            bytes: &mut CanonicalBytes,
            tag: u8,
            operation: semantic_vocabulary::OperationId,
            obligation: Option<semantic_vocabulary::ObligationId>,
            result: semantic_vocabulary::ValueId,
            value_type: IntegerType,
            count_type: IntegerType,
            value: semantic_vocabulary::ValueId,
            count: semantic_vocabulary::ValueId,
        ) {
            bytes.u8(tag);
            bytes.id(operation);
            encode_optional(bytes, obligation.as_ref(), |bytes, value| bytes.id(*value));
            bytes.id(result);
            encode_integer_type(bytes, value_type);
            encode_integer_type(bytes, count_type);
            bytes.id(value);
            bytes.id(count);
        }

        pub(super) fn encode_trapping(
            bytes: &mut CanonicalBytes,
            tag: u8,
            operation: semantic_vocabulary::OperationId,
            result: semantic_vocabulary::ValueId,
            scalar_type: IntegerType,
            operand_type: IntegerType,
            primitive: terminal_psi::TrappingIntegerOperation,
        ) {
            use terminal_psi::TrappingIntegerPrimitive as P;
            bytes.u8(tag);
            bytes.id(operation);
            bytes.id(result);
            encode_integer_type(bytes, scalar_type);
            encode_integer_type(bytes, operand_type);
            bytes.u8(match primitive.primitive() {
                P::Add => 0,
                P::Subtract => 1,
                P::Multiply => 2,
                P::Divide => 3,
                P::Remainder => 4,
                P::ShiftLeft => 5,
                P::ShiftRight => 6,
                P::Convert => 7,
            });
            for operand in primitive.operands() {
                bytes.id(operand);
            }
        }
    }

    mod structural {
        use crate::identity::carrier_encoding::{
            encode_abstract_result, encode_ids, encode_optional_id, encode_structural_argument,
            encode_structural_parameter, encode_structural_path_segment, encode_window_root,
        };
        use crate::identity::structural_encoding::{
            encode_place_declaration, encode_structural_operation_result, encode_structural_type,
        };

        use super::CanonicalBytes;
        use super::scalar::encode_byte_sequence_operand;
        use abstract_operations::AbstractOperation;
        use abstract_operations::StructuralCopyPath;
        pub(super) fn encode(bytes: &mut CanonicalBytes, operation: &AbstractOperation) {
            use AbstractOperation as O;
            macro_rules! tag {
                ($bytes:expr, $tag:expr, $operation:expr) => {
                    $bytes.u8($tag);
                    $bytes.id(*$operation);
                };
            }

            match operation {
                O::EstablishActivationClaim {
                    psi_operation,
                    result,
                    bound,
                    committed,
                    backing_type,
                    established_case,
                    rejected_case,
                } => {
                    tag!(bytes, 253, psi_operation);
                    encode_structural_operation_result(bytes, result);
                    bytes.u64(*bound);
                    bytes.id(*committed);
                    bytes.id(*backing_type);
                    bytes.id(*established_case);
                    bytes.id(*rejected_case);
                }
                O::ReleaseActivationClaim {
                    psi_operation,
                    source,
                    claim,
                } => {
                    tag!(bytes, 254, psi_operation);
                    encode_structural_argument(bytes, source);
                    bytes.id(*claim);
                }
                O::SplitActivationClaim {
                    psi_operation,
                    result,
                    source,
                    claim,
                    bound_at,
                    committed_at,
                    covered_case,
                    rejected_case,
                } => {
                    tag!(bytes, 251, psi_operation);
                    encode_structural_operation_result(bytes, result);
                    encode_structural_argument(bytes, source);
                    bytes.id(*claim);
                    bytes.u64(*bound_at);
                    bytes.id(*committed_at);
                    bytes.id(*covered_case);
                    bytes.id(*rejected_case);
                }
                O::MergeActivationClaim {
                    psi_operation,
                    result,
                    lower,
                    upper,
                    lower_claim,
                    upper_claim,
                    covered_case,
                    rejected_case,
                } => {
                    tag!(bytes, 252, psi_operation);
                    encode_structural_operation_result(bytes, result);
                    encode_structural_argument(bytes, lower);
                    encode_structural_argument(bytes, upper);
                    bytes.id(*lower_claim);
                    bytes.id(*upper_claim);
                    bytes.id(*covered_case);
                    bytes.id(*rejected_case);
                }

                O::EstablishNativeCodeLayout {
                    psi_operation,
                    result,
                    machine,
                } => {
                    tag!(bytes, 255, psi_operation);
                    bytes.id(*machine);
                    encode_structural_operation_result(bytes, result);
                }
                O::StorageAddress {
                    psi_operation,
                    result,
                    source,
                    path,
                } => {
                    tag!(bytes, 122, psi_operation);
                    encode_abstract_result(bytes, *result);
                    bytes.id(*source);
                    bytes.slice(path, encode_structural_path_segment);
                }
                O::EstablishByteSequenceConcatenation {
                    psi_operation,
                    result,
                    left,
                    right,
                    capacity_obligation,
                } => {
                    tag!(bytes, 92, psi_operation);
                    encode_structural_operation_result(bytes, result);
                    for operand in [left, right] {
                        encode_byte_sequence_operand(bytes, operand);
                    }
                    bytes.id(*capacity_obligation);
                }
                O::EstablishStructuralArray {
                    psi_operation,
                    result,
                    elements,
                } => {
                    tag!(bytes, 105, psi_operation);
                    encode_structural_operation_result(bytes, result);
                    bytes.slice(elements, encode_structural_argument);
                }
                O::EstablishScalarArray {
                    psi_operation,
                    result,
                    elements,
                } => {
                    tag!(bytes, 69, psi_operation);
                    encode_structural_operation_result(bytes, result);
                    bytes.slice(elements, |bytes, value| bytes.id(*value));
                }
                O::EstablishPrimitiveLocal {
                    psi_operation,
                    result,
                    value,
                    range_obligation,
                } => {
                    tag!(bytes, 65, psi_operation);
                    encode_structural_operation_result(bytes, result);
                    encode_abstract_result(bytes, *value);
                    encode_optional_id(bytes, range_obligation.as_ref());
                }
                O::PrimitiveLocalStore {
                    psi_operation,
                    destination,
                    value,
                } => {
                    tag!(bytes, 66, psi_operation);
                    bytes.id(*destination);
                    encode_abstract_result(bytes, *value);
                }
                O::StructuralCaseMembership {
                    psi_operation,
                    result,
                    source,
                    path,
                    case,
                } => {
                    tag!(bytes, 71, psi_operation);
                    encode_abstract_result(bytes, *result);
                    bytes.id(*source);
                    bytes.slice(path, encode_structural_path_segment);
                    bytes.id(*case);
                }
                O::PrimitiveScalarRead {
                    psi_operation,
                    result,
                    source,
                    path,
                } => {
                    tag!(bytes, if path.is_empty() { 67 } else { 73 }, psi_operation);
                    encode_abstract_result(bytes, *result);
                    bytes.id(*source);
                    if !path.is_empty() {
                        bytes.slice(path, encode_structural_path_segment);
                    }
                }
                O::ByteSequenceSubslice {
                    psi_operation,
                    result,
                    source,
                    start,
                    end,
                    length,
                    obligation,
                } => {
                    tag!(bytes, 64, psi_operation);
                    encode_structural_operation_result(bytes, result);
                    encode_structural_argument(bytes, source);
                    bytes.id(*start);
                    bytes.id(*end);
                    bytes.id(*length);
                    bytes.id(*obligation);
                }
                O::ElementViewSubslice {
                    psi_operation,
                    result,
                    source,
                    start,
                    end,
                    length,
                    obligation,
                } => {
                    tag!(bytes, 84, psi_operation);
                    encode_structural_operation_result(bytes, result);
                    encode_structural_argument(bytes, source);
                    bytes.id(*start);
                    bytes.id(*end);
                    bytes.id(*length);
                    bytes.id(*obligation);
                }
                O::EstablishElementView {
                    psi_operation,
                    result,
                    destination,
                    source,
                    element,
                    region,
                } => {
                    tag!(bytes, 85, psi_operation);
                    encode_structural_operation_result(bytes, result);
                    bytes.id(*destination);
                    encode_structural_argument(bytes, source);
                    bytes.id(*element);
                    match region {
                        terminal_psi::ElementViewRegion::WholeCollection => bytes.u8(0),
                        terminal_psi::ElementViewRegion::WholeRepresentation {
                            source_length,
                            obligation,
                        } => {
                            bytes.u8(2);
                            bytes.id(*source_length);
                            bytes.id(*obligation);
                        }
                        terminal_psi::ElementViewRegion::RepresentationBytes {
                            start,
                            end,
                            obligation,
                        } => {
                            bytes.u8(1);
                            bytes.id(*start);
                            bytes.id(*end);
                            bytes.id(*obligation);
                        }
                    }
                }
                O::WriteOnlyPrimitiveStore {
                    psi_operation,
                    destination,
                    path,
                    value,
                    range_obligation,
                } => {
                    bytes.u8(if range_obligation.is_some() {
                        106
                    } else if path.is_empty() {
                        49
                    } else {
                        74
                    });
                    bytes.id(*psi_operation);
                    encode_store_destination(bytes, destination);
                    if range_obligation.is_some() || !path.is_empty() {
                        bytes.slice(path, encode_structural_path_segment);
                    }
                    encode_abstract_result(bytes, *value);
                    if let Some(obligation) = range_obligation {
                        bytes.id(*obligation);
                    }
                }
                O::StructuralScalarFieldStore {
                    psi_operation,
                    destination,
                    path,
                    field,
                    value,
                    range_obligation,
                } => {
                    bytes.u8(if range_obligation.is_some() { 75 } else { 50 });
                    bytes.id(*psi_operation);
                    encode_store_destination(bytes, destination);
                    bytes.slice(path, encode_structural_path_segment);
                    bytes.id(*field);
                    encode_abstract_result(bytes, *value);
                    if let Some(obligation) = range_obligation {
                        bytes.id(*obligation);
                    }
                }
                O::EstablishCase {
                    psi_operation,
                    result,
                    result_case,
                    fields,
                } => {
                    tag!(bytes, 48, psi_operation);
                    encode_structural_operation_result(bytes, result);
                    bytes.id(*result_case);
                    bytes.slice(fields, encode_record_initializer);
                }
                O::EstablishByteSequenceLiteral {
                    psi_operation,
                    place,
                    structural_type,
                    bytes: literal,
                    qualifications,
                } => {
                    tag!(bytes, 1, psi_operation);
                    encode_place_declaration(bytes, *place);
                    encode_structural_type(bytes, structural_type);
                    bytes.len(literal.len());
                    bytes.bytes(literal);
                    encode_ids(bytes, qualifications);
                }
                O::EstablishTrivialAffineLocal {
                    psi_operation,
                    place,
                    structural_type,
                } => {
                    tag!(bytes, 2, psi_operation);
                    encode_place_declaration(bytes, *place);
                    encode_structural_type(bytes, structural_type);
                }
                O::EstablishZeroedStructural {
                    psi_operation,
                    result,
                } => {
                    tag!(bytes, 90, psi_operation);
                    encode_structural_operation_result(bytes, result);
                }
                O::EstablishStructuralLocal {
                    psi_operation,
                    result,
                } => {
                    tag!(bytes, 103, psi_operation);
                    encode_structural_operation_result(bytes, result);
                }
                O::EstablishRecord {
                    psi_operation,
                    result,
                    fields,
                } => {
                    tag!(bytes, 56, psi_operation);
                    encode_structural_operation_result(bytes, result);
                    bytes.slice(fields, encode_record_initializer);
                }
                O::EstablishAccessReference {
                    psi_operation,
                    result,
                    source,
                    correspondence,
                } => {
                    tag!(bytes, 113, psi_operation);
                    encode_structural_operation_result(bytes, result);
                    encode_structural_argument(bytes, source);
                    match correspondence {
                        abstract_operations::AccessReferenceBacking::OrdinaryRam(
                            correspondence,
                        ) => {
                            bytes.u8(0);
                            bytes.id(correspondence.schema);
                            bytes.id(correspondence.carrier);
                            bytes.u64(correspondence.source_byte_length);
                            bytes.u64(correspondence.required_base.modulus());
                            bytes.u64(correspondence.required_base.residue());
                            bytes.slice(&correspondence.fields, |bytes, field| {
                                bytes.u32(field.access_slot);
                                bytes.id(field.source_field);
                                bytes.u64(field.byte_offset);
                                bytes.u16(field.transfer_width_bits);
                                super::super::carrier_encoding::encode_scalar_type(
                                    bytes,
                                    field.logical_scalar,
                                );
                                super::super::carrier_encoding::encode_scalar_type(
                                    bytes,
                                    field.stored_scalar,
                                );
                            });
                        }
                        abstract_operations::AccessReferenceBacking::Resident(resident) => {
                            bytes.u8(1);
                            bytes.id(resident.owner);
                            bytes.id(resident.schema);
                            bytes.id(resident.carrier);
                            bytes.id(resident.roles.base);
                            bytes.id(resident.roles.length);
                            bytes.id(resident.roles.granted);
                            bytes.id(resident.roles.resident);
                            bytes.bytes(&resident.placement_commitment);
                        }
                        abstract_operations::AccessReferenceBacking::BorrowedExtent(backing) => {
                            bytes.u8(2);
                            bytes.id(backing.owner);
                            bytes.id(backing.schema);
                            bytes.id(backing.carrier);
                            bytes.id(backing.base);
                            bytes.id(backing.length);
                            bytes.id(backing.granted);
                            bytes.bytes(&backing.placement_commitment);
                        }
                    }
                }
                O::EstablishReference {
                    psi_operation,
                    result,
                    source,
                    storage,
                } => {
                    bytes.u8(if storage.is_some() { 110 } else { 79 });
                    bytes.id(*psi_operation);
                    encode_structural_operation_result(bytes, result);
                    encode_structural_argument(bytes, source);
                    if let Some(storage) = storage {
                        bytes.id(storage.byte_offset);
                        bytes.boolean(storage.stored_scalar.is_some());
                        if let Some(stored_scalar) = storage.stored_scalar {
                            super::super::carrier_encoding::encode_scalar_type(
                                bytes,
                                stored_scalar,
                            );
                        }
                    }
                }
                O::ReleaseReference {
                    psi_operation,
                    source,
                    path,
                } => {
                    tag!(bytes, 80, psi_operation);
                    bytes.id(*source);
                    bytes.slice(path, encode_structural_path_segment);
                }
                O::MoveStructuralField {
                    psi_operation,
                    result,
                    source,
                    path,
                    field,
                } => {
                    tag!(bytes, 81, psi_operation);
                    encode_structural_operation_result(bytes, result);
                    encode_window_root(bytes, source);
                    bytes.slice(path, encode_structural_path_segment);
                    super::encode_optional_field(bytes, *field);
                }
                O::StoreStructuralField {
                    disposition,
                    psi_operation,
                    destination,
                    path,
                    field,
                    value,
                } => {
                    bytes.u8(match disposition {
                        terminal_psi::StructuralStoreDisposition::Vacant => 82,
                        terminal_psi::StructuralStoreDisposition::ReplaceDiscardable => 117,
                    });
                    bytes.id(*psi_operation);
                    encode_window_root(bytes, destination);
                    bytes.slice(path, encode_structural_path_segment);
                    super::encode_optional_field(bytes, *field);
                    encode_structural_argument(bytes, value);
                }
                O::TransformContent {
                    psi_operation,
                    result,
                    source,
                    parents,
                    derivations,
                } => {
                    tag!(bytes, 124, psi_operation);
                    encode_structural_operation_result(bytes, result);
                    encode_structural_argument(bytes, source);
                    bytes.slice(parents, encode_structural_argument);
                    bytes.slice(
                        derivations,
                        super::super::structural_encoding::encode_content_transformation,
                    );
                }
                O::MoveStructuralValue {
                    psi_operation,
                    result,
                    source,
                } => {
                    tag!(bytes, 123, psi_operation);
                    encode_structural_operation_result(bytes, result);
                    encode_structural_argument(bytes, source);
                }
                O::StructuralLeafCopy {
                    psi_operation,
                    result,
                    source,
                    path,
                } => {
                    bytes.u8(match path {
                        StructuralCopyPath::Ordinary(_) => 88,
                        StructuralCopyPath::Case(_) => 91,
                    });
                    bytes.id(*psi_operation);
                    encode_structural_operation_result(bytes, result);
                    bytes.id(*source);
                    match path {
                        StructuralCopyPath::Ordinary(path) => {
                            bytes.slice(path, encode_structural_path_segment);
                        }
                        StructuralCopyPath::Case(path) => {
                            super::super::proposition_encoding::encode_canonical_path(bytes, path);
                        }
                    }
                }
                _ => unreachable!("operation family routing admitted a non-structural operation"),
            }
        }

        fn encode_store_destination(
            bytes: &mut CanonicalBytes,
            destination: &abstract_operations::StructuralScalarStoreDestination,
        ) {
            use abstract_operations::StructuralScalarStoreDestination as Destination;
            match destination {
                Destination::Parameter(parameter) => {
                    bytes.u8(0);
                    encode_structural_parameter(bytes, parameter);
                }
                Destination::OwnedResult { producer, result } => {
                    bytes.u8(1);
                    bytes.id(*producer);
                    encode_structural_operation_result(bytes, result);
                }
                Destination::OwnedBlockParameter { block, parameter } => {
                    bytes.u8(2);
                    bytes.id(*block);
                    encode_structural_parameter(bytes, parameter);
                }
            }
        }

        fn encode_record_initializer(
            bytes: &mut super::CanonicalBytes,
            initializer: &terminal_psi::RecordFieldInitializer,
        ) {
            bytes.id(initializer.field);
            match &initializer.value {
                terminal_psi::RecordFieldValue::Scalar {
                    value,
                    range_obligation,
                } => {
                    bytes.u8(0);
                    bytes.id(*value);
                    encode_optional_id(bytes, range_obligation.as_ref());
                }
                terminal_psi::RecordFieldValue::Structural(argument) => {
                    bytes.u8(1);
                    encode_structural_argument(bytes, argument);
                }
            }
        }
    }
}
