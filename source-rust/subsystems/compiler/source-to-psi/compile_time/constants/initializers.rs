//! Evaluates retained constant computations through the accepted invocation
//! bridge, then stores their snapshots in their original typed owner.
//! Dependency discovery reads identities only; it never interprets an
//! expression or chooses a result. Every job still passes typed preparation,
//! structural discharge, publication and hermetic invocation admission.

use std::collections::VecDeque;

use crate::source_state::TypedTrees;
use crate::source_state::expression::ExpressionHandle;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

use crate::evaluation::{
    FixedLayoutTargetScope, PreparedBuildMachineProgram, SourceCompletionAccounting,
    demanded_declarations, invocation_error,
};

use super::{References, decoded, family_position, insert, key, named_uses};

fn dependencies(program: &TypedTrees, entry: SymbolHandle) -> Vec<SymbolHandle> {
    let mut references = References::default();
    let mut pending = vec![entry];
    let mut visited = Vec::new();
    while let Some(callee) = pending.pop() {
        let Some(machine) = program
            .machine_by_symbol(callee)
            .or_else(|| program.machine_holding_state(callee))
        else {
            continue;
        };
        if !insert(&mut visited, machine.symbol) {
            continue;
        }
        crate::source_state::references::visit_machine(program, &mut references, machine);
        pending.append(&mut references.calls);
    }
    references.symbols
}

fn materialize_uses(
    program: &mut TypedTrees,
    symbol: SymbolHandle,
    expressions: &[ExpressionHandle],
) -> Result<(), String> {
    let declaration = program
        .const_declaration_by_symbol(symbol)
        .ok_or("constant use lost its exact declaration")?;
    let carrier = declaration.declared_type;
    let value = crate::source_state::canonical_const_declaration_value(program, symbol)
        .and_then(|value| value.decode_encoding())
        .ok_or("constant use has no evaluated canonical value")?;
    for expression in expressions {
        let node = program
            .materialize_canonical_const_value(carrier, &value)
            .ok_or("constant value does not materialize in its declared carrier")?;
        *program.expression_table.expression_mut(*expression) = node;
    }
    Ok(())
}

/// Nominate computation from the same request the lowered-form checker will
/// judge. This discovers identities only; neither membership nor an empty
/// dependency row admits an invocation or supplies its result.
fn demanded_constants(
    program: &TypedTrees,
    request: crate::production::TerminalSourceCompilation<'_>,
) -> Result<Vec<SymbolHandle>, String> {
    let selected = demanded_declarations(program, request)?;
    let mut constants = program
        .const_declarations()
        .iter()
        .filter(|constant| selected.contains(constant.symbol))
        .map(|constant| constant.symbol)
        .collect::<Vec<_>>();
    constants.sort_unstable_by_key(|symbol| key(*symbol));
    constants.dedup();
    Ok(constants)
}

/// The deployments whose declaration closure reaches each constant, in
/// target order. A constant computed through an authored storage layout
/// runs once under each of them; others ignore this.
fn deployment_targets<'scope>(
    program: &TypedTrees,
    scopes: &'scope [FixedLayoutTargetScope],
    constants: &[SymbolHandle],
) -> Result<Vec<Vec<&'scope FixedLayoutTargetScope>>, String> {
    let mut reached = vec![Vec::new(); constants.len()];
    if constants.is_empty() {
        return Ok(reached);
    }
    let mut ordered = scopes
        .iter()
        .filter(|scope| {
            !scope.entry_declarations.is_empty() || !scope.provider_declarations.is_empty()
        })
        .collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.target.cmp(&right.target));
    for scope in ordered {
        let selection =
            crate::source_state::references::DeclarationSelection::from_computation_roots(
                program,
                scope.entry_declarations.iter().copied().chain(
                    scope
                        .provider_declarations
                        .iter()
                        .map(|row| row.provider_type.symbol),
                ),
            )?;
        for (position, constant) in constants.iter().enumerate() {
            let targets: &mut Vec<&FixedLayoutTargetScope> = &mut reached[position];
            if !selection.contains(*constant) {
                continue;
            }
            match targets.last() {
                Some(last) if last.target == scope.target => {
                    if last.facts != scope.facts {
                        return Err("layout scopes disagree on one target's immutable facts".into());
                    }
                }
                _ => targets.push(scope),
            }
        }
    }
    Ok(reached)
}

pub fn evaluate_with_source_consent(
    program: &mut TypedTrees,
    request: crate::production::TerminalSourceCompilation<'_>,
    scopes: &[FixedLayoutTargetScope],
    authority: Option<std::sync::Arc<dyn crate::evaluation::BuildTimeSelectionAuthority>>,
    source_consent: Option<std::sync::Arc<dyn crate::evaluation::InvocationSourceConsent>>,
    accounting: &mut SourceCompletionAccounting,
) -> Result<Vec<SymbolHandle>, Vec<Diagnostic>> {
    let demanded =
        demanded_constants(program, request).map_err(|reason| vec![Diagnostic::error(reason)])?;
    let jobs = program
        .roots
        .const_declarations
        .handles()
        .filter_map(|handle| {
            let constant = program.tables.const_declarations.get(handle);
            (family_position(&demanded, constant.symbol).is_some()
                && constant.evaluated_value.is_none()
                && program.tables.machines.is_valid(constant.evaluation_entry))
            .then(|| {
                (
                    handle,
                    constant.symbol,
                    program
                        .tables
                        .machines
                        .get(constant.evaluation_entry)
                        .symbol,
                )
            })
        })
        .collect::<Vec<_>>();
    if jobs.is_empty() {
        return Ok(demanded);
    }
    let mut constants = program
        .const_declarations()
        .iter()
        .map(|constant| constant.symbol)
        .collect::<Vec<_>>();
    constants.sort_unstable_by_key(|symbol| key(*symbol));
    let uses = named_uses(program, &constants);
    for (symbol, expressions) in constants.iter().zip(&uses) {
        if program
            .const_declaration_by_symbol(*symbol)
            .is_some_and(|constant| constant.evaluated_value.is_some())
        {
            materialize_uses(program, *symbol, expressions)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
        }
    }
    let mut positions = jobs
        .iter()
        .enumerate()
        .map(|(index, (_, symbol, _))| (*symbol, index))
        .collect::<Vec<_>>();
    positions.sort_unstable_by_key(|(symbol, _)| key(*symbol));
    let mut successors = vec![Vec::new(); jobs.len()];
    let mut remaining = vec![0usize; jobs.len()];
    for (position, (_, _, entry)) in jobs.iter().enumerate() {
        for dependency in dependencies(program, *entry) {
            if let Ok(index) =
                positions.binary_search_by_key(&key(dependency), |(symbol, _)| key(*symbol))
            {
                let dependency = positions[index].1;
                remaining[position] += 1;
                successors[dependency].push(position);
            }
        }
    }
    let job_symbols = jobs
        .iter()
        .map(|(_, symbol, _)| *symbol)
        .collect::<Vec<_>>();
    let deployments = deployment_targets(program, scopes, &job_symbols)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let mut ready = remaining
        .iter()
        .enumerate()
        .filter_map(|(position, count)| (*count == 0).then_some(position))
        .collect::<VecDeque<_>>();
    let mut completed = 0usize;
    while let Some(position) = ready.pop_front() {
        let (handle, symbol, entry) = jobs[position];
        let result = (|| -> Result<(), Vec<Diagnostic>> {
            let prepared = PreparedBuildMachineProgram::prepare_for(
                program,
                authority.clone(),
                request.purpose,
                source_consent.clone(),
            )?;
            let targets = deployments[position]
                .iter()
                .map(|scope| (scope.target.as_str(), scope.facts.as_ref()))
                .collect::<Vec<_>>();
            let invocation = prepared.prepare_entry(symbol, entry, &[])?;
            let module = invocation.accepted().module();
            if module
                .scalar_applications
                .declarations()
                .iter()
                .any(|application| {
                    matches!(
                        application,
                        terminal_psi::ScalarApplication::Evaluate { .. }
                    )
                })
                || module.boundary_machines.iter().any(|boundary| {
                    boundary
                        .declaration
                        .as_ref()
                        .is_some_and(|row| !row.execution.compiler_owned())
                })
            {
                // This value belongs to an actual selected closure. Keep its
                // exact initializer for the product scalar evaluator.
                program
                    .tables
                    .const_declarations
                    .get_mut(handle)
                    .pending_target_observation = true;
                return Ok(());
            }
            let values = invocation
                .invoke_recorded_in_targets(&targets, accounting)
                .map_err(invocation_error)?;
            let carrier = program.tables.const_declarations.get(handle).declared_type;
            // A constant is one value for every deployment that reads it. Each
            // target's layout decides only where the bytes go, so the
            // semantic snapshot must be the same under every one of them.
            let mut folded = None;
            for value in &values {
                let value = decoded(program, carrier, value).map_err(invocation_error)?;
                let encoding =
                    application_identity::const_value::CanonicalConstIdentity::from_declaration_value(
                        &value,
                    )
                    .ok_or_else(|| {
                        invocation_error(
                            "constant result has no canonical declaration encoding",
                        )
                    })?
                    .encoding;
                match &folded {
                    None => folded = Some((value, encoding)),
                    Some((_, first)) if *first == encoding => {}
                    Some(_) => {
                        return Err(invocation_error(
                            "constant result differs between the deployments that read it",
                        ));
                    }
                }
            }
            let (value, encoding) =
                folded.ok_or_else(|| invocation_error("constant evaluation returned no result"))?;
            let node = program
                .materialize_canonical_const_value(carrier, &value)
                .ok_or_else(|| {
                    invocation_error("constant result does not materialize in its declared carrier")
                })?;
            let expression = program.expression_table.insert(node);
            let span = program
                .tables
                .const_declarations
                .get(handle)
                .initializer_source_span;
            program.expression_table.set_source_span(expression, span);
            let declaration = program.tables.const_declarations.get_mut(handle);
            declaration.materialized_initializer = expression;
            declaration.evaluated_value = Some(encoding.into());
            if let Some(index) = family_position(&constants, symbol) {
                materialize_uses(program, symbol, &uses[index]).map_err(invocation_error)?;
            }
            Ok(())
        })();
        if let Err(mut diagnostics) = result {
            let name = program.symbols.name(symbol);
            let declaration_span = program.symbols.symbol_source_span(symbol);
            for error in diagnostics
                .iter_mut()
                .filter(|diagnostic| diagnostic.is_error())
            {
                error.message = format!("constant `{name}` evaluation: {}", error.message);
                // Name the request while pointing at the operation that failed.
                if error.resolved_location.is_none()
                    && error.source_span.unwrap_or_default() == source_text::SourceSpan::default()
                {
                    error.source_span = declaration_span;
                }
            }
            return Err(diagnostics);
        }
        completed += 1;
        for &successor in &successors[position] {
            remaining[successor] -= 1;
            if remaining[successor] == 0 {
                ready.push_back(successor);
            }
        }
    }
    if completed != jobs.len() {
        return Err(vec![Diagnostic::error(
            "constant evaluation dependency cycle",
        )]);
    }
    Ok(demanded)
}
