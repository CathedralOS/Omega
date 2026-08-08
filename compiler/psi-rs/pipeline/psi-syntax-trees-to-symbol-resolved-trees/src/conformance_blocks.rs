use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees::SymbolResolvedTrees;
use psi_symbol_resolved_trees::name::DiagnosticName;
use psi_symbol_resolved_trees::trait_definition::{
    ConformanceImplementation, ConformanceRow, ConformanceRowSource,
};
use psi_symbols::SymbolHandle;

#[derive(Clone)]
struct TraitCatalogEntry {
    symbol: SymbolHandle,
    name: DiagnosticName,
    parents: Vec<SymbolHandle>,
    requirements: Vec<RequirementCatalogEntry>,
}

#[derive(Clone)]
struct RequirementCatalogEntry {
    declaring_trait: SymbolHandle,
    declaring_trait_name: DiagnosticName,
    requirement: SymbolHandle,
    requirement_name: DiagnosticName,
    is_default: bool,
}

#[derive(Clone)]
struct MachineCatalogEntry {
    symbol: SymbolHandle,
    name: DiagnosticName,
    states: Vec<(DiagnosticName, SymbolHandle)>,
}

pub(crate) fn normalize_closed_conformance_blocks(
    program: &mut SymbolResolvedTrees,
) -> Result<(), Diagnostic> {
    let trait_catalog = build_trait_catalog(program);
    let machine_catalog = build_machine_catalog(program);
    let normalized = program
        .conformances
        .iter()
        .map(|conformance| match &conformance.implementation {
            ConformanceImplementation::LegacyAttachedMachines => {
                Ok(ConformanceImplementation::LegacyAttachedMachines)
            }
            ConformanceImplementation::Closed { rows } => normalize_one(
                conformance.type_name.as_str(),
                conformance.trait_name.as_str(),
                rows,
                &trait_catalog,
                &machine_catalog,
            )
            .map(|rows| ConformanceImplementation::Closed { rows }),
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut normalized = normalized.into_iter();
    program.conformances.for_each_mut(|conformance| {
        conformance.implementation = normalized
            .next()
            .expect("one normalized implementation per conformance");
    });
    Ok(())
}

#[derive(Clone)]
struct RequirementRoute {
    requirement: SymbolHandle,
    requirement_name: DiagnosticName,
    realization_state: SymbolHandle,
}

#[derive(Clone)]
struct ConformanceRoutingPlan {
    machine: SymbolHandle,
    routes: Vec<RequirementRoute>,
    subject_states: Vec<SymbolHandle>,
}

/// An inline member is lexically part of one closed conformance. Calls it
/// makes to inherited requirements therefore select this conformance's exact
/// rows, never an ambient attached machine that happens to share the leaf
/// name. Existing-machine reference rows remain ordinary machines: they have
/// no enclosing conformance context and may be shared by several blocks.
pub(crate) fn route_inline_member_calls(program: &mut SymbolResolvedTrees) {
    let mut plans = Vec::new();
    for conformance in program.conformances.iter() {
        let ConformanceImplementation::Closed { rows } = &conformance.implementation else {
            continue;
        };
        let routes = rows
            .iter()
            .filter(|row| row.realization_state.is_valid())
            .map(|row| RequirementRoute {
                requirement: row.requirement,
                requirement_name: row.requirement_name.clone(),
                realization_state: row.realization_state,
            })
            .collect::<Vec<_>>();
        let subject_states = program
            .machines
            .iter()
            .filter(|machine| {
                machine
                    .attached_data
                    .as_ref()
                    .is_some_and(|subject| subject.as_str() == conformance.type_name.as_str())
            })
            .flat_map(|machine| {
                program
                    .machine_state_handles(machine.states)
                    .iter()
                    .map(|handle| program.machine_state(*handle).symbol)
            })
            .collect::<Vec<_>>();
        plans.extend(
            rows.iter()
                .filter(|row| {
                    matches!(
                        row.source,
                        ConformanceRowSource::Inline | ConformanceRowSource::TraitDefault
                    ) && row.realization_machine.is_valid()
                })
                .map(|row| ConformanceRoutingPlan {
                    machine: row.realization_machine,
                    routes: routes.clone(),
                    subject_states: subject_states.clone(),
                }),
        );
    }

    for plan in plans {
        let (state_spans, contract_expressions) = program
            .machines
            .iter()
            .find(|machine| machine.symbol == plan.machine)
            .map(|machine| {
                let state_spans = program
                    .machine_state_handles(machine.states)
                    .iter()
                    .map(|handle| program.machine_state(*handle).statement_nodes)
                    .collect::<Vec<_>>();
                let mut contract_expressions = Vec::new();
                append_contract_expressions(program, machine.contracts, &mut contract_expressions);
                for state in program
                    .machine_state_handles(machine.states)
                    .iter()
                    .map(|handle| program.machine_state(*handle))
                {
                    append_contract_expressions(
                        program,
                        state.contracts,
                        &mut contract_expressions,
                    );
                }
                (state_spans, contract_expressions)
            })
            .unwrap_or_default();
        let mut visited = Vec::new();
        for expression in contract_expressions {
            route_expression(program, expression, &plan, &mut visited);
        }
        for statements in state_spans {
            route_statement_span(program, statements, &plan);
        }
    }
}

fn append_contract_expressions(
    program: &SymbolResolvedTrees,
    contracts: psi_arena::HandleSpan<psi_symbol_resolved_trees::signature::SignatureContract>,
    expressions: &mut Vec<psi_symbol_resolved_trees::expression::ExpressionHandle>,
) {
    for contract in program.signature_contracts(contracts) {
        for fact in program.proof_facts(contract.facts) {
            match fact {
                psi_symbol_resolved_trees::domain::ProofFact::Expression(expression) => {
                    expressions.push(*expression);
                }
                psi_symbol_resolved_trees::domain::ProofFact::Membership(membership) => {
                    expressions.push(membership.value);
                }
            }
        }
    }
}

fn route_statement_span(
    program: &mut SymbolResolvedTrees,
    statements: psi_arena::HandleSpan<psi_symbol_resolved_trees::statement::StatementNode>,
    plan: &ConformanceRoutingPlan,
) {
    for offset in 0..statements.count() {
        let handle = psi_arena::Handle::from_parts(
            statements.start().arena_index() + offset,
            statements.start().generation(),
        );
        let statement = program.tables.bodies.statements.statement(handle).clone();
        let mut expressions = Vec::new();
        match statement {
            psi_symbol_resolved_trees::statement::StatementNode::AssemblyFact(fact) => {
                expressions.push(fact.expression);
            }
            psi_symbol_resolved_trees::statement::StatementNode::Assignment(assignment) => {
                expressions.extend([assignment.target, assignment.value]);
            }
            psi_symbol_resolved_trees::statement::StatementNode::Call(call) => {
                expressions.extend_from_slice(
                    program
                        .tables
                        .bodies
                        .statements
                        .expression_handles(call.arguments),
                );
                if let Some(target) = routed_target(
                    call.target_symbol,
                    &call.target,
                    call.receiver.is_empty()
                        || call.receiver_starts_at_self
                        || plan.subject_states.contains(&call.target_symbol),
                    &plan.routes,
                ) && let psi_symbol_resolved_trees::statement::StatementNode::Call(call) =
                    program.tables.bodies.statements.statement_mut(handle)
                {
                    call.target_symbol = target;
                }
            }
            psi_symbol_resolved_trees::statement::StatementNode::Expression(expression) => {
                expressions.push(expression);
            }
            psi_symbol_resolved_trees::statement::StatementNode::LocalData(local) => {
                expressions.push(local.initial_value);
            }
            psi_symbol_resolved_trees::statement::StatementNode::Transition(transition) => {
                if let psi_symbol_resolved_trees::statement::TransitionGuardNode::When(guard) =
                    transition.guard
                {
                    expressions.push(guard);
                }
                append_transition_target_expressions(
                    &program.tables.bodies.statements,
                    transition.target,
                    &mut expressions,
                );
                if transition.continuation.is_valid() {
                    append_transition_target_expressions(
                        &program.tables.bodies.statements,
                        transition.continuation,
                        &mut expressions,
                    );
                }
            }
        }
        let mut visited = Vec::new();
        for expression in expressions {
            route_expression(program, expression, plan, &mut visited);
        }
    }
}

fn append_transition_target_expressions(
    statements: &psi_symbol_resolved_trees::statement::StatementTable,
    target: psi_symbol_resolved_trees::statement::TransitionTargetHandle,
    expressions: &mut Vec<psi_symbol_resolved_trees::expression::ExpressionHandle>,
) {
    match statements.transition_target(target) {
        psi_symbol_resolved_trees::statement::TransitionTargetNode::Named { arguments, .. } => {
            expressions.extend_from_slice(statements.expression_handles(*arguments));
        }
        psi_symbol_resolved_trees::statement::TransitionTargetNode::Value(value) => {
            expressions.push(*value);
        }
        psi_symbol_resolved_trees::statement::TransitionTargetNode::SelfTarget
        | psi_symbol_resolved_trees::statement::TransitionTargetNode::Terminal => {}
    }
}

fn route_expression(
    program: &mut SymbolResolvedTrees,
    expression: psi_symbol_resolved_trees::expression::ExpressionHandle,
    plan: &ConformanceRoutingPlan,
    visited: &mut Vec<u32>,
) {
    if !expression.is_valid() || visited.contains(&expression.arena_index()) {
        return;
    }
    visited.push(expression.arena_index());
    let node = program
        .tables
        .bodies
        .expressions
        .expression(expression)
        .clone();
    let mut children = Vec::new();
    match node {
        psi_symbol_resolved_trees::expression::ExpressionNode::ArrayLiteral(values) => {
            children
                .extend_from_slice(program.tables.bodies.expressions.expression_handles(values));
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Atomic(atomic) => {
            children.extend([atomic.value, atomic.result]);
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Binary(binary) => {
            children.extend([binary.left, binary.right]);
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Cast(cast) => {
            children.push(cast.value);
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                children.push(call.receiver);
            }
            children.extend_from_slice(
                program
                    .tables
                    .bodies
                    .expressions
                    .expression_handles(call.arguments),
            );
            let receiver_is_local = !call.receiver.is_valid()
                || expression_is_self(&program.tables.bodies.expressions, call.receiver)
                || plan.subject_states.contains(&call.target_symbol);
            if let Some(target) = routed_target(
                call.target_symbol,
                &call.target,
                receiver_is_local,
                &plan.routes,
            ) && let psi_symbol_resolved_trees::expression::ExpressionNode::Call(call) =
                program.tables.bodies.expressions.expression_mut(expression)
            {
                call.target_symbol = target;
            }
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Indexed(indexed) => {
            children.extend([indexed.collection, indexed.index]);
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Membership(membership) => {
            children.push(membership.value);
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            children.push(member.receiver);
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Mutable(inner) => {
            children.push(inner);
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Range(range) => {
            children.extend([range.start, range.end]);
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::StructLiteral(literal) => {
            children.extend(
                program
                    .tables
                    .bodies
                    .expressions
                    .struct_fields(literal.fields)
                    .iter()
                    .map(|field| field.value),
            );
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Unary(unary) => {
            children.push(unary.operand);
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Boolean(_)
        | psi_symbol_resolved_trees::expression::ExpressionNode::Float(_)
        | psi_symbol_resolved_trees::expression::ExpressionNode::Integer(_)
        | psi_symbol_resolved_trees::expression::ExpressionNode::Name(_)
        | psi_symbol_resolved_trees::expression::ExpressionNode::String(_)
        | psi_symbol_resolved_trees::expression::ExpressionNode::ZeroValue(_) => {}
    }
    for child in children {
        route_expression(program, child, plan, visited);
    }
}

fn expression_is_self(
    expressions: &psi_symbol_resolved_trees::expression::ExpressionTable,
    expression: psi_symbol_resolved_trees::expression::ExpressionHandle,
) -> bool {
    match expressions.expression(expression) {
        psi_symbol_resolved_trees::expression::ExpressionNode::Name(path) => path.is_self_value,
        psi_symbol_resolved_trees::expression::ExpressionNode::Mutable(inner) => {
            expression_is_self(expressions, *inner)
        }
        _ => false,
    }
}

fn routed_target(
    current: SymbolHandle,
    target_name: &DiagnosticName,
    local_receiver: bool,
    routes: &[RequirementRoute],
) -> Option<SymbolHandle> {
    if current.is_valid()
        && let Some(route) = routes.iter().find(|route| route.requirement == current)
    {
        return Some(route.realization_state);
    }
    if !local_receiver {
        return None;
    }
    let mut matching = routes
        .iter()
        .filter(|route| route.requirement_name.as_str() == target_name.as_str());
    let route = matching.next()?;
    matching.next().is_none().then_some(route.realization_state)
}

fn build_trait_catalog(program: &SymbolResolvedTrees) -> Vec<TraitCatalogEntry> {
    program
        .traits
        .iter()
        .map(|trait_definition| TraitCatalogEntry {
            symbol: trait_definition.symbol,
            name: trait_definition.name.clone(),
            parents: program
                .trait_requirements(trait_definition.requires)
                .iter()
                .map(|parent| parent.symbol)
                .collect(),
            requirements: program
                .trait_machine_signatures(trait_definition.machines)
                .iter()
                .map(|requirement| RequirementCatalogEntry {
                    declaring_trait: trait_definition.symbol,
                    declaring_trait_name: trait_definition.name.clone(),
                    requirement: requirement.symbol,
                    requirement_name: requirement.name.clone(),
                    is_default: requirement.is_default,
                })
                .collect(),
        })
        .collect()
}

fn build_machine_catalog(program: &SymbolResolvedTrees) -> Vec<MachineCatalogEntry> {
    program
        .machines
        .iter()
        .map(|machine| MachineCatalogEntry {
            symbol: machine.symbol,
            name: machine.name.clone(),
            states: program
                .machine_state_handles(machine.states)
                .iter()
                .map(|handle| {
                    let state = program.machine_state(*handle);
                    (state.name.clone(), state.symbol)
                })
                .collect(),
        })
        .collect()
}

fn normalize_one(
    subject_name: &str,
    trait_name: &str,
    authored_rows: &[ConformanceRow],
    trait_catalog: &[TraitCatalogEntry],
    machine_catalog: &[MachineCatalogEntry],
) -> Result<Vec<ConformanceRow>, Diagnostic> {
    let root = trait_catalog
        .iter()
        .find(|entry| entry.name.as_str() == trait_name)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "closed conformance `{subject_name} satisfies {trait_name}` names unknown trait `{trait_name}`"
            ))
        })?;
    let mut requirements = Vec::new();
    collect_requirement_closure(
        root.symbol,
        trait_catalog,
        &mut Vec::new(),
        &mut requirements,
    );

    let mut normalized = Vec::new();
    for authored in authored_rows {
        let candidates = requirements
            .iter()
            .filter(|requirement| {
                requirement.requirement_name == authored.requirement_name
                    && (authored.declaring_trait_name.as_str().is_empty()
                        || requirement.declaring_trait_name == authored.declaring_trait_name)
            })
            .collect::<Vec<_>>();
        let requirement = match candidates.as_slice() {
            [] => {
                let qualified = if authored.declaring_trait_name.as_str().is_empty() {
                    authored.requirement_name.as_str().to_owned()
                } else {
                    format!(
                        "{}::{}",
                        authored.declaring_trait_name, authored.requirement_name
                    )
                };
                return Err(Diagnostic::error(format!(
                    "closed conformance `{subject_name} satisfies {trait_name}` has no inherited requirement slot `{qualified}`"
                )));
            }
            [requirement] => *requirement,
            _ => {
                return Err(Diagnostic::error(format!(
                    "closed conformance `{subject_name} satisfies {trait_name}` member `{}` is ambiguous across inherited traits; use `DeclaringTrait::{}`",
                    authored.requirement_name, authored.requirement_name
                )));
            }
        };

        if normalized.iter().any(|row: &ConformanceRow| {
            row.declaring_trait == requirement.declaring_trait
                && row.requirement == requirement.requirement
        }) {
            return Err(Diagnostic::error(format!(
                "closed conformance `{subject_name} satisfies {trait_name}` fills `{}::{}` more than once",
                requirement.declaring_trait_name, requirement.requirement_name
            )));
        }

        let (machine, state) = resolve_realization(authored, machine_catalog).ok_or_else(|| {
            Diagnostic::error(format!(
                "closed conformance `{subject_name} satisfies {trait_name}` row `{}::{}` names no exact callable realization `{}`",
                requirement.declaring_trait_name,
                requirement.requirement_name,
                authored.realization_name,
            ))
        })?;
        let mut row = authored.clone();
        row.declaring_trait = requirement.declaring_trait;
        row.declaring_trait_name = requirement.declaring_trait_name.clone();
        row.requirement = requirement.requirement;
        row.realization_machine = machine;
        row.realization_state = state;
        normalized.push(row);
    }

    for requirement in requirements {
        if normalized.iter().any(|row| {
            row.declaring_trait == requirement.declaring_trait
                && row.requirement == requirement.requirement
        }) {
            continue;
        }
        if !requirement.is_default {
            return Err(Diagnostic::error(format!(
                "closed conformance `{subject_name} satisfies {trait_name}` is incomplete: missing `{}::{}`",
                requirement.declaring_trait_name, requirement.requirement_name
            )));
        }
        normalized.push(ConformanceRow {
            declaring_trait: requirement.declaring_trait,
            declaring_trait_name: requirement.declaring_trait_name.clone(),
            requirement: requirement.requirement,
            requirement_name: requirement.requirement_name.clone(),
            realization_machine: SymbolHandle::invalid(),
            realization_state: SymbolHandle::invalid(),
            realization_name: DiagnosticName::generated(format!(
                "{}::{}#default",
                requirement.declaring_trait_name, requirement.requirement_name
            )),
            source: ConformanceRowSource::TraitDefault,
        });
    }

    normalized.sort_by_key(|row| {
        (
            row.declaring_trait.arena_index(),
            row.requirement.arena_index(),
        )
    });
    Ok(normalized)
}

fn collect_requirement_closure(
    trait_symbol: SymbolHandle,
    catalog: &[TraitCatalogEntry],
    visited: &mut Vec<SymbolHandle>,
    output: &mut Vec<RequirementCatalogEntry>,
) {
    if !trait_symbol.is_valid() || visited.contains(&trait_symbol) {
        return;
    }
    visited.push(trait_symbol);
    let Some(entry) = catalog.iter().find(|entry| entry.symbol == trait_symbol) else {
        return;
    };
    for requirement in &entry.requirements {
        if !output.iter().any(|existing| {
            existing.declaring_trait == requirement.declaring_trait
                && existing.requirement == requirement.requirement
        }) {
            output.push(requirement.clone());
        }
    }
    for parent in &entry.parents {
        collect_requirement_closure(*parent, catalog, visited, output);
    }
}

fn resolve_realization(
    row: &ConformanceRow,
    catalog: &[MachineCatalogEntry],
) -> Option<(SymbolHandle, SymbolHandle)> {
    let machine = catalog
        .iter()
        .find(|machine| machine.name == row.realization_name)?;
    let leaf = machine
        .name
        .as_str()
        .rsplit_once("::")
        .map_or(machine.name.as_str(), |(_, leaf)| leaf);
    let state = machine
        .states
        .iter()
        .find(|(name, _)| name.as_str() == leaf)
        .or_else(|| {
            machine
                .states
                .iter()
                .find(|(name, _)| name.as_str() == "entry")
        })?;
    Some((machine.symbol, state.1))
}
