use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

/// The three disjoint scalar value CLASSES a scalar assignment can conflate.
/// A literal RHS or a resolvable place (`self.field`, a local) names its class
/// unambiguously; a target primitive names one too. Assigning across classes
/// (e.g. a `bool` into an `i32` field) is a type error the backend would
/// otherwise SILENTLY miscompile -- `true` stored as `1`, `"hi"` stored as
/// garbage. We deliberately fold every integer AND float primitive into a
/// single `Numeric` class so that numeric coercions (`f64 = 5`, `i8 = 300`,
/// `i32 = self.i8_field`) are NOT flagged here -- those are the province of the
/// narrowing/domain checks, which carry their own precise diagnostics. This
/// classification describes only cross-class conflicts. The shared store
/// reporter additionally checks exact named-operator results and landed float
/// formats before consulting these classes.
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
            ExpressionNode::Borrow(inner) => Self::of_literal(program, inner.target),
            _ => None,
        }
    }

    fn of_primitive(primitive: PrimitiveType) -> Self {
        match primitive {
            PrimitiveType::Bool => Self::Boolean,
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
pub(super) fn value_class(
    program: &TypedTrees,
    machine: Option<&typed_trees::machine::Machine>,
    state: Option<&typed_trees::state::State>,
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
        use typed_trees::expression::BinaryOperator;
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
    if let ExpressionNode::Unary(unary) = program.expression_table.expression(value) {
        return match unary.operator {
            typed_trees::expression::UnaryOperator::BitwiseNot => Some(ValueClass::Numeric),
            typed_trees::expression::UnaryOperator::LogicalNot => Some(ValueClass::Boolean),
        };
    }
    if let ExpressionNode::Call(call) = program.expression_table.expression(value) {
        let machine = machine?;
        let primitive = crate::arithmetic_domains::call_return_type(program, machine, call)
            .and_then(|handle| program.primitive_type_reference(handle))?;
        return Some(ValueClass::of_primitive(primitive));
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
    machine: Option<&typed_trees::machine::Machine>,
    state: Option<&typed_trees::state::State>,
    value: ExpressionHandle,
    target: PrimitiveType,
) -> Option<(ValueClass, ValueClass)> {
    let value_class = value_class(program, machine, state, value)?;
    let target_class = ValueClass::of_primitive(target);
    (value_class != target_class).then_some((value_class, target_class))
}

/// Report a scalar destination conflict and return `true`, or return `false`
/// when the known representation and class agree. Exact named-operator result
/// types and landed float formats are checked before broad scalar classes.
/// `slot_context` names the store site (e.g. ``"argument `x` for state `s`"``,
/// ``"construction of `Point` field `x`"``, `"array literal element"`) and
/// `slot_noun` its kind (`"place"` / `"parameter"` / `"field"` / `"element"`).
/// SINGLE SOURCE OF TRUTH for the cross-class store diagnostic across every store
/// position -- the class complement of `arithmetic_domains::check_value_narrowing`.
pub(crate) fn report_cross_class_store(
    program: &TypedTrees,
    machine: Option<&typed_trees::machine::Machine>,
    state: Option<&typed_trees::state::State>,
    value: ExpressionHandle,
    target: PrimitiveType,
    slot_context: &str,
    slot_noun: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if let ExpressionNode::Match(dispatch) = program.expression_table.expression(value) {
        let mut rejected = false;
        for arm in program.expression_table.match_arms(dispatch.arms) {
            rejected |= report_cross_class_store(
                program,
                machine,
                state,
                arm.value,
                target,
                slot_context,
                slot_noun,
                diagnostics,
            );
        }
        return rejected;
    }
    if let Some(machine) = machine
        && let Some(actual) =
            crate::builtin_constant_array_projection_type(program, machine.symbol, value)
        && let Some(actual) = program.primitive_type_reference(actual)
        && actual != target
    {
        diagnostics.push(Diagnostic::error(format!(
            "{slot_context} stores a constant array projection of type `{}` into a `{}` {slot_noun}; use an explicit conversion",
            actual.name(), target.name(),
        )));
        return true;
    }
    if let ExpressionNode::Call(call) = program.expression_table.expression(value)
        && let Some(operator) = typed_trees::operator::resolve_named_expression_call(program, call)
        && let Some(source) = program.primitive_type_reference(operator.return_type)
        && source != target
    {
        diagnostics.push(Diagnostic::error(format!(
            "{slot_context} stores the `{}` result of named operator `{}` into a `{}` \
             {slot_noun}; numeric representation changes require an explicit named conversion",
            source.name(),
            call.target,
            target.name(),
        )));
        return true;
    }

    if super::float_destinations::report_mismatch(
        program,
        machine,
        state,
        value,
        target,
        slot_context,
        slot_noun,
        diagnostics,
    ) {
        return true;
    }

    let Some((value_class, target_class)) =
        cross_class_conflict(program, machine, state, value, target)
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

/// The exact declaration of the concrete data type a `handle` denotes, looking through
/// `Reference`/`Constrained` shells -- or `None` for anything that is not a plain
/// data type (a primitive, a trait / boundary / platform, a generic type
/// parameter, or an array). The `None` cases are exactly the
/// ones a nominal argument check must NOT flag, so a data value passed to a trait
/// or generic parameter is never a "wrong type".
fn concrete_data_type_symbol(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<symbols::SymbolHandle> {
    if !handle.is_valid() {
        return None;
    }
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => {
            concrete_data_type_symbol(program, *referee)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            concrete_data_type_symbol(program, *base_type)
        }
        TypeReferenceNode::Named { symbol, .. } if symbol.is_valid() => program
            .data_definitions()
            .iter()
            .any(|definition| definition.symbol == *symbol)
            .then_some(*symbol),
        _ => None,
    }
}

pub(super) fn concrete_data_type_name(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<&str> {
    let symbol = concrete_data_type_symbol(program, handle)?;
    program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == symbol)
        .map(|definition| definition.name.as_str())
}

/// A constructor's resolved declaration is value identity, independent of the
/// use site's spelling. Payloadless cases retain their exact variant parent.
/// Open generic constructions remain with their existing application checker.
fn constructed_data_symbol(
    program: &TypedTrees,
    value: ExpressionHandle,
) -> Option<symbols::SymbolHandle> {
    let symbol = match program.expression_table.expression(value) {
        ExpressionNode::StructLiteral(literal) => literal.type_symbol,
        ExpressionNode::Name(path)
            if program.symbols.get(path.symbol).kind == symbols::SymbolKind::Variant =>
        {
            program.symbols.get(path.symbol).parent
        }
        ExpressionNode::Borrow(inner) => return constructed_data_symbol(program, inner.target),
        _ => return None,
    };
    program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == symbol && definition.type_parameters.is_empty())
        .map(|definition| definition.symbol)
}

fn value_concrete_data_symbol(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: Option<&typed_trees::state::State>,
    value: ExpressionHandle,
) -> Option<symbols::SymbolHandle> {
    constructed_data_symbol(program, value)
        .or_else(|| {
            crate::places::declared_place_type(program, machine, state, value)
                .and_then(|reference| concrete_data_type_symbol(program, reference))
        })
        .or_else(|| {
            let ExpressionNode::Call(call) = program.expression_table.expression(value) else {
                return None;
            };
            crate::calls::resolved_call_result_type(program, call)
                .and_then(|reference| concrete_data_type_symbol(program, reference))
        })
}

/// Names remain diagnostic output; nominal comparisons retain selected symbols
/// across literal materialization, local storage and resolved call results.
pub(super) fn value_concrete_data_name<'program>(
    program: &'program TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: Option<&typed_trees::state::State>,
    value: ExpressionHandle,
) -> Option<&'program str> {
    let symbol = value_concrete_data_symbol(program, machine, state, value)?;
    program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == symbol)
        .map(|definition| definition.name.as_str())
}

/// Compare selected nominal declarations at a receiving position. Names and
/// layouts cannot establish equality across source modules. Concrete literals,
/// stored places, and resolved call results retain their actual owner; open
/// generic and unresolved result paths remain with their existing checkers.
pub(crate) fn report_data_type_conflict(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: Option<&typed_trees::state::State>,
    value: ExpressionHandle,
    expected_type: TypeReferenceHandle,
    slot_context: &str,
    slot_noun: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if let ExpressionNode::Match(dispatch) = program.expression_table.expression(value) {
        let mut rejected = false;
        for arm in program.expression_table.match_arms(dispatch.arms) {
            rejected |= report_data_type_conflict(
                program,
                machine,
                state,
                arm.value,
                expected_type,
                slot_context,
                slot_noun,
                diagnostics,
            );
        }
        return rejected;
    }
    let Some(expected) = concrete_data_type_symbol(program, expected_type) else {
        return false;
    };
    let Some(got) = value_concrete_data_symbol(program, machine, state, value) else {
        return false;
    };
    if expected == got {
        return false;
    }
    let expected = program.symbols.display_path(expected, "::");
    let got = program.symbols.display_path(got, "::");
    diagnostics.push(Diagnostic::error(format!(
        "{slot_context} expects the `{expected}` data type but got `{got}` in the `{slot_noun}` \
         position; these are incompatible data types (a place is accepted structurally, but its \
         declared type must match)",
    )));
    true
}
