use psi_diagnostics::Diagnostic;
use psi_numerics::arithmetic::ArithmeticDomain;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCastExpression};
use psi_typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use std::fmt;

mod float_cast_proofs;
mod value_classification;

use float_cast_proofs::float_to_integer_cast_is_proven;

pub(crate) use value_classification::{
    ValueClass, report_cross_class_store, report_data_type_conflict,
};
use value_classification::{concrete_data_type_name, value_class, value_concrete_data_name};

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
            // A literal may directly establish an owned bounded text carrier
            // (`[u8; N] in Utf8`) in argument and terminal-result positions,
            // just as it already can in field/local writes. Do this before
            // erasing the value-domain constraint: the unconstrained base is
            // an always-full fixed array, which correctly does NOT accept a
            // text literal by itself.
            (matches!(argument_node, ExpressionNode::String(literal)
                if bounded_byte_buffer_capacity(program, type_reference)
                    .is_some_and(|capacity| literal.as_bytes().len() <= capacity))
                || argument_matches_type_reference_handle(program, argument, *base_type))
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
                    || matches!(argument_node, ExpressionNode::Float(_))
                        && primitive_type.accepts_float_literal()
                    || matches!(argument_node, ExpressionNode::Integer(_))
                        && primitive_type.accepts_integer_literal()
                    || matches!(argument_node, ExpressionNode::Unary(unary)
                    if match unary.operator {
                        psi_typed_trees::expression::UnaryOperator::BitwiseNot => {
                            primitive_type.accepts_integer_literal()
                        }
                        psi_typed_trees::expression::UnaryOperator::LogicalNot => {
                            primitive_type == PrimitiveType::Bool
                        }
                    })
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
        TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Unit => false,
    }
}

/// Mirror the backend layout classifier for an owned variable-fill text
/// carrier. A named value domain changes `[u8; N]` from an always-full fixed
/// array into `{len, bytes[N]}`; layout-policy domains do not.
fn bounded_byte_buffer_capacity(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<usize> {
    let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    let has_value_domain = program
        .type_reference_table
        .constraints(*constraints)
        .iter()
        .any(|constraint| match constraint {
            TypeConstraintNode::Domain(name) => {
                !psi_typed_trees::wire::is_layout_domain_constraint(name)
                    && psi_language_semantics::CarryPermission::from_name(name.as_str()).is_none()
            }
            _ => false,
        });
    if !has_value_domain {
        return None;
    }
    let TypeReferenceNode::FixedArray {
        element_type,
        length,
    } = program.type_reference_table.type_reference(*base_type)
    else {
        return None;
    };
    if program.type_reference_table.primitive_type(*element_type) != Some(PrimitiveType::U8) {
        return None;
    }
    match length {
        psi_typed_trees::types::FixedArrayLength::Literal(capacity) => Some(*capacity),
        psi_typed_trees::types::FixedArrayLength::ConstParameter { .. }
        | psi_typed_trees::types::FixedArrayLength::ConstCall { .. } => None,
    }
}

pub(crate) fn validate_expression_type_handle(
    program: &TypedTrees,
    expression: ExpressionHandle,
    type_reference: TypeReferenceHandle,
    diagnostics: &mut Vec<Diagnostic>,
    owner: ExpressionTypeOwner<'_>,
) {
    if let ExpressionNode::String(literal) = program.expression_table.expression(expression)
        && let Some(capacity) = bounded_byte_buffer_capacity(program, type_reference)
        && literal.as_bytes().len() > capacity
    {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} constructs {} byte(s), exceeding the {capacity}-byte capacity of `{}`",
            literal.as_bytes().len(),
            program.display_type_reference_with_constraints(type_reference),
        )));
        return;
    }
    if !argument_matches_type_reference_handle(program, expression, type_reference) {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} expects `{}`, got `{}`",
            program.display_type_reference_with_constraints(type_reference),
            expression_type_name_handle(program, expression)
        )));
    }
}

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

/// Validate an `as` cast -- the single entry point for the Cast arm of
/// `scan_expression_calls` (mirrors `validate_binary_operand_types`). The TARGET
/// must be a scalar primitive (`x as Foo` / `x as Bogus` otherwise lowers as a
/// silent identity no-op), and the SOURCE must be convertible to it: no `<number>
/// as bool` (which yields a non-`{0, 1}` bool) and no text / struct / array source
/// for a numeric/address target (which reinterprets bytes to garbage).
pub(crate) fn validate_cast_types(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: Option<&psi_typed_trees::state::State>,
    cast: &TableCastExpression,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // §5b recasts (`&x as &T`) are judged WHOLE by `recasts::validate_recasts`
    // (size/facts/position); the value-cast rules below do not apply to a
    // re-view.
    if cast.form.is_recast() {
        return;
    }
    crate::type_references::validate_indexed_qualification_arguments(
        program,
        machine,
        cast,
        diagnostics,
    );
    let target_name = program.named_type_reference(cast.target_type);
    // Uniform content embedding returns proof Int. Its carrier-derived range
    // is nonnegative for the closed u8/u16/u32/u64/addr inputs admitted by the
    // content projection validator, so the explicit exact `as Nat` conversion
    // is legal and remains proof-only. No general data-to-data cast is opened.
    if target_name.is_some_and(|target| target.as_str() == "Nat")
        && cast.domain == ArithmeticDomain::Exact
        && cast.semantic_domain.count() == 0
        && matches!(program.expression_table.expression(cast.value),
            ExpressionNode::Call(call)
                if call.target.as_str().rsplit("::").next() == Some("embed"))
    {
        return;
    }
    // N6 quotient mint: `carrier as Quotient` is not a scalar conversion.
    // It introduces an equivalence class while retaining no representative,
    // and is legal only from the quotient's exact carrier family.
    if let Some(target_name) = target_name
        && let Some(quotient_data) = program.data_definitions().iter().find(|definition| {
            definition.name.as_str() == target_name.as_str() && definition.quotient.is_some()
        })
    {
        validate_quotient_mint(program, machine, state, cast, quotient_data, diagnostics);
        return;
    }
    // A non-scalar `as` is legal only as a representation-preserving cast to
    // the same data carrier. This is the explicit erasure surface for a
    // non-owning semantic/provenance qualification (`issued as Token`): the
    // runtime value is unchanged, while the cast's deliberately bare target
    // controls which static domain atoms survive. Arbitrary struct-to-struct
    // conversion remains rejected.
    if let Some(target) = target_name
        && PrimitiveType::from_name(target.as_str()).is_none()
    {
        if base_data_symbol(program, cast.target_type).is_some_and(|target| {
            expression_data_symbol(program, machine, state, cast.value) == Some(target)
        }) {
            return;
        }
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` casts with `as {target}`, but `{target}` is not a scalar \
             type; non-scalar `as` only erases qualifications from the same data carrier",
            machine.name.as_str(),
            state.map(|state| state.name.as_str()).unwrap_or(""),
        )));
    }
    // Source-side checks keyed by the target primitive.
    match target_name.and_then(|target| PrimitiveType::from_name(target.as_str())) {
        Some(PrimitiveType::Bool) => {
            report_number_to_bool_cast(program, machine, state, cast.value, diagnostics);
        }
        // Every scalar target except `bool` is a numeric/address type that only
        // accepts a numeric/bool scalar source.
        Some(primitive) => {
            report_invalid_numeric_cast_source(
                program,
                machine,
                state,
                cast.value,
                primitive.name(),
                diagnostics,
            );
        }
        _ => {}
    }

    if cast.domain == ArithmeticDomain::Wrapping
        && expression_is_float_typed(program, machine, state, cast.value)
        && target_name
            .and_then(|target| PrimitiveType::from_name(target.as_str()))
            .is_some_and(|target| target.accepts_integer_literal())
    {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` casts a float to `{}` in `Wrapping`, but floats have no \
             modular conversion semantics; use an Exact cast with a proven finite/in-range \
             operand, `in Saturating`, or `in Trapping`",
            machine.name.as_str(),
            state.map(|state| state.name.as_str()).unwrap_or(""),
            target_name.map(|name| name.as_str()).unwrap_or("integer"),
        )));
    }

    if cast.domain == ArithmeticDomain::Exact
        && expression_is_float_typed(program, machine, state, cast.value)
        && let Some(target) = target_name
            .and_then(|target| PrimitiveType::from_name(target.as_str()))
            .filter(|target| target.accepts_integer_literal())
        && !float_to_integer_cast_is_proven(program, machine, state, cast.value, target)
    {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` cannot prove Exact float-to-`{}` cast operand `{}` is \
             finite and in range; constrain the float with a declared range or a dominating \
             guard, or select `in Saturating`/`in Trapping`",
            machine.name.as_str(),
            state.map(|state| state.name.as_str()).unwrap_or(""),
            target.name(),
            program.expression_table.display_name(cast.value),
        )));
    }
}

fn validate_quotient_mint(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: Option<&psi_typed_trees::state::State>,
    cast: &TableCastExpression,
    quotient_data: &psi_typed_trees::data::DataDefinition,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let quotient = quotient_data
        .quotient
        .as_ref()
        .expect("quotient target was selected by quotient metadata");
    let expected = base_data_symbol(program, quotient.carrier);
    let actual = expression_data_symbol(program, machine, state, cast.value);
    if expected.is_none() || actual != expected {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` constructs quotient `{}` from `{}`, but quotient construction is carrier-only: expected `{}`",
            machine.name,
            state.map(|state| state.name.as_str()).unwrap_or(""),
            quotient_data.name,
            expression_type_name_handle(program, cast.value),
            program.display_type_reference_with_constraints(quotient.carrier),
        )));
    }
    if cast.semantic_domain.count() != 0 || cast.domain != ArithmeticDomain::Exact {
        diagnostics.push(Diagnostic::error(format!(
            "quotient construction `as {}` cannot carry an arithmetic or semantic-domain policy suffix",
            quotient_data.name,
        )));
    }
}

fn base_data_symbol(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<psi_symbols::SymbolHandle> {
    if !type_reference.is_valid() {
        return None;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
        TypeReferenceNode::Generic { base_symbol, .. } => Some(*base_symbol),
        TypeReferenceNode::Constrained { base_type, .. } => base_data_symbol(program, *base_type),
        _ => None,
    }
}

fn expression_data_symbol(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: Option<&psi_typed_trees::state::State>,
    expression: ExpressionHandle,
) -> Option<psi_symbols::SymbolHandle> {
    if let Some(type_reference) =
        crate::places::declared_place_type(program, machine, state, expression).or_else(|| {
            match program.expression_table.expression(expression) {
                ExpressionNode::Call(call) => {
                    crate::arithmetic_domains::call_return_type(program, machine, call)
                }
                _ => None,
            }
        })
    {
        return base_data_symbol(program, type_reference);
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::StructLiteral(literal) => program
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == literal.type_name.as_str())
            .map(|definition| definition.symbol),
        ExpressionNode::Mutable(inner) => expression_data_symbol(program, machine, state, *inner),
        ExpressionNode::Atomic(atomic) => {
            expression_data_symbol(program, machine, state, atomic.value)
        }
        ExpressionNode::Cast(cast) => base_data_symbol(program, cast.target_type),
        _ => None,
    }
}

/// Whether `operand`'s type is a float (`f32`/`f64`): a float literal, or a place
/// whose declared type resolves to a float primitive. Looks through `Mutable`.
fn expression_is_float_typed(
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

/// Reject `<number/text> as bool` (`5 as bool`). Such a cast reinterprets the
/// source bits into a `bool` without normalizing to `{0, 1}`, producing an INVALID
/// bool (`5 as bool` yields a bool holding 5). `as` has no meaningful number->bool
/// conversion -- write an explicit comparison (`n != 0`). Only a PROVABLY non-bool
/// source (Numeric/Text) is flagged; a comparison / logical / call source (None) or
/// a `bool` source is allowed, so `(a == 1) as bool` and `b as bool` stay fine.
/// (Same no-int-in-bool principle as `report_non_bool_logical_not`.)
fn report_number_to_bool_cast(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: Option<&psi_typed_trees::state::State>,
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
fn report_invalid_numeric_cast_source(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: Option<&psi_typed_trees::state::State>,
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
    machine: &psi_typed_trees::machine::Machine,
    state: Option<&psi_typed_trees::state::State>,
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
    machine: &psi_typed_trees::machine::Machine,
    state: Option<&psi_typed_trees::state::State>,
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
    machine: &psi_typed_trees::machine::Machine,
    state: Option<&psi_typed_trees::state::State>,
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
        ExpressionNode::Atomic(atomic) => expression_type_name_handle(program, atomic.value),
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
        ExpressionNode::Unary(unary) => match unary.operator {
            psi_typed_trees::expression::UnaryOperator::BitwiseNot => "integer",
            psi_typed_trees::expression::UnaryOperator::LogicalNot => "bool",
        },
        ExpressionNode::ZeroValue(_) => "zero-value representation observation",
    }
}
