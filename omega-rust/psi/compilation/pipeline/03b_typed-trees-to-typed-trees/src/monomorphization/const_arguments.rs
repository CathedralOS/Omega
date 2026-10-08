//! Const tuple derivation and concrete binding obligations.
//! Authored argument admission belongs to immutable checking, not this loop.

use super::Candidate;
use diagnostics::Diagnostic;
use symbols::SymbolKind;
use typed_trees::expression::StaticMachineArgument;
use typed_trees::types::TypeReferenceHandle;
pub(super) use validation::expression_traversal::collect_type_expressions;
pub(super) use validation::static_arguments::{forwarded_type, is_runtime_value_subject, spelling};

/// Resolve the exact value-scope subject one runtime `Value` argument
/// denotes: the caller's own parameter or the nearest `let` of the same name
/// declared before the call. Only a single-segment spelling without static
/// payload can resolve, and only an ordinary `Local`/`Parameter` symbol
/// counts — a runtime subject is never a type, machine, or evidence name.
pub(super) fn resolve_runtime_subject(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    scope_limit: usize,
    argument: &StaticMachineArgument,
) -> Option<symbols::SymbolHandle> {
    if argument.type_reference.is_valid()
        || argument.application.is_some()
        || argument.evidence_projection.is_some()
        || argument.const_literal.is_some()
    {
        return None;
    }
    if argument.symbol.is_valid() {
        let kind = program.symbols.get(argument.symbol).kind;
        // A resolved parameter is exact: a realized generic binder forwarded
        // into a specialization names that parameter, never a same-named
        // local. A resolved local re-derives its nearest binding by scope
        // below, since the symbol alone cannot order shadowed `let`s.
        if kind == SymbolKind::Parameter {
            return Some(argument.symbol);
        }
        if kind != SymbolKind::Local {
            return None;
        }
    }
    validation::nearest_scoped_local(program, state, scope_limit, argument.path.as_ref())
}

pub(super) fn validate_bindings(
    program: &dyn ::validation::CheckingInput,
    candidate: &Candidate,
) -> Result<(), Diagnostic> {
    let machine = &program.machines()[candidate.template.machine_index];
    for (index, ((symbol, _, _), binding)) in candidate
        .template
        .const_parameters
        .iter()
        .zip(&candidate.const_bindings)
        .enumerate()
    {
        // A runtime-bound `Value` slot carries its declared carrier, not a
        // closed static value; its subject is checked as an ordinary argument.
        if candidate.runtime_value_bindings[index].is_some() {
            continue;
        }
        let Some(binding) = binding else { continue };
        let Some(parameter) = program
            .machine_type_parameters(machine)
            .iter()
            .find(|parameter| parameter.symbol == *symbol)
        else {
            return Err(Diagnostic::error(
                "const specialization lost its declared binder",
            ));
        };
        validation::validate_closed_const_argument(
            program,
            &candidate.template.template_name,
            parameter,
            *binding,
        )
        .map_err(|errors| {
            errors.into_iter().next().unwrap_or_else(|| {
                Diagnostic::error("const argument validation failed without a diagnostic")
            })
        })?;
    }
    Ok(())
}

pub(super) fn has_pending_type_arguments(
    program: &dyn ::validation::CheckingInput,
    arguments: &[Option<TypeReferenceHandle>],
) -> bool {
    if program.pending_const_range_endpoints.is_empty() {
        return false;
    }
    let mut visited = Vec::new();
    let mut expressions = Vec::new();
    for argument in arguments.iter().flatten() {
        collect_type_expressions(program, *argument, &mut visited, &mut expressions);
    }
    expressions
        .iter()
        .any(|expression| program.pending_const_range_endpoints.contains(expression))
}
