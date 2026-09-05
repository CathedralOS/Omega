use super::expression_type_name_handle;
use super::float_cast_proofs::float_to_integer_cast_is_proven;
use super::operator_validation::expression_is_float_typed;
use super::shape_validation::value_shape_is_array;
use super::value_classification::{ValueClass, value_class, value_concrete_data_name};
use diagnostics::Diagnostic;
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCastExpression};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

/// Validate an `as` cast -- the single entry point for the Cast arm of
/// `scan_expression_calls` (mirrors `validate_binary_operand_types`). The TARGET
/// must be a scalar primitive (`x as Foo` / `x as Bogus` otherwise lowers as a
/// silent identity no-op), and the SOURCE must be convertible to it: no `<number>
/// as bool` (which yields a non-`{0, 1}` bool) and no text / struct / array source
/// for a numeric/address target (which reinterprets bytes to garbage).
pub(crate) fn validate_cast_types(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: Option<&typed_trees::state::State>,
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
    if crate::contract_entailment::proof_nat_cast(program, cast) {
        let hypotheses: Vec<_> = program
            .machine_contracts(machine)
            .iter()
            .chain(
                state
                    .into_iter()
                    .flat_map(|state| program.state_contracts(state)),
            )
            .filter(|contract| {
                contract.kind == typed_trees::signature::SignatureContractKind::Requires
            })
            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
            .filter_map(|fact| match fact {
                typed_trees::domain::ProofFact::Expression(expression) => Some(*expression),
                _ => None,
            })
            .collect();
        // This entry point scans executable-looking bodies, not contracts.
        // Ordered contract formation is handled by total_specification.
        if !crate::contract_entailment::proof_integer_nonnegative(program, cast.value, &hypotheses)
        {
            diagnostics.push(Diagnostic::error(
                "Exact proof Int-to-Nat conversion requires a previously proven nonnegative value",
            ));
        }
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
    machine: &typed_trees::machine::Machine,
    state: Option<&typed_trees::state::State>,
    cast: &TableCastExpression,
    quotient_data: &typed_trees::data::DataDefinition,
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
) -> Option<symbols::SymbolHandle> {
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
    machine: &typed_trees::machine::Machine,
    state: Option<&typed_trees::state::State>,
    expression: ExpressionHandle,
) -> Option<symbols::SymbolHandle> {
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
        ExpressionNode::Borrow(inner) => {
            expression_data_symbol(program, machine, state, inner.target)
        }
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
    machine: &typed_trees::machine::Machine,
    state: Option<&typed_trees::state::State>,
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
    machine: &typed_trees::machine::Machine,
    state: Option<&typed_trees::state::State>,
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
