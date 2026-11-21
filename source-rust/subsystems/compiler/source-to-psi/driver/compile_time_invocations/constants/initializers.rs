//! Evaluates retained constant computations through the accepted invocation
//! bridge, then stores their snapshots in their original typed owner.
//! Dependency discovery reads identities only; it never interprets an
//! expression or chooses a result. Every job still passes typed preparation,
//! structural discharge, publication and hermetic invocation admission.

use checked_program_to_psi::production;
use std::collections::VecDeque;
use typed_trees_to_prepared_typed_trees::{PreparedValue, TargetInputs, ValueInputs};

use crate::source_state::TypedTrees;
use crate::source_state::expression::ExpressionHandle;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

use crate::compile_time_invocations::{
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
    let value = declaration
        .evaluated_value
        .as_ref()
        .and_then(numeric_literals::EvaluatedConstant::encoding)
        .and_then(application_identity::const_value::decode_declaration_value)
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
    request: production::TerminalSourceCompilation<'_>,
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
        let selection = scope.demanded_declarations(program)?;
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
    request: production::TerminalSourceCompilation<'_>,
    scopes: &[FixedLayoutTargetScope],
    authority: Option<std::sync::Arc<declaration_scope::PackageDeclarationScope>>,
    source_consent: Option<
        std::sync::Arc<dyn crate::compile_time_invocations::InvocationSourceConsent>,
    >,
    accounting: &mut SourceCompletionAccounting,
) -> Result<Vec<ValueInputs>, Vec<Diagnostic>> {
    let demanded =
        demanded_constants(program, request).map_err(|reason| vec![Diagnostic::error(reason)])?;
    let mut declarations = program
        .const_declarations()
        .iter()
        .filter(|decl| family_position(&demanded, decl.symbol).is_some())
        .collect::<Vec<_>>();
    declarations.sort_unstable_by_key(|decl| key(decl.symbol));
    let mut values = declarations
        .into_iter()
        .map(|declaration| {
            let inputs = if program
                .expression_table
                .expression_is_valid(declaration.authored_initializer)
                && !program
                    .tables
                    .machines
                    .is_valid(declaration.evaluation_entry)
            {
                super::request_references(program, declaration.authored_initializer)
                    .symbols
                    .into_iter()
                    .filter(|&symbol| {
                        program.symbols.symbols().nodes().is_valid(symbol)
                            && program.symbols.get(symbol).kind == symbols::SymbolKind::Const
                    })
                    .collect()
            } else {
                Vec::new()
            };
            ValueInputs::new(
                program,
                PreparedValue::Constant(declaration.symbol),
                inputs,
                if declaration.pending_target_observation {
                    TargetInputs::Deferred
                } else {
                    TargetInputs::None
                },
            )
        })
        .collect::<Vec<_>>();
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
        return Ok(values);
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
    for (position, (_, symbol, entry)) in jobs.iter().enumerate() {
        let value = family_position(&demanded, *symbol).expect("constant job was demanded");
        for dependency in dependencies(program, *entry) {
            if family_position(&constants, dependency).is_some() {
                values[value].constants.push(dependency);
            }
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
        let value_index = family_position(&demanded, symbol).expect("constant job was demanded");
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
            let applications = module.scalars.scalar_applications.declarations();
            // A scalar produced with target inputs belongs to that deployment.
            // Publish the initializer so each use closes it with its own facts.
            use crate::compile_time_invocations::PreparedBuildMachineEntry;
            let observes_target = PreparedBuildMachineEntry::uses_target_layout(module);
            let scalar_result = matches!(module.entry,
                Some(terminal_psi::TargetMachineCallable::Machine(entry))
                    if module.machine(entry).is_some_and(|machine| machine.result.scalar().is_some()));
            if observes_target && (targets.is_empty() || scalar_result)
                || applications.iter().any(|application| {
                    matches!(
                        application,
                        terminal_psi::ScalarApplication::Evaluate { .. }
                    )
                })
                || PreparedBuildMachineEntry::needs_selected_provider(module)
            {
                // The checked initializer is the recipe until a deployment
                // supplies its layout facts or selects its implementation.
                program
                    .tables
                    .const_declarations
                    .get_mut(handle)
                    .pending_target_observation = true;
                values[value_index].targets = TargetInputs::Deferred;
                return Ok(());
            }
            let (outcomes, target_inputs) = invocation
                .invoke_recorded_in_targets(&targets, accounting)
                .map_err(invocation_error)?;
            values[value_index].targets = target_inputs;
            let carrier = program.tables.const_declarations.get(handle).declared_type;
            // A constant is one value for every deployment that reads it. Each
            // target's layout decides only where the bytes go, so the
            // semantic snapshot must be the same under every one of them.
            let mut folded = None;
            for value in &outcomes {
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
    Ok(values)
}
