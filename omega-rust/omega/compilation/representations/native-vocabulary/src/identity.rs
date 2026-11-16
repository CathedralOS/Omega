//! Canonical identities for native leaf records shared by native program forms.
//!
//! These encoders carry no program container. Each native form uses the same
//! bytes for scalar meaning and access catalogs without reading an earlier graph.

use semantic_vocabulary::{
    ByteSequenceStructuralField, CanonicalStructuralPathSegment, ContentAlgebra,
    ContentAlgebraKind, ContentPlaceSegment, ContentPlaceVersion, ContentStructuralPlace,
    ContentTerm, IeeeFloatComparisonKind, IeeeFloatFormat, IeeeFloatStructuralField,
    IntegerCarrier, IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, Proposition,
    PsiSemanticId, ScalarTerm, ScalarType, StructuralCaseSubject,
};

pub enum CanonicalBytes {
    Length(usize),
    Digest(optimization_core::CanonicalIdentityEncoder),
    Collect(Vec<u8>),
}

impl CanonicalBytes {
    pub fn length() -> Self {
        Self::Length(0)
    }

    pub fn digest(encoder: optimization_core::CanonicalIdentityEncoder) -> Self {
        Self::Digest(encoder)
    }

    pub fn collect() -> Self {
        Self::Collect(Vec::new())
    }

    pub fn finish(self) -> Vec<u8> {
        match self {
            Self::Collect(bytes) => bytes,
            _ => unreachable!("collect target precedes byte inspection"),
        }
    }

    pub fn finish_length(self) -> u64 {
        match self {
            Self::Length(length) => {
                u64::try_from(length).expect("canonical optimization-unit length fits u64")
            }
            Self::Digest(_) => unreachable!("digest pass follows the completed length pass"),
            Self::Collect(_) => unreachable!("length target precedes length inspection"),
        }
    }

    pub fn finish_digest(self) -> optimization_core::CanonicalIdentityEncoder {
        match self {
            Self::Digest(encoder) => encoder,
            Self::Length(_) => unreachable!("length pass precedes the digest pass"),
            Self::Collect(_) => unreachable!("digest target precedes digest inspection"),
        }
    }

    pub fn bytes(&mut self, bytes: &[u8]) {
        match self {
            Self::Length(length) => *length += bytes.len(),
            Self::Digest(encoder) => encoder.update(bytes),
            Self::Collect(buffer) => buffer.extend_from_slice(bytes),
        }
    }

    pub fn u8(&mut self, value: u8) {
        self.bytes(&[value]);
    }

    pub fn boolean(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    pub fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }

    pub fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    pub fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    pub fn u128(&mut self, value: u128) {
        self.bytes(&value.to_le_bytes());
    }

    pub fn id(&mut self, value: impl PsiSemanticId) {
        self.u64(value.get());
    }

    pub fn len(&mut self, len: usize) {
        self.u64(u64::try_from(len).expect("canonical optimization-unit length fits u64"));
    }

    pub fn string(&mut self, value: &str) {
        self.len(value.len());
        self.bytes(value.as_bytes());
    }

    pub fn slice<T>(&mut self, values: &[T], encode: impl Fn(&mut Self, &T)) {
        self.len(values.len());
        for value in values {
            encode(self, value);
        }
    }
}

pub fn encode_scalar_type<B>(
    bytes: &mut B,
    scalar_type: ScalarType,
    push_u8: impl Fn(&mut B, u8),
    encode_integer: impl FnOnce(&mut B, IntegerType),
) {
    match scalar_type {
        ScalarType::Boolean => push_u8(bytes, 1),
        ScalarType::Integer(integer) => {
            push_u8(bytes, 2);
            encode_integer(bytes, integer);
        }
        ScalarType::IeeeFloat(format) => {
            push_u8(bytes, 3);
            push_u8(
                bytes,
                match format {
                    semantic_vocabulary::IeeeFloatFormat::Binary32 => 1,
                    semantic_vocabulary::IeeeFloatFormat::Binary64 => 2,
                },
            );
        }
    }
}

pub fn encode_integer_carrier(canonical: &mut Vec<u8>, scalar_type: IntegerType) {
    canonical.push(match scalar_type.carrier() {
        IntegerCarrier::Fixed => 1,
        IntegerCarrier::Address => 2,
    });
    canonical.push(match scalar_type.sign() {
        IntegerSign::Signed => 1,
        IntegerSign::Unsigned => 2,
    });
    // Rewrite/range fingerprints describe raw metadata as well as closed facts.
    // Identity never grants a parameter the concrete interval needed for a fact.
    scalar_type
        .width()
        .write_identity(|part| canonical.extend_from_slice(part));
}

pub mod carrier_encoding {
    pub fn encode_access(bytes: &mut CanonicalBytes, access: StructuralAccess) {
        bytes.u8(match access {
            StructuralAccess::Owned => 1,
            StructuralAccess::SharedBorrow => 2,
            StructuralAccess::MutableBorrow => 3,
            StructuralAccess::WriteOnlyBorrow => 4,
        });
    }

    use super::{
        CanonicalBytes, IntegerSign, IntegerType, IntegerValue, PsiSemanticId, ScalarType,
    };
    use terminal_psi::StructuralAccess;
    pub fn encode_optional_id<T: PsiSemanticId>(bytes: &mut CanonicalBytes, value: Option<&T>) {
        bytes.boolean(value.is_some());
        if let Some(value) = value {
            bytes.id(*value);
        }
    }

    pub fn encode_optional<T>(
        bytes: &mut CanonicalBytes,
        value: Option<&T>,
        encode: impl Fn(&mut CanonicalBytes, &T),
    ) {
        bytes.boolean(value.is_some());
        if let Some(value) = value {
            encode(bytes, value);
        }
    }

    pub fn encode_ids<T: PsiSemanticId>(bytes: &mut CanonicalBytes, ids: &[T]) {
        bytes.len(ids.len());
        for id in ids {
            bytes.id(*id);
        }
    }

    pub fn encode_scalar_type(bytes: &mut CanonicalBytes, scalar_type: ScalarType) {
        super::encode_scalar_type(
            bytes,
            scalar_type,
            |bytes, tag| bytes.u8(tag),
            encode_integer_type,
        )
    }

    pub fn encode_integer_type(bytes: &mut CanonicalBytes, integer_type: IntegerType) {
        bytes.u8(match integer_type.sign() {
            IntegerSign::Unsigned => 1,
            IntegerSign::Signed => 2,
        });
        // This fingerprints raw refinement/receiving metadata; it does not
        // establish executable closure. Keep scoped widths distinct while the
        // published native owner remains responsible for concrete carriers.
        integer_type
            .width()
            .write_identity(|part| bytes.bytes(part));
    }

    pub fn encode_integer_value(bytes: &mut CanonicalBytes, value: IntegerValue) {
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
}

pub mod proposition_encoding {
    use super::carrier_encoding::{
        encode_integer_type, encode_integer_value, encode_optional, encode_scalar_type,
    };
    use super::content_encoding::{encode_content_algebra, encode_content_place};
    use super::{
        ByteSequenceStructuralField, CanonicalBytes, CanonicalStructuralPathSegment, ContentTerm,
        IeeeFloatComparisonKind, IeeeFloatFormat, IeeeFloatStructuralField, IntegerMathTerm,
        IntegerType, Proposition, ScalarTerm, StructuralCaseSubject,
    };
    pub fn encode_proposition(bytes: &mut CanonicalBytes, proposition: &Proposition) {
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
            Proposition::FloatMeaningEqual(clause) => {
                use semantic_vocabulary::{
                    FloatMeaningClauseOperand as Operand, FloatMeaningClauseValue as Value,
                };
                bytes.u8(19);
                bytes.slice(&clause.values, |bytes, value| match value {
                    Value::Projection { source, contract } => {
                        bytes.u8(0);
                        encode_scalar_term(bytes, source);
                        bytes.u16(contract.format);
                        bytes.u8(contract.operation);
                        bytes.u8(contract.declaration);
                        bytes.u16(contract.catalog_version);
                        bytes.bytes(&contract.commitment);
                    }
                    Value::Application {
                        contract,
                        format,
                        operands,
                    } => {
                        bytes.u8(1);
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
                });
                bytes.u32(clause.left.0);
                bytes.u32(clause.right.0);
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

    pub fn encode_content_term(bytes: &mut CanonicalBytes, term: &ContentTerm) {
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

    pub fn encode_scalar_term(bytes: &mut CanonicalBytes, term: &ScalarTerm) {
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

    pub fn encode_scalar_unary(
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
    pub fn encode_scalar_binary(
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
    pub fn encode_scalar_cast(
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
    pub fn encode_scalar_shift(
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

    pub fn encode_canonical_path(
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
    pub fn encode_float_format(bytes: &mut CanonicalBytes, value: IeeeFloatFormat) {
        bytes.u8(match value {
            IeeeFloatFormat::Binary32 => 1,
            IeeeFloatFormat::Binary64 => 2,
        });
    }
    pub fn encode_float_comparison(bytes: &mut CanonicalBytes, value: IeeeFloatComparisonKind) {
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
            pub fn $name(bytes: &mut CanonicalBytes, value: &$t) {
                bytes.id(value.root());
                encode_canonical_path(bytes, value.path());
            }
        };
    }

    encode_rooted_field!(encode_float_field, IeeeFloatStructuralField);
    encode_rooted_field!(encode_byte_field, ByteSequenceStructuralField);
    encode_rooted_field!(encode_case_subject, StructuralCaseSubject);
}

pub mod content_encoding {
    use super::{
        CanonicalBytes, ContentAlgebra, ContentAlgebraKind, ContentPlaceSegment,
        ContentPlaceVersion, ContentStructuralPlace,
    };
    pub fn encode_content_algebra(bytes: &mut CanonicalBytes, algebra: &ContentAlgebra) {
        bytes.u8(match algebra.kind {
            ContentAlgebraKind::IntervalSet => 1,
            ContentAlgebraKind::CountedQuantity => 2,
        });
        bytes.string(&algebra.parameter);
    }

    pub fn encode_content_place(bytes: &mut CanonicalBytes, place: &ContentStructuralPlace) {
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
}
pub fn scalar_refinement_identity_bytes(
    refinement: &semantic_vocabulary::ScalarRefinement,
) -> Vec<u8> {
    let mut bytes = CanonicalBytes::collect();
    carrier_encoding::encode_scalar_type(&mut bytes, refinement.carrier());
    proposition_encoding::encode_proposition(&mut bytes, refinement.predicate());
    bytes.finish()
}

pub mod access_view_encoding {
    use super::CanonicalBytes;
    use super::carrier_encoding::encode_access;
    pub fn encode_access_view_plans(
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

    pub fn encode_access_view_catalog(
        bytes: &mut CanonicalBytes,
        catalog: &crate::structural_type_catalog::StructuralTypeCatalog,
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
        catalog: &crate::structural_type_catalog::StructuralTypeCatalog,
    ) -> Vec<u8> {
        let mut bytes = CanonicalBytes::collect();
        encode_access_view_catalog(&mut bytes, catalog);
        bytes.finish()
    }
}
pub use access_view_encoding::{
    access_view_catalog_identity_bytes, encode_access_view_catalog, encode_access_view_plans,
};
