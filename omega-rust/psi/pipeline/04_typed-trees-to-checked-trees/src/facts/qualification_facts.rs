//! Qualification facts and service reach facts.

use crate::facts::canonical_encoding::domain_is_vacuous;
use typed_trees::TypedTrees;

pub(crate) fn build_qualification_facts(program: &TypedTrees) -> checked_trees::QualificationFacts {
    use checked_trees::VacuousQualificationUse;
    use language_semantics::SemanticDomainTable;
    use std::collections::HashSet;
    use symbols::SymbolHandle;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};

    fn collect_casts(
        program: &TypedTrees,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement_index: u32,
        expression: ExpressionHandle,
        committed: &mut Vec<language_semantics::SemanticDomainId>,
        vacuous_uses: &mut Vec<VacuousQualificationUse>,
        visited: &mut HashSet<u32>,
    ) {
        if !expression.is_valid() || !visited.insert(expression.arena_index()) {
            return;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Match(dispatch) => {
                collect_casts(
                    program,
                    machine,
                    state,
                    statement_index,
                    dispatch.subject,
                    committed,
                    vacuous_uses,
                    visited,
                );
                for arm in program.expression_table.match_arms(dispatch.arms) {
                    if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                        collect_casts(
                            program,
                            machine,
                            state,
                            statement_index,
                            pattern,
                            committed,
                            vacuous_uses,
                            visited,
                        );
                    }
                    collect_casts(
                        program,
                        machine,
                        state,
                        statement_index,
                        arm.value,
                        committed,
                        vacuous_uses,
                        visited,
                    );
                }
            }
            ExpressionNode::Cast(cast) => {
                let policy = match cast.domain {
                    numerics::arithmetic::ArithmeticDomain::Exact => None,
                    numerics::arithmetic::ArithmeticDomain::Wrapping => {
                        Some(SemanticDomainTable::WRAPPING)
                    }
                    numerics::arithmetic::ArithmeticDomain::Saturating => {
                        Some(SemanticDomainTable::SATURATING)
                    }
                    numerics::arithmetic::ArithmeticDomain::Trapping => {
                        Some(SemanticDomainTable::TRAPPING)
                    }
                };
                if let Some(policy) = policy {
                    committed.push(policy);
                }
                // Declared-domain casts already carry the normalized symbol
                // selected before validation. Checked facts consume that
                // identity directly rather than repeating short-name lookup.
                if cast.semantic_domain_symbol.is_valid()
                    && let Some(domain) = program
                        .domain_definitions()
                        .iter()
                        .find(|domain| domain.symbol == cast.semantic_domain_symbol)
                {
                    let semantic_id = if cast.semantic_domain_id.is_valid() {
                        cast.semantic_domain_id
                    } else {
                        domain.semantic_id
                    };
                    if semantic_id.is_valid() {
                        committed.push(semantic_id);
                    }
                    if domain_is_vacuous(program, domain.symbol, &mut Vec::new()) {
                        vacuous_uses.push(VacuousQualificationUse {
                            machine,
                            state,
                            statement_index,
                            expression,
                            domain: domain.symbol,
                            semantic_domain: semantic_id,
                        });
                    }
                }
                collect_casts(
                    program,
                    machine,
                    state,
                    statement_index,
                    cast.value,
                    committed,
                    vacuous_uses,
                    visited,
                );
            }
            ExpressionNode::Binary(binary) => {
                collect_casts(
                    program,
                    machine,
                    state,
                    statement_index,
                    binary.left,
                    committed,
                    vacuous_uses,
                    visited,
                );
                collect_casts(
                    program,
                    machine,
                    state,
                    statement_index,
                    binary.right,
                    committed,
                    vacuous_uses,
                    visited,
                );
            }
            ExpressionNode::Unary(unary) => collect_casts(
                program,
                machine,
                state,
                statement_index,
                unary.operand,
                committed,
                vacuous_uses,
                visited,
            ),
            ExpressionNode::Member(member) => collect_casts(
                program,
                machine,
                state,
                statement_index,
                member.receiver,
                committed,
                vacuous_uses,
                visited,
            ),
            ExpressionNode::Borrow(inner) => collect_casts(
                program,
                machine,
                state,
                statement_index,
                inner.target,
                committed,
                vacuous_uses,
                visited,
            ),
            ExpressionNode::Indexed(indexed) => {
                collect_casts(
                    program,
                    machine,
                    state,
                    statement_index,
                    indexed.collection,
                    committed,
                    vacuous_uses,
                    visited,
                );
                collect_casts(
                    program,
                    machine,
                    state,
                    statement_index,
                    indexed.index,
                    committed,
                    vacuous_uses,
                    visited,
                );
            }
            ExpressionNode::Range(range) => {
                collect_casts(
                    program,
                    machine,
                    state,
                    statement_index,
                    range.start,
                    committed,
                    vacuous_uses,
                    visited,
                );
                collect_casts(
                    program,
                    machine,
                    state,
                    statement_index,
                    range.end,
                    committed,
                    vacuous_uses,
                    visited,
                );
            }
            ExpressionNode::Call(call) => {
                collect_casts(
                    program,
                    machine,
                    state,
                    statement_index,
                    call.receiver,
                    committed,
                    vacuous_uses,
                    visited,
                );
                for argument in program.expression_table.expression_handles(call.arguments) {
                    collect_casts(
                        program,
                        machine,
                        state,
                        statement_index,
                        *argument,
                        committed,
                        vacuous_uses,
                        visited,
                    );
                }
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in program.expression_table.struct_fields(literal.fields) {
                    collect_casts(
                        program,
                        machine,
                        state,
                        statement_index,
                        field.value,
                        committed,
                        vacuous_uses,
                        visited,
                    );
                }
            }
            ExpressionNode::ArrayLiteral(items) => {
                for item in program.expression_table.expression_handles(*items) {
                    collect_casts(
                        program,
                        machine,
                        state,
                        statement_index,
                        *item,
                        committed,
                        vacuous_uses,
                        visited,
                    );
                }
            }
            _ => {}
        }
    }

    let mut machines = Vec::new();
    let mut vacuous_uses = Vec::new();
    for machine in program.machines() {
        let mut committed = Vec::new();
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                use typed_trees::statement::StatementNode;
                let statement_index =
                    u32::try_from(statement_index).expect("qualification statement index overflow");
                let mut visited = HashSet::new();
                match statement {
                    StatementNode::RootBinding(binding) => {
                        for expression in [binding.receiver, binding.implementation_operand] {
                            if expression.is_valid() {
                                collect_casts(
                                    program,
                                    machine.symbol,
                                    state.symbol,
                                    statement_index,
                                    expression,
                                    &mut committed,
                                    &mut vacuous_uses,
                                    &mut visited,
                                );
                            }
                        }
                    }
                    StatementNode::AssemblyFact(_) => {}
                    StatementNode::Assignment(assignment) => {
                        collect_casts(
                            program,
                            machine.symbol,
                            state.symbol,
                            statement_index,
                            assignment.target,
                            &mut committed,
                            &mut vacuous_uses,
                            &mut visited,
                        );
                        collect_casts(
                            program,
                            machine.symbol,
                            state.symbol,
                            statement_index,
                            assignment.value,
                            &mut committed,
                            &mut vacuous_uses,
                            &mut visited,
                        );
                    }
                    StatementNode::Expression(expression) => {
                        collect_casts(
                            program,
                            machine.symbol,
                            state.symbol,
                            statement_index,
                            *expression,
                            &mut committed,
                            &mut vacuous_uses,
                            &mut visited,
                        );
                    }
                    StatementNode::LocalData(local) => {
                        collect_casts(
                            program,
                            machine.symbol,
                            state.symbol,
                            statement_index,
                            local.initial_value,
                            &mut committed,
                            &mut vacuous_uses,
                            &mut visited,
                        );
                    }
                    StatementNode::Call(call) => {
                        for argument in program.statement_table.expression_handles(call.arguments) {
                            collect_casts(
                                program,
                                machine.symbol,
                                state.symbol,
                                statement_index,
                                *argument,
                                &mut committed,
                                &mut vacuous_uses,
                                &mut visited,
                            );
                        }
                    }
                    StatementNode::Transition(transition) => {
                        if let typed_trees::statement::TransitionGuardNode::When(guard) =
                            &transition.guard
                        {
                            collect_casts(
                                program,
                                machine.symbol,
                                state.symbol,
                                statement_index,
                                *guard,
                                &mut committed,
                                &mut vacuous_uses,
                                &mut visited,
                            );
                        }
                        for target in [transition.target, transition.continuation] {
                            if !target.is_valid() {
                                continue;
                            }
                            match program.statement_table.transition_target(target) {
                                typed_trees::statement::TransitionTargetNode::Value(value) => {
                                    collect_casts(
                                        program,
                                        machine.symbol,
                                        state.symbol,
                                        statement_index,
                                        *value,
                                        &mut committed,
                                        &mut vacuous_uses,
                                        &mut visited,
                                    )
                                }
                                typed_trees::statement::TransitionTargetNode::Named {
                                    arguments,
                                    ..
                                } => {
                                    for argument in
                                        program.statement_table.expression_handles(*arguments)
                                    {
                                        collect_casts(
                                            program,
                                            machine.symbol,
                                            state.symbol,
                                            statement_index,
                                            *argument,
                                            &mut committed,
                                            &mut vacuous_uses,
                                            &mut visited,
                                        );
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
        committed.sort_by_key(|id| id.0);
        committed.dedup();
        if !committed.is_empty() {
            machines.push(checked_trees::MachineQualifications {
                machine: machine.symbol,
                body_committed: committed,
            });
        }
    }
    checked_trees::QualificationFacts {
        machines,
        vacuous_uses,
        content: checked_trees::ContentProjectionFacts {
            plans: validation::build_content_projection_plans(program),
            conservation_plans: validation::build_content_conservation_plans(program)
                .into_iter()
                .map(|source| source.plan)
                .collect(),
            identity_reshuffles: Vec::new(),
            partition_compositions: Vec::new(),
            retained_borrow_custodies: Vec::new(),
        },
    }
}

/// Build the boundary-symbol service fixed point without consulting the
/// legacy global effect catalog. A direct boundary-signature call contributes
/// its containing service plus the signature's explicitly reached services.
/// Checked callees contribute authored services plus transitive call reach;
/// requirements, boundaries, and external realizations retain fixed published
/// ceilings independently of a selected implementation.
pub(crate) fn build_service_reach_facts(
    program: &TypedTrees,
    inferred: flow_effects::ServiceReachInferencePlan,
) -> checked_trees::ServiceReachFacts {
    checked_trees::ServiceReachFacts {
        services: program.service_reaches.clone(),
        rows: inferred.rows,
        dependency_parameters: inferred.dependency_parameters,
        root_machines: remap_service_reach_span(inferred.root_machines),
        machines: inferred
            .machines
            .map(|machine| checked_trees::MachineServiceReachRows {
                machine: machine.machine,
                dependency: machine.dependency,
                interface: machine.interface,
                published_ceiling: machine.published,
                inferred_direct: machine.inferred_direct,
                inferred_transitive: machine.inferred_transitive,
                concrete_transitive: machine.concrete_transitive,
                effective: machine.effective,
                concrete_effective: machine.concrete_effective,
                unresolved_installation_reaches: machine.unresolved_installation_reaches,
                states: remap_service_reach_span(machine.states),
            }),
        states: inferred
            .states
            .map(|state| checked_trees::StateServiceReachRows {
                state: state.state,
                inferred_direct: state.inferred_direct,
                inferred_transitive: state.inferred_transitive,
                concrete_direct: state.concrete_direct,
                concrete_transitive: state.concrete_transitive,
                unresolved_installation_reaches: state.unresolved_installation_reaches,
                calls: remap_service_reach_span(state.calls),
            }),
        calls: inferred
            .calls
            .map(|call| checked_trees::CallServiceReachRows {
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
                target_state: call.target_state,
                target_machine: call.target_machine,
                inferred_direct: call.inferred_direct,
                inferred_transitive: call.inferred_transitive,
                concrete_direct: call.concrete_direct,
                concrete_transitive: call.concrete_transitive,
                unresolved_installation_reaches: call.unresolved_installation_reaches,
            }),
    }
}

fn remap_service_reach_span<From, To>(span: arena::HandleSpan<From>) -> arena::HandleSpan<To> {
    let start = span.start();
    arena::HandleSpan::from_parts(
        arena::Handle::from_parts(start.arena_index(), start.generation()),
        span.count(),
    )
}
