//! Evaluate const arguments written as expressions (`Buffer<capacity()>`)
//! the way a constant's initializer is evaluated, then give each argument
//! the canonical value typing gives a literal.
//!
//! generics.md, "Transparent type aliases": "Closed argument expressions may
//! execute ordinary admitted compile-time code. A computed const value
//! retains its declared carrier and canonical value". So `Buffer<capacity()>`
//! is the application `Buffer<4>` once `capacity()` evaluates to 4. Typing
//! leaves such an argument as written (it folds only literals); this pass is
//! the evaluator it leaves the argument for.
//!
//! Each argument is one job, run through the same bridge as a constant
//! (`constant_jobs`): the expression becomes the body of a zero-argument
//! machine returning the parameter's declared carrier, that machine is
//! judged by the lowered-form checker as its own invocation, and only an
//! accepted invocation runs. Nothing here decides whether a call may be
//! evaluated; the checker's verdict on the invocation does.
//!
//! The entry machine lives in a copy of the program made for the one job.
//! The program itself changes only where the value lands
//! (`TypedTrees::settle_const_argument`).

use std::collections::VecDeque;

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::constant_recipes::{
    ConstantExpressionContext, RetainedConstantPosition, RetainedConstantRecipe,
    retain_constant_recipes,
};
use typed_trees::data::TypeParameterKind;
use typed_trees::deferred_refusals::DeferredRequest;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableTransition, TransitionTargetNode};
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

use super::constant_jobs::{decoded, insert, key};
use crate::{InvocationTarget, SourceCompletionAccounting};

/// One value required by a type and still written as an expression.
struct Job {
    argument: TypeReferenceHandle,
    expression: ExpressionHandle,
    /// The argument's exact builtin carrier.
    carrier: TypeReferenceHandle,
    /// The declaration the application is written in.
    owner: SymbolHandle,
    /// The application and parameter that ask for the value, if any.
    application: Option<(SymbolHandle, String)>,
}

impl Job {
    fn description(&self, program: &TypedTrees) -> String {
        match &self.application {
            Some((applied, parameter)) => {
                format!(
                    "const argument for `{parameter}` of `{}`",
                    program.symbols.name(*applied)
                )
            }
            None => "computed array extent".to_owned(),
        }
    }
}

fn pending_argument(
    program: &TypedTrees,
    jobs: &mut Vec<Job>,
    argument: TypeReferenceHandle,
    carrier: TypeReferenceHandle,
    application: Option<(SymbolHandle, &str)>,
) {
    let types = &program.type_reference_table;
    let TypeReferenceNode::ConstExpression(expression) = *types.type_reference(argument) else {
        return;
    };
    let builtin = types
        .contains_type_reference(carrier)
        .then(|| types.type_reference(carrier));
    let carrier_atom = match builtin {
        Some(TypeReferenceNode::Named { symbol, .. }) => program.symbols.builtin_type_atom(*symbol),
        _ => None,
    };
    if !(matches!(carrier_atom, Some(symbols::BuiltinTypeAtom::Integer(_)))
        || (application.is_some() && carrier_atom == Some(symbols::BuiltinTypeAtom::Bool)))
        || program
            .static_const_literal_from_type_reference(argument)
            .is_some()
        || matches!(
            program.expression_table.expression(expression),
            typed_trees::expression::ExpressionNode::Boolean(_)
        )
        || program.pending_const_range_endpoints.contains(&expression)
        || !typed_trees::domain::index_argument_is_closed(program, argument)
        || jobs.iter().any(|known| known.argument == argument)
    {
        return;
    }
    jobs.push(Job {
        argument,
        expression,
        carrier,
        owner: SymbolHandle::invalid(),
        application: application.map(|(symbol, parameter)| (symbol, parameter.to_owned())),
    });
}

/// The const arguments of type applications that are closed expressions and
/// not literals, each with the parameter it answers. A carrier other than a
/// builtin integer or `bool` (a domain, a data type, a type parameter of the
/// same header) asks more than a value of the carrier, so its argument is
/// left as written and stays refused where it is demanded.
fn pending(program: &TypedTrees) -> Vec<Job> {
    let mut found: Vec<Job> = Vec::new();
    let mut array_arguments = Vec::new();
    let mut generic_arguments = Vec::new();
    for (_, node) in program.type_reference_table.type_reference_rows() {
        if let TypeReferenceNode::FixedArray {
            length: FixedArrayLength::Expression(argument),
            ..
        } = node
            && let TypeReferenceNode::ConstExpression(expression) =
                program.type_reference_table.type_reference(*argument)
        {
            array_arguments.push((*argument, *expression));
        }
        let TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } = node
        else {
            continue;
        };
        let arguments = program
            .type_reference_table
            .type_reference_handles(*arguments);
        if !arguments.iter().any(|argument| {
            matches!(
                program.type_reference_table.type_reference(*argument),
                TypeReferenceNode::ConstExpression(_)
            )
        }) || !program.symbols.symbols().nodes().is_valid(*base_symbol)
        {
            continue;
        }
        let parameters = program.declaration_type_parameters(*base_symbol);
        if parameters.len() != arguments.len() {
            continue;
        }
        for (parameter, argument) in parameters.iter().zip(arguments) {
            let TypeParameterKind::Const { type_reference } = parameter.kind else {
                continue;
            };
            if !matches!(
                program.type_reference_table.type_reference(*argument),
                TypeReferenceNode::ConstExpression(_)
            ) {
                continue;
            }
            generic_arguments.push(*argument);
            pending_argument(
                program,
                &mut found,
                *argument,
                type_reference,
                Some((*base_symbol, parameter.name.as_str())),
            );
        }
    }
    // Array extents derived from a generic argument keep that parameter's
    // declared carrier. Only written array expressions supply their own.
    let coordinate =
        |argument: TypeReferenceHandle| (argument.arena_index(), argument.generation());
    generic_arguments.sort_unstable_by_key(|&argument| coordinate(argument));
    for (argument, expression) in array_arguments {
        if generic_arguments
            .binary_search_by_key(&coordinate(argument), |&argument| coordinate(argument))
            .is_err()
        {
            let carrier = program
                .expression_table
                .constructed_value_type_reference(expression);
            pending_argument(program, &mut found, argument, carrier, None);
        }
    }
    found
}

/// The symbols an expression names, with the machines it calls.
#[derive(Default)]
struct Named(Vec<SymbolHandle>);
impl Extend<SymbolHandle> for Named {
    fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: T) {
        self.0.extend(symbols);
    }
}
impl typed_trees::references::TypedReferences for Named {
    fn derived_declaration_references(&self) -> bool {
        false
    }
    fn callable_use(&mut self, symbol: SymbolHandle) {
        self.0.push(symbol);
    }
}

/// Every declaration reachable from `start` through the declarations each
/// one names (`rows`, sorted by declaration), sorted.
fn reached(
    program: &TypedTrees,
    rows: &[(SymbolHandle, Vec<SymbolHandle>)],
    start: Vec<SymbolHandle>,
) -> Vec<SymbolHandle> {
    let mut pending = start;
    let mut reached = Vec::new();
    while let Some(symbol) = pending.pop() {
        if !program.symbols.symbols().nodes().is_valid(symbol) {
            continue;
        }
        let symbol = program.refusal_declaration(symbol);
        if insert(&mut reached, symbol)
            && let Ok(index) = rows.binary_search_by_key(&key(symbol), |(symbol, _)| key(*symbol))
        {
            pending.extend(rows[index].1.iter().copied());
        }
    }
    reached
}

/// The zero-argument machine whose value is `job`'s expression, added to
/// `probe`: `machine @const-argument<..>() -> Carrier { expression }`.
fn entry(probe: &mut TypedTrees, job: &Job) -> SymbolHandle {
    let name = match &job.application {
        Some((applied, parameter)) => format!(
            "@const-argument<{}::{parameter}>",
            probe.symbols.display_path(*applied, "::"),
        ),
        None => format!(
            "@array-extent<{}>",
            probe
                .expression_table
                .source_span(job.expression)
                .span
                .start
        ),
    };
    let symbol =
        probe
            .symbols
            .insert_generated_root_from(job.owner, symbols::SymbolKind::Machine, &name);
    let state_symbol = probe
        .symbols
        .insert_generated_children(symbol, [(symbols::SymbolKind::State, name.as_str())])
        .start();
    let mut machine = Machine {
        symbol,
        name: typed_trees::name::Identifier::generated(&name),
        ..Default::default()
    };
    let mut state = State {
        symbol: state_symbol,
        name: machine.name.clone(),
        return_type: job.carrier,
        ..Default::default()
    };
    let target = probe
        .statement_table
        .insert_transition_target(TransitionTargetNode::Value(job.expression));
    let source_span = probe.expression_table.source_span(job.expression);
    probe.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Transition(TableTransition {
            target,
            source_span,
            ..Default::default()
        }),
    );
    probe.push_machine_state(&mut machine, state);
    probe.push_machine(machine);
    symbol
}

/// Evaluates every const argument written as an expression in a declaration
/// the request demands, in dependency order, and settles each value into the
/// program. Runs before constant initializers: a constant may be computed
/// through a type whose argument is evaluated here.
pub fn evaluate(
    program: &mut TypedTrees,
    request: terminal_production::TerminalSourceCompilation<'_>,
    authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    source_consent: Option<std::sync::Arc<dyn crate::InvocationSourceConsent>>,
    accounting: &mut SourceCompletionAccounting,
) -> Result<(), Vec<Diagnostic>> {
    let found = pending(program);
    if found.is_empty() {
        return Ok(());
    }
    let refuse = |reason: String| vec![Diagnostic::error(reason)];
    let demanded = super::constant_jobs::demanded_declarations(program, request).map_err(refuse)?;
    let owners = typed_trees::references::collect_program_operand_owners(program);
    let jobs = found
        .into_iter()
        .filter_map(|job| {
            // The application is its declaration's to carry; an argument no
            // demanded declaration holds is not asked for by this request.
            let owner = owners
                .type_owners(job.argument)
                .filter(|owner| owner.is_valid())
                .map(|owner| program.refusal_declaration(owner))
                .find(|owner| demanded.contains(*owner))?;
            Some(Job { owner, ..job })
        })
        .collect::<Vec<_>>();
    if jobs.is_empty() {
        return Ok(());
    }
    // A job waits for every job held by a declaration its expression
    // reaches: `Outer<outer()>` waits for the `Buffer<leaf()>` in `outer`.
    let mut rows = typed_trees::references::collect_computation_symbol_dependencies(program);
    rows.sort_unstable_by_key(|(symbol, _)| key(*symbol));
    let mut successors = vec![Vec::new(); jobs.len()];
    let mut remaining = vec![0usize; jobs.len()];
    for (position, job) in jobs.iter().enumerate() {
        let mut named = Named::default();
        typed_trees::references::visit_expression(program, &mut named, job.expression);
        let reached = reached(program, &rows, named.0);
        for (dependency, other) in jobs.iter().enumerate() {
            if reached
                .binary_search_by_key(&key(other.owner), |symbol| key(*symbol))
                .is_ok()
            {
                remaining[position] += 1;
                successors[dependency].push(position);
            }
        }
    }
    let mut ready = remaining
        .iter()
        .enumerate()
        .filter_map(|(position, count)| (*count == 0).then_some(position))
        .collect::<VecDeque<_>>();
    let mut completed = 0usize;
    while let Some(position) = ready.pop_front() {
        let job = &jobs[position];
        let result = (|| -> Result<(), String> {
            let mut probe = program.clone();
            let entry = entry(&mut probe, job);
            let prepared = super::PreparedBuildMachineProgram::prepare_for(
                &probe,
                authority.clone(),
                request.purpose,
                source_consent.clone(),
            )
            .map_err(|errors| diagnostics::join_messages(&errors))?;
            let invocation = prepared.entry(job.owner, entry, &[])?;
            if job.application.is_none()
                && invocation
                    .accepted()
                    .module()
                    .boundary_machines
                    .iter()
                    .any(|boundary| {
                        boundary
                            .declaration
                            .as_ref()
                            .is_some_and(|row| !row.execution.compiler_owned())
                    })
            {
                let symbol = self::entry(program, job);
                let contexts =
                    program
                        .constant_expression_contexts
                        .insert_many([ConstantExpressionContext {
                            owner: job.owner,
                            result_type: job.carrier,
                            evaluation_entry: symbol,
                            evaluation_expression: job.expression,
                        }]);
                let source_span = program.expression_table.source_span(job.expression);
                retain_constant_recipes(
                    program,
                    [RetainedConstantRecipe {
                        position: RetainedConstantPosition::ConstApplication {
                            site: job.argument,
                            root: job.expression,
                        },
                        contexts,
                        machine: job.owner,
                        source_span,
                        ..Default::default()
                    }],
                );
                return Ok(());
            }
            let value = invocation
                .invoke_recorded(
                    InvocationTarget {
                        name: None,
                        layout_target: None,
                        carrier_layout: None,
                        address_model: None,
                    },
                    accounting,
                )?
                .into_value();
            use language_semantics::const_value::DecodedCanonicalConstValue;
            let spelling = match decoded(program, job.carrier, &value)? {
                DecodedCanonicalConstValue::Integer { value, .. } => value.to_string(),
                DecodedCanonicalConstValue::Boolean(value) => value.to_string(),
                _ => return Err("the value is neither an integer nor a Boolean".into()),
            };
            program.settle_const_argument(job.argument, &spelling)?;
            Ok(())
        })();
        if let Err(reason) = result {
            return Err(vec![
                Diagnostic::error(format!("{}: {reason}", job.description(program)))
                    .with_source_span(program.expression_table.source_span(job.expression)),
            ]);
        }
        // The declaration held a request no pass served, and is refused
        // wherever it is demanded while it carries that row. The next job
        // may demand it, so the row goes as soon as nothing backs it.
        withdraw_served_requests(program)?;
        completed += 1;
        for &successor in &successors[position] {
            remaining[successor] -= 1;
            if remaining[successor] == 0 {
                ready.push_back(successor);
            }
        }
    }
    if completed != jobs.len() {
        let job = remaining
            .iter()
            .position(|count| *count != 0)
            .map(|position| &jobs[position])
            .expect("an unfinished job");
        return Err(vec![
            Diagnostic::error(format!(
                "{} is computed through a declaration that needs \
                 this argument's value: the evaluation has no first step",
                job.description(program),
            ))
            .with_source_span(program.expression_table.source_span(job.expression)),
        ]);
    }
    Ok(())
}

/// Drops the type-expression and array-extent rows of declarations that no
/// longer hold such a request. Those two kinds are recorded only from the
/// program's declared requests (`source_frontier::declared_jobs`), so a row
/// is current exactly when that inventory still lists it.
fn withdraw_served_requests(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let evaluated = |request| {
        matches!(
            request,
            DeferredRequest::TypeExpressionEvaluation | DeferredRequest::ArrayExtentEvaluation
        )
    };
    let current = crate::source_frontier::declared_jobs(program)?
        .into_iter()
        .filter(|(_, request)| evaluated(*request))
        .map(|(declaration, request)| (program.refusal_declaration(declaration), request))
        .collect::<Vec<_>>();
    program
        .deferred_refusals
        .retain(|row| !evaluated(row.request) || current.contains(&(row.declaration, row.request)));
    Ok(())
}
