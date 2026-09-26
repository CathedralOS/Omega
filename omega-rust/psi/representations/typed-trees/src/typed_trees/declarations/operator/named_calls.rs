//! Named expression and statement calls and their operator candidates.

use crate::TypedTrees;
use crate::typed_trees::declarations::operator::OperatorDefinition;
use symbols::SymbolHandle;

/// Resolve an explicitly named operator call from its path and arity facts.
///
/// Named calls may reach this stage without an early `target_symbol`, so every
/// consumer must use this same ambiguity-checked fallback rather than accepting
/// a leaf spelling on its own. Return types never distinguish overloads.
pub fn resolve_named_call<'program>(
    program: &'program TypedTrees,
    target_symbol: SymbolHandle,
    static_receiver_segments: Option<&[&str]>,
    target_name: &str,
    argument_count: usize,
    has_value_receiver: bool,
) -> Option<&'program OperatorDefinition> {
    if target_symbol.is_valid() {
        return program
            .operators()
            .iter()
            .find(|operator| operator.symbol == target_symbol);
    }

    let mut candidates = program.operators().iter().filter(|operator| {
        named_operator_path_matches_call(program, operator, static_receiver_segments, target_name)
            && named_operator_arity_fits_call(program, operator, argument_count, has_value_receiver)
    });
    let first = candidates.next()?;
    candidates.next().is_none().then_some(first)
}

/// Resolve an expression-position named operator call using the receiver path
/// already retained in the typed expression table.
pub fn resolve_named_expression_call<'program>(
    program: &'program TypedTrees,
    call: &crate::expression::TableCallExpression,
) -> Option<&'program OperatorDefinition> {
    if call.target_symbol.is_valid() {
        return program
            .operators()
            .iter()
            .find(|operator| operator.symbol == call.target_symbol);
    }
    let candidates = named_expression_call_candidates(program, call);
    let [selected] = candidates.as_slice() else {
        return None;
    };
    Some(*selected)
}

/// Return every named operator matching an expression call's retained path and
/// arity. Result-overload normalization uses this before one exact result
/// dispatch has been selected, when ordinary single-candidate resolution must
/// deliberately report ambiguity.
pub fn named_expression_call_candidates<'program>(
    program: &'program TypedTrees,
    call: &crate::expression::TableCallExpression,
) -> Vec<&'program OperatorDefinition> {
    if call.target_symbol.is_valid() {
        return program
            .operators()
            .iter()
            .filter(|operator| operator.symbol == call.target_symbol)
            .collect();
    }
    let mut static_segments = Vec::new();
    let has_value_receiver = if !call.receiver.is_valid() {
        false
    } else if let crate::expression::ExpressionNode::Name(path) =
        program.expression_table.expression(call.receiver)
    {
        let receiver_members = program.expression_table.name_path_members(path.members);
        let is_known_static_namespace = path.symbol.is_valid()
            && (program
                .data_definitions()
                .iter()
                .any(|definition| definition.symbol == path.symbol)
                || program
                    .domain_definitions()
                    .iter()
                    .any(|definition| definition.symbol == path.symbol)
                || program
                    .machines()
                    .iter()
                    .any(|definition| definition.symbol == path.symbol)
                || program
                    .traits()
                    .iter()
                    .any(|definition| definition.symbol == path.symbol));
        // Named operator lowering intentionally may leave a static namespace
        // path symbol unresolved while retaining the exact call selection.
        // Reconstruct only the namespace/value classification from the closed
        // operator vocabulary and complete authored path; selection itself is
        // still finalized separately.
        let is_operator_namespace = !path.symbol.is_valid()
            && program.operators().iter().any(|operator| {
                let operator_path = program.operator_path_members(operator.name);
                operator_path
                    .split_last()
                    .is_some_and(|(member, namespace)| {
                        member.as_str() == call.target.as_str()
                            && namespace.len() == receiver_members.len()
                            && namespace
                                .iter()
                                .zip(receiver_members)
                                .all(|(expected, actual)| expected == actual)
                    })
            });
        let is_static_namespace = is_known_static_namespace || is_operator_namespace;
        if is_static_namespace {
            static_segments.extend(receiver_members.iter().map(|segment| segment.as_str()));
        }
        !is_static_namespace
    } else {
        true
    };
    let static_receiver_segments =
        (!static_segments.is_empty()).then_some(static_segments.as_slice());
    let argument_count = program
        .expression_table
        .expression_handles(call.arguments)
        .len();
    program
        .operators()
        .iter()
        .filter(|operator| {
            named_operator_path_matches_call(
                program,
                operator,
                static_receiver_segments,
                call.target.as_str(),
            ) && named_operator_arity_fits_call(
                program,
                operator,
                argument_count,
                has_value_receiver,
            )
        })
        .collect()
}

/// Statement-position counterpart to [`named_expression_call_candidates`].
/// Statement calls retain their receiver path in the statement table rather
/// than as an expression node, but static namespaces and value receivers obey
/// the same path/arity rule.
pub fn named_statement_call_candidates<'program>(
    program: &'program TypedTrees,
    call: &crate::statement::TableCall,
) -> Vec<&'program OperatorDefinition> {
    if call.target_symbol.is_valid() {
        return program
            .operators()
            .iter()
            .filter(|operator| operator.symbol == call.target_symbol)
            .collect();
    }
    let receiver = program.statement_table.name_path_members(call.receiver);
    // The receiver names a static namespace when it resolved to one of the
    // namespace kinds the statement-call gate admits
    // (`is_named_operator_namespace`), or when it did not resolve at all but
    // an operator's own path prefix spells it exactly — the same
    // `is_operator_namespace` fallback the expression-side counterpart uses.
    // Without it `Ns::probe(arg);` classified `Ns` as the value operand,
    // shifted every arity check by one, and the statement call never
    // selected: the downstream rewrite that produces the checked
    // `named_uses` row then never ran, leaving requires/crash obligations
    // unexamined while flow still applied the call's ensures.
    let receiver_resolves_to_namespace = call.receiver_symbol.is_valid()
        && matches!(
            program.symbols.get(call.receiver_symbol).kind,
            symbols::SymbolKind::Module
                | symbols::SymbolKind::BuiltinType
                | symbols::SymbolKind::Data
                | symbols::SymbolKind::Domain
                | symbols::SymbolKind::Machine
                | symbols::SymbolKind::Trait
        );
    let is_operator_namespace = !call.receiver_symbol.is_valid()
        && !receiver.is_empty()
        && program.operators().iter().any(|operator| {
            let operator_path = program.operator_path_members(operator.name);
            operator_path
                .split_last()
                .is_some_and(|(member, namespace)| {
                    member.as_str() == call.target.as_str()
                        && namespace.len() == receiver.len()
                        && namespace
                            .iter()
                            .zip(receiver.iter())
                            .all(|(expected, actual)| expected == actual)
                })
        });
    let is_static_namespace = receiver_resolves_to_namespace || is_operator_namespace;
    let static_segments = is_static_namespace.then(|| {
        receiver
            .iter()
            .map(|segment| segment.as_str())
            .collect::<Vec<_>>()
    });
    let has_value_receiver = !receiver.is_empty() && !is_static_namespace;
    let argument_count = program
        .statement_table
        .expression_handles(call.arguments)
        .len();
    program
        .operators()
        .iter()
        .filter(|operator| {
            named_operator_path_matches_call(
                program,
                operator,
                static_segments.as_deref(),
                call.target.as_str(),
            ) && named_operator_arity_fits_call(
                program,
                operator,
                argument_count,
                has_value_receiver,
            )
        })
        .collect()
}

fn named_operator_path_matches_call(
    program: &TypedTrees,
    operator: &OperatorDefinition,
    static_receiver_segments: Option<&[&str]>,
    target_name: &str,
) -> bool {
    let path = program.operator_path_members(operator.name);
    let Some((last, prefix)) = path.split_last() else {
        return false;
    };
    if last.as_str() != target_name {
        return false;
    }

    match static_receiver_segments {
        Some(segments) => {
            prefix.len() == segments.len()
                && prefix
                    .iter()
                    .zip(segments.iter())
                    .all(|(member, segment)| member.as_str() == *segment)
        }
        None => true,
    }
}

fn named_operator_arity_fits_call(
    program: &TypedTrees,
    operator: &OperatorDefinition,
    argument_count: usize,
    has_value_receiver: bool,
) -> bool {
    let parameters = program.operator_parameters(operator);
    let has_self = parameters.iter().any(|parameter| parameter.is_self);
    let positional = parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .count();

    if has_self {
        return has_value_receiver && positional == argument_count;
    }
    if has_value_receiver {
        return positional == argument_count + 1;
    }
    positional == argument_count
}
