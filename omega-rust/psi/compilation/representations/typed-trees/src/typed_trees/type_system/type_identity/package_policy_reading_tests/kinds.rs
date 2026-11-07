//! Kind tables of the node enums a renderer witness must cover.

use crate::expression::{BinaryOperator, ExpressionNode, MatchFieldAction, MatchPattern};
use crate::types::{
    DomainConstraintSubject, FixedArrayLength, TypeConstraintNode, TypeReferenceNode,
};

/// A table of the kinds of one node enum. Its `match` has no wildcard, so a new
/// variant fails to compile here, and the row it needs extends `ALL`, which the
/// coverage test requires some witness to reach.
macro_rules! node_kinds {
    ($kinds:ident, $kind_of:ident for $node:ty { $($kind:ident => $pattern:pat,)+ }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
        pub(super) enum $kinds {
            $($kind,)+
        }

        impl $kinds {
            pub(super) const ALL: &'static [Self] = &[$(Self::$kind,)+];
        }

        pub(super) fn $kind_of(node: &$node) -> $kinds {
            match node {
                $($pattern => $kinds::$kind,)+
            }
        }
    };
}

node_kinds!(TypeKind, type_kind for TypeReferenceNode {
    Reference => TypeReferenceNode::Reference { .. },
    Constrained => TypeReferenceNode::Constrained { .. },
    FixedArray => TypeReferenceNode::FixedArray { .. },
    Slice => TypeReferenceNode::Slice { .. },
    Generic => TypeReferenceNode::Generic { .. },
    ConstExpression => TypeReferenceNode::ConstExpression(_),
    DynamicTrait => TypeReferenceNode::DynamicTrait { .. },
    Named => TypeReferenceNode::Named { .. },
    Unit => TypeReferenceNode::Unit,
});

node_kinds!(ConstraintKind, constraint_kind for TypeConstraintNode {
    Named => TypeConstraintNode::Named(_),
    ArithmeticDomain => TypeConstraintNode::ArithmeticDomain(_),
    Domain => TypeConstraintNode::Domain(_),
});

node_kinds!(DomainKind, domain_kind for DomainConstraintSubject {
    Declared => DomainConstraintSubject::Declared,
    Carry => DomainConstraintSubject::Carry(_),
    Value => DomainConstraintSubject::Value(_),
    OmegaLayout => DomainConstraintSubject::OmegaLayout { .. },
});

node_kinds!(LengthKind, length_kind for FixedArrayLength {
    Literal => FixedArrayLength::Literal(_),
    Expression => FixedArrayLength::Expression(_),
    ConstParameter => FixedArrayLength::ConstParameter { .. },
    ConstCall => FixedArrayLength::ConstCall { .. },
});

node_kinds!(ExpressionKind, expression_kind for ExpressionNode {
    TypeExpression => ExpressionNode::TypeExpression(_),
    Match => ExpressionNode::Match(_),
    SavedSubject => ExpressionNode::SavedSubject(_),
    ArrayLiteral => ExpressionNode::ArrayLiteral(_),
    Atomic => ExpressionNode::Atomic(_),
    Binary => ExpressionNode::Binary(_),
    Boolean => ExpressionNode::Boolean(_),
    Cast => ExpressionNode::Cast(_),
    Call => ExpressionNode::Call(_),
    Float => ExpressionNode::Float(_),
    UndeterminedNan => ExpressionNode::UndeterminedNaN(_),
    Indexed => ExpressionNode::Indexed(_),
    Integer => ExpressionNode::Integer(_),
    Member => ExpressionNode::Member(_),
    Borrow => ExpressionNode::Borrow(_),
    Name => ExpressionNode::Name(_),
    Range => ExpressionNode::Range(_),
    StructLiteral => ExpressionNode::StructLiteral(_),
    String => ExpressionNode::String(_),
    Unary => ExpressionNode::Unary(_),
    ZeroValue => ExpressionNode::ZeroValue(_),
});

node_kinds!(OperatorKind, operator_kind for BinaryOperator {
    Add => BinaryOperator::Add,
    And => BinaryOperator::And,
    BitwiseAnd => BinaryOperator::BitwiseAnd,
    BitwiseOr => BinaryOperator::BitwiseOr,
    BitwiseXor => BinaryOperator::BitwiseXor,
    Divide => BinaryOperator::Divide,
    Equal => BinaryOperator::Equal,
    Greater => BinaryOperator::Greater,
    GreaterOrEqual => BinaryOperator::GreaterOrEqual,
    Less => BinaryOperator::Less,
    LessOrEqual => BinaryOperator::LessOrEqual,
    Modulo => BinaryOperator::Modulo,
    Multiply => BinaryOperator::Multiply,
    NotEqual => BinaryOperator::NotEqual,
    Or => BinaryOperator::Or,
    ShiftLeft => BinaryOperator::ShiftLeft,
    ShiftRight => BinaryOperator::ShiftRight,
    Subtract => BinaryOperator::Subtract,
    CaseMembership => BinaryOperator::CaseMembership,
});

node_kinds!(PatternKind, pattern_kind for MatchPattern {
    Value => MatchPattern::Value(_),
    Destructure => MatchPattern::Destructure(_),
    Wildcard => MatchPattern::Wildcard,
});

node_kinds!(ActionKind, action_kind for MatchFieldAction {
    Bind => MatchFieldAction::Bind(_),
    Equal => MatchFieldAction::Equal(_),
    Waive => MatchFieldAction::Waive,
});
