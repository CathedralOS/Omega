//! COMPTIME evaluation of NAMED RANGE ENDPOINTS.
//!
//! `u64[0..=limit(256)]` puts a build-time-admissible machine call
//! in a declared integer range endpoint. The endpoint is a constant position
//! exactly as a fixed-array length is, so it takes the same route as
//! `const_lengths`: admit the callee through the shared
//! [`BuildTimeAdmissionPlan`], execute it with the checked interpreter under
//! the callee's declared integer return type, and substitute the resulting
//! decimal literal into the endpoint expression before checking.
//!
//! Folding here, in the typed trees, is what lets every later reader agree on
//! one value: declaration validation, structural generic inference in
//! `04_typed-trees-to-checked-trees/src/monomorphization/range_arguments.rs`,
//! proof, and layout all read an ordinary literal endpoint. The
//! context-free `i64` interval evaluator in `validation` never sees the call,
//! so it cannot become the identity of a named computation.
//!
//! An endpoint callee whose reachable closure holds a resolved
//! boundary-operator use cannot run before provider selection: it defers with
//! the owning pre-check continuation, then evaluates under the same selected
//! rows the deferred fixed-array lengths use -- sealed primitive-float
//! meanings plus each selected provider's ordinary checked body, rebound on a
//! private execution copy so the caller's tree keeps its authored selection.
//!
//! A type qualifier is not a runtime receiver. Reuse the typed call's resolved
//! entry and receiver classification, then evaluate that exact machine symbol;
//! rebuilding a name could select an unrelated same-spelled machine. Calls with
//! runtime receivers or unresolved arguments remain outside this closed route.
//! A type/const application is admitted only when its tuple closes.
//! Ordinary static specialization (its all-expression scan covers calls in
//! type positions) rewrites the call to a concrete instance, and the endpoint
//! then resolves that instance from the prepared tree. Its signature types
//! live in the prepared tree, so their positions and original argument
//! expressions read that same tree. Declaration-type calls in a machine retain
//! the same caller context as its body calls, so ordinary argument inference
//! can close a tuple. Pending discovery does not grant execution: missing or
//! conflicting binders still reject unless specialization produces a concrete
//! instance. Data-field calls still need explicit static arguments because
//! their owner has no machine argument-evaluation context.
//! Closed integer arguments share the type system's exact numeric evaluation,
//! with argument carrier and declaration-selection checks before interpreter
//! snapshots erase their authored types. No runtime flow bound supplies a value.
//! Closed range refinements use the same exact bound queries: input values must
//! satisfy every range before invocation, and returned values before folding.
//! A declared, argument-free integer domain on a parameter or result is proved
//! for the same concrete value through the shared domain-fact evaluator, so
//! `bounded(value: u64 in Positive)` admits `bounded(256)` and rejects
//! `bounded(0)` before the callee runs. Neither concrete check replaces
//! ordinary body checking. Wrapping/Saturating positions accept exact initial
//! anonymous landings or already-landed values with the same carrier and policy.
//! Calls, surrounding arithmetic and Match retain that policy through the shared
//! scalar evaluator; explicit conversion alone changes it. Published endpoint
//! literals preserve the completed landing, including its arithmetic policy.
//! Parameterized domains remain outside this route. Calls computing signature bounds are invocation
//! dependencies, independent of declaration order.
//!
//! Evaluation runs in rounds. Each round prepares the execution program from
//! the current working tree and folds every endpoint that closes; the driver
//! publishes those folds and prepares again when an invocation
//! failed while progress was made, because an instance's cloned signature
//! bounds are read from the prepared tree and a template bound that folded
//! this round (`bounded<const N>(value: u64[0..=limit()])`) is only visible
//! there after re-preparation. A round with no progress reports its
//! failures and every published fold is restored, so a rejected program
//! keeps its authored calls.
//! Type-argument bounds follow the same rounds: private preparation retains
//! their pending marks, and specialization leaves the enclosing call untouched
//! until those arguments validate. Even an unused binder cannot disappear
//! before its bound is evaluated. No pending application is executable.
//!
//! Each complete endpoint runs through the shared closed scalar evaluator.
//! Discovery collects roots independently of calls: removing a helper cannot
//! prevent a closed composition from reaching evaluation. Unclosed optional
//! roots remain authored for ordinary dependent/runtime range validation.
//! Its shape pass checks all calls and operators before selective execution;
//! the separate exhaustive custody walk preserves every original selection,
//! including skipped branches. Call results retain their declared carriers,
//! while an anonymous root remains an exact proof integer (including an
//! exclusive bound one beyond the subject carrier). No callee activation is
//! borrowed to interpret the caller's expression.
//! The evaluator reads an immutable prepared tree. Whole endpoint literals
//! and their deferral marks publish transactionally; the working tree supplies
//! previously folded signature bounds between rounds.

use crate::SelectedBuildTimeOperators;
use crate::build_time_evaluation::EvaluationDomain;
use diagnostics::Diagnostic;
use numerics::{
    arithmetic::ArithmeticDomain,
    literals::{IntegerLanding, IntegerLiteral, IntegerRadix, LandedIntegerType},
};
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

use crate::BuildTimeAdmissionPlan;

pub(super) mod arguments;
mod integer_type;

/// The builtin integer carrier's literal landing for endpoint literal
/// spellings — snapshot argument substitution and discharge stand-ins.
pub(super) fn integer_landing(carrier: PrimitiveType) -> Result<LandedIntegerType, String> {
    match carrier {
        PrimitiveType::I8 => Ok(LandedIntegerType::I8),
        PrimitiveType::I16 => Ok(LandedIntegerType::I16),
        PrimitiveType::I32 => Ok(LandedIntegerType::I32),
        PrimitiveType::I64 => Ok(LandedIntegerType::I64),
        PrimitiveType::U8 => Ok(LandedIntegerType::U8),
        PrimitiveType::U16 => Ok(LandedIntegerType::U16),
        PrimitiveType::U32 => Ok(LandedIntegerType::U32),
        PrimitiveType::U64 => Ok(LandedIntegerType::U64),
        _ => Err("range endpoint lost its builtin integer carrier".into()),
    }
}

/// Closed stand-in literals for the range bounds a private discharge probe
/// cannot keep authored: signature bounds are checked before the outer
/// endpoint evaluation completes, so a probe rewrites each still-spelled
/// bound at its own carrier's extreme — minimum at the carrier's least
/// spelling, maximum at its greatest. The rewritten constraint stays
/// inhabited and every caller's store-range proof keeps its room; the
/// stand-in only preserves signature checkability and never reaches the
/// call's contract judgment — the outer evaluation still computes and
/// publishes the bound's real fold. An endpoint that is already literal is
/// left alone; a bound whose carrier is not a builtin integer keeps a `0`
/// spelling, the most conservative closed choice.
pub(crate) fn deferred_endpoint_stand_ins(
    program: &TypedTrees,
) -> Vec<(ExpressionHandle, ExpressionNode)> {
    let mut stand_ins = Vec::new();
    for constrained_type in EvaluationDomain::all().references(program).type_references {
        let TypeReferenceNode::Constrained { constraints, .. } = program
            .type_reference_table
            .type_reference(constrained_type)
        else {
            continue;
        };
        let carrier = crate::const_evaluation::const_generic_expressions::exact_probe_destination(
            program,
            constrained_type,
        );
        for constraint in program.type_reference_table.constraints(*constraints) {
            let TypeConstraintNode::Range {
                minimum, maximum, ..
            } = constraint
            else {
                continue;
            };
            for (endpoint, minimum_role) in [(*minimum, true), (*maximum, false)] {
                if matches!(
                    program.expression_table.expression(endpoint),
                    ExpressionNode::Integer(_)
                ) {
                    continue;
                }
                let (negative, spelling) = endpoint_stand_in(carrier, minimum_role);
                let literal = IntegerLiteral::from_parts(negative, IntegerRadix::Decimal, spelling)
                    .map(|literal| {
                        match carrier.and_then(|carrier| integer_landing(carrier).ok()) {
                            Some(landed_type) => literal.with_landing(IntegerLanding {
                                landed_type,
                                domain: ArithmeticDomain::Exact,
                            }),
                            None => literal,
                        }
                    });
                let Ok(literal) = literal else {
                    continue;
                };
                stand_ins.push((endpoint, ExpressionNode::Integer(literal)));
            }
        }
    }
    stand_ins
}

/// The extreme spelling a rewritten endpoint bound takes at `carrier`:
/// minimums take the least value the carrier can hold, maximums the
/// greatest, so any literal the sibling bound spells stays within the
/// rewritten range and caller store proofs keep their room.
fn endpoint_stand_in(carrier: Option<PrimitiveType>, minimum: bool) -> (bool, &'static str) {
    match (carrier, minimum) {
        (Some(PrimitiveType::I8), true) => (true, "128"),
        (Some(PrimitiveType::I8), false) => (false, "127"),
        (Some(PrimitiveType::I16), true) => (true, "32768"),
        (Some(PrimitiveType::I16), false) => (false, "32767"),
        (Some(PrimitiveType::I32), true) => (true, "2147483648"),
        (Some(PrimitiveType::I32), false) => (false, "2147483647"),
        (Some(PrimitiveType::I64), true) => (true, "9223372036854775808"),
        (Some(PrimitiveType::I64), false) => (false, "9223372036854775807"),
        (Some(PrimitiveType::U8), false) => (false, "255"),
        (Some(PrimitiveType::U16), false) => (false, "65535"),
        (Some(PrimitiveType::U32), false) => (false, "4294967295"),
        (Some(PrimitiveType::U64), false) => (false, "18446744073709551615"),
        (_, true) => (false, "0"),
        (_, false) => (false, "9223372036854775807"),
    }
}

pub(crate) struct PendingEndpoint {
    pub(crate) expression: ExpressionHandle,
    pub(crate) root: ExpressionHandle,
    /// The resolved callee in the working tree: a plain machine, or the
    /// generic template of a static application, explicit or inferred.
    pub(crate) machine: symbols::SymbolHandle,
    /// The authored target is generic; the executable callee must be the
    /// prepared tree's specialized instance.
    pub(crate) static_application: bool,
}

struct EndpointRoot {
    constrained_type: TypeReferenceHandle,
    expression: ExpressionHandle,
}

/// Every nonliteral bound is a candidate for the shared evaluator. Calls
/// separately name required constant invocations and provider dependencies;
/// an unclosed call-free bound remains owned by ordinary range validation.
struct EndpointPlan {
    roots: Vec<EndpointRoot>,
    calls: Vec<PendingEndpoint>,
}

pub fn evaluate_const_range_endpoints(
    typed: &mut TypedTrees,
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
) -> Result<(), Vec<Diagnostic>> {
    evaluate_selected_range_endpoints(
        typed,
        selection_authority,
        SelectedBuildTimeOperators {
            operators: &[],
            provider_bodies: &[],
        },
        &EvaluationDomain::all(),
    )
}

/// Whether any still-authored endpoint call's reachable machine closure holds
/// a resolved boundary-operator use that only exact selected execution can
/// run. Mirrors the fixed-array-length deferral gate: the owning continuation
/// waits for Omega's settled rows rather than failing such an endpoint against
/// an unselected admission.
pub(crate) fn pending_endpoint_calls_need_operator_selection(
    typed: &TypedTrees,
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    domain: &EvaluationDomain,
) -> Result<bool, Vec<Diagnostic>> {
    let pending = pending_endpoints(typed, domain)?;
    if pending.is_empty() {
        return Ok(false);
    }
    // Admission and the deferral scan must see the same closed static
    // applications as the eventual evaluation; prepare once and reuse it.
    let prepared = crate::PreparedBuildMachineProgram::prepare_in(typed, domain)?;
    let execution = prepared.typed();
    let facts = validation::derive_pre_flow_operator_selections(execution);
    let admission = domain.admission(execution, selection_authority);
    Ok(pending.iter().any(|endpoint| {
        // A type argument awaiting its own bound keeps this application
        // unspecialized. Its original template can still demand a provider;
        // this conservative deferral scan grants no invocation admission.
        let machine = resolve_endpoint_callee(execution, endpoint)
            .map_or(endpoint.machine, |callee| callee.instance);
        admission.closure_needs_selected_execution(execution, machine, &facts)
    }))
}

/// The executable instance and the arena owning its prepared signature.
#[derive(Clone, Copy)]
pub(super) struct EndpointCallee {
    pub(super) instance: symbols::SymbolHandle,
    pub(super) static_application: bool,
}

impl EndpointCallee {}

/// Resolve the executable callee in the prepared tree. A static application
/// must have been rewritten there to its concrete instance; a call that still
/// carries static arguments was not specialized, which after `prepare` means
/// a binder is missing or open, so ask for the explicit argument.
pub(super) fn resolve_endpoint_callee(
    execution: &TypedTrees,
    endpoint: &PendingEndpoint,
) -> Result<EndpointCallee, String> {
    if !endpoint.static_application {
        return Ok(EndpointCallee {
            instance: endpoint.machine,
            static_application: false,
        });
    }
    let Some(call) = execution.expression_table.call_node(endpoint.expression) else {
        return Err("range endpoint lost its authored static application".to_owned());
    };
    let template = execution
        .machine(endpoint.machine)
        .ok_or("range endpoint lost its generic template")?;
    if !call.machine_arguments.is_empty() {
        if call.machine_arguments.len() == template.type_parameters.len() {
            return Err(format!(
                "static application of `{}` awaits a complete validated specialization",
                template.name,
            ));
        }
        return Err(format!(
            "static application of `{}` supplies {} of {} static arguments; supply every static argument explicitly",
            template.name,
            call.machine_arguments.len(),
            template.type_parameters.len(),
        ));
    }
    let instance = execution
        .machines()
        .iter()
        .find(|machine| {
            typed_trees::state::state_by_symbol(
                execution.machine_states(machine),
                call.target_symbol,
            )
            .is_some()
        })
        .filter(|machine| machine.type_parameters.is_empty())
        .ok_or_else(|| {
            format!(
                "static application of `{}` was not specialized to a concrete instance",
                template.name
            )
        })?;
    Ok(EndpointCallee {
        instance: instance.symbol,
        static_application: true,
    })
}

/// Record every still-authored endpoint call as deferred to selected
/// execution. Interim checking passes over the typed tree before the owning
/// continuation resumes, so the marks keep a pending endpoint from being
/// misread as a non-constant or dependent bound; each fold removes its mark.
pub(crate) fn defer_pending_endpoint_calls(typed: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    defer_pending_endpoint_calls_in(typed, &EvaluationDomain::all())
}

pub(crate) fn defer_pending_endpoint_calls_in(
    typed: &mut TypedTrees,
    domain: &EvaluationDomain,
) -> Result<(), Vec<Diagnostic>> {
    for endpoint in pending_endpoints(typed, domain)? {
        typed
            .pending_const_range_endpoints
            .insert(endpoint.expression);
        typed.pending_const_range_endpoints.insert(endpoint.root);
    }
    Ok(())
}

/// Finish still-authored endpoint calls under the exact selected rows Omega
/// settled for this program. `operators` supplies the sealed primitive-float
/// meanings; `provider_bodies` rebinds each retained boundary-operator
/// occurrence to its selected provider's ordinary checked body on the private
/// execution copy -- the caller's tree keeps its authored selection and
/// source-owned handles for final checking, exactly as deferred fixed-array
/// lengths do.
pub(crate) fn evaluate_selected_range_endpoints(
    typed: &mut TypedTrees,
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    selected: crate::SelectedBuildTimeOperators<'_>,
    domain: &EvaluationDomain,
) -> Result<(), Vec<Diagnostic>> {
    let crate::SelectedBuildTimeOperators {
        operators,
        provider_bodies,
    } = selected;
    // Folds published by earlier rounds, with their authored originals and
    // whether the deferral path had marked them. A round that ends in
    // rejection restores every one of them so a rejected program keeps its
    // authored calls, exactly as a single failing round did.
    let mut published: Vec<(ExpressionHandle, ExpressionNode, Vec<ExpressionHandle>)> = Vec::new();
    let mut warnings = Vec::new();
    let restore =
        |typed: &mut TypedTrees,
         published: Vec<(ExpressionHandle, ExpressionNode, Vec<ExpressionHandle>)>| {
            for (expression, original, marked) in published.into_iter().rev() {
                *typed.expression_table.expression_mut(expression) = original;
                typed.pending_const_range_endpoints.extend(marked);
            }
        };
    loop {
        let plan = match endpoint_plan(typed, domain) {
            Ok(plan) => plan,
            Err(errors) => {
                restore(typed, published);
                return Err(errors);
            }
        };
        if plan.roots.is_empty() {
            break;
        }
        let round = match evaluate_round(
            typed,
            &plan,
            selection_authority.clone(),
            operators,
            provider_bodies,
            &mut warnings,
            domain,
        ) {
            Ok(round) => round,
            // Optional closed probing must not make preparation a new
            // requirement for runtime or dependent range declarations.
            Err(_) if plan.calls.is_empty() => break,
            Err(errors) => {
                restore(typed, published);
                return Err(errors);
            }
        };
        let progress = !round.folds.is_empty();
        for (expression, folded) in round.folds {
            let original =
                std::mem::replace(typed.expression_table.expression_mut(expression), folded);
            let mut marked: Vec<_> = plan
                .calls
                .iter()
                .filter(|call| call.root == expression)
                .filter_map(|call| {
                    typed
                        .pending_const_range_endpoints
                        .remove(&call.expression)
                        .then_some(call.expression)
                })
                .collect();
            if typed.pending_const_range_endpoints.remove(&expression) {
                marked.push(expression);
            }
            published.push((expression, original, marked));
        }
        if round.diagnostics.is_empty() {
            break;
        }
        // A static tuple may belong to this endpoint or a transitive callee.
        // Reprepare after a fold so either closure sees its newly closed types.
        // Each progressing round permanently removes a nonliteral root; no
        // progress means another round would repeat the same failures.
        if !progress {
            restore(typed, published);
            return Err(round.diagnostics);
        }
    }
    for warning in warnings {
        eprintln!("{warning}");
    }
    Ok(())
}

/// One round over the still-authored endpoint roots against a program
/// prepared from the current working tree.
struct Round {
    /// Successfully folded whole bounds, in dependency order.
    folds: Vec<(ExpressionHandle, ExpressionNode)>,
    diagnostics: Vec<Diagnostic>,
}

fn evaluate_round(
    typed: &mut TypedTrees,
    plan: &EndpointPlan,
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    operators: &[crate::SelectedBuildTimeBinaryOperator],
    provider_bodies: &[crate::SelectedBuildTimeProviderBody],
    warnings: &mut Vec<Diagnostic>,
    domain: &EvaluationDomain,
) -> Result<Round, Vec<Diagnostic>> {
    // Bind exact authored provider occurrences before cloning specializations.
    // Rebinding only the template after preparation would leave its cloned
    // body with an unselected operator. Both steps remain private, and the
    // existing provider validator checks the original occurrence first.
    let (prepared, admission) = crate::const_evaluation::prepared_admission(
        typed,
        provider_bodies,
        operators,
        selection_authority.clone(),
        domain,
    )?;
    let execution = prepared.typed();

    let mut round = Round {
        folds: Vec::new(),
        diagnostics: Vec::new(),
    };
    let mut originals = Vec::new();
    for endpoint in &plan.roots {
        let result = evaluate_endpoint(
            typed,
            execution,
            &admission,
            endpoint,
            selection_authority.as_deref(),
            warnings,
        );
        match result {
            Ok(folded) => {
                // Later endpoints in this round read this fold from the
                // working tree (surrounding arithmetic, enclosing arguments).
                let original = std::mem::replace(
                    typed.expression_table.expression_mut(endpoint.expression),
                    folded.clone(),
                );
                originals.push((endpoint.expression, original));
                round.folds.push((endpoint.expression, folded));
            }
            Err(reason) => {
                let required = plan
                    .calls
                    .iter()
                    .any(|call| call.root == endpoint.expression);
                if !required {
                    continue;
                }
                round.diagnostics.push(Diagnostic::error(format!(
                    "range endpoint of `{}`: const evaluation failed: {reason}",
                    typed
                        .type_reference_table
                        .display_name(endpoint.constrained_type),
                )));
            }
        }
    }

    // Execute against the immutable prepared program, never a graph changed
    // underneath its admission plan. The working tree only supplies closed
    // argument arithmetic and signature bounds. Restore it before the driver
    // publishes any folds so a failed outer call cannot leave its
    // successfully evaluated children erased.
    for (expression, original) in originals.into_iter().rev() {
        *typed.expression_table.expression_mut(expression) = original;
    }
    Ok(round)
}

fn evaluate_endpoint(
    typed: &TypedTrees,
    execution: &TypedTrees,
    admission: &BuildTimeAdmissionPlan,
    endpoint: &EndpointRoot,
    selection_authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<ExpressionNode, String> {
    crate::machine_execution::admission::require_closed_expression_custody(
        execution,
        endpoint.expression,
        selection_authority,
    )?;
    let invocation = arguments::Invocation {
        working: typed,
        execution,
        admission,
        authority: selection_authority,
    };
    let (literal, additions) =
        crate::const_evaluation::const_generic_expressions::value::evaluate_integer_endpoint(
            execution,
            endpoint.expression,
            &invocation,
        )?;
    for warning in additions {
        if !warnings.contains(&warning) {
            warnings.push(warning);
        }
    }
    Ok(ExpressionNode::Integer(literal))
}

pub(crate) fn pending_endpoints(
    typed: &TypedTrees,
    domain: &EvaluationDomain,
) -> Result<Vec<PendingEndpoint>, Vec<Diagnostic>> {
    Ok(endpoint_plan(typed, domain)?.calls)
}

fn endpoint_plan(
    typed: &TypedTrees,
    domain: &EvaluationDomain,
) -> Result<EndpointPlan, Vec<Diagnostic>> {
    let mut pending = Vec::new();
    let mut roots = Vec::new();
    let mut visited = Vec::new();
    for constrained_type in domain.references(typed).type_references {
        let TypeReferenceNode::Constrained { constraints, .. } =
            typed.type_reference_table.type_reference(constrained_type)
        else {
            continue;
        };
        for constraint in typed.type_reference_table.constraints(*constraints) {
            let TypeConstraintNode::Range {
                minimum, maximum, ..
            } = constraint
            else {
                continue;
            };
            let mut work = vec![(*maximum, false, *maximum), (*minimum, false, *minimum)];
            let mut active = Vec::new();
            while let Some((expression, leaving, root)) = work.pop() {
                if !typed.expression_table.expression_is_valid(expression) {
                    continue;
                }
                if !leaving {
                    if active.contains(&expression) {
                        return Err(vec![Diagnostic::error("cyclic range endpoint expression")]);
                    }
                    if visited.contains(&(expression, root)) {
                        continue;
                    }
                    active.push(expression);
                    work.push((expression, true, root));
                    let children =
                        crate::machine_execution::admission::expression_children(typed, expression);
                    work.extend(children.into_iter().rev().map(|child| (child, false, root)));
                    if let Some((machine, _)) = selected_endpoint_machine(typed, expression) {
                        append_signature_bounds(typed, machine, &mut work)?;
                    }
                    continue;
                }
                active.pop();
                visited.push((expression, root));
                if expression == root
                    && !matches!(
                        typed.expression_table.expression(root),
                        ExpressionNode::Integer(_)
                    )
                {
                    roots.push(EndpointRoot {
                        constrained_type,
                        expression: root,
                    });
                }
                let Some((machine, static_application)) =
                    selected_endpoint_machine(typed, expression)
                else {
                    continue;
                };
                pending.push(PendingEndpoint {
                    expression,
                    root,
                    machine: machine.symbol,
                    static_application,
                });
            }
        }
    }
    Ok(EndpointPlan {
        roots,
        calls: pending,
    })
}

/// The endpoint's callee in the working tree and whether the call is an
/// static application. Returns `None` for calls this route never
/// folds: runtime receivers and evidence/dispatch forms. Generic callees still
/// require an exact concrete instance from ordinary specialization.
pub(super) fn selected_endpoint_machine(
    typed: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<(&typed_trees::machine::Machine, bool)> {
    let call = typed.expression_table.call_node(expression)?;
    if !call.evidence_arguments.is_empty()
        || call.static_machine_parameter.is_valid()
        || !call.selects_only_nominal_route()
    {
        return None;
    }
    // Discover type/const applications, not a second specialization
    // rule. Preparation validates types in the caller's scope and resolves the
    // complete tuple; resolve_endpoint_callee then requires its concrete
    // instance. The instance's substituted signature owns scalar admission.
    // Machine/evidence applications still need their full dispatch context.
    if !call.machine_arguments.iter().all(|argument| {
        argument.application.is_none()
            && argument.evidence_projection.is_none()
            && (argument.type_reference.is_valid()
                || matches!(
                    typed.symbols.get(argument.symbol).kind,
                    symbols::SymbolKind::BuiltinType | symbols::SymbolKind::Data
                )
                || argument.const_literal.is_some()
                || typed.const_declaration_by_symbol(argument.symbol).is_some())
    }) {
        return None;
    }
    let machine = typed.machines().iter().find(|machine| {
        typed
            .machine_states(machine)
            .first()
            .is_some_and(|entry| typed.call_has_no_runtime_receiver(call, machine, entry))
    })?;
    if machine.type_parameters.is_empty() {
        return call
            .machine_arguments
            .is_empty()
            .then_some((machine, false));
    }
    // Discovery is not inference or admission. Let ordinary specialization
    // infer from declared argument types, then require its complete instance
    // in resolve_endpoint_callee before interpreting any value. An unused,
    // underdetermined binder cannot disappear merely because this is a bound.
    machine
        .conformance_bounds
        .is_empty()
        .then_some((machine, true))
}

fn append_signature_bounds(
    typed: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    work: &mut Vec<(ExpressionHandle, bool, ExpressionHandle)>,
) -> Result<(), Vec<Diagnostic>> {
    let Some(entry) = typed.machine_states(machine).first() else {
        return Ok(());
    };
    // A called declaration may appear later in source. Its static input/result
    // bounds are dependencies of invocation admission, not a reason to rely on
    // the type arena's order or repeatedly retry unsuccessful evaluations.
    for mut reference in std::iter::once(entry.return_type).chain(
        typed
            .state_parameters(entry)
            .iter()
            .map(|parameter| parameter.type_reference),
    ) {
        let mut visited = Vec::new();
        while let typed_trees::types::TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = typed.type_reference_table.type_reference(reference)
        {
            if visited.contains(&reference) {
                return Err(vec![Diagnostic::error(
                    "cyclic range endpoint integer type",
                )]);
            }
            visited.push(reference);
            for constraint in typed.type_reference_table.constraints(*constraints) {
                if let TypeConstraintNode::Range {
                    minimum, maximum, ..
                } = constraint
                {
                    work.push((*maximum, false, *maximum));
                    work.push((*minimum, false, *minimum));
                }
            }
            reference = *base_type;
        }
    }
    Ok(())
}
