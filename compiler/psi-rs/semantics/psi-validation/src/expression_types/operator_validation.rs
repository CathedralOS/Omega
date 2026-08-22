use super::value_classification::{ValueClass, concrete_data_type_name, value_class};
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

/// Run every binary-operand TYPE check for a binary expression -- the checks that
/// reject an operator applied to operands it is not defined for. The single entry
/// point for `scan_expression_calls`'s Binary arm: it calls this once, and new
/// operand-type checks are added here (one place), not threaded through the walker.
pub(crate) fn validate_binary_operand_types(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: Option<&psi_typed_trees::state::State>,
    operator: psi_typed_trees::expression::BinaryOperator,
    left: ExpressionHandle,
    right: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    report_cross_class_binary_operands(program, machine, state, left, right, diagnostics);
    report_invalid_text_operator(program, machine, state, operator, left, right, diagnostics);
    report_non_bool_logical_operands(program, machine, state, operator, left, right, diagnostics);
    report_array_operator_operands(program, machine, state, operator, left, right, diagnostics);
    report_undeclared_struct_operator(program, machine, state, operator, left, right, diagnostics);
    report_float_bitwise_operator(program, machine, state, operator, left, right, diagnostics);
    crate::arithmetic_domains::report_out_of_range_comparison_literal(
        program,
        machine,
        state,
        operator,
        left,
        right,
        diagnostics,
    );
    crate::arithmetic_domains::report_mismatched_width_operands(
        program,
        machine,
        state,
        operator,
        left,
        right,
        diagnostics,
    );
}

/// Whether `operand`'s type is a float (`f32`/`f64`): a float literal, or a place
/// whose declared type resolves to a float primitive. Looks through `Mutable`.
pub(super) fn expression_is_float_typed(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: Option<&psi_typed_trees::state::State>,
    operand: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(operand) {
        ExpressionNode::Float(_) => true,
        ExpressionNode::Mutable(inner) => {
            expression_is_float_typed(program, machine, state, *inner)
        }
        ExpressionNode::Binary(binary) => {
            expression_is_float_typed(program, machine, state, binary.left)
                || expression_is_float_typed(program, machine, state, binary.right)
        }
        ExpressionNode::Unary(unary) => {
            expression_is_float_typed(program, machine, state, unary.operand)
        }
        ExpressionNode::Cast(cast) => program
            .primitive_type_reference(cast.target_type)
            .is_some_and(|primitive| primitive.accepts_float_literal()),
        ExpressionNode::Call(call) => {
            crate::arithmetic_domains::call_return_type(program, machine, call)
                .and_then(|return_type| program.primitive_type_reference(return_type))
                .is_some_and(|primitive| primitive.accepts_float_literal())
        }
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
    machine: &psi_typed_trees::machine::Machine,
    state: Option<&psi_typed_trees::state::State>,
    operator: psi_typed_trees::expression::BinaryOperator,
    left: ExpressionHandle,
    right: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    use psi_typed_trees::expression::BinaryOperator;
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
    machine: &psi_typed_trees::machine::Machine,
    state: Option<&psi_typed_trees::state::State>,
    left: ExpressionHandle,
    right: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let (Some(left_class), Some(right_class)) = (
        value_class(program, Some(machine), state, left),
        value_class(program, Some(machine), state, right),
    ) else {
        return false;
    };
    // Any two DIFFERENT value classes mixed in one binary op is an implicit
    // coercion Omega does not perform: a `bool` fed to arithmetic/comparison as its
    // `{0, 1}` value (`self.flag + 5`, `self.flag == self.count`), or text combined
    // with a number. Both are rejected -- write the conversion explicitly. Only a
    // PROVABLE class on each side counts, so a comparison RESULT (value_class None,
    // e.g. `(a == b)`) is NOT a Boolean here: `let n: i32 = (a == b)` (the intended
    // 0/1 coercion of a comparison into a numeric slot) is untouched.
    if left_class == right_class {
        return false;
    }
    let detail =
        if matches!(left_class, ValueClass::Text) || matches!(right_class, ValueClass::Text) {
            "text and non-text operands cannot be compared or combined"
        } else {
            // Boolean vs Numeric: the magic 0/1 coercion modern languages reject.
            "Omega does not coerce a boolean to a number -- compare booleans directly \
             (`b == true`) or convert a number explicitly (`n != 0`)"
        };
    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}` applies an operator to {} and {} -- {detail}",
        machine.name.as_str(),
        state.map(|state| state.name.as_str()).unwrap_or(""),
        left_class.describe(),
        right_class.describe(),
    )));
    true
}

/// Whether a type reference is a TEXT carrier: the `String` primitive, or a fixed
/// array / slice of `u8` (text is `&[u8]`, so a `String`, a byte slice, and a
/// `[u8; N]` are the same shape family and values flow between them). The shape
/// check skips these -- the array-vs-scalar dichotomy does not apply to text.
pub(super) fn type_reference_is_text_carrier(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> bool {
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
        _ => false,
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
    machine: &psi_typed_trees::machine::Machine,
    state: Option<&psi_typed_trees::state::State>,
    operator: psi_typed_trees::expression::BinaryOperator,
    left: ExpressionHandle,
    right: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    use psi_typed_trees::expression::BinaryOperator;
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
    machine: &psi_typed_trees::machine::Machine,
    state: Option<&psi_typed_trees::state::State>,
    operator: psi_typed_trees::expression::BinaryOperator,
    left: ExpressionHandle,
    right: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    use psi_typed_trees::expression::BinaryOperator;
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

/// Reject logical `!` on a NON-bool operand (`!5`, `!x` for `x: i32`). `!` is
/// bool-only in Omega; bitwise-not is the separate `~`. Only a PROVABLY non-bool
/// operand is flagged -- a numeric/text literal, an arithmetic result, or a place
/// whose declared type is a numeric/text primitive (all classify as Numeric/Text).
/// A comparison / logical / call / unresolved operand classifies as None and is
/// allowed, so a real bool (including a bare `a == 1`) is never rejected.
pub(crate) fn report_non_bool_logical_not(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: Option<&psi_typed_trees::state::State>,
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

/// Reject `~` on a definitely non-integer operand. Bitwise complement is total
/// over one fixed-width integer representation and preserves that width; it is
/// neither Boolean negation nor a float/text bit reinterpretation.
pub(crate) fn report_non_integer_bitwise_not(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: Option<&psi_typed_trees::state::State>,
    operand: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let class = value_class(program, Some(machine), state, operand);
    let invalid = matches!(class, Some(ValueClass::Boolean | ValueClass::Text))
        || expression_is_float_typed(program, machine, state, operand);
    if !invalid {
        return false;
    }
    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}` applies bitwise `~` to {}, but `~` requires a fixed-width integer operand",
        machine.name.as_str(),
        state.map(|state| state.name.as_str()).unwrap_or(""),
        class.map_or("a non-integer value", ValueClass::describe),
    )));
    true
}

/// Map a binary operator to its overloadable spelling, or `None` for operators
/// that cannot carry a domain meaning here: `==`/`!=` (the structural-equality /
/// Equatable path owns those), the logical `&&`/`||`, and bitwise/shift (which
/// have no `OperatorSpelling`, so no domain operator can be declared for them).
fn binary_operator_spelling(
    operator: psi_typed_trees::expression::BinaryOperator,
) -> Option<psi_language_core::operator_spelling::OperatorSpelling> {
    use psi_language_core::operator_spelling::OperatorSpelling;
    use psi_typed_trees::expression::BinaryOperator;
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
/// DOMAIN operators (`operator + Quantity::Additive::add ...`),
/// so we ask the use-site authority `resolve_spelling`: an EMPTY candidate set for
/// a concrete-data receiver means the operator is undeclared. Scalars (intrinsic
/// builtins) and arrays are not concrete-data receivers, so they are untouched;
/// when candidates DO exist, admissibility (the proof context) is enforced
/// downstream from static binding selections, so a valid domain op
/// (`Quantity + Quantity`) is never rejected.
fn report_undeclared_struct_operator(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: Option<&psi_typed_trees::state::State>,
    operator: psi_typed_trees::expression::BinaryOperator,
    left: ExpressionHandle,
    right: ExpressionHandle,
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
    if !psi_typed_trees::operator::resolve_spelling(program, spelling, Some(receiver_type))
        .is_empty()
    {
        return false;
    }
    let operand_types = [
        Some(receiver_type),
        crate::places::declared_place_type(program, machine, state, right),
    ];
    if !psi_typed_trees::operator::selected_trait_operator_meanings(
        program,
        machine.symbol,
        spelling,
        &operand_types,
    )
    .is_empty()
    {
        return false;
    }
    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}` applies `{operator:?}` to a `{type_name}` value, but no such \
         operator is declared for it -- only `==`/`!=` (via `{type_name} satisfies Equatable`) \
         or a top-level `operator {type_name}::Domain::name ...` meaning operates on a data type",
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
    machine: &psi_typed_trees::machine::Machine,
    state: Option<&psi_typed_trees::state::State>,
    operator: psi_typed_trees::expression::BinaryOperator,
    left: ExpressionHandle,
    right: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    use psi_typed_trees::expression::BinaryOperator;
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
pub(super) fn type_reference_is_array(program: &TypedTrees, handle: TypeReferenceHandle) -> bool {
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
