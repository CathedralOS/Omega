//! Qualification facts and service reach facts.

/// Declaration-only content plans are available before the ownership walk.
/// A normalized equation is an obligation, not a successful body theorem or
/// permission to consume its inputs. Keep the same table through later
/// qualification construction so completion-owned evidence retains its indices.
pub(crate) fn build_content_facts(
    program: &dyn ::validation::CheckingInput,
) -> checked_trees::ContentProjectionFacts {
    checked_trees::ContentProjectionFacts {
        plans: validation::build_content_projection_plans(program),
        conservation_plans: validation::content_conservation_plans(program)
            .iter()
            .map(|source| source.plan.clone())
            .collect(),
        ..Default::default()
    }
}

/// The cast-collection walk context: the program, the (machine, state) pair,
/// the statement occurrence, and the committed-domain / vacuous-use / visited
/// accumulators every step threads. Each recursive cast step restated this
/// eight-argument tuple at its call site; the walk now carries it once, and a
/// step names only the expression it descends into.
struct CastWalk<'a, 'b> {
    program: &'a dyn ::validation::CheckingInput,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement_index: u32,
    committed: &'b mut Vec<language_semantics::SemanticDomainId>,
    vacuous_uses: &'b mut Vec<checked_trees::VacuousQualificationUse>,
    visited: &'b mut std::collections::HashSet<u32, symbols::BuildSymbolHasher>,
}

impl CastWalk<'_, '_> {
    fn scan(&mut self, expression: typed_trees::expression::ExpressionHandle) {
        use checked_trees::VacuousQualificationUse;
        use language_semantics::SemanticDomainTable;
        use typed_trees::expression::ExpressionNode;

        let program = self.program;
        let machine = self.machine;
        let state = self.state;
        let statement_index = self.statement_index;
        if !expression.is_valid() || !self.visited.insert(expression.arena_index()) {
            return;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Match(dispatch) => {
                self.scan(dispatch.subject);
                for arm in program.expression_table.match_arms(dispatch.arms) {
                    for pattern in program.expression_table.match_pattern_operands(arm.pattern) {
                        self.scan(pattern);
                    }
                    self.scan(arm.value);
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
                    self.committed.push(policy);
                }
                // The result type retains the normalized atomic obligations.
                // An alias is their conjunction, not another membership to
                // publish. Checking and the Terminal catalog consume these
                // same exact instances, independent of authored spelling.
                for domain in ::validation::field_domains::declared_domain_constraint_records(
                    program,
                    cast.result_type,
                ) {
                    if domain.semantic_id.is_valid() {
                        self.committed.push(domain.semantic_id);
                    }
                    if ::validation::field_domains::domain_is_vacuous(
                        program,
                        domain.symbol,
                        &mut Vec::new(),
                    ) {
                        self.vacuous_uses.push(VacuousQualificationUse {
                            machine,
                            state,
                            statement_index,
                            expression,
                            domain: domain.symbol,
                            semantic_domain: domain.semantic_id,
                        });
                    }
                }
                self.scan(cast.value);
            }
            ExpressionNode::Binary(binary) => {
                self.scan(binary.left);
                self.scan(binary.right);
            }
            ExpressionNode::Unary(unary) => self.scan(unary.operand),
            ExpressionNode::Member(member) => self.scan(member.receiver),
            ExpressionNode::Borrow(inner) => self.scan(inner.target),
            ExpressionNode::Indexed(indexed) => {
                self.scan(indexed.collection);
                self.scan(indexed.index);
            }
            ExpressionNode::Range(range) => {
                self.scan(range.start);
                self.scan(range.end);
            }
            ExpressionNode::Call(call) => {
                self.scan(call.receiver);
                for argument in program.expression_table.expression_handles(call.arguments) {
                    self.scan(*argument);
                }
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in program.expression_table.struct_fields(literal.fields) {
                    self.scan(field.value);
                }
            }
            ExpressionNode::ArrayLiteral(items) => {
                for item in program.expression_table.expression_handles(*items) {
                    self.scan(*item);
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn build_qualification_facts(
    program: &dyn ::validation::CheckingInput,
    content: checked_trees::ContentProjectionFacts,
) -> checked_trees::QualificationFacts {
    /// Membership sets take the integer hasher; none is iterated for output order.
    type HashSet<T> = std::collections::HashSet<T, symbols::BuildSymbolHasher>;

    let mut machines = Vec::new();
    let mut vacuous_uses = Vec::new();
    for machine in program.machines() {
        let mut committed = Vec::new();
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .indexed_statements(state.statement_nodes)
            {
                use typed_trees::statement::StatementNode;
                let statement_index =
                    u32::try_from(statement_index).expect("qualification statement index overflow");
                let mut visited = HashSet::default();
                let mut walk = CastWalk {
                    program,
                    machine: machine.symbol,
                    state: state.symbol,
                    statement_index,
                    committed: &mut committed,
                    vacuous_uses: &mut vacuous_uses,
                    visited: &mut visited,
                };
                match statement {
                    StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => {}
                    StatementNode::Assignment(assignment) => {
                        walk.scan(assignment.target);
                        walk.scan(assignment.value);
                    }
                    StatementNode::Expression(expression) => {
                        walk.scan(*expression);
                    }
                    StatementNode::LocalData(local) => {
                        walk.scan(local.initial_value);
                    }
                    StatementNode::Call(call) => {
                        for argument in program.statement_table.expression_handles(call.arguments) {
                            walk.scan(*argument);
                        }
                    }
                    StatementNode::Transition(transition) => {
                        if let typed_trees::statement::TransitionGuardNode::When(guard) =
                            &transition.guard
                        {
                            walk.scan(*guard);
                        }
                        for target in transition.targets() {
                            match program.statement_table.transition_target(target) {
                                typed_trees::statement::TransitionTargetNode::Value(value) => {
                                    walk.scan(*value)
                                }
                                typed_trees::statement::TransitionTargetNode::Named {
                                    arguments,
                                    ..
                                } => {
                                    for argument in
                                        program.statement_table.expression_handles(*arguments)
                                    {
                                        walk.scan(*argument);
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
        scalar_returns: Vec::new(),
        value_formations: Default::default(),
        machines,
        vacuous_uses,
        content,
    }
}

/// Build the boundary-symbol service fixed point without consulting the
/// legacy global effect catalog. A direct boundary-signature call contributes
/// its containing service plus the signature's explicitly reached services.
/// Checked callees contribute authored services plus transitive call reach;
/// requirements, boundaries, and external realizations retain fixed published
/// ceilings independently of a selected implementation.
pub(crate) fn build_service_reach_facts(
    program: &dyn ::validation::CheckingInput,
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
                cleanup_contribution: machine.cleanup_contribution,
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
