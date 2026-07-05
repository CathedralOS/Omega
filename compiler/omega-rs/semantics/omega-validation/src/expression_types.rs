use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
use std::fmt;

#[derive(Debug, Clone, Copy)]
pub(crate) enum ExpressionTypeOwner<'program> {
    StateTerminalExpression {
        machine: &'program str,
        state: &'program str,
    },
}

impl fmt::Display for ExpressionTypeOwner<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StateTerminalExpression { machine, state } => {
                write!(
                    formatter,
                    "machine `{machine}` state `{state}` terminal expression"
                )
            }
        }
    }
}

pub(crate) fn argument_matches_type_reference_handle(
    program: &TypedTrees,
    argument: ExpressionHandle,
    type_reference: TypeReferenceHandle,
) -> bool {
    if let ExpressionNode::Mutable(inner_expression) = program.expression_table.expression(argument)
    {
        return argument_matches_type_reference_handle(program, *inner_expression, type_reference);
    }

    let argument_node = program.expression_table.expression(argument);

    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            argument_matches_type_reference_handle(program, argument, *referee)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            argument_matches_type_reference_handle(program, argument, *base_type)
        }
        TypeReferenceNode::FixedArray { .. } => matches!(
            argument_node,
            ExpressionNode::ArrayLiteral(_)
                | ExpressionNode::Call(_)
                | ExpressionNode::Indexed(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Name(_)
        ),
        TypeReferenceNode::Slice { element_type } => {
            // A string literal is a byte sequence, so it satisfies a `[u8]` slice
            // target (`&[u8] in Utf8 = "..."`) -- the basis for migrating string
            // literals to the `[u8] in Utf8` view. Other element types keep the
            // reference/place forms only.
            let element_is_u8 = matches!(
                program.type_reference_table.primitive_type(*element_type),
                Some(PrimitiveType::U8)
            );
            matches!(
                argument_node,
                ExpressionNode::Call(_)
                    | ExpressionNode::Indexed(_)
                    | ExpressionNode::Member(_)
                    | ExpressionNode::Name(_)
            ) || (element_is_u8 && matches!(argument_node, ExpressionNode::String(_)))
        }
        TypeReferenceNode::Generic { .. } => matches!(
            argument_node,
            ExpressionNode::Binary(_)
                | ExpressionNode::Call(_)
                | ExpressionNode::Cast(_)
                | ExpressionNode::Indexed(_)
                | ExpressionNode::Integer(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Name(_)
                | ExpressionNode::StructLiteral(_)
                | ExpressionNode::Unary(_)
        ),
        TypeReferenceNode::DynamicTrait { .. } => matches!(
            argument_node,
            ExpressionNode::Call(_)
                | ExpressionNode::Cast(_)
                | ExpressionNode::Indexed(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Name(_)
        ),
        TypeReferenceNode::Named {
            name: type_name, ..
        } => {
            if let Some(primitive_type) = PrimitiveType::from_name(type_name) {
                return matches!(argument_node, ExpressionNode::Boolean(_))
                    && primitive_type == PrimitiveType::Bool
                    || matches!(argument_node, ExpressionNode::String(_))
                        && primitive_type == PrimitiveType::String
                    || matches!(argument_node, ExpressionNode::Float(_))
                        && primitive_type.accepts_float_literal()
                    || matches!(argument_node, ExpressionNode::Integer(_))
                        && primitive_type.accepts_integer_literal()
                    || matches!(argument_node, ExpressionNode::Unary(_))
                        && primitive_type == PrimitiveType::Bool
                    || matches!(
                        argument_node,
                        ExpressionNode::Binary(_)
                            | ExpressionNode::Call(_)
                            | ExpressionNode::Cast(_)
                            | ExpressionNode::Indexed(_)
                            | ExpressionNode::Member(_)
                            | ExpressionNode::Name(_)
                            | ExpressionNode::StructLiteral(_)
                    );
            }

            matches!(
                argument_node,
                ExpressionNode::Binary(_)
                    | ExpressionNode::Call(_)
                    | ExpressionNode::Cast(_)
                    | ExpressionNode::Indexed(_)
                    | ExpressionNode::Member(_)
                    | ExpressionNode::Name(_)
                    | ExpressionNode::StructLiteral(_)
                    | ExpressionNode::Unary(_)
            )
        }
        TypeReferenceNode::Unit => false,
    }
}

pub(crate) fn validate_expression_type_handle(
    program: &TypedTrees,
    expression: ExpressionHandle,
    type_reference: TypeReferenceHandle,
    diagnostics: &mut Vec<Diagnostic>,
    owner: ExpressionTypeOwner<'_>,
) {
    if !argument_matches_type_reference_handle(program, expression, type_reference) {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} expects `{}`, got `{}`",
            program.display_type_reference_with_constraints(type_reference),
            expression_type_name_handle(program, expression)
        )));
    }
}

/// The three disjoint scalar value CLASSES a scalar assignment can conflate.
/// A literal RHS or a resolvable place (`self.field`, a local) names its class
/// unambiguously; a target primitive names one too. Assigning across classes
/// (e.g. a `bool` into an `i32` field) is a type error the backend would
/// otherwise SILENTLY miscompile -- `true` stored as `1`, `"hi"` stored as
/// garbage. We deliberately fold every integer AND float primitive into a
/// single `Numeric` class so that numeric coercions (`f64 = 5`, `i8 = 300`,
/// `i32 = self.i8_field`) are NOT flagged here -- those are the province of the
/// narrowing/domain checks, which carry their own precise diagnostics. This
/// gate fires ONLY on cross-class conflicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValueClass {
    Boolean,
    Text,
    Numeric,
}

impl ValueClass {
    pub(crate) fn describe(self) -> &'static str {
        match self {
            Self::Boolean => "a boolean",
            Self::Text => "text",
            Self::Numeric => "a numeric value",
        }
    }

    /// The class of a literal RHS, or `None` for a non-literal expression.
    fn of_literal(program: &TypedTrees, value: ExpressionHandle) -> Option<Self> {
        match program.expression_table.expression(value) {
            ExpressionNode::Boolean(_) => Some(Self::Boolean),
            ExpressionNode::String(_) => Some(Self::Text),
            ExpressionNode::Integer(_) | ExpressionNode::Float(_) => Some(Self::Numeric),
            ExpressionNode::Mutable(inner) => Self::of_literal(program, *inner),
            _ => None,
        }
    }

    fn of_primitive(primitive: PrimitiveType) -> Self {
        match primitive {
            PrimitiveType::Bool => Self::Boolean,
            PrimitiveType::String => Self::Text,
            _ => Self::Numeric,
        }
    }
}

/// The value class of an assignment RHS, if it is unambiguously determinable:
/// a literal (its literal class) OR a resolvable place -- `self.field`, a local
/// -- whose declared type is a scalar primitive. Returns `None` for any
/// computed expression (binary, call, cast, indexed) whose result type we do
/// not resolve here -- those are left to the blanket-accepting general gate, so
/// this never false-positives on them.
fn value_class(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: Option<&omega_typed_trees::state::State>,
    value: ExpressionHandle,
) -> Option<ValueClass> {
    if let Some(class) = ValueClass::of_literal(program, value) {
        return Some(class);
    }
    let primitive = crate::places::declared_place_type(program, machine, state, value)
        .and_then(|handle| program.primitive_type_reference(handle))?;
    Some(ValueClass::of_primitive(primitive))
}

/// If the `value`'s scalar class conflicts with the `target` primitive's, return
/// `(value_class, target_class)` for a diagnostic. Returns `None` for in-class
/// stores and for any value whose class is not resolvable here. Used for both
/// assignment RHS and call/transition ARGUMENTS -- both store a value into a
/// typed slot, and both silently miscompiled on a cross-class scalar.
pub(crate) fn cross_class_conflict(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: Option<&omega_typed_trees::state::State>,
    value: ExpressionHandle,
    target: PrimitiveType,
) -> Option<(ValueClass, ValueClass)> {
    let value_class = value_class(program, machine, state, value)?;
    let target_class = ValueClass::of_primitive(target);
    (value_class != target_class).then_some((value_class, target_class))
}

pub(crate) fn expression_type_name_handle(
    program: &TypedTrees,
    argument: ExpressionHandle,
) -> &'static str {
    match program.expression_table.expression(argument) {
        ExpressionNode::ArrayLiteral(_) => "array literal",
        ExpressionNode::Binary(_) => "binary expression",
        ExpressionNode::Boolean(_) => "bool",
        ExpressionNode::Call(_) => "call expression",
        ExpressionNode::Cast(_) => "cast expression",
        ExpressionNode::Float(_) => "float literal",
        ExpressionNode::Indexed(_) => "indexed value",
        ExpressionNode::Integer(_) => "integer literal",
        ExpressionNode::Member(_) => "member access",
        ExpressionNode::Mutable(inner_expression) => {
            expression_type_name_handle(program, *inner_expression)
        }
        ExpressionNode::Name(_) => "named value",
        ExpressionNode::Range(_) => "range expression",
        ExpressionNode::StructLiteral(_) => "struct literal",
        ExpressionNode::String(_) => "String",
        ExpressionNode::Unary(_) => "bool",
    }
}
