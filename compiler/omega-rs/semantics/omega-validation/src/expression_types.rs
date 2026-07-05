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
    machine: Option<&omega_typed_trees::machine::Machine>,
    state: Option<&omega_typed_trees::state::State>,
    value: ExpressionHandle,
) -> Option<ValueClass> {
    if let Some(class) = ValueClass::of_literal(program, value) {
        return Some(class);
    }
    // Classify a computed binary so that storing an out-of-range result into a
    // `bool` (or a number into text) is caught. Comparison / logical binaries stay
    // unclassified (None -> blanket-accepted): a raw comparison / logical result IS
    // an intended 0/1 coercion into a numeric slot -- runtime_comparison_value_signedness
    // stores `a > b` straight into an `i32` -- so it must NOT be flagged. (The narrow
    // cost: an arithmetic/bitwise op OVER raw comparison results, `(a == 1) + (a == 1)`,
    // stored into a bool is not caught -- its comparison operands classify as None.)
    if let ExpressionNode::Binary(binary) = program.expression_table.expression(value) {
        use omega_typed_trees::expression::BinaryOperator;
        let left = value_class(program, machine, state, binary.left);
        let right = value_class(program, machine, state, binary.right);
        if crate::arithmetic_domains::is_arithmetic(binary.operator) {
            // Arithmetic / shift (`+ - * / % << >>`) is integer arithmetic even over
            // bool operands, since bool feeds in as its 0/1 value (the match desugar
            // relies on this) and the result can leave `{0, 1}` -- so `let x: bool =
            // b + b` (which silently produced a bool holding 2) is caught. But `+` is
            // OVERLOADED: `string + string` is concatenation, so a text operand means
            // concat (Text); any numeric/bool operand means numeric.
            return match (left, right) {
                (Some(ValueClass::Text), _) | (_, Some(ValueClass::Text)) => Some(ValueClass::Text),
                (Some(_), _) | (_, Some(_)) => Some(ValueClass::Numeric),
                (None, None) => None,
            };
        }
        if matches!(
            binary.operator,
            BinaryOperator::BitwiseAnd | BinaryOperator::BitwiseOr | BinaryOperator::BitwiseXor
        ) {
            // Bitwise `& | ^` preserve `{0, 1}` for bool operands, so `b & b` into a
            // bool stays fine -- only a NUMERIC operand makes the result numeric
            // (`let x: bool = 2 & 3`, which silently produced a bool holding 2, is
            // caught). Bool-only bitwise stays unclassified.
            return match (left, right) {
                (Some(ValueClass::Numeric), _) | (_, Some(ValueClass::Numeric)) => {
                    Some(ValueClass::Numeric)
                }
                _ => None,
            };
        }
    }
    // A place RHS (`self.field`, a local) needs the machine/state to resolve its
    // declared type. Without a machine context (e.g. a data field DEFAULT, which is
    // always a literal/const), only the literal path above applies.
    let machine = machine?;
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
    machine: Option<&omega_typed_trees::machine::Machine>,
    state: Option<&omega_typed_trees::state::State>,
    value: ExpressionHandle,
    target: PrimitiveType,
) -> Option<(ValueClass, ValueClass)> {
    let value_class = value_class(program, machine, state, value)?;
    let target_class = ValueClass::of_primitive(target);
    (value_class != target_class).then_some((value_class, target_class))
}

/// If `value`'s scalar class conflicts with the `target` primitive's, push the
/// cross-class store diagnostic and return `true`; else return `false`.
/// `slot_context` names the store site (e.g. ``"argument `x` for state `s`"``,
/// ``"construction of `Point` field `x`"``, `"array literal element"`) and
/// `slot_noun` its kind (`"place"` / `"parameter"` / `"field"` / `"element"`).
/// SINGLE SOURCE OF TRUTH for the cross-class store diagnostic across every store
/// position -- the class complement of `arithmetic_domains::check_value_narrowing`.
pub(crate) fn report_cross_class_store(
    program: &TypedTrees,
    machine: Option<&omega_typed_trees::machine::Machine>,
    state: Option<&omega_typed_trees::state::State>,
    value: ExpressionHandle,
    target: PrimitiveType,
    slot_context: &str,
    slot_noun: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let Some((value_class, target_class)) = cross_class_conflict(program, machine, state, value, target)
    else {
        return false;
    };
    diagnostics.push(Diagnostic::error(format!(
        "{slot_context} stores {} into a `{}` {slot_noun}, which holds {}",
        value_class.describe(),
        target.name(),
        target_class.describe(),
    )));
    true
}

/// The name of the CONCRETE DATA type a `handle` denotes, looking through
/// `Reference`/`Constrained` shells -- or `None` for anything that is not a plain
/// data type (a primitive, a trait / boundary / platform, a generic type
/// parameter, an array, a versioned selector). The `None` cases are exactly the
/// ones a nominal argument check must NOT flag, so a data value passed to a trait
/// or generic parameter is never a "wrong type".
fn concrete_data_type_name(program: &TypedTrees, handle: TypeReferenceHandle) -> Option<&str> {
    if !handle.is_valid() {
        return None;
    }
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => concrete_data_type_name(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => {
            concrete_data_type_name(program, *base_type)
        }
        TypeReferenceNode::Named { name, .. } => {
            let name = name.as_str();
            // Versioned selectors (`Foo::v1`) are excluded -- conservative, avoids
            // treating `Foo::v1` and `Foo` as different concrete data types.
            if name
                .rsplit("::")
                .next()
                .is_some_and(omega_core::versioning::is_version_selector)
            {
                return None;
            }
            program
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == name)
                .map(|_| name)
        }
        _ => None,
    }
}

/// The concrete data type NAME a struct-literal value constructs (`B { .. }` ->
/// `"B"`; a case literal `Event::Score { .. }` -> `"Event"`), or `None` when the
/// value is not a struct literal or names a type that is not a data definition
/// (the unknown-type case, rejected separately). Looks through a `Mutable`
/// wrapper. This lets the nominal check resolve a LITERAL's type directly, where
/// `declared_place_type` (place-only) resolves nothing.
fn struct_literal_type_name(program: &TypedTrees, value: ExpressionHandle) -> Option<&str> {
    match program.expression_table.expression(value) {
        ExpressionNode::StructLiteral(literal) => {
            let name = literal.type_name.as_str();
            program
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == name)
                // Skip GENERIC data types: the literal names the bare base (`Box`)
                // while the target is instantiated (`Box<i32>`), so a raw-name
                // compare would false-positive. Matching instantiated type args is a
                // deeper check; mirror `validate_literal_field_names`, which also
                // bails on generic definitions.
                .filter(|definition| definition.type_parameters.count() == 0)
                .map(|definition| definition.name.as_str())
        }
        ExpressionNode::Mutable(inner) => struct_literal_type_name(program, *inner),
        _ => None,
    }
}

/// If `value`'s CONCRETE DATA type differs from the `expected_type`'s concrete
/// data type, push a diagnostic and return `true`. BOTH sides must resolve to a
/// concrete data type name; every other form (a primitive, a trait / boundary /
/// generic parameter, an array, a versioned type, or a COMPUTED value whose type
/// is unresolved) yields `None` on one side and is skipped -- so this only ever
/// rejects the unambiguous type confusion. The value's type resolves from a
/// struct LITERAL's own type name (`B { .. }`) or, failing that, a PLACE's
/// declared type (`self.bar`). The nominal complement of `report_cross_class_store`.
pub(crate) fn report_data_type_conflict(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: Option<&omega_typed_trees::state::State>,
    value: ExpressionHandle,
    expected_type: TypeReferenceHandle,
    slot_context: &str,
    slot_noun: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let Some(expected) = concrete_data_type_name(program, expected_type) else {
        return false;
    };
    let Some(got) = struct_literal_type_name(program, value).or_else(|| {
        crate::places::declared_place_type(program, machine, state, value)
            .and_then(|value_type| concrete_data_type_name(program, value_type))
    }) else {
        return false;
    };
    if expected == got {
        return false;
    }
    diagnostics.push(Diagnostic::error(format!(
        "{slot_context} expects the `{expected}` data type but got `{got}` in the `{slot_noun}` \
         position; these are incompatible data types (a place is accepted structurally, but its \
         declared type must match)",
    )));
    true
}

/// Reject a binary operator that MIXES a text operand with a numeric/bool one:
/// `n == s` (`n: i32`, `s: String`) and `b + s` compile and run on a meaningless
/// comparison/combination of a number and a string pointer. Fires ONLY when one
/// operand resolves to `Text` and the other to a resolved `Numeric`/`Boolean` --
/// both-text (string equality / concatenation) and numeric<->bool (the 0/1
/// coercion) are fine, and an operand that does not classify (a call result, a
/// nested comparison) is skipped, so this never false-positives on them.
pub(crate) fn report_cross_class_binary_operands(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: Option<&omega_typed_trees::state::State>,
    left: ExpressionHandle,
    right: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let left_class = value_class(program, Some(machine), state, left);
    let right_class = value_class(program, Some(machine), state, right);
    let mixed = matches!(
        (left_class, right_class),
        (
            Some(ValueClass::Text),
            Some(ValueClass::Numeric | ValueClass::Boolean),
        ) | (
            Some(ValueClass::Numeric | ValueClass::Boolean),
            Some(ValueClass::Text),
        )
    );
    if !mixed {
        return false;
    }
    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}` applies an operator to {} and {} -- text and non-text \
         operands cannot be compared or combined",
        machine.name.as_str(),
        state.map(|state| state.name.as_str()).unwrap_or(""),
        left_class.unwrap().describe(),
        right_class.unwrap().describe(),
    )));
    true
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
