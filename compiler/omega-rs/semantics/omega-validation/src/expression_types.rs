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

/// The concrete data type NAME a `value` expression denotes: a struct LITERAL's own
/// type name (`B { .. }` -> `"B"`), or failing that a PLACE's declared concrete data
/// type (`self.bar` -> `"Bar"`). `None` for a primitive, array, generic, versioned,
/// or unresolvable computed value. The shared resolver behind the nominal checks
/// (`report_data_type_conflict` value-vs-target, `report_cross_type_equality`
/// operand-vs-operand) and the cast-source non-scalar detection.
fn value_concrete_data_name<'program>(
    program: &'program TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: Option<&omega_typed_trees::state::State>,
    value: ExpressionHandle,
) -> Option<&'program str> {
    struct_literal_type_name(program, value).or_else(|| {
        crate::places::declared_place_type(program, machine, state, value)
            .and_then(|value_type| concrete_data_type_name(program, value_type))
    })
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
    let Some(got) = value_concrete_data_name(program, machine, state, value) else {
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

/// Run every binary-operand TYPE check for a binary expression -- the checks that
/// reject an operator applied to operands it is not defined for. The single entry
/// point for `scan_expression_calls`'s Binary arm: it calls this once, and new
/// operand-type checks are added here (one place), not threaded through the walker.
pub(crate) fn validate_binary_operand_types(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: Option<&omega_typed_trees::state::State>,
    operator: omega_typed_trees::expression::BinaryOperator,
    left: ExpressionHandle,
    right: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    report_cross_class_binary_operands(program, machine, state, left, right, diagnostics);
    report_invalid_text_operator(program, machine, state, operator, left, right, diagnostics);
    report_non_bool_logical_operands(program, machine, state, operator, left, right, diagnostics);
    report_array_operator_operands(program, machine, state, operator, left, right, diagnostics);
    report_undeclared_struct_operator(program, machine, state, operator, left, diagnostics);
    report_float_bitwise_operator(program, machine, state, operator, left, right, diagnostics);
    crate::arithmetic_domains::report_out_of_range_comparison_literal(
        program, machine, state, operator, left, right, diagnostics,
    );
}

/// Whether `operand`'s type is a float (`f32`/`f64`): a float literal, or a place
/// whose declared type resolves to a float primitive. Looks through `Mutable`.
fn expression_is_float_typed(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: Option<&omega_typed_trees::state::State>,
    operand: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(operand) {
        ExpressionNode::Float(_) => true,
        ExpressionNode::Mutable(inner) => expression_is_float_typed(program, machine, state, *inner),
        _ => crate::places::declared_place_type(program, machine, state, operand)
            .and_then(|type_reference| program.primitive_type_reference(type_reference))
            .is_some_and(|primitive| matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64)),
    }
}

/// Reject bitwise/shift/modulo on a FLOAT operand: the interpreter rejects the set
/// ("float modulo/shift/bitwise not supported") and the backend cannot encode them,
/// yet `--check` passed silently. If float bit-ops are ever added, update the
/// interpreter and this together.
fn report_float_bitwise_operator(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: Option<&omega_typed_trees::state::State>,
    operator: omega_typed_trees::expression::BinaryOperator,
    left: ExpressionHandle,
    right: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    use omega_typed_trees::expression::BinaryOperator;
    if !matches!(
        operator,
        BinaryOperator::BitwiseAnd
            | BinaryOperator::BitwiseOr
            | BinaryOperator::BitwiseXor
            | BinaryOperator::ShiftLeft
            | BinaryOperator::ShiftRight
            | BinaryOperator::Modulo
    ) {
        return false;
    }
    if !expression_is_float_typed(program, machine, state, left)
        && !expression_is_float_typed(program, machine, state, right)
    {
        return false;
    }
    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}` applies `{operator:?}` to a float operand, but bitwise, shift, \
         and modulo operators are defined for integers only",
        machine.name.as_str(),
        state.map(|state| state.name.as_str()).unwrap_or(""),
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
fn report_cross_class_binary_operands(
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

/// Whether a type reference is a TEXT carrier: the `String` primitive, or a fixed
/// array / slice of `u8` (text is `&[u8]`, so a `String`, a byte slice, and a
/// `[u8; N]` are the same shape family and values flow between them). The shape
/// check skips these -- the array-vs-scalar dichotomy does not apply to text.
fn type_reference_is_text_carrier(program: &TypedTrees, handle: TypeReferenceHandle) -> bool {
    if !handle.is_valid() {
        return false;
    }
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => {
            type_reference_is_text_carrier(program, *referee)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_is_text_carrier(program, *base_type)
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            program.primitive_type_reference(*element_type) == Some(PrimitiveType::U8)
        }
        _ => program.primitive_type_reference(handle) == Some(PrimitiveType::String),
    }
}

/// Reject logical `&&`/`||` on a NON-bool operand (`5 && 3`, `a && n` for int `n`).
/// The connectives require `bool` operands; `5 && 3` otherwise uses int truthiness
/// (`== 1`), the C behavior Omega rejects (no int-in-bool -- same principle as
/// logical `!` and `<number> as bool`). Fires when EITHER operand classifies as
/// Numeric/Text; a comparison / logical / call / bool operand (None/Boolean) is
/// allowed, so `(a == 1) && (b < 5)` and `x && y` (bools) stay valid.
fn report_non_bool_logical_operands(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: Option<&omega_typed_trees::state::State>,
    operator: omega_typed_trees::expression::BinaryOperator,
    left: ExpressionHandle,
    right: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    use omega_typed_trees::expression::BinaryOperator;
    if !matches!(operator, BinaryOperator::And | BinaryOperator::Or) {
        return false;
    }
    let non_bool = |value| {
        matches!(
            value_class(program, Some(machine), state, value),
            Some(ValueClass::Numeric) | Some(ValueClass::Text)
        )
    };
    if !non_bool(left) && !non_bool(right) {
        return false;
    }
    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}` applies logical `{operator:?}` to a non-bool operand, but \
         `&&`/`||` require `bool` operands",
        machine.name.as_str(),
        state.map(|state| state.name.as_str()).unwrap_or(""),
    )));
    true
}

/// Reject a non-`+` arithmetic / shift / bitwise operator on TEXT operands
/// (`s - t`, `s * t` for strings). Text supports only `+` (concatenation) and
/// `==`/`!=`; there is no subtraction/multiplication/shift/etc. of strings, and
/// these otherwise lower to a garbage byte op. Fires ONLY when BOTH operands
/// classify as Text -- a text-vs-numeric MIX is `report_cross_class_binary_operands`'s
/// job, and a text-vs-unresolved pair is left alone. (Ordering `< <= > >=` on text is
/// a separate, plausible-future case, not rejected here.)
fn report_invalid_text_operator(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: Option<&omega_typed_trees::state::State>,
    operator: omega_typed_trees::expression::BinaryOperator,
    left: ExpressionHandle,
    right: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    use omega_typed_trees::expression::BinaryOperator;
    // Everything except `+` (concat), `==`, and `!=` -- text has no defined
    // arithmetic, bit, or ORDERING operators. Ordering in particular (`s < t`)
    // otherwise reaches the backend as a 16-byte runtime compare it cannot encode,
    // surfacing a cryptic "cannot load 16-byte runtime operands" error instead of a
    // precise one. (Lexicographic text ordering is a possible future feature; until
    // it exists, reject here rather than emit garbage or a confusing late error.)
    if !matches!(
        operator,
        BinaryOperator::Subtract
            | BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Modulo
            | BinaryOperator::ShiftLeft
            | BinaryOperator::ShiftRight
            | BinaryOperator::BitwiseAnd
            | BinaryOperator::BitwiseOr
            | BinaryOperator::BitwiseXor
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
    ) {
        return false;
    }
    let is_text =
        |value| value_class(program, Some(machine), state, value) == Some(ValueClass::Text);
    if !is_text(left) || !is_text(right) {
        return false;
    }
    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}` applies `{operator:?}` to text operands, but text supports only \
         concatenation (`+`), `==`, and `!=`",
        machine.name.as_str(),
        state.map(|state| state.name.as_str()).unwrap_or(""),
    )));
    true
}

/// Reject `<number/text> as bool` (`5 as bool`). Such a cast reinterprets the
/// source bits into a `bool` without normalizing to `{0, 1}`, producing an INVALID
/// bool (`5 as bool` yields a bool holding 5). `as` has no meaningful number->bool
/// conversion -- write an explicit comparison (`n != 0`). Only a PROVABLY non-bool
/// source (Numeric/Text) is flagged; a comparison / logical / call source (None) or
/// a `bool` source is allowed, so `(a == 1) as bool` and `b as bool` stay fine.
/// (Same no-int-in-bool principle as `report_non_bool_logical_not`.)
pub(crate) fn report_number_to_bool_cast(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: Option<&omega_typed_trees::state::State>,
    value: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let class = value_class(program, Some(machine), state, value);
    if !matches!(class, Some(ValueClass::Numeric) | Some(ValueClass::Text)) {
        return false;
    }
    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}` casts {} to `bool`, but `as` has no number-to-bool conversion \
         (a `bool` is {{0, 1}}; write an explicit comparison like `n != 0`)",
        machine.name.as_str(),
        state.map(|state| state.name.as_str()).unwrap_or(""),
        class.unwrap().describe(),
    )));
    true
}

/// Reject a cast to a NUMERIC/address scalar (`as i32`, `as f64`, `as u8`, `as addr`)
/// from a provably NON-scalar or TEXT source: `s as i32` (a `String` carrier),
/// `self.p as i32` (a struct), `self.xs as i32` (an array). `as` resolves the target
/// primitive, finds the source has no scalar conversion, and passes the bytes through
/// unchanged -- a silent reinterpret to garbage. Only a PROVABLY non-scalar/text source
/// is flagged; a numeric/bool source, a comparison result, or an unresolvable computed
/// source (a call) classifies as scalar/unknown and is left alone. (`as bool` targets
/// are handled by `report_number_to_bool_cast`; this covers the numeric/addr targets.)
pub(crate) fn report_invalid_numeric_cast_source(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: Option<&omega_typed_trees::state::State>,
    source: ExpressionHandle,
    target_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let mut reject = |reason: String| {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` {reason} to `{target_name}`, but `as` converts between \
             scalar types only",
            machine.name.as_str(),
            state.map(|state| state.name.as_str()).unwrap_or(""),
        )));
        true
    };
    // Text carrier source (`s as i32`): text is a `{len, bytes}` carrier, not a number.
    if value_class(program, Some(machine), state, source) == Some(ValueClass::Text) {
        return reject("casts text".to_owned());
    }
    // Array source (`self.xs as i32` / `[1, 2, 3] as i32`).
    if value_shape_is_array(program, machine, state, source) == Some(true) {
        return reject("casts an array value".to_owned());
    }
    // Struct source (`self.p as i32` / `P { .. } as i32`) -- a struct literal names
    // its own type; a struct place resolves to a concrete data type name.
    if let Some(name) = value_concrete_data_name(program, machine, state, source) {
        return reject(format!("casts a `{name}` value"));
    }
    false
}

/// Reject indexing the `String` carrier -- `s[i]` as a READ (`is_write == false`,
/// e.g. `let b = s[i]`) or as an assignment TARGET (`is_write == true`, `s[i] = x`).
/// `String` is a `{len, bytes}` carrier, not a flat byte array, so a byte index
/// silently reads / writes a ZII `0` (the backend has no index-into-carrier lowering
/// and the store checks skip an unresolved element type). A `[u8; N] in Utf8` byte
/// array resolves to a NON-primitive type, so `primitive_type_reference` returns
/// `None` and the supported byte-array indexing is left alone -- only the `String`
/// PRIMITIVE is caught here. `s.len` MEMBER access is not an `Indexed` node, so it is
/// never touched. Returns true if it reported. (Byte access on text is a possible
/// future feature; this closes the silent read/write-0 without precluding it.)
pub(crate) fn report_string_index_access(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: Option<&omega_typed_trees::state::State>,
    expression: ExpressionHandle,
    is_write: bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        return false;
    };
    let Some(collection_type) =
        crate::places::declared_place_type(program, machine, state, indexed.collection)
    else {
        return false;
    };
    if program.primitive_type_reference(collection_type) != Some(PrimitiveType::String) {
        return false;
    }
    let action = if is_write {
        "assigns to an index of a `String` value (`s[i] = ..`)"
    } else {
        "indexes a `String` value (`s[i]`)"
    };
    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}` {action}, but the text carrier does not support byte indexing; \
         use a `[u8; N] in Utf8` array for byte access",
        machine.name.as_str(),
        state.map(|state| state.name.as_str()).unwrap_or(""),
    )));
    true
}

/// Reject logical `!` on a NON-bool operand (`!5`, `!x` for `x: i32`). `!` is
/// bool-only in Omega; bitwise-not is the separate `~`. Only a PROVABLY non-bool
/// operand is flagged -- a numeric/text literal, an arithmetic result, or a place
/// whose declared type is a numeric/text primitive (all classify as Numeric/Text).
/// A comparison / logical / call / unresolved operand classifies as None and is
/// allowed, so a real bool (including a bare `a == 1`) is never rejected.
pub(crate) fn report_non_bool_logical_not(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: Option<&omega_typed_trees::state::State>,
    operand: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let class = value_class(program, Some(machine), state, operand);
    if !matches!(class, Some(ValueClass::Numeric) | Some(ValueClass::Text)) {
        return false;
    }
    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}` applies logical `!` to {}, but `!` requires a `bool` operand \
         (bitwise-not is spelled `~`)",
        machine.name.as_str(),
        state.map(|state| state.name.as_str()).unwrap_or(""),
        class.unwrap().describe(),
    )));
    true
}

/// Map a binary operator to its overloadable spelling, or `None` for operators
/// that cannot carry a domain meaning here: `==`/`!=` (the structural-equality /
/// Equatable path owns those), the logical `&&`/`||`, and bitwise/shift (which
/// have no `OperatorSpelling`, so no domain operator can be declared for them).
fn binary_operator_spelling(
    operator: omega_typed_trees::expression::BinaryOperator,
) -> Option<omega_core::operator_spelling::OperatorSpelling> {
    use omega_core::operator_spelling::OperatorSpelling;
    use omega_typed_trees::expression::BinaryOperator;
    Some(match operator {
        BinaryOperator::Add => OperatorSpelling::Add,
        BinaryOperator::Subtract => OperatorSpelling::Subtract,
        BinaryOperator::Multiply => OperatorSpelling::Multiply,
        BinaryOperator::Divide => OperatorSpelling::Divide,
        BinaryOperator::Modulo => OperatorSpelling::Modulo,
        BinaryOperator::Less => OperatorSpelling::Less,
        BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
        BinaryOperator::Greater => OperatorSpelling::Greater,
        BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
        _ => return None,
    })
}

/// Reject an arithmetic / ordering operator on a STRUCT operand for which no
/// operator with that spelling is DECLARED (`self.a + self.b` for a plain
/// `data P {}` lowered to a garbage byte op). A struct's only such operators are
/// DOMAIN operators (`domain Quantity::Additive { operator add ... spelling + }`),
/// so we ask the use-site authority `resolve_spelling`: an EMPTY candidate set for
/// a concrete-data receiver means the operator is undeclared. Scalars (intrinsic
/// builtins) and arrays are not concrete-data receivers, so they are untouched;
/// when candidates DO exist, admissibility (the proof context) is enforced
/// downstream, so a valid domain op (`Quantity + Quantity`) is never rejected.
fn report_undeclared_struct_operator(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: Option<&omega_typed_trees::state::State>,
    operator: omega_typed_trees::expression::BinaryOperator,
    left: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let Some(spelling) = binary_operator_spelling(operator) else {
        return false;
    };
    let Some(receiver_type) = crate::places::declared_place_type(program, machine, state, left)
    else {
        return false;
    };
    let Some(type_name) = concrete_data_type_name(program, receiver_type) else {
        return false;
    };
    if !omega_typed_trees::operator::resolve_spelling(program, spelling, Some(receiver_type))
        .is_empty()
    {
        return false;
    }
    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}` applies `{operator:?}` to a `{type_name}` value, but no such \
         operator is declared for it -- only `==`/`!=` (via `{type_name} satisfies Equatable`) \
         or a `domain {type_name}::... {{ operator ... }}` meaning operates on a data type",
        machine.name.as_str(),
        state.map(|state| state.name.as_str()).unwrap_or(""),
    )));
    true
}

/// Reject an ordering / arithmetic / bitwise operator whose operand is a NON-TEXT
/// array (`xs < ys`, `xs + ys` for `[i32; N]`). Arrays cannot carry domain
/// operators (only data types can, e.g. `Quantity::Additive`'s `+`), so these are
/// always meaningless and otherwise lower to a garbage byte op. `==`/`!=` and the
/// logical `&&`/`||` are left alone; text carriers (`String`, `[u8]`) are excluded
/// (string concat / comparison). Only PLACE operands are resolved.
fn report_array_operator_operands(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: Option<&omega_typed_trees::state::State>,
    operator: omega_typed_trees::expression::BinaryOperator,
    left: ExpressionHandle,
    right: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    use omega_typed_trees::expression::BinaryOperator;
    // Logical `&&`/`||` on a non-bool array operand are the province of the
    // non-bool-logical check (which reports a bool-operand requirement); skip them.
    if matches!(operator, BinaryOperator::And | BinaryOperator::Or) {
        return false;
    }
    // `==`/`!=` are excluded for STRUCT/data operands (they expand to synthesized
    // structural equality) but an ARRAY operand never expands -- there is no array
    // element-wise equality yet, so `xs == ys` reaches the backend as a multi-byte
    // runtime compare it cannot encode ("cannot load N-byte runtime operands"). Give
    // it a precise message here instead, alongside the ordering/arithmetic rejection.
    let is_equality = matches!(operator, BinaryOperator::Equal | BinaryOperator::NotEqual);
    for operand in [left, right] {
        if let Some(operand_type) =
            crate::places::declared_place_type(program, machine, state, operand)
            && type_reference_is_array(program, operand_type)
            && !type_reference_is_text_carrier(program, operand_type)
        {
            let detail = if is_equality {
                "arrays do not support `==` / `!=` yet (element-wise array equality is not \
                 synthesized -- compare elements individually)"
            } else {
                "ordering, arithmetic, and bitwise operators are not defined for arrays"
            };
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{}` applies `{operator:?}` to an array operand, but {detail}",
                machine.name.as_str(),
                state.map(|state| state.name.as_str()).unwrap_or(""),
            )));
            return true;
        }
    }
    false
}

/// Whether a type reference denotes an ARRAY (a fixed array or a slice), looking
/// through `Reference`/`Constrained` shells.
fn type_reference_is_array(program: &TypedTrees, handle: TypeReferenceHandle) -> bool {
    if !handle.is_valid() {
        return false;
    }
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => type_reference_is_array(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_is_array(program, *base_type)
        }
        TypeReferenceNode::FixedArray { .. } | TypeReferenceNode::Slice { .. } => true,
        _ => false,
    }
}

/// Whether a value's SHAPE is an array (`Some(true)`), a non-array scalar/struct
/// (`Some(false)`), or undeterminable here (`None` -> skipped): an array literal
/// vs a scalar literal, or a place resolved through `declared_place_type`. A
/// computed value (call, binary, indexed) is `None` so this never false-positives.
fn value_shape_is_array(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: Option<&omega_typed_trees::state::State>,
    value: ExpressionHandle,
) -> Option<bool> {
    match program.expression_table.expression(value) {
        ExpressionNode::ArrayLiteral(_) => Some(true),
        ExpressionNode::Integer(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::String(_)
        | ExpressionNode::StructLiteral(_) => Some(false),
        ExpressionNode::Mutable(inner) => value_shape_is_array(program, machine, state, *inner),
        ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
            crate::places::declared_place_type(program, machine, state, value)
                .map(|type_reference| type_reference_is_array(program, type_reference))
        }
        _ => None,
    }
}

/// Reject binding a value of the wrong SHAPE to a target: an array into a
/// non-array slot (`let y: i32 = self.xs`, which silently read a ZII 0) or a
/// non-array value into an array slot (`let xs: [i32; 3] = 5`). Both sides must be
/// determinable; a computed value (a call result) is skipped. Complements the
/// scalar-CLASS and nominal-DATA checks, which both classify only scalar shapes.
pub(crate) fn report_array_scalar_shape_mismatch(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: Option<&omega_typed_trees::state::State>,
    value: ExpressionHandle,
    target_type: TypeReferenceHandle,
    slot_context: &str,
    slot_noun: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    // TEXT is `&[u8]`-backed, so a `String`, a byte slice, and a `[u8; N]` are one
    // shape family and values flow between them freely (`write_line([u8])` takes a
    // String; a byte-slice value fills a String param). The array-vs-scalar
    // dichotomy does not apply -- skip when EITHER side is a text carrier (the
    // cross-class store gate still governs text-vs-numeric).
    if type_reference_is_text_carrier(program, target_type)
        || value_class(program, Some(machine), state, value) == Some(ValueClass::Text)
        || crate::places::declared_place_type(program, machine, state, value)
            .is_some_and(|value_type| type_reference_is_text_carrier(program, value_type))
    {
        return false;
    }
    let Some(value_is_array) = value_shape_is_array(program, machine, state, value) else {
        return false;
    };
    if value_is_array == type_reference_is_array(program, target_type) {
        return false;
    }
    diagnostics.push(Diagnostic::error(if value_is_array {
        format!("{slot_context} binds an ARRAY value into a non-array {slot_noun}")
    } else {
        format!("{slot_context} binds a non-array value into an ARRAY {slot_noun}")
    }));
    true
}

/// Reject binding a SCALAR value into a DATA (struct/enum) target, or a DATA value
/// into a SCALAR target: `self.point = 5` (a scalar into a struct field) silently
/// clobbers the struct's leading bytes, and `let n: i32 = self.point` (a struct into
/// a scalar slot) silently reads a ZII `0`. This cross-shape case fell between the
/// two type gates: the scalar-CLASS gate needs a primitive TARGET (a struct target
/// has none, so it is skipped) and the nominal gate needs BOTH sides to resolve to
/// data names (a scalar does not). Fires only when one side is a PROVABLE scalar
/// (`value_class` / a primitive target) and the other a concrete data type; arrays
/// (owned by the array-shape check), text carriers, and unresolvable computed values
/// are left alone.
pub(crate) fn report_scalar_data_shape_mismatch(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: Option<&omega_typed_trees::state::State>,
    value: ExpressionHandle,
    target_type: TypeReferenceHandle,
    slot_context: &str,
    slot_noun: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    // Scalar VALUE (a bool/number/text literal or a primitive-typed place) into a
    // DATA target.
    if let Some(target_name) = concrete_data_type_name(program, target_type)
        && let Some(value_scalar_class) = value_class(program, Some(machine), state, value)
    {
        diagnostics.push(Diagnostic::error(format!(
            "{slot_context} binds {} into the `{target_name}` data {slot_noun}; a scalar value \
             cannot fill a struct or enum slot",
            value_scalar_class.describe(),
        )));
        return true;
    }
    // DATA VALUE (a struct literal or a data-typed place) into a SCALAR target.
    if program.primitive_type_reference(target_type).is_some()
        && let Some(value_name) = value_concrete_data_name(program, machine, state, value)
    {
        diagnostics.push(Diagnostic::error(format!(
            "{slot_context} binds a `{value_name}` value into a scalar {slot_noun}; a struct or \
             enum value cannot fill a scalar slot",
        )));
        return true;
    }
    false
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
