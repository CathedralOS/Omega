//! Checked discharge of authored `requires` premises in a call closure.
//!
//! An authored `requires` premise has no meaning outside an invocation, so
//! the common floor fences every call closure carrying one until ordinary
//! contract checking proves it. Two routes produce that proof here.
//!
//! A premise on the invoked machine or its entry state depends on the
//! invocation's arguments. Positions that own an authored call node —
//! constant initializers — build a checked probe around it. A const-position
//! leg such as a fixed-array length's `machine()` owns no call node, but the
//! invocation is just as concrete: the exact call is `machine()` at the leg's
//! own source span. Admission therefore builds the private probe itself,
//! appending a generated machine whose body is the synthesized call, and
//! treats a clean checked lowering with no retained crash routes as the
//! discharge evidence.
//!
//! Every other premise — on a callee, a later state, or a callable target —
//! sits at a source call or transition site whose proof reads only that
//! caller's own facts. When the entry carries no premise, a clean checked
//! lowering of the demanded checking closure proves it for any arguments, so an
//! argument-carrying policy or layout invocation needs no synthesized call.
//! Post-check evaluation borrows that judgment from the same checked program;
//! early typed-only evaluation still establishes it on the demanded closure.
//!
//! Every other floor axis — service reach, suspension, blocking,
//! termination, linear carriers, declaration-selection authority — is
//! enforced unchanged.

use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableTransition, TransitionTargetNode};

use super::BuildTimeProgram;
use super::closure_validation::has_authored_requires;
use crate::syntax_probes::PreparedEvaluationInputs;
use typed_trees_to_checked_trees::lower_typed_trees;
use typed_trees_to_typed_trees::prepare_typed_trees;

/// Whether a private checked probe discharges `machine`'s authored
/// `requires` premises for the concrete zero-argument call `machine()`.
/// Returns `true` only when the synthesized call survives ordinary checked
/// lowering and the probe machine retains no unhandled crash routes; a
/// premise that ordinary checking cannot prove at that call keeps the
/// conservative closure fence.
pub(super) fn zero_argument_invocation_discharges(
    program: &TypedTrees,
    machine: &Machine,
    source_span: Option<source::SourceSpan>,
) -> bool {
    value_invocation_discharges(program, machine, &[], source_span).is_ok()
}

/// Check the exact supplied values through an ordinary caller. A guarded
/// public ceiling is specialized at this call, never replaced by body purity.
pub(super) fn value_invocation_discharges(
    program: &TypedTrees,
    machine: &Machine,
    arguments: &[crate::BuildTimeValue],
    source_span: Option<source::SourceSpan>,
) -> Result<(), String> {
    let entry =
        entry_state(program, machine).ok_or("constant invocation has no exact entry state")?;
    let parameters = program.state_parameters(entry);
    if parameters.len() != arguments.len() {
        return Err("constant invocation arguments differ from its checked entry arity".into());
    }
    let mut probe = program.clone();
    probe.pending_const_range_endpoints.clear();
    let (probe_machine, mut state) = invocation_probe_declarations(
        &mut probe,
        machine.symbol,
        "@const-invocation",
        entry.return_type,
    );
    let mut actuals = Vec::with_capacity(arguments.len());
    for (position, (parameter, argument)) in parameters.iter().zip(arguments).enumerate() {
        if matches!(argument, crate::BuildTimeValue::LayoutTarget(_)) {
            // Validated compiler contexts have no source constructor. Preserve
            // the exact carrier as a caller formal and prove the call for every
            // value of that opaque input; do not invent concrete target facts.
            let name = format!("@context-{position}");
            let symbol = probe
                .symbols
                .insert_generated_children(state.symbol, [(SymbolKind::Parameter, name.as_str())])
                .start();
            let mut retained = parameter.clone();
            retained.symbol = symbol;
            retained.name = typed_trees::name::Identifier::generated(&name);
            probe.push_state_parameter(&mut state, retained.clone());
            let path = probe_name_path(&mut probe, symbol, retained.name);
            actuals.push(probe.expression_table.insert(ExpressionNode::Name(path)));
            continue;
        }
        let mut carrier = parameter.type_reference;
        while let typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } =
            program.type_reference_table.type_reference(carrier)
        {
            carrier = *base_type;
        }
        let reference = match program.type_reference_table.type_reference(carrier) {
            typed_trees::types::TypeReferenceNode::Reference {
                referee, access, ..
            } => Some((*referee, *access)),
            _ => None,
        };
        let mut snapshot = argument.clone();
        let value = super::supplied_values::literal(
            &mut probe,
            reference.map_or(parameter.type_reference, |(referee, _)| referee),
            &mut snapshot,
        )
        .map_err(|reason| format!("constant argument `{}`: {reason}", parameter.name))?;
        let actual = if let Some((referee, access)) = reference {
            // The evaluator supplies fresh owned storage for borrowed snapshots.
            // Give ordinary checking the same local home and access permission.
            let name = format!("@argument-{position}");
            let symbol = probe
                .symbols
                .insert_generated_children(state.symbol, [(SymbolKind::Local, name.as_str())])
                .start();
            let name = typed_trees::name::Identifier::generated(&name);
            probe.statement_table.push_statement(
                &mut state.statement_nodes,
                StatementNode::LocalData(typed_trees::statement::TableLocalData {
                    symbol,
                    name: name.clone(),
                    type_reference: referee,
                    initial_value: value,
                    is_mutable: access.is_exclusive(),
                    ..Default::default()
                }),
            );
            let path = probe_name_path(&mut probe, symbol, name);
            let target = probe.expression_table.insert(ExpressionNode::Name(path));
            probe.expression_table.insert(ExpressionNode::Borrow(
                typed_trees::expression::TableBorrowExpression { target, access },
            ))
        } else {
            value
        };
        actuals.push(actual);
    }
    let call = TableCallExpression {
        authored_call_selection: None,
        authored_call_binding: None,
        authored_machine_application: Default::default(),
        requirement_application: Default::default(),
        exact_coercion_target: typed_trees::types::TypeReferenceHandle::invalid(),
        receiver: ExpressionHandle::invalid(),
        target_symbol: entry.symbol,
        static_machine_parameter: SymbolHandle::invalid(),
        target: entry.name.clone(),
        static_requirement_dispatch: None,
        machine_arguments: Box::default(),
        quotient_operation: None,
        private_layout_operation: None,
        layout_query: None,
        native_code_layout: None,
        build_selection: Default::default(),
        arguments: probe.expression_table.insert_expression_handles(actuals),
        evidence_arguments: Box::default(),
        operational_acknowledgement: language_semantics::CallOperationalAcknowledgement::default(),
    };
    let concrete = probe.expression_table.insert(ExpressionNode::Call(call));
    let call_span = source_span
        .or_else(|| probe.symbols.symbol_source_span(machine.symbol))
        .unwrap_or_default();
    probe.expression_table.set_source_span(concrete, call_span);
    let owner = finish_invocation_probe(&mut probe, probe_machine, state, concrete);
    let preparation_request = typed_trees_to_typed_trees::PreparationRequest::preliminary();
    let checking = typed_trees_to_checked_trees::CheckingRequest::new();
    let checked = prepare_typed_trees(probe, &preparation_request)
        .and_then(|(probe, preparation)| lower_typed_trees(probe, preparation, &checking))
        .map_err(|diagnostics| {
            format!(
                "constant invocation failed ordinary checking: {}",
                diagnostics::join_messages(&diagnostics),
            )
        })?;
    require_complete_invocation_conditions(&checked, owner)?;
    require_empty_failure_summary(&checked, owner)
}

fn probe_name_path(
    probe: &mut TypedTrees,
    symbol: SymbolHandle,
    name: typed_trees::name::Identifier,
) -> typed_trees::expression::TableNamePath {
    let mut members = arena::HandleSpan::empty();
    probe
        .expression_table
        .push_name_path_member(&mut members, name);
    let mut member_symbols = arena::HandleSpan::empty();
    probe
        .expression_table
        .push_name_path_member_symbol(&mut member_symbols, symbol);
    typed_trees::expression::TableNamePath::new(members, member_symbols, symbol, symbol)
}

pub(super) fn require_empty_failure_summary(
    checked: &checked_trees::CheckedTrees,
    owner: SymbolHandle,
) -> Result<(), String> {
    let summary = checked
        .body_crash_summary(owner)
        .ok_or("constant invocation has no complete checked failure summary")?;
    if summary.buckets().is_empty() {
        Ok(())
    } else {
        let mut causes = summary
            .buckets()
            .iter()
            .map(|bucket| bucket.cause())
            .collect::<Vec<_>>();
        causes.sort();
        causes.dedup();
        Err(format!(
            "constant invocation retains unhandled {causes:?} routes at its concrete arguments"
        ))
    }
}

/// Whether ordinary checking alone discharges every authored `requires`
/// premise in `machine`'s call closure, whatever the invocation's arguments.
/// Holds only when neither the machine nor its entry state carries an
/// authored premise and ordinary checking has established its internal calls.
/// Reuse that judgment when supplied; otherwise check the demanded closure.
/// An entry premise keeps its concrete routes or the conservative fence.
pub(super) fn argument_independent_closure_discharges(
    admission: &super::BuildTimeAdmissionPlan,
    input: BuildTimeProgram<'_>,
    machine: &Machine,
) -> Result<bool, String> {
    let program = input.typed();
    let entry_is_unconditional = !has_authored_requires(program.machine_contracts(machine))
        && entry_state(program, machine)
            .is_some_and(|entry| !has_authored_requires(program.state_contracts(entry)));
    if !entry_is_unconditional {
        return Ok(false);
    }
    // The same immutable checked program already established all internal
    // call/transition premises. This proves nothing about entry arguments,
    // hence the unconditional-entry test above remains mandatory. The caller
    // still checks every independent execution-admission axis.
    if let BuildTimeProgram::Checked(checked) = input {
        require_complete_invocation_conditions(checked, machine.symbol)?;
        return Ok(true);
    }
    admission.checked_closure_admission(program, machine.symbol)?;
    Ok(true)
}

/// Retain the complete ordinary checking judgment for the demanded execution.
/// Consumers must not discard operator, custody or pending-condition facts
/// when crossing from source admission into the existing checked evaluator.
pub(super) fn checked_demanded_closure(
    domain: &crate::build_time_evaluation::EvaluationDomain,
    program: &TypedTrees,
) -> Result<checked_trees::CheckedTrees, String> {
    // A layout policy may be admitted before unrelated runtime declarations
    // have their authoritative layout/accessor plans. Those declarations are
    // not premises of this invocation. Close the root's real type, proof,
    // operator and body dependencies through the shared evaluation projection;
    // retain their contracts unchanged and check that exact demanded program.
    let mut probe = domain.execution_program(program);
    // Same preliminary-package window as the zero-argument probe above.
    probe.pending_const_range_endpoints.clear();
    let preparation_request = typed_trees_to_typed_trees::PreparationRequest::preliminary();
    let checking = typed_trees_to_checked_trees::CheckingRequest::new();
    let (probe, preparation) =
        prepare_typed_trees(probe, &preparation_request).map_err(|error| {
            format!(
                "demanded premise closure preparation failed: {}",
                diagnostics::join_messages(&error)
            )
        })?;
    let checked = lower_typed_trees(probe, preparation, &checking).map_err(|error| {
        format!(
            "demanded premise closure checking failed: {}",
            diagnostics::join_messages(&error)
        )
    })?;
    Ok(checked)
}

/// The state an invocation of `machine` enters: the one named by the
/// machine's leaf, else its first state.
pub(super) fn entry_state<'a>(program: &'a TypedTrees, machine: &Machine) -> Option<&'a State> {
    let leaf = machine
        .name
        .as_str()
        .rsplit("::")
        .next()
        .unwrap_or_default();
    let states = program.machine_states(machine);
    states
        .iter()
        .find(|state| state.name.as_str() == leaf)
        .or_else(|| states.first())
}

/// One evaluated operand substituted into an independent copy of the call.
/// Its node belongs to the supplied program, including any aggregate children.
pub(crate) struct SnapshotArgument {
    pub original: ExpressionHandle,
    pub literal: ExpressionNode,
}

/// Whether `expression`'s authored call survives ordinary checked lowering
/// with its operands substituted by evaluated snapshots — the concrete
/// discharge evidence for authored `requires` premises on the invoked
/// machine or its entry state, and for any unhandled failure route at the
/// exact call. A published guarded ceiling has no private whole-body
/// summary, and an authored premise has no meaning outside an invocation,
/// so admission checks the exact call rather than inspecting the callee
/// body; successful interpretation is never admission evidence.
/// `retained` names the machines the probe program keeps. Both current
/// callers keep the whole authored program (every non-probe machine):
/// library satisfaction references and declaration reach read machines far
/// outside the callee's call closure.
pub(crate) fn snapshot_arguments_discharge(
    admission: &super::BuildTimeAdmissionPlan,
    program: &TypedTrees,
    retained: &[SymbolHandle],
    owner: SymbolHandle,
    probe_name: &str,
    preparation_request: &typed_trees_to_typed_trees::PreparationRequest,
    checking: &typed_trees_to_checked_trees::CheckingRequest,
    expression: ExpressionHandle,
    callee: &str,
    return_type: typed_trees::types::TypeReferenceHandle,
    snapshots: Vec<SnapshotArgument>,
) -> Result<(), String> {
    // The private invocation checks its evaluated argument snapshots. Pending
    // contract folds belong to the outer continuation, not this private check.
    let mut probe = program.clone();
    // The graph copy shares RuntimeValue operands with their saved bindings.
    if probe.expression_table.call_node(expression).is_none() {
        return Err("constant invocation has no selected call".into());
    }
    let copied = probe.expression_table.copy_graph(None, expression);
    typed_trees_to_typed_trees::restore_copied_call_sources(&mut probe, &[copied.root])
        .map_err(|reason| format!("constant call source restoration failed: {reason}"))?;
    probe.pending_const_range_endpoints.clear();
    let machines = probe
        .machines()
        .iter()
        .filter(|candidate| retained.contains(&candidate.symbol))
        .cloned()
        .collect::<Vec<_>>();
    probe.roots.machines = arena::HandleSpan::empty();
    for candidate in machines {
        probe.push_machine(candidate);
    }
    for snapshot in snapshots {
        let copied_argument = copied
            .correspondence
            .iter()
            .find(|(original, _)| *original == snapshot.original)
            .map(|(_, copied)| *copied)
            .ok_or("constant snapshot has no exact copied operand")?;
        // Replacing the node preserves every copied edge to this evaluation,
        // including the source application that will regenerate a Value tail.
        *probe.expression_table.expression_mut(copied_argument) = snapshot.literal;
    }
    let concrete = copied.root;
    let owner = append_invocation_probe(&mut probe, owner, probe_name, concrete, return_type);
    let checked = prepare_typed_trees(probe, preparation_request)
        .and_then(|(probe, preparation)| lower_typed_trees(probe, preparation, checking))
        .map_err(|diagnostics| {
            format!(
                "constant invocation of `{callee}` failed checking: {}",
                diagnostics::join_messages(&diagnostics)
            )
        })?;
    require_complete_invocation_conditions(&checked, owner)?;
    require_empty_failure_summary(&checked, owner)?;
    require_public_call_floor(&checked, owner, concrete, callee)?;
    // A selected body may accept more inputs or advertise fewer effects than
    // its public requirement. Admit the checked call itself, after its concrete
    // premises are proved, so that public ceiling remains part of the floor.
    let plan = admission.checked_program_admission(&checked)?;
    let machine = checked
        .typed
        .machine_by_symbol(owner)
        .ok_or("constant snapshot invocation lost its checked machine")?;
    plan.require_floor(
        BuildTimeProgram::Checked(&checked),
        machine,
        Some(crate::BuildTimeInvocationCustody::Source(
            program.expression_table.source_span(expression),
        )),
        true,
    )
    .map_err(String::from)
}

/// A selected implementation may reach fewer services than its public method.
/// Check the applied public envelope as well as the executable closure below.
fn require_public_call_floor(
    checked: &checked_trees::CheckedTrees,
    owner: SymbolHandle,
    expression: ExpressionHandle,
    callee: &str,
) -> Result<(), String> {
    let program = checked.typed.as_ref();
    let call = program
        .expression_table
        .call_node(expression)
        .ok_or("checked constant invocation has no call expression")?;
    let site = validation::CallSite::Expression { expression, call };
    if call.static_requirement_dispatch.is_none()
        && validation::selected_requirement_signature(program, call.target_symbol).is_none()
    {
        return Ok(());
    }
    let (_, signature) = validation::resolve_requirement_contract(program, owner, &site)?;
    let services = validation::trait_signature_service_reach(checked, signature.symbol)
        .ok_or("constant call has no exact applied public service envelope")?;
    let services = services
        .iter()
        .map(|service| {
            program
                .service_reaches
                .definition(*service)
                .map(|definition| definition.name.to_string())
                .ok_or("constant call's public envelope names an unavailable service")
        })
        .collect::<Result<Vec<_>, _>>()?;
    semantic_vocabulary::compiler_evaluation::require_compiler_evaluation_floor(
        callee,
        &semantic_vocabulary::compiler_evaluation::CompilerEvaluationEffects {
            services,
            may_suspend: signature.suspends,
            may_block: signature.blocks,
        },
        None,
        None,
    )
}

/// Append the generated probe machine whose single state returns the
/// synthesized call: `machine @const-length() -> <entry return> { machine() }`.
pub(super) fn append_invocation_probe(
    probe: &mut TypedTrees,
    owner: SymbolHandle,
    name: &str,
    expression: ExpressionHandle,
    destination: typed_trees::types::TypeReferenceHandle,
) -> SymbolHandle {
    let (machine, state) = invocation_probe_declarations(probe, owner, name, destination);
    finish_invocation_probe(probe, machine, state, expression)
}

fn invocation_probe_declarations(
    probe: &mut TypedTrees,
    owner: SymbolHandle,
    name: &str,
    destination: typed_trees::types::TypeReferenceHandle,
) -> (Machine, State) {
    let symbol = probe
        .symbols
        .insert_generated_root_from(owner, SymbolKind::Machine, name);
    let children = probe
        .symbols
        .insert_generated_children(symbol, [(SymbolKind::State, name)]);
    (
        Machine {
            symbol,
            name: typed_trees::name::Identifier::generated(name),
            ..Default::default()
        },
        State {
            symbol: children.start(),
            name: typed_trees::name::Identifier::generated(name),
            return_type: destination,
            ..Default::default()
        },
    )
}

fn finish_invocation_probe(
    probe: &mut TypedTrees,
    mut machine: Machine,
    mut state: State,
    expression: ExpressionHandle,
) -> SymbolHandle {
    let target = probe
        .statement_table
        .insert_transition_target(TransitionTargetNode::Value(expression));
    let source_span = probe.expression_table.source_span(expression);
    probe.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Transition(TableTransition {
            target,
            source_span,
            ..Default::default()
        }),
    );
    let owner = machine.symbol;
    probe.push_machine_state(&mut machine, state);
    probe.push_machine(machine);
    owner
}

/// A checked source judgment can retain target conditions for later closure;
/// it is not yet permission to execute a partial invocation. Only conditions
/// owned by this demanded declaration closure matter: probes retain unrelated
/// declarations for visibility, and their outer pending work grants no premise
/// and need not finish to evaluate this independent call. Unknown ownership
/// cannot establish independence. The same fence serves every admission route.
pub(crate) fn require_complete_invocation_conditions(
    checked: &checked_trees::CheckedTrees,
    root: SymbolHandle,
) -> Result<(), String> {
    let has_pending_boundary_scalars = checked
        .facts
        .flow
        .terminal_unit_effects
        .boundary_machines
        .iter()
        .any(|plan| {
            !plan.pending_scalar_requires.is_empty() || !plan.pending_scalar_recipes.is_empty()
        });
    let has_pending_recipes = checked
        .facts
        .constant_recipe_obligations
        .iter()
        .any(|obligation| obligation.is_pending());
    let has_pending_predicates_or_ranges = checked
        .facts
        .constant_predicate_obligations
        .iter()
        .any(|obligation| obligation.is_pending())
        || checked
            .facts
            .integer_range_obligations
            .iter()
            .any(|obligation| obligation.is_pending());
    if !has_pending_boundary_scalars && !has_pending_recipes && !has_pending_predicates_or_ranges {
        return Ok(());
    }

    let domain = PreparedEvaluationInputs::domain_for_program(&checked.typed, &[root], &[]);
    let demanded = |owner: SymbolHandle| !owner.is_valid() || domain.contains(owner);
    if has_pending_boundary_scalars
        && checked
            .facts
            .flow
            .terminal_unit_effects
            .boundary_machines
            .iter()
            .any(|plan| {
                (!plan.pending_scalar_requires.is_empty()
                    || !plan.pending_scalar_recipes.is_empty())
                    && (demanded(plan.machine)
                        || demanded(plan.state)
                        || checked.typed.target_machine_families.iter().any(|family| {
                            family.canonical == plan.machine
                                && family.members.iter().any(|member| demanded(member.machine))
                        }))
            })
    {
        return Err(
            "constant invocation retains pending scalar contract clauses in its demanded closure"
                .to_owned(),
        );
    }

    if has_pending_recipes
        && let Some(obligation) = checked
            .facts
            .constant_recipe_obligations
            .iter()
            .find(|obligation| obligation.is_pending() && demanded(obligation.owner()))
    {
        return Err(format!(
            "constant invocation cannot establish its own pending target condition: {}",
            obligation.description()
        ));
    }
    if has_pending_predicates_or_ranges
        && (checked
            .facts
            .constant_predicate_obligations
            .iter()
            .any(|obligation| obligation.is_pending() && demanded(obligation.caller()))
            || checked
                .facts
                .integer_range_obligations
                .iter()
                .any(|obligation| obligation.is_pending() && demanded(obligation.owner())))
    {
        return Err(
            "constant invocation retains a pending target proof condition in its demanded closure"
                .to_owned(),
        );
    }
    Ok(())
}
