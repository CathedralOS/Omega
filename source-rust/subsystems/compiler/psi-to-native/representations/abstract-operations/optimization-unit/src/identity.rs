/*
A native receipt identifies the checked Psi program, the inputs Stage 02 used to
lower it and the accepted facts attached for target lowering. Changing the
target, selected reach, admitted scalar values or laid-out storage can change
the unit even when the Psi program is the same.

The seed constructor also accepts independently built abstract plans. Their
operands need a content identity of their own. Construction hashes the ordered
functions once and retains that digest with the read-only graph. Module and
fact attachment reuse it. Definitions, uses, effect links, edges and ownership
events are determined by construction from those same functions.
*/

use abstract_operations::{AbstractFunctionResult, CompletionClaimSource, ValueBinding};
use arena::NumericId;
use optimization_core::OptimizationUnitIdentity;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, ContentAlgebra, ContentAlgebraKind, ContentConservation,
    ContentPlaceSegment, ContentPlaceVersion, ContentStructuralPlace, ContentTerm,
    IeeeFloatComparisonKind, IeeeFloatFormat, IeeeFloatStructuralField, IntegerMathTerm,
    IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm, ScalarType,
    StructuralCaseSubject, StructuralPlaceKind,
};
use terminal_psi::{
    BindingRelevance, ByteSequenceCarrier, ClaimContentProjection, ContentConservationGuarantee,
    CrashCause, CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard, EntryClaim,
    EvidenceInterfaceIdentity, OutcomeSpecificCallEvidence, ProviderCandidateConformance,
    StructuralAccess, StructuralArgument, StructuralDomainRequirement, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralPlaceDeclaration,
    StructuralResultDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalAffineCleanupAction,
};

use crate::{AcceptedObligationFact, PsiOptimizationUnit};

use unit_encoding::CanonicalBytes;

pub(crate) fn scalar_receiving_identity_bytes(
    receiving: &crate::SourceCheckedScalarReceiving,
) -> Vec<u8> {
    let mut bytes = CanonicalBytes::collect();
    bytes.id(receiving.value);
    structural_encoding::encode_scalar_restriction(&mut bytes, &receiving.restriction);
    bytes.finish()
}

pub use unit_encoding::recompute_psi_optimization_unit_identity;

pub(crate) mod carrier_encoding {
    use super::{
        CanonicalBytes, IntegerSign, IntegerType, IntegerValue, NumericId, ScalarType,
        StructuralAccess, StructuralArgument, StructuralMultiplicity,
        StructuralParameterDeclaration, StructuralPathSegment, ValueBinding,
        structural_encoding::encode_structural_operation_result,
    };
    pub(super) fn encode_binding(bytes: &mut CanonicalBytes, binding: &ValueBinding) {
        bytes.id(binding.parameter);
        bytes.id(binding.argument);
        encode_scalar_type(bytes, binding.scalar_type);
    }

    pub(super) fn encode_optional_id<T: NumericId>(bytes: &mut CanonicalBytes, value: Option<&T>) {
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

    pub(super) fn encode_ids<T: NumericId>(bytes: &mut CanonicalBytes, ids: &[T]) {
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
        use semantic_vocabulary::IntegerCarrier;
        bytes.u8(match (integer_type.carrier(), integer_type.sign()) {
            (IntegerCarrier::Fixed, IntegerSign::Unsigned) => 1,
            (IntegerCarrier::Fixed, IntegerSign::Signed) => 2,
            (IntegerCarrier::Address, IntegerSign::Unsigned) => 3,
            (IntegerCarrier::Address, IntegerSign::Signed) => 4,
        });
        // This fingerprints raw refinement/receiving metadata; it does not
        // establish executable closure. Keep scoped widths distinct while the
        // published native owner remains responsible for concrete carriers.
        integer_type
            .width()
            .write_identity(|part| bytes.bytes(part));
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
        use super::structural_encoding::encode_projected_qualification_roster as roster;
        roster(bytes, &parameter.projected_qualifications);
        roster(bytes, &parameter.additional_input_qualifications);
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
        terminal_psi::structural_path_encoding::encode_structural_path_segment(
            bytes,
            segment,
            CanonicalBytes::bytes,
            encode_scalar_type,
        );
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
        CanonicalBytes, CanonicalStructuralPathSegment, ContentTerm, IeeeFloatComparisonKind,
        IeeeFloatFormat, IeeeFloatStructuralField, IntegerMathTerm, IntegerType, Proposition,
        ScalarTerm, StructuralCaseSubject,
    };
    pub(super) fn encode_proposition(bytes: &mut CanonicalBytes, proposition: &Proposition) {
        match proposition {
            Proposition::Truth => bytes.u8(1),
            Proposition::Falsehood => bytes.u8(2),
            Proposition::IsMere(inner) => {
                bytes.u8(24);
                encode_proposition(bytes, inner);
            }
            Proposition::Squash(inner) => {
                bytes.u8(26);
                encode_proposition(bytes, inner);
            }
            Proposition::Boxed(inner) => {
                bytes.u8(27);
                encode_proposition(bytes, inner);
            }
            Proposition::Universal {
                carrier,
                binder,
                body,
            } => {
                bytes.u8(25);
                bytes.id(*carrier);
                bytes.id(*binder);
                encode_proposition(bytes, body);
            }
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
            Proposition::StructuralCaseMembership { subject, case } => {
                bytes.u8(13);
                encode_case_subject(bytes, subject);
                bytes.id(*case);
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
            Proposition::FloatIntegerConversion(clause) => {
                bytes.u8(20);
                encode_scalar_term(bytes, &clause.source);
                encode_scalar_term(bytes, &clause.result);
                encode_float_projection_contract(bytes, &clause.projection);
                encode_float_semantic_contract(bytes, &clause.operation);
            }
            Proposition::IeeeFloatIntegerExact { source, target } => {
                bytes.u8(21);
                encode_scalar_term(bytes, source);
                encode_integer_type(bytes, *target);
            }
            Proposition::IntegerIeeeFloatExact { source, target } => {
                bytes.u8(22);
                encode_scalar_term(bytes, source);
                encode_float_format(bytes, *target);
            }
            // Psi publishes only an Identity of inputs, scalar terms and float
            // meanings, but this encoder cannot refuse, so it binds every
            // field of the clause instead.
            Proposition::DeclarationIdentity(clause) => {
                use semantic_vocabulary::{ClauseCarrier, ClauseOperand};
                bytes.u8(23);
                bytes.slice(&clause.values, |bytes, value| {
                    match value.carrier {
                        ClauseCarrier::Parameter(index) => {
                            bytes.u8(0);
                            bytes.u32(index);
                        }
                        ClauseCarrier::Scalar(scalar) => {
                            bytes.u8(1);
                            encode_scalar_type(bytes, scalar);
                        }
                        ClauseCarrier::Structural(structural) => {
                            bytes.u8(2);
                            bytes.id(structural);
                        }
                        ClauseCarrier::FloatMeaning(format) => {
                            bytes.u8(3);
                            encode_float_format(bytes, format);
                        }
                    }
                    match &value.operand {
                        ClauseOperand::Scalar(term) => {
                            bytes.u8(0);
                            encode_scalar_term(bytes, term);
                        }
                        ClauseOperand::Input(subject) => {
                            bytes.u8(1);
                            encode_case_subject(bytes, subject);
                        }
                        ClauseOperand::Formal(index) => {
                            bytes.u8(2);
                            bytes.u32(*index);
                        }
                        ClauseOperand::Application {
                            declaration,
                            arguments,
                        } => {
                            bytes.u8(3);
                            encode_clause_declaration(bytes, *declaration);
                            bytes.slice(arguments, |bytes, argument| bytes.u32(*argument));
                        }
                        ClauseOperand::FloatMeaning(row) => {
                            use semantic_vocabulary::{
                                FloatMeaningClauseOperand as Operand,
                                FloatMeaningClauseValue as Value,
                            };
                            bytes.u8(4);
                            match row {
                                Value::Projection { source, contract } => {
                                    bytes.u8(0);
                                    encode_scalar_term(bytes, source);
                                    encode_float_projection_contract(bytes, contract);
                                }
                                Value::Application {
                                    contract,
                                    format,
                                    operands,
                                } => {
                                    bytes.u8(1);
                                    encode_float_semantic_contract(bytes, contract);
                                    encode_float_format(bytes, *format);
                                    bytes.slice(operands, |bytes, operand| match operand {
                                        Operand::Format(format) => {
                                            bytes.u8(0);
                                            encode_float_format(bytes, *format);
                                        }
                                        Operand::Meaning(slot) => {
                                            bytes.u8(1);
                                            bytes.u32(slot.0);
                                        }
                                        Operand::Integer(source) => {
                                            bytes.u8(2);
                                            encode_scalar_term(bytes, source);
                                        }
                                    });
                                }
                            }
                        }
                    }
                });
                bytes.u32(clause.left);
                bytes.u32(clause.right);
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

    fn encode_clause_declaration(
        bytes: &mut CanonicalBytes,
        declaration: semantic_vocabulary::ClauseDeclaration,
    ) {
        bytes.boolean(declaration.supplier.is_some());
        if let Some((slot, generation)) = declaration.supplier {
            bytes.u32(slot);
            bytes.u32(generation);
        }
        bytes.u32(declaration.member.0);
        bytes.u32(declaration.member.1);
    }

    pub(super) fn encode_scalar_term(bytes: &mut CanonicalBytes, term: &ScalarTerm) {
        use ScalarTerm as S;
        match term {
            S::DeclarationApplication {
                declaration,
                arguments,
                scalar_type,
            } => {
                bytes.u8(45);
                encode_clause_declaration(bytes, *declaration);
                encode_scalar_type(bytes, *scalar_type);
                bytes.slice(arguments, encode_scalar_term);
            }
            S::Application {
                application,
                scalar_type,
            } => {
                bytes.u8(41);
                bytes.id(*application);
                encode_scalar_type(bytes, *scalar_type);
            }
            S::IeeeFloatField {
                version,
                root,
                path,
                format,
            } => {
                bytes.u8(match version {
                    semantic_vocabulary::ContentPlaceVersion::Current => 38,
                    semantic_vocabulary::ContentPlaceVersion::Entry => 44,
                });
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
            S::BooleanField {
                version,
                root,
                path,
            } => {
                bytes.u8(match version {
                    semantic_vocabulary::ContentPlaceVersion::Current => 4,
                    semantic_vocabulary::ContentPlaceVersion::Entry => 42,
                });
                bytes.id(*root);
                encode_canonical_path(bytes, path);
            }
            S::IntegerField {
                version,
                root,
                path,
                scalar_type,
            } => {
                bytes.u8(match version {
                    semantic_vocabulary::ContentPlaceVersion::Current => 5,
                    semantic_vocabulary::ContentPlaceVersion::Entry => 43,
                });
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

    fn encode_float_projection_contract(
        bytes: &mut CanonicalBytes,
        contract: &numerics::float_projection::FloatProjectionContractIdentity,
    ) {
        bytes.u16(contract.format);
        bytes.u8(contract.operation);
        bytes.u8(contract.declaration);
        bytes.u16(contract.catalog_version);
        bytes.bytes(&contract.commitment);
    }

    fn encode_float_semantic_contract(
        bytes: &mut CanonicalBytes,
        contract: &numerics::float_semantics::FloatSemanticContractIdentity,
    ) {
        use numerics::float_semantics::FloatSemanticOperationIdentity as Operation;
        let (tag, carrier) = match contract.operation {
            Operation::Fixed(row) => {
                bytes.u8(0);
                bytes.u8(row);
                (0, None)
            }
            Operation::FromInteger(carrier) => (1, Some(carrier)),
            Operation::ToIntegerExact(carrier) => (2, Some(carrier)),
            Operation::ToIntegerSaturating(carrier) => (3, Some(carrier)),
        };
        if let Some(carrier) = carrier {
            bytes.u8(tag);
            bytes.boolean(carrier.is_signed());
            bytes.u64(carrier.width());
        }
        bytes.u16(contract.catalog_version);
        bytes.bytes(&contract.commitment);
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
    encode_rooted_field!(encode_case_subject, StructuralCaseSubject);

    #[cfg(test)]
    mod tests {
        use super::*;
        use semantic_vocabulary::{
            ClauseCarrier, ClauseDeclaration, ClauseIdentity, ClauseOperand, ClauseValue,
        };

        fn identity_bytes(supplier: Option<(u32, u32)>, member: (u32, u32)) -> Vec<u8> {
            let carrier = ClauseCarrier::Parameter(0);
            let proposition = Proposition::DeclarationIdentity(ClauseIdentity {
                values: vec![
                    ClauseValue {
                        carrier,
                        operand: ClauseOperand::Formal(0),
                    },
                    ClauseValue {
                        carrier,
                        operand: ClauseOperand::Application {
                            declaration: ClauseDeclaration { supplier, member },
                            arguments: vec![0],
                        },
                    },
                ],
                left: 0,
                right: 1,
            });
            let mut bytes = CanonicalBytes::collect();
            encode_proposition(&mut bytes, &proposition);
            bytes.finish()
        }

        #[test]
        fn mere_proposition_identity_includes_its_operand() {
            let encoded = |proposition| {
                let mut bytes = CanonicalBytes::collect();
                encode_proposition(&mut bytes, &proposition);
                bytes.finish()
            };
            let mere = |inner| Proposition::IsMere(Box::new(inner));
            assert_ne!(
                encoded(mere(Proposition::Truth)),
                encoded(Proposition::Truth)
            );
            assert_ne!(
                encoded(mere(Proposition::Truth)),
                encoded(mere(Proposition::Falsehood))
            );
        }

        #[test]
        fn declaration_identity_commits_its_cited_declaration() {
            let cited = identity_bytes(Some((1, 1)), (2, 1));
            assert_eq!(cited[0], 23);
            assert_eq!(cited, identity_bytes(Some((1, 1)), (2, 1)));
            assert_ne!(cited, identity_bytes(Some((1, 2)), (2, 1)));
            assert_ne!(cited, identity_bytes(None, (2, 1)));
            assert_ne!(cited, identity_bytes(Some((1, 1)), (3, 1)));
        }
    }
}

pub(crate) mod structural_encoding {
    use super::carrier_encoding::{
        encode_access, encode_ids, encode_integer_type, encode_integer_value, encode_multiplicity,
        encode_optional, encode_optional_access, encode_scalar_type,
        encode_structural_path_segment,
    };
    use super::proposition_encoding::{
        encode_content_term, encode_float_format, encode_proposition,
    };
    use super::{
        BindingRelevance, ByteSequenceCarrier, CanonicalBytes, ClaimContentProjection,
        CompletionClaimSource, ContentAlgebra, ContentAlgebraKind, ContentConservation,
        ContentConservationGuarantee, ContentPlaceSegment, ContentPlaceVersion,
        ContentStructuralPlace, EntryClaim, ProviderCandidateConformance,
        StructuralDomainRequirement, StructuralFieldDeclaration, StructuralFieldType,
        StructuralOperationResult, StructuralPlaceDeclaration, StructuralPlaceKind,
        StructuralTypeDeclaration, StructuralTypeShape, TerminalAffineCleanupAction,
    };

    mod contract {
        use super::super::carrier_encoding::{
            encode_ids, encode_optional, encode_structural_path_segment,
        };
        use super::super::proposition_encoding::encode_proposition;
        use super::super::{
            CanonicalBytes, CrashCause, CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard,
            EvidenceInterfaceIdentity, OutcomeSpecificCallEvidence,
        };

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
    }

    pub(super) use contract::{
        encode_crash_cause, encode_crash_predicate, encode_crash_route_bucket,
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

    pub(super) fn encode_domain_requirement(
        bytes: &mut CanonicalBytes,
        requirement: &StructuralDomainRequirement,
    ) {
        bytes.u32(requirement.argument_index);
        bytes.slice(&requirement.path, encode_structural_path_segment);
        bytes.id(requirement.domain);
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
            encode_access(bytes, parameter.access);
            encode_ids(bytes, &parameter.qualifications);
            encode_projected_qualification_roster(bytes, &parameter.projected_qualifications);
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
        bytes.u32(declaration.declaration_identity.0);
        bytes.u8(u8::from(declaration.no_code_affine_cleanup));
        bytes.u8(u8::from(declaration.owns_linear_claim));
        bytes.u8(u8::from(declaration.descriptor_cleanup));
        encode_multiplicity(bytes, declaration.multiplicity);
        encode_optional(
            bytes,
            declaration.nominal_cleanup.as_ref(),
            |bytes, selection| {
                bytes.u32(selection.declaration.identity().0);
                bytes.id(selection.cleanup_machine);
            },
        );
        bytes.slice(&declaration.requires, encode_proposition);
        encode_optional(
            bytes,
            declaration.scalar_restriction.as_ref(),
            encode_scalar_restriction,
        );
        match &declaration.shape {
            // This tag distinguishes hash inputs; executable codecs reject open storage.
            StructuralTypeShape::TypeParameter => bytes.u8(20),
            StructuralTypeShape::Opaque => bytes.u8(19),
            StructuralTypeShape::Reference { referent, access } => {
                bytes.u8(7);
                bytes.id(*referent);
                encode_access(bytes, *access);
            }
            StructuralTypeShape::PrimitiveScalar(scalar_type) => {
                bytes.u8(6);
                encode_scalar_type(bytes, *scalar_type);
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
            }
            StructuralTypeShape::FixedArray { element, length } => {
                bytes.u8(3);
                bytes.id(*element);
                match length {
                    terminal_psi::ArrayExtent::Parameter { owner, value } => {
                        bytes.u8(2);
                        bytes.id(*owner);
                        bytes.id(*value);
                    }
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
                    bytes.u32(case.declaration_identity.0);
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
                    bytes.u32(case.declaration_identity.0);
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

    pub(super) fn encode_structural_field(
        bytes: &mut CanonicalBytes,
        field: &StructuralFieldDeclaration,
    ) {
        bytes.id(field.id);
        bytes.u32(field.identity.0);
        bytes.u32(field.declaration_identity.0);
        bytes.u8(u8::from(field.inline_carrier.is_some()));
        if let Some(carrier) = field.inline_carrier {
            bytes.id(carrier);
        }
        bytes.u8(match field.relevance {
            BindingRelevance::Relevant => 1,
            BindingRelevance::Erased => 2,
        });
        encode_scalar_restriction(bytes, &field.field_type);
        encode_authored_field_storage(bytes, field.storage.as_ref());
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
        bytes.slice(
            &result.qualification_establishments,
            |bytes, establishment| {
                bytes.id(establishment.domain);
                bytes.u32(establishment.route);
            },
        );
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

    pub(super) fn encode_owned_cleanup(
        bytes: &mut CanonicalBytes,
        cleanup: &terminal_psi::OwnedCleanupInvocation,
    ) {
        bytes.u8(6);
        super::carrier_encoding::encode_structural_argument(bytes, &cleanup.subject);
        bytes.id(cleanup.structural_type);
        bytes.id(cleanup.cleanup_machine);
        bytes.slice(
            &cleanup.claim_transfers,
            super::operation_encoding::encode_claim_transfer,
        );
        encode_ids(bytes, &cleanup.requirement_obligations);
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
            TerminalAffineCleanupAction::InvokeOwned(cleanup) => {
                encode_owned_cleanup(bytes, cleanup)
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
            StructuralFieldType::Erased {
                type_identity,
                scalar,
            } => {
                bytes.u8(5);
                bytes.u32(type_identity.0);
                encode_optional(bytes, scalar.as_ref(), |bytes, scalar| {
                    encode_scalar_type(bytes, *scalar)
                });
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use semantic_vocabulary::{MachineId, PlaceId, StructuralTypeId};
        use terminal_psi::{
            OwnedCleanupInvocation, StructuralAccess, StructuralArgument, StructuralPathSegment,
            TerminalAffineCleanupAction,
        };

        #[test]
        fn exact_inline_carrier_changes_field_identity() {
            let encodings = [None, Some(2), Some(3)].map(|identity| {
                let field = terminal_psi::StructuralFieldDeclaration {
                    declaration_identity: Default::default(),
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
        fn selected_cleanup_changes_structural_identity() {
            let mut encodings = Vec::new();
            for selected in [None, Some((2, 3)), Some((4, 3)), Some((2, 5))] {
                let declaration = StructuralTypeDeclaration {
                    requires: Vec::new(),
                    quotient_carrier: None,
                    owns_linear_claim: false,
                    no_code_affine_cleanup: false,
                    descriptor_cleanup: false,
                    nominal_cleanup: selected.map(|(identity, machine)| {
                        terminal_psi::NominalCleanupSelection {
                            declaration: terminal_psi::NominalDeclarationIdentity::new(
                                terminal_psi::TerminalIdentityIndex(identity),
                            ),
                            cleanup_machine: MachineId::new(machine).unwrap(),
                        }
                    }),
                    multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                    scalar_restriction: None,
                    id: StructuralTypeId::new(1).unwrap(),
                    identity: terminal_psi::TerminalIdentityIndex(1),
                    declaration_identity: terminal_psi::TerminalIdentityIndex(1),
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
        fn declared_multiplicity_changes_encoded_identity() {
            let mut encodings = Vec::new();
            for multiplicity in [
                terminal_psi::StructuralMultiplicity::Unrestricted,
                terminal_psi::StructuralMultiplicity::Affine,
                terminal_psi::StructuralMultiplicity::Linear,
            ] {
                let declaration = StructuralTypeDeclaration {
                    declaration_identity: Default::default(),
                    requires: Vec::new(),
                    quotient_carrier: None,
                    owns_linear_claim: false,
                    no_code_affine_cleanup: false,
                    descriptor_cleanup: true,
                    nominal_cleanup: None,
                    multiplicity,
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
        fn owned_cleanup_identity_distinguishes_sources_paths_and_claim_transfers() {
            let original = OwnedCleanupInvocation {
                subject: StructuralArgument::whole(
                    PlaceId::new(1).unwrap(),
                    StructuralAccess::Owned,
                ),
                structural_type: StructuralTypeId::new(2).unwrap(),
                cleanup_machine: MachineId::new(3).unwrap(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
            };
            let mut encodings = Vec::new();
            for change in 0..6 {
                let mut cleanup = original.clone();
                match change {
                    0 => {}
                    1 => cleanup.subject.place = PlaceId::new(2).unwrap(),
                    2 => cleanup.subject.path.push(StructuralPathSegment::Field(
                        terminal_psi::TerminalIdentityIndex(1),
                    )),
                    3 => cleanup.cleanup_machine = MachineId::new(4).unwrap(),
                    4 | 5 => cleanup.claim_transfers.push(terminal_psi::ClaimTransfer {
                        claim: semantic_vocabulary::ClaimId::new(1).unwrap(),
                        argument_index: 0,
                        path: if change == 5 {
                            vec![StructuralPathSegment::FixedIndex(1)]
                        } else {
                            Vec::new()
                        },
                    }),
                    _ => unreachable!(),
                }
                let action = TerminalAffineCleanupAction::InvokeOwned(cleanup);
                let mut bytes = CanonicalBytes::collect();
                encode_cleanup(&mut bytes, &action);
                let encoded = bytes.finish();
                assert!(!encodings.contains(&encoded));
                encodings.push(encoded);
            }
        }
    }
}

pub(crate) mod unit_encoding {
    use super::carrier_encoding::{
        encode_abstract_result, encode_access, encode_ids, encode_multiplicity, encode_optional,
        encode_scalar_type, encode_structural_parameter, encode_structural_path_segment,
    };
    use super::operation_encoding::encode_operation;
    use super::structural_encoding::{
        encode_entry_claim, encode_projected_qualification_roster, encode_provider_candidate,
        encode_structural_type,
    };
    use super::{
        AbstractFunctionResult, AcceptedObligationFact, NumericId, OptimizationUnitIdentity,
        PsiOptimizationUnit, StructuralResultDeclaration,
    };

    const UNIT_IDENTITY_DOMAIN: &[u8] = b"omega.psi-optimization-unit-content.v64\0";
    pub fn recompute_psi_optimization_unit_identity(
        unit: &PsiOptimizationUnit,
    ) -> OptimizationUnitIdentity {
        OptimizationUnitIdentity::from_canonical_bytes(&CanonicalBytes::digest(
            UNIT_IDENTITY_DOMAIN,
            |bytes| encode_unit(bytes, unit),
        ))
    }

    fn encode_unit(bytes: &mut CanonicalBytes, unit: &PsiOptimizationUnit) {
        bytes.u16(unit.psi.vocabulary_marker.get());
        bytes.bytes(unit.psi.program_fingerprint.as_bytes());
        bytes.u32(unit.work_schedule.marker());
        bytes.id(unit.entry);
        encode_optional(
            bytes,
            unit.scope_provider_closure.as_ref(),
            |bytes, digest| bytes.bytes(digest),
        );
        encode_optional(bytes, unit.lowering_target.as_ref(), |bytes, target| {
            bytes.string(target)
        });
        encode_optional(
            bytes,
            unit.scalar_values_digest.as_ref(),
            |bytes, digest| bytes.bytes(digest),
        );
        bytes.slice(
            unit.structural_types.owned_sequence_bounds(),
            |bytes, (ty, bound)| {
                bytes.id(*ty);
                bytes.u64(*bound);
            },
        );
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
        // Rows copied verbatim from the Terminal module are already committed
        // by `unit.psi`, the fingerprint of the whole module encoding: the
        // structural domains, services, root service reach and runtime
        // evidence attached from the verified module, and the plan's callable
        // and dynamic-dispatch catalogs (`identities` was never hashed for the
        // same reason). Only the choice of rows enters here: whether the
        // module context is attached, and which boundary declarations the
        // native scope kept.
        encode_optional(bytes, unit.runtime_evidence.as_deref(), |bytes, catalog| {
            bytes.u16(catalog.psi().vocabulary_marker.get());
            bytes.bytes(catalog.psi().program_fingerprint.as_bytes());
        });
        bytes.slice(&unit.boundary_machines, |bytes, boundary| {
            bytes.id(boundary.id)
        });
        bytes.slice(&unit.provider_candidates, encode_provider_candidate);
        bytes.slice(&unit.accepted_obligation_facts, encode_accepted_fact);
        bytes.bytes(&unit.functions.digest);
    }

    pub(super) enum CanonicalBytes {
        Digest(wire::digest_encoder::DomainDigestEncoder),
        Collect(Vec<u8>),
    }

    impl CanonicalBytes {
        fn digest(domain: &[u8], encode: impl FnOnce(&mut Self)) -> [u8; 32] {
            let mut output = Self::Digest(wire::digest_encoder::DomainDigestEncoder::new(domain));
            encode(&mut output);
            match output {
                Self::Digest(digest) => digest.finish(),
                Self::Collect(_) => unreachable!("digest sink"),
            }
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

        pub(super) fn bytes(&mut self, bytes: &[u8]) {
            match self {
                Self::Digest(encoder) => encoder.raw(bytes),
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

        pub(super) fn id(&mut self, value: impl NumericId) {
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
            crate::AcceptedObligationOwner::Operation(operation) => {
                bytes.u8(0);
                bytes.id(operation);
            }
            crate::AcceptedObligationOwner::Edge(edge) => {
                bytes.u8(1);
                bytes.id(edge);
            }
            crate::AcceptedObligationOwner::Terminator(block) => {
                bytes.u8(2);
                bytes.id(block);
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

    pub(crate) fn function_digest(functions: &[abstract_operations::AbstractFunction]) -> [u8; 32] {
        CanonicalBytes::digest(b"omega.abstract-functions.v1\0", |bytes| {
            bytes.slice(functions, encode_function);
        })
    }

    fn encode_parameter(
        bytes: &mut CanonicalBytes,
        parameter: &abstract_operations::AbstractParameter,
    ) {
        bytes.id(parameter.value);
        encode_scalar_type(bytes, parameter.scalar_type);
    }

    fn encode_function(
        bytes: &mut CanonicalBytes,
        function: &abstract_operations::AbstractFunction,
    ) {
        let abstract_operations::AbstractFunction {
            machine,
            attachment,
            entry,
            parameters,
            structural_parameters,
            result,
            entry_claims,
            block_entry_claims,
            published_service_ceiling,
            blocks,
        } = function;
        bytes.id(*machine);
        encode_optional(bytes, attachment.as_ref(), |bytes, attachment| {
            bytes.id(*attachment)
        });
        bytes.id(*entry);
        bytes.slice(parameters, encode_parameter);
        bytes.slice(structural_parameters, encode_structural_parameter);
        encode_function_result(bytes, result);
        bytes.slice(entry_claims, encode_entry_claim);
        bytes.slice(block_entry_claims, encode_entry_claim);
        encode_ids(bytes, published_service_ceiling);
        bytes.slice(blocks, |bytes, block| {
            let abstract_operations::AbstractBlock {
                block,
                parameters,
                structural_parameters,
                operations,
            } = block;
            bytes.id(*block);
            bytes.slice(parameters, encode_parameter);
            bytes.slice(structural_parameters, encode_structural_parameter);
            bytes.slice(operations, encode_operation);
        });
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

    fn encode_access_view_plans(
        bytes: &mut CanonicalBytes,
        plans: &[terminal_psi::TerminalAccessViewPlan],
    ) {
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

    fn encode_access_view_catalog(
        bytes: &mut CanonicalBytes,
        catalog: &abstract_operations::StructuralTypeCatalog,
    ) {
        encode_access_view_plans(bytes, catalog.access_views());
        let lifecycles = catalog.access_lifecycle_identity_bytes();
        bytes.len(lifecycles.len());
        bytes.bytes(lifecycles);
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

pub(crate) mod operation_encoding {
    use super::CanonicalBytes;
    use super::carrier_encoding::*;
    use super::proposition_encoding::{encode_canonical_path, encode_float_format};
    use super::structural_encoding::*;
    use abstract_operations::{
        AbstractOperation, IntegerArithmetic, IntegerArithmeticOperator, IntegerOverflowPolicy,
        IntegerShiftDirection, IntegerShiftPolicy,
    };
    use semantic_vocabulary::{IntegerType, ScalarType};

    // Bare plans can supply every operand independently. Match every field so that adding
    // an operand to a variant requires choosing its canonical encoding.
    trait OperationField {
        fn encode(&self, bytes: &mut CanonicalBytes);
    }

    impl<T: OperationField> OperationField for Vec<T> {
        fn encode(&self, bytes: &mut CanonicalBytes) {
            bytes.slice(self, |bytes, field| field.encode(bytes));
        }
    }
    impl<T: OperationField> OperationField for Option<T> {
        fn encode(&self, bytes: &mut CanonicalBytes) {
            encode_optional(bytes, self.as_ref(), |bytes, field| field.encode(bytes));
        }
    }

    macro_rules! field {
        ($ty:ty, |$bytes:ident, $value:ident| $body:expr) => {
            impl OperationField for $ty {
                fn encode(&self, $bytes: &mut CanonicalBytes) {
                    let $value = self;
                    $body
                }
            }
        };
    }

    macro_rules! ids {
        ($($ty:ident),+) => { $(field!(semantic_vocabulary::$ty, |bytes, value| bytes.id(*value));)+ };
    }
    ids!(
        OperationId,
        ValueId,
        MachineId,
        StructuralTypeId,
        StructuralDomainId,
        StructuralCaseId,
        StructuralFieldId,
        ClaimId,
        PlaceId,
        ServiceId,
        ObligationId,
        BoundaryMachineId,
        MachineDescriptorId,
        EdgeId,
        BlockId
    );
    field!(u8, |bytes, value| bytes.u8(*value));
    field!(u16, |bytes, value| bytes.u16(*value));
    field!(u32, |bytes, value| bytes.u32(*value));
    field!(u64, |bytes, value| bytes.u64(*value));
    field!(bool, |bytes, value| bytes.boolean(*value));
    field!(abstract_operations::AbstractResult, |bytes, value| {
        encode_abstract_result(bytes, *value)
    });
    field!(IntegerType, |bytes, value| encode_integer_type(
        bytes, *value
    ));
    field!(ScalarType, |bytes, value| encode_scalar_type(bytes, *value));
    field!(semantic_vocabulary::IntegerValue, |bytes, value| {
        encode_integer_value(bytes, *value)
    });
    field!(terminal_psi::CrashCause, |bytes, value| encode_crash_cause(
        bytes, *value
    ));
    field!(terminal_psi::StructuralPlaceDeclaration, |bytes, value| {
        encode_place_declaration(bytes, *value)
    });
    field!(semantic_vocabulary::IeeeFloatFormat, |bytes, value| {
        encode_float_format(bytes, *value)
    });
    field!(abstract_operations::ValueBinding, |bytes, value| {
        encode_binding(bytes, value)
    });
    field!(terminal_psi::StructuralOperationResult, |bytes, value| {
        encode_structural_operation_result(bytes, value)
    });
    field!(terminal_psi::StructuralTypeDeclaration, |bytes, value| {
        encode_structural_type(bytes, value)
    });
    field!(terminal_psi::StructuralArgument, |bytes, value| {
        encode_structural_argument(bytes, value)
    });
    field!(
        abstract_operations::AbstractStructuralBinding,
        |bytes, value| encode_structural_binding(bytes, value)
    );
    field!(terminal_psi::StructuralPathSegment, |bytes, value| {
        encode_structural_path_segment(bytes, value)
    });
    field!(terminal_psi::TerminalAffineCleanupAction, |bytes, value| {
        encode_cleanup(bytes, value)
    });
    field!(
        abstract_operations::CompletionClaimSource,
        |bytes, value| encode_completion_claim_source(bytes, value)
    );
    field!(terminal_psi::CrashPredicateTerm, |bytes, value| {
        encode_crash_predicate(bytes, value)
    });
    field!(terminal_psi::CrashRouteBucket, |bytes, value| {
        encode_crash_route_bucket(bytes, value)
    });
    field!(terminal_psi::ClaimTransfer, |bytes, value| {
        encode_claim_transfer(bytes, value)
    });
    field!(
        terminal_psi::ContentTransformationDerivation,
        |bytes, value| encode_content_transformation(bytes, value)
    );
    field!(
        terminal_psi::control_flow::ByteSequenceOperand,
        |bytes, value| encode_byte_sequence_operand(bytes, value)
    );
    field!(abstract_operations::StructuralWindowRoot, |bytes, value| {
        encode_window_root(bytes, value)
    });
    field!(terminal_psi::RecordFieldInitializer, |bytes, value| {
        encode_record_initializer(bytes, value)
    });
    field!(abstract_operations::AbstractSuccessor, |bytes, value| {
        encode_successor(bytes, value)
    });
    field!(
        Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
        |bytes, value| encode_canonical_path(bytes, value)
    );
    field!(terminal_psi::SegmentRegister, |bytes, value| bytes
        .u8(value.code()));
    field!(terminal_psi::SegmentReload, |bytes, value| bytes
        .u8(value.code()));
    field!(terminal_psi::ByteSequencePredicate, |bytes, value| bytes
        .u8(value.mask_bit()));
    field!(abstract_operations::AbstractCallTarget, |bytes, value| {
        value.visit_canonical_bytes(|part| bytes.bytes(part))
    });
    field!(
        semantic_vocabulary::PackedIntegerUnaryOperation,
        |bytes, value| match value {
            semantic_vocabulary::PackedIntegerUnaryOperation::ShiftRightArithmeticI16x8 {
                count,
            } => {
                bytes.u8(0);
                bytes.u8(*count);
            }
            semantic_vocabulary::PackedIntegerUnaryOperation::WidenU8x8ToI16x8 => bytes.u8(1),
            semantic_vocabulary::PackedIntegerUnaryOperation::NarrowI16x8ToU8x8Saturating =>
                bytes.u8(2),
            semantic_vocabulary::PackedIntegerUnaryOperation::LoadI16x4ZeroExtend => bytes.u8(3),
            semantic_vocabulary::PackedIntegerUnaryOperation::WidenU8x4ToI16x8 => bytes.u8(4),
            semantic_vocabulary::PackedIntegerUnaryOperation::NarrowI16x8ToU8x4Saturating =>
                bytes.u8(5),
            semantic_vocabulary::PackedIntegerUnaryOperation::AbsI16x8 => bytes.u8(6),
        }
    );
    field!(
        semantic_vocabulary::PackedIntegerBinaryOperation,
        |bytes, value| match value {
            semantic_vocabulary::PackedIntegerBinaryOperation::WrappingAddI16x8 => {
                bytes.u8(0)
            }
            semantic_vocabulary::PackedIntegerBinaryOperation::WrappingSubI16x8 => {
                bytes.u8(1)
            }
            semantic_vocabulary::PackedIntegerBinaryOperation::ShuffleI16x8 { lanes } => {
                bytes.u8(2);
                for lane in lanes {
                    bytes.u8(*lane);
                }
            }
            semantic_vocabulary::PackedIntegerBinaryOperation::WrappingAddU8x16 => {
                bytes.u8(3)
            }
            semantic_vocabulary::PackedIntegerBinaryOperation::MinI16x8 => bytes.u8(4),
            semantic_vocabulary::PackedIntegerBinaryOperation::CmpGtI16x8 => {
                bytes.u8(5)
            }
            semantic_vocabulary::PackedIntegerBinaryOperation::BitAndI16x8 => {
                bytes.u8(6)
            }
            semantic_vocabulary::PackedIntegerBinaryOperation::BitAndNotI16x8 => {
                bytes.u8(7)
            }
        }
    );
    field!(
        terminal_psi::ElementViewRegion,
        |bytes, value| match value {
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
    );
    field!(
        semantic_vocabulary::IeeeFloatValue,
        |bytes, value| match value {
            semantic_vocabulary::IeeeFloatValue::Binary32(bits) => {
                bytes.u8(0);
                bytes.u32(*bits);
            }
            semantic_vocabulary::IeeeFloatValue::Binary64(bits) => {
                bytes.u8(1);
                bytes.u64(*bits);
            }
        }
    );
    field!(
        abstract_operations::AccessReferenceBacking,
        |bytes, value| match value {
            abstract_operations::AccessReferenceBacking::OrdinaryRam(correspondence) => {
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
                    encode_scalar_type(bytes, field.logical_scalar);
                    encode_scalar_type(bytes, field.stored_scalar);
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
    );
    field!(
        abstract_operations::AbstractBoundaryResult,
        |bytes, value| match value {
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
    );
    field!(
        semantic_vocabulary::ExternalCompletionKind,
        |bytes, value| match value {
            semantic_vocabulary::ExternalCompletionKind::ProcessExit => bytes.u8(1),
        }
    );
    field!(terminal_psi::StorageLocationQuery, |bytes, value| bytes.u8(
        match value {
            terminal_psi::StorageLocationQuery::Address => 0,
            terminal_psi::StorageLocationQuery::Alignment => 1,
        }
    ));
    field!(
        terminal_psi::GlobalDescriptorTableOperation,
        |bytes, value| bytes.u8(match value {
            terminal_psi::GlobalDescriptorTableOperation::Store => 0,
            terminal_psi::GlobalDescriptorTableOperation::Load => 1,
        })
    );
    field!(
        semantic_vocabulary::IeeeFloatComparisonOperation,
        |bytes, value| bytes.u8(match value {
            semantic_vocabulary::IeeeFloatComparisonOperation::Equal => 0,
            semantic_vocabulary::IeeeFloatComparisonOperation::NotEqual => 1,
            semantic_vocabulary::IeeeFloatComparisonOperation::Less => 2,
            semantic_vocabulary::IeeeFloatComparisonOperation::LessOrEqual => 3,
            semantic_vocabulary::IeeeFloatComparisonOperation::Greater => 4,
            semantic_vocabulary::IeeeFloatComparisonOperation::GreaterOrEqual => 5,
        })
    );
    field!(terminal_psi::StructuralStoreDisposition, |bytes, value| {
        bytes.u8(match value {
            terminal_psi::StructuralStoreDisposition::Vacant => 0,
            terminal_psi::StructuralStoreDisposition::ReplaceDiscardable => 1,
        })
    });
    field!(terminal_psi::FloatResultPolicy, |bytes, value| {
        bytes.u8(match value {
            terminal_psi::FloatResultPolicy::Trapping { .. } => 0,
            terminal_psi::FloatResultPolicy::Saturating { .. } => 1,
        });
        for operand in value.operands() {
            bytes.id(operand);
        }
    });
    field!(IntegerShiftPolicy, |bytes, value| match value {
        IntegerShiftPolicy::Wrapping => bytes.u8(0),
        IntegerShiftPolicy::Exact { obligation } => {
            bytes.u8(1);
            bytes.id(*obligation);
        }
    });
    field!(IntegerShiftDirection, |bytes, value| bytes.u8(
        match value {
            IntegerShiftDirection::Left => 0,
            IntegerShiftDirection::Right => 1,
        }
    ));
    field!(IntegerArithmetic, |bytes, value| {
        bytes.u8(match value.operator() {
            IntegerArithmeticOperator::Add => 0,
            IntegerArithmeticOperator::Subtract => 1,
            IntegerArithmeticOperator::Multiply => 2,
            IntegerArithmeticOperator::Divide => 3,
            IntegerArithmeticOperator::Remainder => 4,
        });
        bytes.u8(match value.policy() {
            IntegerOverflowPolicy::Wrapping => 0,
            IntegerOverflowPolicy::Exact => 1,
            IntegerOverflowPolicy::Saturating => 2,
        });
        value.obligation().encode(bytes);
    });
    field!(terminal_psi::TrappingIntegerOperation, |bytes, value| {
        use terminal_psi::TrappingIntegerPrimitive as P;
        bytes.u8(match value.primitive() {
            P::Add => 0,
            P::Subtract => 1,
            P::Multiply => 2,
            P::Divide => 3,
            P::Remainder => 4,
            P::ShiftLeft => 5,
            P::ShiftRight => 6,
            P::Convert => 7,
        });
        for operand in value.operands() {
            bytes.id(operand);
        }
    });
    field!(terminal_psi::RegisterAccessEvent, |bytes, value| {
        bytes.u16(value.operation().catalog_tag());
        bytes.len(value.inputs().count());
        for input in value.inputs() {
            bytes.id(*input);
        }
    });
    field!(terminal_psi::ReferenceStorageView, |bytes, value| {
        bytes.id(value.byte_offset);
        value.stored_scalar.encode(bytes);
    });
    field!(
        abstract_operations::StructuralCopyPath,
        |bytes, value| match value {
            abstract_operations::StructuralCopyPath::Ordinary(path) => {
                bytes.u8(0);
                path.encode(bytes);
            }
            abstract_operations::StructuralCopyPath::Case(path) => {
                bytes.u8(1);
                path.encode(bytes);
            }
        }
    );
    field!(
        (
            semantic_vocabulary::OperationId,
            terminal_psi::StructuralPlaceDeclaration,
            terminal_psi::StructuralTypeDeclaration
        ),
        |bytes, value| {
            value.0.encode(bytes);
            value.1.encode(bytes);
            value.2.encode(bytes);
        }
    );
    field!(terminal_psi::CompletionReceipt, |bytes, value| {
        bytes.id(value.claim);
        bytes.u32(value.argument_index);
        value.path.encode(bytes);
    });
    field!(
        terminal_psi::StructuralResultClaimBinding,
        |bytes, value| {
            bytes.id(value.claim);
            value.path.encode(bytes);
        }
    );
    field!(
        abstract_operations::AbstractStructuralCaseSuccessor,
        |bytes, value| {
            value.psi_edge.encode(bytes);
            value.target.encode(bytes);
            value.case.encode(bytes);
            bytes.slice(&value.payloads, |bytes, payload| {
                bytes.id(payload.parameter);
                bytes.id(payload.field);
                encode_scalar_type(bytes, payload.scalar_type);
            });
            value.structural_bindings.encode(bytes);
            value.cleanup_actions.encode(bytes);
        }
    );
    field!(
        abstract_operations::AbstractStructuralCallResult,
        |bytes, value| match value {
            abstract_operations::AbstractStructuralCallResult::Unit => bytes.u8(0),
            abstract_operations::AbstractStructuralCallResult::Scalar(result) => {
                bytes.u8(1);
                result.encode(bytes);
            }
            abstract_operations::AbstractStructuralCallResult::Structural {
                result,
                selected_evidence,
            } => {
                bytes.u8(2);
                result.encode(bytes);
                bytes.slice(selected_evidence, encode_outcome_specific_call_evidence);
            }
        }
    );
    pub(super) fn encode_claim_transfer(
        bytes: &mut CanonicalBytes,
        transfer: &terminal_psi::ClaimTransfer,
    ) {
        bytes.id(transfer.claim);
        bytes.u32(transfer.argument_index);
        bytes.slice(&transfer.path, encode_structural_path_segment);
    }
    fn encode_record_initializer(
        bytes: &mut CanonicalBytes,
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
    fn encode_successor(
        bytes: &mut CanonicalBytes,
        successor: &abstract_operations::AbstractSuccessor,
    ) {
        bytes.id(successor.psi_edge);
        bytes.id(successor.target);
        bytes.slice(&successor.bindings, encode_binding);
        bytes.slice(&successor.structural_bindings, encode_structural_binding);
        bytes.slice(&successor.cleanup_actions, encode_cleanup);
    }
    fn encode_byte_sequence_operand(
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
    mod atomic {
        use crate::identity::carrier_encoding::{
            encode_abstract_result, encode_structural_path_segment,
        };
        use crate::identity::structural_encoding::encode_structural_operation_result;

        use super::CanonicalBytes;
        use abstract_operations::AbstractAtomicEvent;
        use abstract_operations::atomic::AbstractAtomicLocation;
        use abstract_operations::{
            AbstractAtomicFenceOrdering, AbstractAtomicReadModifyWrite, AtomicResidencyEdge,
            AtomicResidencySource,
        };
        use language_core::atomic::{
            AtomicCompareExchangeAttemptAxis, AtomicCompareExchangeObservationAxis,
            AtomicCompareExchangeOutcomeIdentity, MemoryOrdering,
        };

        pub(super) fn encode(bytes: &mut CanonicalBytes, event: &AbstractAtomicEvent) {
            use abstract_operations::AbstractAtomicEvent as E;
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
        }

        pub(super) fn encode_residency_edge(
            bytes: &mut CanonicalBytes,
            edge: &Option<AtomicResidencyEdge>,
        ) {
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
    field!(abstract_operations::AbstractAtomicEvent, |bytes, value| {
        atomic::encode(bytes, value)
    });
    field!(
        Option<abstract_operations::AtomicResidencyEdge>,
        |bytes, value| atomic::encode_residency_edge(bytes, value)
    );

    pub(super) fn encode_operation(bytes: &mut CanonicalBytes, operation: &AbstractOperation) {
        macro_rules! encode_fields {
            ($tag:expr $(, $operand:ident)*) => {{
                bytes.u8($tag);
                $($operand.encode(bytes);)*
            }};
        }
        abstract_operations::visit_abstract_operation_fields!(operation, encode_fields);
    }
}

#[cfg(test)]
mod width_identity_tests {
    use semantic_vocabulary::{IntegerSign, IntegerType, MachineId, ValueId, WidthParameter};

    fn identity(integer: IntegerType) -> Vec<u8> {
        let mut bytes = super::unit_encoding::CanonicalBytes::collect();
        super::carrier_encoding::encode_integer_type(&mut bytes, integer);
        bytes.finish()
    }

    fn range_identity(integer: IntegerType) -> Vec<u8> {
        let mut bytes = Vec::new();
        semantic_vocabulary::identity_encoding::encode_integer_type(&mut bytes, integer);
        bytes
    }

    #[test]
    fn width_identities_keep_fixed_bytes_and_distinguish_rigid_parameter_owners() {
        let fixed = IntegerType::new(IntegerSign::Signed, 32).unwrap();
        assert_eq!(identity(fixed), [2, 32, 0]);
        assert_ne!(
            identity(IntegerType::address(64).unwrap()),
            identity(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
        );
        assert_eq!(range_identity(fixed), [1, 1, 32, 0]);
        let parameter = |owner, value| {
            IntegerType::parameterized(
                IntegerSign::Signed,
                WidthParameter {
                    owner: MachineId::new(owner).unwrap(),
                    value: ValueId::new(value).unwrap(),
                },
            )
        };
        let first = parameter(1, 1);
        for other in [fixed, parameter(2, 1), parameter(1, 2)] {
            assert_ne!(identity(first), identity(other));
            assert_ne!(range_identity(first), range_identity(other));
        }
        assert!(semantic_vocabulary::ClosedIntegerType::try_from(first).is_err());
    }
}

#[cfg(test)]
mod float_meaning_identity_tests {
    use numerics::float_semantics::{
        FloatSemanticOperation,
        FloatSemanticValueKind::{Format, Meaning},
    };
    use semantic_vocabulary::{
        ClauseIdentity, FloatMeaningClauseValue, IeeeFloatFormat, Proposition, ScalarTerm,
        ScalarType, ValueId,
    };

    fn bytes(clause: ClauseIdentity) -> Vec<u8> {
        let mut bytes = super::unit_encoding::CanonicalBytes::collect();
        super::proposition_encoding::encode_proposition(
            &mut bytes,
            &Proposition::DeclarationIdentity(clause),
        );
        bytes.finish()
    }

    #[test]
    fn float_meaning_identity_keeps_sources_catalogs_and_ordered_operands() {
        let format = IeeeFloatFormat::Binary32;
        let value =
            |id| ScalarTerm::value(ValueId::new(id).unwrap(), ScalarType::IeeeFloat(format));
        let operation = FloatSemanticOperation::from_source_identity(
            "FloatSemantics",
            "subtract",
            &[Format, Meaning, Meaning],
            Meaning,
        )
        .unwrap();
        let clause = ClauseIdentity::float_result_equation(
            value(3),
            operation.contract_identity(),
            format,
            [value(1), value(2)],
        )
        .unwrap();
        let original = bytes(clause.clone());
        for position in 0..clause.values.len() {
            let mut changed = clause.clone();
            match changed.float_meaning_row_mut(position).unwrap() {
                FloatMeaningClauseValue::Projection { source, .. } => {
                    *source = value(30);
                    assert_ne!(original, bytes(changed.clone()));
                    changed = clause.clone();
                    let FloatMeaningClauseValue::Projection { contract, .. } =
                        changed.float_meaning_row_mut(position).unwrap()
                    else {
                        unreachable!()
                    };
                    contract.commitment[0] ^= 1;
                }
                FloatMeaningClauseValue::Application { contract, .. } => {
                    contract.commitment[0] ^= 1;
                    assert_ne!(original, bytes(changed.clone()));
                    changed = clause.clone();
                    let FloatMeaningClauseValue::Application { operands, .. } =
                        changed.float_meaning_row_mut(position).unwrap()
                    else {
                        unreachable!()
                    };
                    operands.swap(1, 2);
                }
            }
            assert_ne!(original, bytes(changed));
        }
        let mut changed = clause;
        std::mem::swap(&mut changed.left, &mut changed.right);
        assert_ne!(original, bytes(changed));
    }

    #[test]
    fn float_meaning_identity_keeps_integer_operands() {
        use numerics::{
            float_semantics::FloatSemanticValueKind::Integer, integer_carrier::FixedInteger,
        };
        let carrier = FixedInteger::signed(32);
        let integer =
            ScalarType::Integer(semantic_vocabulary::IntegerType::from_fixed(carrier).unwrap());
        let operation = FloatSemanticOperation::from_source_identity(
            "FloatSemantics",
            "from_integer",
            &[Format, Integer(carrier)],
            Meaning,
        )
        .unwrap();
        let clause = ClauseIdentity::float_result_equation(
            ScalarTerm::value(
                ValueId::new(2).unwrap(),
                ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
            ),
            operation.contract_identity(),
            IeeeFloatFormat::Binary64,
            [ScalarTerm::value(ValueId::new(1).unwrap(), integer)],
        )
        .unwrap();
        let changed = clause
            .map_float_sources::<&'static str>(|source| {
                Ok(if source.scalar_type() == integer {
                    ScalarTerm::value(ValueId::new(3).unwrap(), integer)
                } else {
                    source.clone()
                })
            })
            .unwrap();
        assert_ne!(bytes(clause), bytes(changed));
    }
}

#[cfg(test)]
mod declaration_identity_tests {
    use semantic_vocabulary::{
        ClauseCarrier, ClauseDeclaration, ClauseIdentity, ClauseOperand, ClauseValue, IntegerSign,
        IntegerType, Proposition, ScalarTerm, ScalarType, ValueId,
    };

    fn bytes(clause: &ClauseIdentity) -> Vec<u8> {
        let mut bytes = super::unit_encoding::CanonicalBytes::collect();
        super::proposition_encoding::encode_proposition(
            &mut bytes,
            &Proposition::DeclarationIdentity(clause.clone()),
        );
        bytes.finish()
    }

    #[test]
    fn declaration_identity_binds_its_values_declarations_and_sides() {
        let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        let value = |id| ClauseValue {
            carrier: ClauseCarrier::Scalar(scalar),
            operand: ClauseOperand::Scalar(ScalarTerm::value(ValueId::new(id).unwrap(), scalar)),
        };
        let apply = |arguments: Vec<u32>| ClauseValue {
            carrier: ClauseCarrier::Scalar(scalar),
            operand: ClauseOperand::Application {
                declaration: ClauseDeclaration {
                    supplier: Some((1, 2)),
                    member: (3, 4),
                },
                arguments,
            },
        };
        let clause = ClauseIdentity {
            values: vec![value(1), value(2), apply(vec![0, 1]), apply(vec![1, 0])],
            left: 2,
            right: 3,
        };
        clause.validate().unwrap();
        let original = bytes(&clause);
        assert_eq!(original, bytes(&clause));

        let mut changed = clause.clone();
        changed.values[0] = value(5);
        assert_ne!(original, bytes(&changed));

        let mut changed = clause.clone();
        let ClauseOperand::Application { declaration, .. } = &mut changed.values[2].operand else {
            unreachable!()
        };
        declaration.supplier = None;
        assert_ne!(original, bytes(&changed));

        let mut changed = clause.clone();
        let ClauseOperand::Application { declaration, .. } = &mut changed.values[2].operand else {
            unreachable!()
        };
        declaration.member.1 = 5;
        assert_ne!(original, bytes(&changed));

        let mut changed = clause.clone();
        changed.values[0].operand = ClauseOperand::Formal(0);
        assert_ne!(original, bytes(&changed));

        let mut changed = clause;
        std::mem::swap(&mut changed.left, &mut changed.right);
        assert_ne!(original, bytes(&changed));
    }
}

#[cfg(test)]
mod unit_identity_tests {
    use crate::{
        PsiOptimizationUnit, RuntimeEvidenceCatalog, recompute_psi_optimization_unit_identity,
    };
    use optimization_core::OptimizationUnitIdentity;
    use semantic_vocabulary::{BoundaryMachineId, MachineId, WorkScheduleIdentity};
    use std::sync::Arc;
    use terminal_psi::{
        BoundaryCompletion, BoundaryMachineDeclaration, BoundaryMachineResult,
        BoundaryRequirementKind, NominalDeclarationIdentity, SemanticFingerprint,
        TerminalIdentityIndex, TerminalModule, TerminalPsiIdentity, VocabularyMarker,
    };

    fn psi(byte: u8) -> TerminalPsiIdentity {
        TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([byte; 32]),
        }
    }

    fn boundary(id: u64) -> BoundaryMachineDeclaration {
        BoundaryMachineDeclaration {
            id: BoundaryMachineId::new(id).unwrap(),
            declaration: None,
            role: terminal_psi::BoundaryDeclarationRole::Callable,
            kind: BoundaryRequirementKind::Machine,
            completion: BoundaryCompletion::NormalReturn,
            numeric_intrinsic: None,
            unconditional_termination: false,
            identity: TerminalIdentityIndex(1),
            owner: NominalDeclarationIdentity::new(TerminalIdentityIndex(2)),
            attachment: None,
            parameter_order: Vec::new(),
            scalar_parameters: Vec::new(),
            crash_routes: Vec::new(),
            structural_parameters: Vec::new(),
            result: BoundaryMachineResult::Unit,
            requires: Vec::new(),
            membership_requires: Vec::new(),
            membership_ensures: Vec::new(),
            normal_qualifications: Vec::new(),
            scalar_domain_requirements: Vec::new(),
            scalar_requires: Vec::new(),
            scalar_ensures: Vec::new(),
            result_reference_regions: Vec::new(),
            result_lifetimes: None,
            reference_outputs: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            fixed_service_reach: Vec::new(),
            service_reach_is_installation_bound: false,
            owning_service: None,
            published_synchronous_entries: Vec::new(),
            published_service_ceiling: Vec::new(),
        }
    }

    fn unit() -> PsiOptimizationUnit {
        PsiOptimizationUnit {
            identity: OptimizationUnitIdentity::from_canonical_bytes(b"uncomputed test seed"),
            psi: psi(7),
            work_schedule: WorkScheduleIdentity::new(1).unwrap(),
            scope_provider_closure: None,
            lowering_target: None,
            scalar_values_digest: None,
            entry: MachineId::new(1).unwrap(),
            structural_types: Vec::new().into(),
            structural_domains: Arc::new([]),
            services: Arc::new([]),
            root_service_reach: Default::default(),
            boundary_machines: vec![boundary(1)],
            provider_candidates: Vec::new(),
            dynamic_dispatch: Default::default(),
            callables: Default::default(),
            accepted_obligation_facts: Vec::new(),
            identities: Default::default(),
            runtime_evidence: None,
            functions: crate::OptimizationFunctions {
                rows: Vec::new(),
                digest: crate::identity::unit_encoding::function_digest(&[]),
            },
        }
    }

    #[test]
    fn unit_identity_commits_the_terminal_program_fingerprint() {
        let original = recompute_psi_optimization_unit_identity(&unit());
        assert_eq!(original, recompute_psi_optimization_unit_identity(&unit()));
        let mut changed = unit();
        changed.psi = psi(8);
        assert_ne!(original, recompute_psi_optimization_unit_identity(&changed));
    }

    #[test]
    fn unit_identity_commits_the_kept_boundary_declarations() {
        let original = recompute_psi_optimization_unit_identity(&unit());
        let mut renamed = unit();
        renamed.boundary_machines[0] = boundary(2);
        assert_ne!(original, recompute_psi_optimization_unit_identity(&renamed));
        let mut widened = unit();
        widened.boundary_machines.push(boundary(2));
        assert_ne!(original, recompute_psi_optimization_unit_identity(&widened));
        let mut dropped = unit();
        dropped.boundary_machines.clear();
        assert_ne!(original, recompute_psi_optimization_unit_identity(&dropped));
    }

    #[test]
    fn independent_plans_bind_operations_even_when_graph_provenance_and_uses_agree() {
        use abstract_operations::{
            AbstractBlock, AbstractFunction, AbstractFunctionResult, AbstractOperation as Op,
            AbstractOperationPlan,
        };
        use semantic_vocabulary::{BlockId, OperationId, ValueId};

        let id = ValueId::new(1).unwrap();
        let second = ValueId::new(2).unwrap();
        let site = OperationId::new(1).unwrap();
        let machine = MachineId::new(1).unwrap();
        let block = BlockId::new(1).unwrap();
        let functions = |operation| {
            vec![AbstractFunction {
                machine,
                attachment: None,
                entry: block,
                parameters: vec![],
                structural_parameters: vec![],
                result: AbstractFunctionResult::Unit,
                entry_claims: vec![],
                block_entry_claims: vec![],
                published_service_ceiling: vec![],
                blocks: vec![AbstractBlock {
                    block,
                    parameters: vec![],
                    structural_parameters: vec![],
                    operations: vec![operation],
                }],
            }]
        };
        let mut plan = AbstractOperationPlan {
            psi: psi(7),
            entry: machine,
            structural_types: vec![].into(),
            boundary_machines: vec![],
            provider_candidates: vec![],
            callables: Default::default(),
            dynamic_dispatch: Default::default(),
            identities: Default::default(),
            functions: functions(Op::IntegerLessThan {
                psi_operation: site,
                result: id,
                left: id,
                right: second,
            }),
        };
        let identity = |plan: &AbstractOperationPlan| {
            crate::reconstruct_psi_optimization_unit_seed(
                plan,
                WorkScheduleIdentity::new(1).unwrap(),
            )
            .unwrap()
            .identity
        };
        let original = identity(&plan);
        assert_eq!(original, identity(&plan.clone()));
        plan.functions = functions(Op::IntegerLessOrEqual {
            psi_operation: site,
            result: id,
            left: id,
            right: second,
        });
        assert_ne!(original, identity(&plan));
        plan.functions = functions(Op::IntegerLessThan {
            psi_operation: site,
            result: id,
            left: second,
            right: id,
        });
        assert_ne!(original, identity(&plan));
        plan.functions.clear();
        assert_ne!(original, identity(&plan));
    }

    #[test]
    fn unit_identity_binds_schedule_entry_scope_geometry_providers_and_facts() {
        use semantic_vocabulary::{ObligationId, OperationId, StructuralTypeId};
        let original = recompute_psi_optimization_unit_identity(&unit());
        let changed = |edit: fn(&mut PsiOptimizationUnit)| {
            let mut seed = unit();
            edit(&mut seed);
            assert_ne!(original, recompute_psi_optimization_unit_identity(&seed));
        };
        changed(|seed| seed.work_schedule = WorkScheduleIdentity::new(2).unwrap());
        changed(|seed| seed.entry = MachineId::new(2).unwrap());
        changed(|seed| seed.scope_provider_closure = Some([0; 32]));
        let mut scoped = unit();
        scoped.scope_provider_closure = Some([1; 32]);
        let first_scope = recompute_psi_optimization_unit_identity(&scoped);
        scoped.scope_provider_closure = Some([2; 32]);
        assert_ne!(
            first_scope,
            recompute_psi_optimization_unit_identity(&scoped)
        );
        changed(|seed| {
            seed.structural_types = vec![terminal_psi::StructuralTypeDeclaration {
                id: StructuralTypeId::new(1).unwrap(),
                declaration_identity: Default::default(),
                identity: TerminalIdentityIndex(1),
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: false,
                descriptor_cleanup: false,
                multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                nominal_cleanup: None,
                shape: terminal_psi::StructuralTypeShape::Record {
                    fields: vec![],
                    storage: terminal_psi::StructuralRecordStorage::Authored(
                        semantic_vocabulary::AuthoredRecordStorage {
                            byte_size: 16,
                            byte_alignment: 8,
                        },
                    ),
                },
                requires: vec![],
                scalar_restriction: None,
            }]
            .into()
        });
        changed(|seed| {
            seed.structural_types = seed
                .structural_types
                .clone()
                .with_owned_sequence_bounds(vec![(StructuralTypeId::new(1).unwrap(), 16)])
        });
        changed(|seed| {
            seed.provider_candidates
                .push(terminal_psi::ProviderCandidateConformance {
                    boundary: seed.boundary_machines[0].id,
                    requirement_identity: TerminalIdentityIndex(1),
                    source_requirement_identity: TerminalIdentityIndex(1),
                    declaration_application: Default::default(),
                    receiving_application: Default::default(),
                    receiving_region_arguments: vec![],
                    provider_identity: TerminalIdentityIndex(2),
                    provider_declaration: NominalDeclarationIdentity::new(TerminalIdentityIndex(2)),
                    candidate_identity: TerminalIdentityIndex(3),
                    source_candidate_identity: TerminalIdentityIndex(3),
                    source_candidate_package_identity: None,
                    candidate: seed.entry,
                    signature: terminal_psi::ProviderSignature::empty(),
                    refinement: terminal_psi::ProviderRefinement::empty(),
                })
        });
        changed(|seed| {
            seed.accepted_obligation_facts
                .push(crate::AcceptedObligationFact::new(
                    seed.psi,
                    [1; 32],
                    seed.entry,
                    OperationId::new(1).unwrap(),
                    ObligationId::new(1).unwrap(),
                    vec![5],
                ))
        });
    }

    #[test]
    fn unit_identity_commits_module_context_attachment() {
        let seed = unit();
        let mut attached = unit();
        attached.runtime_evidence = Some(Arc::new(RuntimeEvidenceCatalog::from_verified_module(
            attached.psi,
            &TerminalModule::for_entry(attached.entry),
        )));
        assert_ne!(
            recompute_psi_optimization_unit_identity(&seed),
            recompute_psi_optimization_unit_identity(&attached)
        );
    }
}
