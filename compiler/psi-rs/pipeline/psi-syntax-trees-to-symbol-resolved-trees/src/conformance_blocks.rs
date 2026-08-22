use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees::SymbolResolvedTrees;
use psi_symbol_resolved_trees::name::DiagnosticName;
use psi_symbol_resolved_trees::signature::StateSignature;
use psi_symbol_resolved_trees::state::State;
use psi_symbol_resolved_trees::trait_definition::{
    ConformanceImplementation, ConformanceRow, ConformanceRowSource,
};
use psi_symbol_resolved_trees::types::{TypeConstraint, TypeReference};
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
    ordinal: usize,
    declaring_trait: SymbolHandle,
    declaring_trait_name: DiagnosticName,
    requirement: SymbolHandle,
    requirement_name: DiagnosticName,
    is_default: bool,
    signature: StateSignature,
}

#[derive(Clone)]
struct MachineCatalogEntry {
    ordinal: usize,
    symbol: SymbolHandle,
    name: DiagnosticName,
    states: Vec<State>,
}

pub(crate) fn normalize_closed_conformance_blocks(
    program: &mut SymbolResolvedTrees,
) -> Result<(), Diagnostic> {
    let trait_catalog = build_trait_catalog(program);
    let machine_catalog = build_machine_catalog(program);
    let synthesized_default_candidates = program
        .conformances
        .iter()
        .flat_map(|conformance| match &conformance.implementation {
            ConformanceImplementation::AttachedRequirementMachines => Vec::new(),
            ConformanceImplementation::Closed { rows } => rows
                .iter()
                .filter(|row| row.source == ConformanceRowSource::TraitDefault)
                .filter_map(|row| {
                    let ordinal = row.provisional_realization_ordinal?;
                    machine_catalog
                        .iter()
                        .find(|machine| machine.ordinal == ordinal)
                        .map(|machine| machine.symbol)
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    let normalized = program
        .conformances
        .iter()
        .map(|conformance| match &conformance.implementation {
            ConformanceImplementation::AttachedRequirementMachines => {
                Ok(ConformanceImplementation::AttachedRequirementMachines)
            }
            ConformanceImplementation::Closed { rows } => normalize_one(
                program,
                match &conformance.subject {
                    psi_symbol_resolved_trees::trait_definition::ConformanceSubject::Carrier(
                        type_name,
                    ) => type_name.as_str(),
                    psi_symbol_resolved_trees::trait_definition::ConformanceSubject::Subjectless => {
                        conformance
                            .alias
                            .as_ref()
                            .map_or("<subjectless>", DiagnosticName::as_str)
                    }
                },
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
    let selected_realizations = program
        .conformances
        .iter()
        .flat_map(|conformance| match &conformance.implementation {
            ConformanceImplementation::AttachedRequirementMachines => Vec::new(),
            ConformanceImplementation::Closed { rows } => rows
                .iter()
                .map(|row| row.realization_machine)
                .filter(|symbol| symbol.is_valid())
                .collect(),
        })
        .collect::<Vec<_>>();
    let retained_machines = program
        .machines
        .iter()
        .filter(|machine| {
            !synthesized_default_candidates.contains(&machine.symbol)
                || selected_realizations.contains(&machine.symbol)
        })
        .cloned()
        .collect::<Vec<_>>();
    program.machines = Default::default();
    for machine in retained_machines {
        program.machines.push(machine);
    }
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
        let subject_states = match &conformance.subject {
            psi_symbol_resolved_trees::trait_definition::ConformanceSubject::Carrier(type_name) => {
                program
                    .machines
                    .iter()
                    .filter(|machine| {
                        machine
                            .attached_data
                            .as_ref()
                            .is_some_and(|subject| subject.as_str() == type_name.as_str())
                    })
                    .flat_map(|machine| {
                        program
                            .machine_state_handles(machine.states)
                            .iter()
                            .map(|handle| program.machine_state(*handle).symbol)
                    })
                    .collect::<Vec<_>>()
            }
            psi_symbol_resolved_trees::trait_definition::ConformanceSubject::Subjectless => {
                Vec::new()
            }
        };
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
            psi_symbol_resolved_trees::statement::StatementNode::ProofOutputBindingStatement(
                package,
            ) => {
                expressions.push(package.call);
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
                .enumerate()
                .map(|(ordinal, requirement)| RequirementCatalogEntry {
                    ordinal,
                    declaring_trait: trait_definition.symbol,
                    declaring_trait_name: trait_definition.name.clone(),
                    requirement: requirement.symbol,
                    requirement_name: requirement.name.clone(),
                    is_default: requirement.is_default,
                    signature: requirement.clone(),
                })
                .collect(),
        })
        .collect()
}

fn build_machine_catalog(program: &SymbolResolvedTrees) -> Vec<MachineCatalogEntry> {
    program
        .machines
        .iter()
        .enumerate()
        .map(|(ordinal, machine)| MachineCatalogEntry {
            ordinal,
            symbol: machine.symbol,
            name: machine.name.clone(),
            states: program
                .machine_state_handles(machine.states)
                .iter()
                .map(|handle| program.machine_state(*handle).clone())
                .collect(),
        })
        .collect()
}

fn normalize_one(
    program: &SymbolResolvedTrees,
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
    // Authored rows own their exact slots. Synthesized defaults are fallback
    // candidates and normalize only after every authored row, independent of
    // incidental source-table order.
    for authored in authored_rows
        .iter()
        .filter(|row| row.source != ConformanceRowSource::TraitDefault)
        .chain(
            authored_rows
                .iter()
                .filter(|row| row.source == ConformanceRowSource::TraitDefault),
        )
    {
        let candidates = requirements
            .iter()
            .filter(|instance| {
                let requirement = *instance;
                requirement.requirement_name == authored.requirement_name
                    && (authored.declaring_trait_name.as_str().is_empty()
                        || requirement.declaring_trait_name == authored.declaring_trait_name)
                    && authored
                        .provisional_requirement_ordinal
                        .is_none_or(|ordinal| requirement.ordinal == ordinal)
            })
            .collect::<Vec<_>>();
        let realizations = resolve_realizations(authored, machine_catalog);
        let mut matching = Vec::new();
        for candidate in &candidates {
            // Legal same-path overload families share one normalized
            // parameter signature and differ by dispatch-bearing result
            // domains. The typed conformance validator remains authoritative
            // for the complete parameter/contract compatibility check; this
            // pre-typed phase uses the result set only to retain the exact
            // declaration symbol rather than collapsing to the leaf name.
            let requirement_shape =
                result_dispatch_shape(program, candidate.signature.return_type.as_ref());
            for (machine, state) in &realizations {
                if result_dispatch_shape(program, state.return_type.as_ref()) == requirement_shape {
                    matching.push((*candidate, *machine, *state));
                }
            }
        }
        let (requirement, machine, state) = match (
            candidates.as_slice(),
            realizations.as_slice(),
            matching.as_slice(),
        ) {
            ([], _, _) => {
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
            ([requirement], [(machine, state)], _) => (*requirement, *machine, *state),
            (_, _, [(requirement, machine, state)]) => (*requirement, *machine, *state),
            _ => {
                let inherited_collision = authored.declaring_trait_name.as_str().is_empty()
                    && candidates.first().is_some_and(|first| {
                        candidates
                            .iter()
                            .skip(1)
                            .any(|candidate| candidate.declaring_trait != first.declaring_trait)
                    });
                if inherited_collision {
                    return Err(Diagnostic::error(format!(
                        "closed conformance `{subject_name} satisfies {trait_name}` member `{}` is ambiguous across inherited traits; use `DeclaringTrait::{}`",
                        authored.requirement_name, authored.requirement_name
                    )));
                }
                return Err(Diagnostic::error(format!(
                    "closed conformance `{subject_name} satisfies {trait_name}` member `{}` does not identify one exact inherited overload; qualify the declaring trait and match the complete parameter/result-domain signature",
                    authored.requirement_name
                )));
            }
        };

        if normalized.iter().any(|row: &ConformanceRow| {
            row.declaring_trait == requirement.declaring_trait
                && row.requirement == requirement.requirement
        }) {
            if authored.source == ConformanceRowSource::TraitDefault {
                continue;
            }
            return Err(Diagnostic::error(format!(
                "closed conformance `{subject_name} satisfies {trait_name}` fills `{}::{}` more than once",
                requirement.declaring_trait_name, requirement.requirement_name
            )));
        }

        let mut row = authored.clone();
        row.declaring_trait = requirement.declaring_trait;
        row.declaring_trait_name = requirement.declaring_trait_name.clone();
        row.requirement = requirement.requirement;
        row.provisional_requirement_ordinal = None;
        row.realization_machine = machine.symbol;
        row.realization_state = state.symbol;
        row.provisional_realization_ordinal = None;
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
            provisional_requirement_ordinal: None,
            realization_machine: SymbolHandle::invalid(),
            realization_state: SymbolHandle::invalid(),
            realization_name: DiagnosticName::generated(format!(
                "{}::{}#default",
                requirement.declaring_trait_name, requirement.requirement_name
            )),
            provisional_realization_ordinal: None,
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

fn result_dispatch_shape(
    program: &SymbolResolvedTrees,
    return_type: Option<&TypeReference>,
) -> String {
    let mut result_dispatch = Vec::new();
    if let Some(return_type) = return_type {
        collect_result_dispatch(program, return_type, &mut result_dispatch, &mut Vec::new());
    }
    result_dispatch.sort();
    result_dispatch.dedup();
    result_dispatch.join("&")
}

fn collect_result_dispatch(
    program: &SymbolResolvedTrees,
    type_reference: &TypeReference,
    terms: &mut Vec<String>,
    alias_stack: &mut Vec<SymbolHandle>,
) {
    match type_reference {
        TypeReference::Reference(reference) => collect_result_dispatch(
            program,
            program.child_type_reference(reference.referee),
            terms,
            alias_stack,
        ),
        TypeReference::Constrained(constrained) => {
            collect_result_dispatch(
                program,
                program.child_type_reference(constrained.base_type),
                terms,
                alias_stack,
            );
            for constraint in program
                .tables
                .types
                .constraints
                .span_or_empty(constrained.constraints)
            {
                match constraint {
                    TypeConstraint::ArithmeticDomain(domain) => {
                        terms.push(format!("arithmetic:{}", domain.name()));
                    }
                    TypeConstraint::Domain(domain) => {
                        collect_declared_result_dispatch(
                            program,
                            domain.name.as_str(),
                            terms,
                            alias_stack,
                        );
                    }
                    TypeConstraint::Named(_) | TypeConstraint::Range { .. } => {}
                }
            }
        }
        TypeReference::FixedArray(_)
        | TypeReference::Slice(_)
        | TypeReference::Generic(_)
        | TypeReference::ConstExpression(_)
        | TypeReference::DynamicTrait { .. }
        | TypeReference::Named { .. }
        | TypeReference::SelfType { .. }
        | TypeReference::Unit => {}
    }
}

fn collect_declared_result_dispatch(
    program: &SymbolResolvedTrees,
    name: &str,
    terms: &mut Vec<String>,
    alias_stack: &mut Vec<SymbolHandle>,
) {
    let Some(definition) = program
        .domain_definitions
        .iter()
        .find(|definition| definition.name.as_str() == name)
    else {
        terms.push(format!("declared:{name}"));
        return;
    };
    if alias_stack.contains(&definition.symbol) {
        return;
    }
    if let Some(alias) = definition.alias.as_ref() {
        alias_stack.push(definition.symbol);
        for constituent in &alias.constituents {
            let constituent_name = program
                .domain_definitions
                .iter()
                .find(|candidate| candidate.symbol == constituent.domain_symbol)
                .map(|candidate| candidate.name.as_str())
                .unwrap_or("<unresolved-domain>");
            collect_declared_result_dispatch(program, constituent_name, terms, alias_stack);
        }
        alias_stack.pop();
        return;
    }
    if definition.predicate_body.is_present()
        && definition.semantic_roles.is_empty()
        && definition.establishment_routes.is_empty()
    {
        return;
    }
    terms.push(format!("declared:{}", definition.name));
}

fn resolve_realizations<'catalog>(
    row: &ConformanceRow,
    catalog: &'catalog [MachineCatalogEntry],
) -> Vec<(&'catalog MachineCatalogEntry, &'catalog State)> {
    catalog
        .iter()
        .filter(|machine| {
            row.provisional_realization_ordinal
                .map_or(machine.name == row.realization_name, |ordinal| {
                    machine.ordinal == ordinal
                })
        })
        .filter_map(|machine| {
            let leaf = machine
                .name
                .as_str()
                .rsplit_once("::")
                .map_or(machine.name.as_str(), |(_, leaf)| leaf);
            machine
                .states
                .iter()
                .find(|state| state.name.as_str() == leaf)
                .or_else(|| {
                    machine
                        .states
                        .iter()
                        .find(|state| state.name.as_str() == "entry")
                })
                .map(|state| (machine, state))
        })
        .collect()
}
