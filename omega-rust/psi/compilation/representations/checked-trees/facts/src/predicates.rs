/*
We represent canonical boolean predicates, integer math terms, and byte-sequence
comparison operands (`predicates`) stored in `FactPlan`.
*/

use crate::IntegerRange as CheckedIntegerRange;
use symbols::SymbolHandle;
use typed_trees::type_identity::TypeIdentityId;

pub(crate) mod bindings {
    use crate::{
        CheckedBooleanExpression, CheckedScalarExpression, CheckedStructuralCaseSubject, Place,
    };
    use arena::{Handle, HandleSpan};
    use symbols::SymbolHandle;
    use typed_trees::types::TypeReferenceHandle;

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct BooleanPredicate {
        pub role: BooleanPredicateRole,
        pub machine: SymbolHandle,
        pub entry_state: SymbolHandle,
        pub expression: CheckedBooleanExpression,
        pub bindings: HandleSpan<BooleanPredicateBinding>,
    }

    impl Default for BooleanPredicate {
        fn default() -> Self {
            Self {
                role: BooleanPredicateRole::CurrentObservation,
                machine: SymbolHandle::invalid(),
                entry_state: SymbolHandle::invalid(),
                expression: CheckedBooleanExpression::Constant(false),
                bindings: HandleSpan::empty(),
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum BooleanPredicateOperand {
        Scalar(CheckedScalarExpression),
        CaseSubject(CheckedStructuralCaseSubject),
    }

    impl BooleanPredicateOperand {
        pub fn scalar(&self) -> Option<&CheckedScalarExpression> {
            match self {
                Self::Scalar(value) => Some(value),
                Self::CaseSubject(_) => None,
            }
        }

        pub fn primitive_type(&self) -> Option<typed_trees::types::PrimitiveType> {
            self.scalar()?.primitive_type()
        }
    }

    impl From<CheckedScalarExpression> for BooleanPredicateOperand {
        fn from(value: CheckedScalarExpression) -> Self {
            Self::Scalar(value)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct BooleanPredicateBinding {
        pub operand: BooleanPredicateOperand,
        pub place: Handle<Place>,
        pub type_reference: TypeReferenceHandle,
    }

    impl Default for BooleanPredicateBinding {
        fn default() -> Self {
            Self {
                operand: BooleanPredicateOperand::Scalar(CheckedScalarExpression::Boolean(
                    Box::new(CheckedBooleanExpression::Constant(false)),
                )),
                place: Handle::invalid(),
                type_reference: TypeReferenceHandle::invalid(),
            }
        }
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum BooleanPredicateRole {
        #[default]
        CurrentObservation,
        EntryPathCondition,
        UnknownEntryPathCondition,
    }
}
pub use bindings::{
    BooleanPredicate, BooleanPredicateBinding, BooleanPredicateOperand, BooleanPredicateRole,
};

pub(crate) mod byte_comparison {
    use super::{CheckedBooleanExpression, CheckedScalarExpression, CheckedStorageRoot};
    use crate::{CheckedIntegerComparisonKind, CheckedStructuralPredicatePathSegment};
    use typed_trees::types::PrimitiveType;
    impl CheckedBooleanExpression {
        pub fn byte_literal_equality_in_extent(
            root: CheckedStorageRoot,
            path: Vec<CheckedStructuralPredicatePathSegment>,
            bytes: &[u8],
            maximum_length: Option<u64>,
        ) -> Option<Self> {
            let mut equality = CheckedBooleanExpression::IntegerComparison {
                kind: CheckedIntegerComparisonKind::Equal,
                left: Box::new(CheckedScalarExpression::StructuralSequenceLength {
                    root,
                    path: path
                        .iter()
                        .cloned()
                        .map(super::CheckedScalarReadProjection::Static)
                        .collect(),
                }),
                right: Box::new(literal(
                    u64::try_from(bytes.len()).ok()?,
                    PrimitiveType::U64,
                )?),
            };
            for (index, byte) in bytes.iter().enumerate().take(
                maximum_length
                    .and_then(|length| usize::try_from(length).ok())
                    .unwrap_or(bytes.len()),
            ) {
                let read = CheckedScalarExpression::StructuralParameterIndexedRead {
                    observation: typed_trees::expression::ExpressionHandle::invalid(),
                    root,
                    path: path.clone(),
                    index: Box::new(super::CheckedScalarSelector {
                        expression: literal(u64::try_from(index).ok()?, PrimitiveType::U64)?,
                        occurrence: typed_trees::expression::ExpressionHandle::invalid(),
                        entry_bound: None,
                    }),
                    element_path: Vec::new(),
                    primitive_type: PrimitiveType::U8,
                };
                let comparison = CheckedBooleanExpression::IntegerComparison {
                    kind: CheckedIntegerComparisonKind::Equal,
                    left: Box::new(read),
                    right: Box::new(literal(u64::from(*byte), PrimitiveType::U8)?),
                };
                equality = CheckedBooleanExpression::And {
                    left: Box::new(equality),
                    right: Box::new(comparison),
                };
            }
            Some(equality)
        }
    }
    fn literal(value: u64, primitive_type: PrimitiveType) -> Option<CheckedScalarExpression> {
        super::u64_integer_literal(value, primitive_type)
    }
}
pub(crate) mod integer_math {
    use super::CheckedScalarExpression;
    use arena::{Arena, Handle};
    use semantic_vocabulary::{IntegerMathLiteral, IntegerValue};
    use typed_trees::types::PrimitiveType;

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedIntegerMathExpression {
        pub terms: Arena<CheckedIntegerMathTerm>,
        pub root: Handle<CheckedIntegerMathTerm>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedIntegerMathTerm {
        Embed {
            primitive_type: PrimitiveType,
            operand: CheckedScalarExpression,
        },
        Literal(IntegerMathLiteral),
        BuiltinTypeMember(symbols::BuiltinTypeMember),
        Add {
            left: Handle<Self>,
            right: Handle<Self>,
        },
        Subtract {
            left: Handle<Self>,
            right: Handle<Self>,
        },
        Multiply {
            left: Handle<Self>,
            right: Handle<Self>,
        },
    }

    impl Default for CheckedIntegerMathTerm {
        fn default() -> Self {
            Self::Literal(IntegerMathLiteral::from_integer_value(
                IntegerValue::Unsigned(0),
            ))
        }
    }

    impl CheckedIntegerMathExpression {
        pub(crate) fn contains_trapping_operation_with_certificates(
            &self,
            use_certificates: bool,
        ) -> bool {
            let mut pending = vec![self.root];
            let mut remaining = 4096usize;
            while let Some(handle) = pending.pop() {
                if remaining == 0 || !self.terms.is_valid(handle) {
                    return true;
                }
                remaining -= 1;
                match self.terms.get(handle) {
                    CheckedIntegerMathTerm::Embed { operand, .. } => {
                        if operand.contains_trapping_operation_with_certificates(use_certificates) {
                            return true;
                        }
                    }
                    CheckedIntegerMathTerm::Literal(_)
                    | CheckedIntegerMathTerm::BuiltinTypeMember(_) => {}
                    CheckedIntegerMathTerm::Add { left, right }
                    | CheckedIntegerMathTerm::Subtract { left, right }
                    | CheckedIntegerMathTerm::Multiply { left, right } => {
                        pending.push(*right);
                        pending.push(*left);
                    }
                }
            }
            false
        }
    }
}
pub use integer_math::{CheckedIntegerMathExpression, CheckedIntegerMathTerm};

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum CheckedStorageRoot {
    Parameter { index: u32 },
    ViewLocal { symbol: SymbolHandle },
    OwnedLocal { symbol: SymbolHandle },
}

impl CheckedStorageRoot {
    pub fn parameter(self) -> Option<u32> {
        match self {
            Self::Parameter { index } => Some(index),
            Self::ViewLocal { .. } | Self::OwnedLocal { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub enum CheckedEntryIndexSelector {
    Expression(typed_trees::expression::ExpressionHandle),
    Constant(u64),
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedEntryIndexBound {
    pub state: SymbolHandle,
    pub parameter: SymbolHandle,
    pub path_step: u32,
    pub selector: CheckedEntryIndexSelector,
    pub collection_path: Vec<crate::PlaceSegment>,
    pub collection_type: typed_trees::types::TypeReferenceHandle,
    pub element_type: typed_trees::types::TypeReferenceHandle,
    pub extent: Option<u64>,
    pub primitive_type: typed_trees::types::PrimitiveType,
}

impl Default for CheckedEntryIndexBound {
    fn default() -> Self {
        Self {
            state: SymbolHandle::invalid(),
            parameter: SymbolHandle::invalid(),
            path_step: 0,
            selector: CheckedEntryIndexSelector::Expression(
                typed_trees::expression::ExpressionHandle::invalid(),
            ),
            collection_path: Vec::new(),
            collection_type: typed_trees::types::TypeReferenceHandle::invalid(),
            element_type: typed_trees::types::TypeReferenceHandle::invalid(),
            extent: Some(0),
            primitive_type: typed_trees::types::PrimitiveType::U64,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedScalarSelector {
    pub expression: CheckedScalarExpression,
    pub occurrence: typed_trees::expression::ExpressionHandle,
    pub entry_bound: Option<CheckedEntryIndexBound>,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub enum CheckedScalarReadProjection {
    Static(CheckedStructuralPredicatePathSegment),
    RuntimeIndex(CheckedScalarSelector),
}

impl CheckedScalarReadProjection {
    pub fn static_path(path: &[Self]) -> Option<Vec<CheckedStructuralPredicatePathSegment>> {
        path.iter()
            .map(|step| match step {
                Self::Static(step) => Some(step.clone()),
                Self::RuntimeIndex(CheckedScalarSelector {
                    expression: CheckedScalarExpression::IntegerLiteral { literal },
                    ..
                }) => Some(CheckedStructuralPredicatePathSegment::FixedIndex(
                    literal.value_u64()?,
                )),
                Self::RuntimeIndex(_) => None,
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub enum CheckedScalarExpression {
    ConstantApplication {
        recipe: typed_trees::constant_recipes::ConstantRecipeHandle,
        owner: symbols::SymbolHandle,
        primitive_type: typed_trees::types::PrimitiveType,
    },
    StructuralSequenceLength {
        root: CheckedStorageRoot,
        path: Vec<CheckedScalarReadProjection>,
    },
    StorageRead {
        symbol: SymbolHandle,
        primitive_type: typed_trees::types::PrimitiveType,
    },
    Parameter {
        position: usize,
        primitive_type: typed_trees::types::PrimitiveType,
    },
    ErasedParameter {
        position: usize,
        primitive_type: typed_trees::types::PrimitiveType,
    },
    Local {
        position: usize,
        primitive_type: typed_trees::types::PrimitiveType,
    },
    StructuralParameterField {
        observation: typed_trees::expression::ExpressionHandle,
        parameter_position: u32,
        path: Vec<CheckedStructuralPredicatePathSegment>,
        primitive_type: typed_trees::types::PrimitiveType,
    },
    /// A scalar field of an immutable structural local, named as a proof-side
    /// observation of that local's place. Only an erased call actual forms
    /// one; it is never an executable read.
    StructuralLocalField {
        symbol: SymbolHandle,
        path: Vec<CheckedStructuralPredicatePathSegment>,
        primitive_type: typed_trees::types::PrimitiveType,
    },
    StructuralParameterIndexedRead {
        observation: typed_trees::expression::ExpressionHandle,
        root: CheckedStorageRoot,
        path: Vec<CheckedStructuralPredicatePathSegment>,
        index: Box<CheckedScalarSelector>,
        element_path: Vec<CheckedScalarReadProjection>,
        primitive_type: typed_trees::types::PrimitiveType,
    },
    IntegerLiteral {
        literal: numerics::literals::IntegerLiteral,
    },
    IeeeFloatLiteral {
        value: semantic_vocabulary::IeeeFloatValue,
    },
    ImplementationSelection {
        implementation: symbols::SymbolHandle,
        interface_identity: TypeIdentityId,
    },
    IntegerBinary {
        kind: CheckedIntegerBinaryKind,
        primitive_type: typed_trees::types::PrimitiveType,
        left: Box<CheckedScalarExpression>,
        right: Box<CheckedScalarExpression>,
        formation_decided: bool,
        trap_free: bool,
    },
    IntegerBitwiseNot {
        primitive_type: typed_trees::types::PrimitiveType,
        operand: Box<CheckedScalarExpression>,
    },
    IntegerWiden {
        primitive_type: typed_trees::types::PrimitiveType,
        operand: Box<CheckedScalarExpression>,
    },
    IntegerExactCast {
        primitive_type: typed_trees::types::PrimitiveType,
        operand: Box<CheckedScalarExpression>,
        range: CheckedIntegerRange,
        formation_decided: bool,
    },
    Boolean(Box<CheckedBooleanExpression>),
    RepresentationView {
        primitive_type: typed_trees::types::PrimitiveType,
        operand: Box<CheckedScalarExpression>,
    },
}

pub fn u64_integer_literal(
    value: u64,
    primitive_type: typed_trees::types::PrimitiveType,
) -> Option<CheckedScalarExpression> {
    let landed_type = match primitive_type {
        typed_trees::types::PrimitiveType::U8 => numerics::literals::LandedIntegerType::U8,
        typed_trees::types::PrimitiveType::U16 => numerics::literals::LandedIntegerType::U16,
        typed_trees::types::PrimitiveType::U32 => numerics::literals::LandedIntegerType::U32,
        typed_trees::types::PrimitiveType::U64 => numerics::literals::LandedIntegerType::U64,
        _ => return None,
    };
    let literal = numerics::literals::IntegerLiteral::from_parts(
        false,
        numerics::literals::IntegerRadix::Decimal,
        &value.to_string(),
    )
    .ok()?
    .with_landing(numerics::literals::IntegerLanding {
        landed_type,
        domain: numerics::arithmetic::ArithmeticDomain::Exact,
    });
    Some(CheckedScalarExpression::IntegerLiteral { literal })
}

impl CheckedScalarExpression {
    pub fn parameter_observation_projection(&self) -> Option<CheckedStructuralParameterField> {
        let (parameter_position, path) = match self {
            Self::StructuralParameterField {
                parameter_position,
                path,
                ..
            } => (*parameter_position, path.clone()),
            Self::StructuralParameterIndexedRead {
                root,
                path,
                index,
                element_path,
                ..
            } => {
                let mut projections = path
                    .iter()
                    .cloned()
                    .map(CheckedScalarReadProjection::Static)
                    .collect::<Vec<_>>();
                projections.push(CheckedScalarReadProjection::RuntimeIndex(
                    index.as_ref().clone(),
                ));
                projections.extend_from_slice(element_path);
                (
                    root.parameter()?,
                    CheckedScalarReadProjection::static_path(&projections)?,
                )
            }
            _ => return None,
        };
        Some(CheckedStructuralParameterField {
            parameter_position,
            path,
        })
    }

    pub fn contains_trapping_operation(&self) -> bool {
        self.contains_trapping_operation_with_certificates(false)
    }

    pub fn contains_possible_trapping_operation(&self) -> bool {
        self.contains_trapping_operation_with_certificates(true)
    }

    pub(crate) fn contains_trapping_operation_with_certificates(
        &self,
        use_certificates: bool,
    ) -> bool {
        match self {
            Self::IntegerBinary {
                kind,
                left,
                right,
                trap_free,
                ..
            } => {
                (kind.is_trapping() && !(use_certificates && *trap_free))
                    || left.contains_trapping_operation_with_certificates(use_certificates)
                    || right.contains_trapping_operation_with_certificates(use_certificates)
            }
            Self::IntegerBitwiseNot { operand, .. }
            | Self::IntegerWiden { operand, .. }
            | Self::IntegerExactCast { operand, .. }
            | Self::RepresentationView { operand, .. } => {
                operand.contains_trapping_operation_with_certificates(use_certificates)
            }
            Self::StructuralParameterIndexedRead {
                index,
                element_path,
                ..
            } => {
                index
                    .expression
                    .contains_trapping_operation_with_certificates(use_certificates)
                    || element_path.iter().any(|projection| match projection {
                        CheckedScalarReadProjection::Static(_) => false,
                        CheckedScalarReadProjection::RuntimeIndex(selector) => selector
                            .expression
                            .contains_trapping_operation_with_certificates(use_certificates),
                    })
            }
            Self::Boolean(expression) => {
                expression.contains_trapping_operation_with_certificates(use_certificates)
            }
            Self::StructuralSequenceLength { path, .. } => path.iter().any(|step| match step {
                CheckedScalarReadProjection::Static(_) => false,
                CheckedScalarReadProjection::RuntimeIndex(selector) => selector
                    .expression
                    .contains_trapping_operation_with_certificates(use_certificates),
            }),
            Self::ConstantApplication { .. }
            | Self::StorageRead { .. }
            | Self::Parameter { .. }
            | Self::ErasedParameter { .. }
            | Self::Local { .. }
            | Self::StructuralParameterField { .. }
            | Self::StructuralLocalField { .. }
            | Self::IntegerLiteral { .. }
            | Self::IeeeFloatLiteral { .. }
            | Self::ImplementationSelection { .. } => false,
        }
    }

    pub fn primitive_type(&self) -> Option<typed_trees::types::PrimitiveType> {
        use typed_trees::types::PrimitiveType;
        match self {
            Self::StructuralSequenceLength { .. } => Some(PrimitiveType::U64),
            Self::ConstantApplication { primitive_type, .. }
            | Self::Parameter { primitive_type, .. }
            | Self::ErasedParameter { primitive_type, .. }
            | Self::StorageRead { primitive_type, .. }
            | Self::Local { primitive_type, .. }
            | Self::StructuralParameterField { primitive_type, .. }
            | Self::StructuralLocalField { primitive_type, .. }
            | Self::StructuralParameterIndexedRead { primitive_type, .. }
            | Self::IntegerBinary { primitive_type, .. }
            | Self::IntegerBitwiseNot { primitive_type, .. }
            | Self::IntegerWiden { primitive_type, .. }
            | Self::IntegerExactCast { primitive_type, .. }
            | Self::RepresentationView { primitive_type, .. } => Some(*primitive_type),
            Self::IntegerLiteral { literal } => Some(PrimitiveType::from_landed_integer(
                literal.landing()?.landed_type,
            )),
            Self::IeeeFloatLiteral { value } => Some(match value {
                semantic_vocabulary::IeeeFloatValue::Binary32(_) => PrimitiveType::F32,
                semantic_vocabulary::IeeeFloatValue::Binary64(_) => PrimitiveType::F64,
            }),
            Self::Boolean(_) => Some(PrimitiveType::Bool),
            Self::ImplementationSelection { .. } => Some(PrimitiveType::Selector),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum CheckedIntegerBinaryKind {
    ExactAdd,
    ExactSubtract,
    ExactMultiply,
    ExactDivide,
    ExactRemainder,
    WrappingDivide,
    WrappingRemainder,
    SaturatingDivide,
    SaturatingRemainder,
    WrappingAdd,
    SaturatingAdd,
    WrappingSubtract,
    SaturatingSubtract,
    WrappingMultiply,
    SaturatingMultiply,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    WrappingShiftLeft,
    WrappingShiftRight,
    ExactShiftLeft,
    ExactShiftRight,
    SaturatingShiftLeft,
    SaturatingShiftRight,
    TrappingShiftLeft,
    TrappingShiftRight,
    TrappingAdd,
    TrappingSubtract,
    TrappingMultiply,
    TrappingDivide,
    TrappingRemainder,
}

impl CheckedIntegerBinaryKind {
    pub const fn is_trapping(self) -> bool {
        matches!(
            self,
            Self::TrappingShiftLeft
                | Self::TrappingShiftRight
                | Self::TrappingAdd
                | Self::TrappingSubtract
                | Self::TrappingMultiply
                | Self::TrappingDivide
                | Self::TrappingRemainder
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub enum CheckedBooleanExpression {
    StorageRead {
        symbol: SymbolHandle,
    },
    Constant(bool),
    Parameter {
        position: usize,
    },
    ErasedParameter {
        position: usize,
    },
    Local {
        position: usize,
    },
    StructuralParameterField {
        observation: typed_trees::expression::ExpressionHandle,
        parameter_position: u32,
        path: Vec<CheckedStructuralPredicatePathSegment>,
    },
    Not(Box<CheckedBooleanExpression>),
    Equal {
        left: Box<CheckedBooleanExpression>,
        right: Box<CheckedBooleanExpression>,
    },
    IntegerComparison {
        kind: CheckedIntegerComparisonKind,
        left: Box<CheckedScalarExpression>,
        right: Box<CheckedScalarExpression>,
    },
    IntegerMathComparison {
        kind: CheckedIntegerComparisonKind,
        left: CheckedIntegerMathExpression,
        right: CheckedIntegerMathExpression,
    },
    ScalarIeeeFloatComparison {
        kind: CheckedIeeeFloatComparisonKind,
        left: Box<CheckedScalarExpression>,
        right: Box<CheckedScalarExpression>,
    },
    IeeeFloatComparison {
        kind: CheckedIeeeFloatComparisonKind,
        primitive_type: typed_trees::types::PrimitiveType,
        left: CheckedStructuralParameterField,
        right: CheckedStructuralParameterField,
    },
    StructuralIdentity {
        left: CheckedStructuralParameterField,
        right: CheckedStructuralParameterField,
    },
    ByteSequenceEqual {
        left: CheckedByteSequenceOperand,
        right: CheckedByteSequenceOperand,
    },
    ByteSequencePredicate {
        predicate: language_semantics::byte_predicates::ByteSequencePredicate,
        root: CheckedStorageRoot,
        path: Vec<CheckedStructuralPredicatePathSegment>,
    },
    StructuralCaseMembership {
        subject: CheckedStructuralCaseSubject,
        case: typed_trees::data::DataMemberIdentity,
    },
    And {
        left: Box<CheckedBooleanExpression>,
        right: Box<CheckedBooleanExpression>,
    },
    Or {
        left: Box<CheckedBooleanExpression>,
        right: Box<CheckedBooleanExpression>,
    },
}

impl Default for CheckedBooleanExpression {
    fn default() -> Self {
        Self::Constant(false)
    }
}

impl CheckedBooleanExpression {
    pub fn contains_trapping_operation(&self) -> bool {
        self.contains_trapping_operation_with_certificates(false)
    }

    pub fn contains_possible_trapping_operation(&self) -> bool {
        self.contains_trapping_operation_with_certificates(true)
    }

    pub(crate) fn contains_trapping_operation_with_certificates(
        &self,
        use_certificates: bool,
    ) -> bool {
        match self {
            Self::Not(operand) => {
                operand.contains_trapping_operation_with_certificates(use_certificates)
            }
            Self::Equal { left, right } | Self::And { left, right } | Self::Or { left, right } => {
                left.contains_trapping_operation_with_certificates(use_certificates)
                    || right.contains_trapping_operation_with_certificates(use_certificates)
            }
            Self::IntegerComparison { left, right, .. }
            | Self::ScalarIeeeFloatComparison { left, right, .. } => {
                left.contains_trapping_operation_with_certificates(use_certificates)
                    || right.contains_trapping_operation_with_certificates(use_certificates)
            }
            Self::IntegerMathComparison { left, right, .. } => {
                left.contains_trapping_operation_with_certificates(use_certificates)
                    || right.contains_trapping_operation_with_certificates(use_certificates)
            }
            Self::StorageRead { .. }
            | Self::Constant(_)
            | Self::Parameter { .. }
            | Self::ErasedParameter { .. }
            | Self::Local { .. }
            | Self::StructuralParameterField { .. }
            | Self::IeeeFloatComparison { .. }
            | Self::StructuralIdentity { .. }
            | Self::ByteSequenceEqual { .. }
            | Self::ByteSequencePredicate { .. }
            | Self::StructuralCaseMembership { .. } => false,
        }
    }
}

pub use semantic_vocabulary::IeeeFloatComparisonKind as CheckedIeeeFloatComparisonKind;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub struct CheckedStructuralParameterField {
    pub parameter_position: u32,
    pub path: Vec<CheckedStructuralPredicatePathSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedByteSequenceOperand {
    pub root: CheckedStorageRoot,
    pub path: Vec<CheckedStructuralPredicatePathSegment>,
}

impl From<CheckedStructuralParameterField> for CheckedByteSequenceOperand {
    fn from(field: CheckedStructuralParameterField) -> Self {
        Self {
            root: CheckedStorageRoot::Parameter {
                index: field.parameter_position,
            },
            path: field.path,
        }
    }
}

impl CheckedByteSequenceOperand {
    pub fn parameter_field(&self) -> Option<CheckedStructuralParameterField> {
        Some(CheckedStructuralParameterField {
            parameter_position: self.root.parameter()?,
            path: self.path.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum CheckedStructuralCaseRoot {
    Parameter { position: u32 },
    Result,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub struct CheckedStructuralCaseSubject {
    pub root: CheckedStructuralCaseRoot,
    pub path: Vec<CheckedStructuralPredicatePathSegment>,
}

impl From<CheckedStructuralParameterField> for CheckedStructuralCaseSubject {
    fn from(field: CheckedStructuralParameterField) -> Self {
        Self {
            root: CheckedStructuralCaseRoot::Parameter {
                position: field.parameter_position,
            },
            path: field.path,
        }
    }
}

impl CheckedStructuralCaseSubject {
    pub fn parameter_field(&self) -> Option<CheckedStructuralParameterField> {
        let CheckedStructuralCaseRoot::Parameter { position } = self.root else {
            return None;
        };
        Some(CheckedStructuralParameterField {
            parameter_position: position,
            path: self.path.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum CheckedStructuralPredicatePathSegment {
    Referent,
    Field(typed_trees::data::DataMemberIdentity),
    Case(typed_trees::data::DataMemberIdentity),
    FixedIndex(u64),
    RuntimeIndex(CheckedRuntimeIndex),
}

impl Default for CheckedStructuralPredicatePathSegment {
    fn default() -> Self {
        Self::Field(typed_trees::data::DataMemberIdentity::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum CheckedIntegerComparisonKind {
    Equal,
    LessThan,
    LessOrEqual,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, compiler_storage::Record)]
pub enum CheckedRuntimeIndex {
    StorageRead { symbol: SymbolHandle },
    Parameter { position: u32 },
    AssignmentIndex { depth: u32 },
    StructuralField(CheckedStructuralParameterField),
    Local { position: u32 },
}

#[macro_export]
macro_rules! checked_boolean_triple_pattern {
    ($($fields:tt)*) => {
        $crate::CheckedBooleanExpression::Equal { $($fields)* }
            | $crate::CheckedBooleanExpression::And { $($fields)* }
            | $crate::CheckedBooleanExpression::Or { $($fields)* }
    };
}

impl PartialOrd for CheckedRuntimeIndex {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for CheckedRuntimeIndex {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let tag = |index: &Self| match index {
            Self::Parameter { .. } => 0,
            Self::AssignmentIndex { .. } => 1,
            Self::StructuralField(_) => 2,
            Self::Local { .. } => 3,
            Self::StorageRead { .. } => 4,
        };
        match tag(self).cmp(&tag(other)) {
            std::cmp::Ordering::Equal => match (self, other) {
                (Self::Parameter { position: left }, Self::Parameter { position: right })
                | (Self::Local { position: left }, Self::Local { position: right }) => {
                    left.cmp(right)
                }
                (Self::AssignmentIndex { depth: left }, Self::AssignmentIndex { depth: right }) => {
                    left.cmp(right)
                }
                (Self::StructuralField(left), Self::StructuralField(right)) => left.cmp(right),
                (Self::StorageRead { symbol: left }, Self::StorageRead { symbol: right }) => {
                    (left.arena_index(), left.generation())
                        .cmp(&(right.arena_index(), right.generation()))
                }
                _ => unreachable!("equal coordinate tags retain their variant"),
            },
            order => order,
        }
    }
}
