//! Symbolic value bounds use immutable, exact current-machine const binders.
//! No selected application supplies premises for the retained generic body.

use super::*;
use typed_trees::data::{TypeParameter, TypeParameterKind};
use typed_trees::expression::StaticMachineArgument;
use typed_trees::state::State;
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

#[cfg(test)]
mod tests;

fn binder<'program>(
    program: &TypedTrees,
    parameters: &'program [TypeParameter],
    expression: ExpressionHandle,
) -> Option<&'program TypeParameter> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    if !path.symbol.is_valid()
        || path.head_symbol != path.symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
    {
        return None;
    }
    parameters.iter().find(|parameter| {
        parameter.symbol == path.symbol
            && matches!(parameter.kind, TypeParameterKind::Const { type_reference }
                if program.primitive_type_reference(type_reference)
                    .is_some_and(|primitive| primitive.accepts_integer_literal()))
    })
}

pub(crate) fn const_range_bound_is_supported(
    program: &TypedTrees,
    parameters: &[TypeParameter],
    expression: ExpressionHandle,
) -> bool {
    crate::closed_integer_range_bound(program, expression).is_some()
        || binder(program, parameters, expression).is_some()
}

fn term(
    program: &TypedTrees,
    parameters: &[TypeParameter],
    engine: &mut Engine<'_>,
    expression: ExpressionHandle,
) -> Option<Polynomial> {
    if let Some(value) = crate::closed_integer_range_bound(program, expression) {
        return Some(Polynomial::constant(value));
    }
    binder(program, parameters, expression)?;
    engine.normalize(expression)
}

fn inclusive_maximum(value: Polynomial, end_inclusive: bool) -> Polynomial {
    if end_inclusive {
        value
    } else {
        value.sub(&Polynomial::constant(BigInt::from_i64(1)))
    }
}

/// Returns `None` only when there is no symbolic integer range to enforce.
/// Unsupported values are not interpreted mathematically: executable formation
/// and ordinary narrowing remain the arithmetic-domain owner's responsibility.
pub(crate) fn symbolic_range_contains(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    return_type: TypeReferenceHandle,
    value: ExpressionHandle,
) -> Option<bool> {
    if let TypeReferenceNode::Reference { referee, .. } =
        program.type_reference_table.type_reference(return_type)
    {
        return symbolic_range_contains(program, machine, state, *referee, value);
    }
    if !program
        .primitive_type_reference(return_type)
        .is_some_and(|primitive| primitive.accepts_integer_literal())
    {
        return None;
    }
    let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(return_type)
    else {
        return None;
    };
    let parameters = program.machine_type_parameters(machine);
    let mut ranges = Vec::new();
    for constraint in program.type_reference_table.constraints(*constraints) {
        if let TypeConstraintNode::Range {
            minimum,
            maximum,
            end_inclusive,
        } = constraint
            && [*minimum, *maximum].into_iter().any(|endpoint| {
                program.machines().iter().any(|owner| {
                    binder(program, program.machine_type_parameters(owner), endpoint).is_some()
                })
            })
        {
            ranges.push((*minimum, *maximum, *end_inclusive));
        }
    }
    let inherited = symbolic_range_contains(program, machine, state, *base_type, value);
    if ranges.is_empty() {
        return inherited;
    }
    if inherited == Some(false) {
        return Some(false);
    }
    let Some(mut engine) = scope_engine(program, machine) else {
        return Some(false);
    };
    let Some([value_minimum, value_maximum]) =
        value_bounds(program, machine, state, &mut engine, value)
    else {
        return Some(false);
    };
    for (minimum, maximum, end_inclusive) in ranges {
        let (Some(minimum), Some(maximum)) = (
            term(program, parameters, &mut engine, minimum),
            term(program, parameters, &mut engine, maximum),
        ) else {
            return Some(false);
        };
        if !contains(
            &engine,
            &[minimum, inclusive_maximum(maximum, end_inclusive)],
            &[value_minimum.clone(), value_maximum.clone()],
        ) {
            return Some(false);
        }
    }
    Some(true)
}

fn scope_engine<'program>(
    program: &'program TypedTrees,
    machine: &Machine,
) -> Option<Engine<'program>> {
    let mut bindings = Vec::new();
    let mut premises = Vec::new();
    for parameter in program.machine_type_parameters(machine) {
        if let TypeParameterKind::Const { type_reference } = parameter.kind
            && let Some(primitive) = program.primitive_type_reference(type_reference)
            && primitive.accepts_integer_literal()
        {
            let identity = format!("const:{:?}", parameter.symbol);
            if let Some((minimum, maximum)) =
                crate::arithmetic_domains::enforced_integer_type_bounds(program, type_reference)
            {
                let atom = Polynomial::atom(identity.clone());
                premises.push((
                    BinaryOperator::LessOrEqual,
                    Polynomial::constant(BigInt::from_i64(minimum)),
                    atom.clone(),
                ));
                premises.push((
                    BinaryOperator::LessOrEqual,
                    atom,
                    Polynomial::constant(BigInt::from_i64(maximum)),
                ));
            }
            bindings.push(StrictArithmeticSymbolBinding {
                symbol: parameter.symbol,
                value: StrictArithmeticBindingValue::Atom {
                    identity,
                    unsigned: matches!(
                        primitive,
                        typed_trees::types::PrimitiveType::U8
                            | typed_trees::types::PrimitiveType::U16
                            | typed_trees::types::PrimitiveType::U32
                            | typed_trees::types::PrimitiveType::U64
                    ),
                },
            });
        }
    }
    let mut engine = Engine::strict_with_symbol_bindings(program, machine, &bindings);
    if !engine.strict_symbol_bindings_are_valid() || !engine.install_hypotheses(premises) {
        return None;
    }
    Some(engine)
}

fn contains(engine: &Engine<'_>, required: &[Polynomial; 2], actual: &[Polynomial; 2]) -> bool {
    // An explicitly empty interval supplies no possible runtime value, even
    // when endpoint inequalities alone would make a containment check vacuous.
    if [required, actual].into_iter().any(|bounds| {
        bounds[1]
            .sub(&bounds[0])
            .constant_value()
            .is_some_and(|width| width < BigInt::zero())
    }) {
        return false;
    }
    engine.prove_at_least(&actual[0].sub(&required[0]), &BigInt::zero())
        && engine.prove_at_least(&required[1].sub(&actual[1]), &BigInt::zero())
}

fn value_type(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    value: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    crate::places::declared_place_type_raw(program, machine, state, value)
}

fn unreferenced(program: &TypedTrees, mut reference: TypeReferenceHandle) -> TypeReferenceHandle {
    while let TypeReferenceNode::Reference { referee, .. } =
        program.type_reference_table.type_reference(reference)
    {
        reference = *referee;
    }
    reference
}

fn has_const_range(
    program: &TypedTrees,
    parameters: &[TypeParameter],
    reference: TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Reference { referee, .. } => has_const_range(program, parameters, *referee),
        TypeReferenceNode::Constrained { base_type, constraints } => {
            program.primitive_type_reference(*base_type).is_some_and(|primitive| primitive.accepts_integer_literal())
                && (program.type_reference_table.constraints(*constraints).iter().any(|constraint| {
                    matches!(constraint, TypeConstraintNode::Range { minimum, maximum, .. }
                        if [*minimum, *maximum].into_iter().any(|endpoint| binder(program, parameters, endpoint).is_some()))
                }) || has_const_range(program, parameters, *base_type))
        }
        _ => false,
    }
}

fn value_bounds(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    engine: &mut Engine<'_>,
    value: ExpressionHandle,
) -> Option<[Polynomial; 2]> {
    if let Some(value) = term(
        program,
        program.machine_type_parameters(machine),
        engine,
        value,
    ) {
        return Some([value.clone(), value]);
    }
    if let ExpressionNode::Call(call) = program.expression_table.expression(value) {
        let (callee, entry) =
            crate::transitions::resolved_transition_target_state(program, call.target_symbol)?;
        let bindings = call_bindings(
            program,
            machine,
            state,
            engine,
            callee,
            entry,
            &call.machine_arguments,
            program.expression_table.expression_handles(call.arguments),
        )?;
        let (_, endpoints, end_inclusive) =
            crate::declared_integer_range(program, entry.return_type)?;
        return Some([
            selected_term(program, engine, machine, endpoints[0], &bindings)?,
            inclusive_maximum(
                selected_term(program, engine, machine, endpoints[1], &bindings)?,
                end_inclusive,
            ),
        ]);
    }
    let reference = unreferenced(program, value_type(program, machine, state, value)?);
    if let Some((_, endpoints, end_inclusive)) = crate::declared_integer_range(program, reference) {
        return Some([
            term(
                program,
                program.machine_type_parameters(machine),
                engine,
                endpoints[0],
            )?,
            inclusive_maximum(
                term(
                    program,
                    program.machine_type_parameters(machine),
                    engine,
                    endpoints[1],
                )?,
                end_inclusive,
            ),
        ]);
    }
    let (minimum, maximum) =
        crate::arithmetic_domains::enforced_integer_type_bounds(program, reference)?;
    Some([
        Polynomial::constant(BigInt::from_i64(minimum)),
        Polynomial::constant(BigInt::from_i64(maximum)),
    ])
}

/// A call's own selected const tuple closes its declared bounds. Destination
/// annotations never supply a missing argument or improve the returned range.
pub(crate) fn selected_const_call_result_bounds(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> Option<(TypeReferenceHandle, i64, i64)> {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return None;
    };
    if call.receiver.is_valid()
        || call.static_requirement_dispatch.is_some()
        || call.quotient_operation.is_some()
        || call.private_layout_operation.is_some()
    {
        return None;
    }
    let (callee, entry) =
        crate::transitions::resolved_transition_target_state(program, call.target_symbol)?;
    if callee.attached_data.is_some()
        || program
            .state_parameters(entry)
            .iter()
            .any(|parameter| parameter.is_self)
    {
        return None;
    }
    let mut engine = scope_engine(program, machine)?;
    let [minimum, maximum] = value_bounds(program, machine, state, &mut engine, expression)?;
    let minimum = minimum.constant_value()?.to_i64()?;
    let maximum = maximum.constant_value()?.to_i64()?;
    (minimum <= maximum).then_some((entry.return_type, minimum, maximum))
}

fn selected_term(
    program: &TypedTrees,
    engine: &mut Engine<'_>,
    caller: &Machine,
    expression: ExpressionHandle,
    bindings: &[(SymbolHandle, Polynomial)],
) -> Option<Polynomial> {
    if let ExpressionNode::Name(path) = program.expression_table.expression(expression)
        && path.symbol.is_valid()
        && path.head_symbol == path.symbol
        && program
            .expression_table
            .name_path_members(path.members)
            .len()
            == 1
        && let Some((_, value)) = bindings.iter().find(|(symbol, _)| *symbol == path.symbol)
    {
        return Some(value.clone());
    }
    term(
        program,
        program.machine_type_parameters(caller),
        engine,
        expression,
    )
}

fn static_const_value(
    program: &TypedTrees,
    caller: &Machine,
    argument: &StaticMachineArgument,
) -> Option<Option<Polynomial>> {
    if argument.application.is_some() || argument.evidence_projection.is_some() {
        return (argument.const_literal.is_some()
            || program.const_declarations().iter().any(|declaration| {
                argument.symbol.is_valid() && declaration.symbol == argument.symbol
            })
            || program
                .machine_type_parameters(caller)
                .iter()
                .any(|parameter| {
                    parameter.symbol == argument.symbol
                        && matches!(parameter.kind, TypeParameterKind::Const { .. })
                }))
        .then_some(None);
    }
    if let Some(literal) = &argument.const_literal {
        return Some(literal.value_bignum().map(Polynomial::constant));
    }
    if let Some(parameter) = program
        .machine_type_parameters(caller)
        .iter()
        .find(|parameter| argument.symbol.is_valid() && parameter.symbol == argument.symbol)
        && let TypeParameterKind::Const { type_reference } = parameter.kind
    {
        return Some(
            program
                .primitive_type_reference(type_reference)
                .filter(|primitive| primitive.accepts_integer_literal())
                .map(|_| Polynomial::atom(format!("const:{:?}", parameter.symbol))),
        );
    }
    use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
    if !argument.symbol.is_valid()
        && matches!(argument.path.as_ref(), [name] if matches!(name.as_str(), "true" | "false"))
    {
        return Some(None);
    }
    if argument.symbol.is_valid()
        && program.machines().iter().any(|machine| {
            program
                .machine_type_parameters(machine)
                .iter()
                .any(|parameter| {
                    parameter.symbol == argument.symbol
                        && matches!(parameter.kind, TypeParameterKind::Const { .. })
                })
        })
    {
        // A foreign binder occupies its explicit slot but supplies no caller
        // value. Inference may not overwrite the unresolved authored argument.
        return Some(None);
    }
    if !argument.symbol.is_valid()
        && let [name] = argument.path.as_ref()
        && let Some(value) = CanonicalConstValue::from_atom(name.as_str())
    {
        return Some(match value.decode_encoding() {
            Some(DecodedCanonicalConstValue::Integer { value, .. }) => {
                Some(Polynomial::constant(BigInt::from_i128(value)))
            }
            _ => None,
        });
    }
    let declaration = program
        .const_declarations()
        .iter()
        .find(|declaration| argument.symbol.is_valid() && declaration.symbol == argument.symbol)?;
    let value = declaration
        .canonical_value_encoding
        .as_ref()
        .and_then(|encoding| {
            CanonicalConstValue::new(
                program.display_type_reference(declaration.declared_type),
                encoding.clone(),
                argument.display_name(),
            )
            .decode_encoding()
        });
    Some(match value {
        Some(DecodedCanonicalConstValue::Integer { value, .. }) => {
            Some(Polynomial::constant(BigInt::from_i128(value)))
        }
        _ => None,
    })
}

#[allow(clippy::too_many_arguments)]
fn call_bindings(
    program: &TypedTrees,
    caller: &Machine,
    state: Option<&State>,
    engine: &mut Engine<'_>,
    callee: &Machine,
    entry: &State,
    selections: &[StaticMachineArgument],
    arguments: &[ExpressionHandle],
) -> Option<Vec<(SymbolHandle, Polynomial)>> {
    let parameters = program
        .machine_type_parameters(callee)
        .iter()
        .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Const { .. }))
        .collect::<Vec<_>>();
    let mut bindings = Vec::new();
    let mut fixed = Vec::new();
    let mut selected = parameters.iter();
    for argument in selections {
        if let Some(value) = static_const_value(program, caller, argument) {
            let parameter = selected.next()?;
            fixed.push(parameter.symbol);
            if let Some(value) = value {
                if !matches!(parameter.kind, TypeParameterKind::Const { type_reference }
                    if program.primitive_type_reference(type_reference).is_some_and(|primitive| primitive.accepts_integer_literal()))
                {
                    return None;
                }
                bindings.push((parameter.symbol, value));
            } else if matches!(parameter.kind, TypeParameterKind::Const { type_reference }
                if program.primitive_type_reference(type_reference).is_some_and(|primitive| primitive.accepts_integer_literal()))
            {
                return None;
            }
        } else if !argument.symbol.is_valid()
            || !matches!(
                program.symbols.get(argument.symbol).kind,
                ::symbols::SymbolKind::BuiltinType
                    | ::symbols::SymbolKind::Data
                    | ::symbols::SymbolKind::TypeParameter
                    | ::symbols::SymbolKind::Machine
                    | ::symbols::SymbolKind::State
                    | ::symbols::SymbolKind::MachineParameter
                    | ::symbols::SymbolKind::Conformance
                    | ::symbols::SymbolKind::ConformanceParameter
                    | ::symbols::SymbolKind::Proposition
                    | ::symbols::SymbolKind::PropositionParameter
            )
        {
            // Only an exact non-const declaration may be filtered out of the
            // const telescope. An unknown authored slot is never omitted and
            // then silently supplied by endpoint inference.
            return None;
        }
    }
    if selections.is_empty() && caller.symbol == callee.symbol {
        for parameter in &parameters {
            fixed.push(parameter.symbol);
            bindings.push((
                parameter.symbol,
                Polynomial::atom(format!("const:{:?}", parameter.symbol)),
            ));
        }
    }
    // As in specialization, inference reads declared range endpoints only.
    // Explicit slots stay fixed. Every repeated omitted endpoint must agree;
    // exact singleton-carrier premises may establish that symbolic agreement.
    for (argument, parameter) in arguments.iter().zip(
        program
            .state_parameters(entry)
            .iter()
            .filter(|parameter| !parameter.is_self),
    ) {
        if let Some(actual_type) = value_type(program, caller, state, *argument) {
            use typed_trees::types::FixedArrayLength;
            if let (
                TypeReferenceNode::FixedArray {
                    length: FixedArrayLength::ConstParameter { symbol, .. },
                    ..
                },
                TypeReferenceNode::FixedArray { length, .. },
            ) = (
                program
                    .type_reference_table
                    .type_reference(unreferenced(program, parameter.type_reference)),
                program
                    .type_reference_table
                    .type_reference(unreferenced(program, actual_type)),
            ) && let Some(required) = parameters
                .iter()
                .find(|parameter| parameter.symbol == *symbol && symbol.is_valid())
                && !fixed.contains(symbol)
            {
                let value = match length {
                    FixedArrayLength::Literal(value) => {
                        Polynomial::constant(BigInt::from_u64(u64::try_from(*value).ok()?))
                    }
                    FixedArrayLength::ConstParameter { symbol, .. } => {
                        let actual = program
                            .machine_type_parameters(caller)
                            .iter()
                            .find(|parameter| parameter.symbol == *symbol && symbol.is_valid())?;
                        let (
                            TypeParameterKind::Const {
                                type_reference: required_type,
                            },
                            TypeParameterKind::Const {
                                type_reference: actual_type,
                            },
                        ) = (&required.kind, &actual.kind)
                        else {
                            return None;
                        };
                        if !crate::type_references::type_references_match(
                            program,
                            *required_type,
                            *actual_type,
                        ) {
                            return None;
                        }
                        Polynomial::atom(format!("const:{symbol:?}"))
                    }
                    FixedArrayLength::ConstCall { .. } => return None,
                };
                if let Some((_, previous)) =
                    bindings.iter().find(|(candidate, _)| *candidate == *symbol)
                {
                    if *previous != value {
                        return None;
                    }
                } else {
                    bindings.push((*symbol, value));
                }
            }
        }
        let Some((carrier, required, required_end_inclusive)) =
            crate::declared_integer_range(program, unreferenced(program, parameter.type_reference))
        else {
            continue;
        };
        let Some(actual_type) = value_type(program, caller, state, *argument) else {
            continue;
        };
        let Some((actual_carrier, actual, actual_end_inclusive)) =
            crate::declared_integer_range(program, unreferenced(program, actual_type))
        else {
            continue;
        };
        if carrier != actual_carrier {
            continue;
        }
        for (endpoint_index, (required, actual)) in required.into_iter().zip(actual).enumerate() {
            let Some(parameter) =
                binder(program, program.machine_type_parameters(callee), required)
            else {
                continue;
            };
            if fixed.contains(&parameter.symbol) {
                continue;
            }
            let value = term(
                program,
                program.machine_type_parameters(caller),
                engine,
                actual,
            )?;
            // Infer the authored binder from equal normalized endpoints; the
            // exclusive adjustment belongs to proof arithmetic, not its AST.
            let value = if endpoint_index == 1 {
                let value = inclusive_maximum(value, actual_end_inclusive);
                if required_end_inclusive {
                    value
                } else {
                    value.add(&Polynomial::constant(BigInt::from_i64(1)))
                }
            } else {
                value
            };
            if let Some((_, previous)) = bindings
                .iter()
                .find(|(symbol, _)| *symbol == parameter.symbol)
            {
                let difference = value.sub(previous);
                if !engine.prove_at_least(&difference, &BigInt::zero())
                    || !engine.prove_at_least(&previous.sub(&value), &BigInt::zero())
                {
                    return None;
                }
            } else {
                bindings.push((parameter.symbol, value));
            }
        }
    }
    Some(bindings)
}

/// Every resolved call checks the complete symbolic parameter range in the
/// caller's namespace. This does not replace carrier/access/arity validation.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_const_range_call(
    program: &TypedTrees,
    caller: &Machine,
    state: Option<&State>,
    target: SymbolHandle,
    selections: &[StaticMachineArgument],
    arguments: &[ExpressionHandle],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some((callee, entry)) =
        crate::transitions::resolved_transition_target_state(program, target)
    else {
        return;
    };
    let has_symbolic_range = program.state_parameters(entry).iter().any(|parameter| {
        has_const_range(
            program,
            program.machine_type_parameters(callee),
            parameter.type_reference,
        )
    });
    if !has_symbolic_range {
        return;
    }
    let proven = (|| {
        let mut engine = scope_engine(program, caller)?;
        let bindings = call_bindings(
            program,
            caller,
            state,
            &mut engine,
            callee,
            entry,
            selections,
            arguments,
        )?;
        for (argument, parameter) in arguments.iter().zip(
            program
                .state_parameters(entry)
                .iter()
                .filter(|parameter| !parameter.is_self),
        ) {
            if !has_const_range(
                program,
                program.machine_type_parameters(callee),
                parameter.type_reference,
            ) {
                continue;
            }
            let (_, endpoints, end_inclusive) = crate::declared_integer_range(
                program,
                unreferenced(program, parameter.type_reference),
            )?;
            let required = [
                selected_term(program, &mut engine, caller, endpoints[0], &bindings)?,
                inclusive_maximum(
                    selected_term(program, &mut engine, caller, endpoints[1], &bindings)?,
                    end_inclusive,
                ),
            ];
            let actual = value_bounds(program, caller, state, &mut engine, *argument)?;
            if !contains(&engine, &required, &actual) {
                return Some(false);
            }
            if matches!(
                program
                    .type_reference_table
                    .type_reference(parameter.type_reference),
                TypeReferenceNode::Reference {
                    access: language_semantics::ReferenceAccess::Mutable
                        | language_semantics::ReferenceAccess::WriteOnly,
                    ..
                }
            ) && !contains(&engine, &actual, &required)
            {
                return Some(false);
            }
        }
        Some(true)
    })();
    if proven != Some(true) {
        diagnostics.push(Diagnostic::error(format!("machine `{}` cannot prove the declared symbolic const parameter ranges of `{}` at this exact call", caller.name, callee.name)));
    }
}
