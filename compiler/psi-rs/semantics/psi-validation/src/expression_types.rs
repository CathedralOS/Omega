use psi_diagnostics::Diagnostic;
use psi_numerics::arithmetic::ArithmeticDomain;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCastExpression};
use psi_typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use std::fmt;

mod float_cast_proofs;
mod operator_validation;
mod shape_validation;
mod value_classification;

use float_cast_proofs::float_to_integer_cast_is_proven;

use operator_validation::expression_is_float_typed;
pub(crate) use operator_validation::{
    report_non_bool_logical_not, report_non_integer_bitwise_not, validate_binary_operand_types,
};

use shape_validation::value_shape_is_array;
pub(crate) use shape_validation::{
    report_array_scalar_shape_mismatch, report_scalar_data_shape_mismatch,
};

pub(crate) use value_classification::{
    ValueClass, report_cross_class_store, report_data_type_conflict,
};
use value_classification::{value_class, value_concrete_data_name};

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
