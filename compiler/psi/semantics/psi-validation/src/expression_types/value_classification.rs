use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

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
    machine: Option<&psi_typed_trees::machine::Machine>,
    state: Option<&psi_typed_trees::state::State>,
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
        use psi_typed_trees::expression::BinaryOperator;
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
            psi_typed_trees::expression::UnaryOperator::BitwiseNot => Some(ValueClass::Numeric),
            psi_typed_trees::expression::UnaryOperator::LogicalNot => Some(ValueClass::Boolean),
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
    machine: Option<&psi_typed_trees::machine::Machine>,
    state: Option<&psi_typed_trees::state::State>,
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
    machine: Option<&psi_typed_trees::machine::Machine>,
    state: Option<&psi_typed_trees::state::State>,
    value: ExpressionHandle,
    target: PrimitiveType,
    slot_context: &str,
    slot_noun: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if let ExpressionNode::Call(call) = program.expression_table.expression(value)
        && let Some(operator) =
            psi_typed_trees::operator::resolve_named_expression_call(program, call)
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

/// The name of the CONCRETE DATA type a `handle` denotes, looking through
/// `Reference`/`Constrained` shells -- or `None` for anything that is not a plain
/// data type (a primitive, a trait / boundary / platform, a generic type
/// parameter, or an array). The `None` cases are exactly the
/// ones a nominal argument check must NOT flag, so a data value passed to a trait
/// or generic parameter is never a "wrong type".
pub(super) fn concrete_data_type_name(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<&str> {
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
/// type (`self.bar` -> `"Bar"`). `None` for a primitive, array, generic,
/// or unresolvable computed value. The shared resolver behind the nominal checks
/// (`report_data_type_conflict` value-vs-target, `report_cross_type_equality`
/// operand-vs-operand) and the cast-source non-scalar detection.
pub(super) fn value_concrete_data_name<'program>(
    program: &'program TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: Option<&psi_typed_trees::state::State>,
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
/// generic parameter, an array, or a COMPUTED value whose type
/// is unresolved) yields `None` on one side and is skipped -- so this only ever
/// rejects the unambiguous type confusion. The value's type resolves from a
/// struct LITERAL's own type name (`B { .. }`) or, failing that, a PLACE's
/// declared type (`self.bar`). The nominal complement of `report_cross_class_store`.
pub(crate) fn report_data_type_conflict(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: Option<&psi_typed_trees::state::State>,
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
