use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;

mod case_preservation;

/// Transient, program-local results of positive entailment judgments. The
/// runtime place prover need not duplicate structural or integer proof rules,
/// but absence of a validation error is not evidence for any postcondition.
pub(in crate::checks) struct ProvenExitExpressions<'program, 'frames> {
    program: &'program dyn ::validation::CheckingInput,
    classification: &'program typed_trees::proof_only::ProofOnlyClassification,
    resolver: Option<&'frames validation::CallFrameResolver<'program>>,
    /// Postconditions the one judgment proved on this program, by machine.
    validated: &'frames [(SymbolHandle, validation::ProvenContractExpressions)],
    machines: Vec<MachineEntailmentOutcome>,
}

pub(in crate::checks) struct MachineEntailmentOutcome {
    machine: SymbolHandle,
    pub(in crate::checks) expressions: Vec<ExpressionHandle>,
    entry_premises_preserved: bool,
}

impl<'program, 'frames> ProvenExitExpressions<'program, 'frames> {
    /// The check pass owns one resolver for the immutable program window;
    /// `resolver` borrows it. `None` disables the isolated-proof-value path
    /// exactly as an unconstructable resolver did before.
    pub(in crate::checks) fn new(
        program: &'program dyn ::validation::CheckingInput,
        classification: &'program typed_trees::proof_only::ProofOnlyClassification,
        resolver: Option<&'frames validation::CallFrameResolver<'program>>,
        validated: &'frames [(SymbolHandle, validation::ProvenContractExpressions)],
    ) -> Self {
        Self {
            program,
            classification,
            resolver,
            validated,
            machines: Vec::new(),
        }
    }

    /// Add an independently established all-normal-return judgment. The caller
    /// must cover the actual entry's complete outgoing graph; a theorem about
    /// one local exit cannot bypass another state's missing origin evidence.
    pub(in crate::checks) fn record_proved_normal_return(
        &mut self,
        facts: &checked_trees::CheckFacts,
        machine: SymbolHandle,
        expression: ExpressionHandle,
    ) {
        self.for_machine(facts, machine);
        if let Some(outcome) = self
            .machines
            .iter_mut()
            .find(|outcome| outcome.machine == machine)
            && !outcome.expressions.contains(&expression)
        {
            outcome.expressions.push(expression);
            let inherited = validation::matched_machine_law_guarantees(self.program, machine)
                .into_iter()
                .filter(|matching| {
                    matching.machine == machine
                        && matching
                            .source_expressions
                            .iter()
                            .all(|source| outcome.expressions.contains(source))
                })
                .map(|matching| matching.expression)
                .collect::<Vec<_>>();
            for expression in inherited {
                if !outcome.expressions.contains(&expression) {
                    outcome.expressions.push(expression);
                }
            }
        }
    }

    /// The one judgment over a checked body is validation's retained row;
    /// this read is the prover's decision point — nothing rejudges it.
    pub(in crate::checks) fn for_machine(
        &mut self,
        facts: &checked_trees::CheckFacts,
        machine_symbol: SymbolHandle,
    ) -> &MachineEntailmentOutcome {
        let position = self
            .machines
            .iter()
            .position(|outcome| outcome.machine == machine_symbol)
            .unwrap_or_else(|| {
                let entry_premises_preserved = entry_premises_are_preserved(
                    self.program,
                    facts,
                    machine_symbol,
                    self.classification,
                    self.resolver,
                );
                let mut expressions = self
                    .validated
                    .iter()
                    .find(|(symbol, _)| *symbol == machine_symbol)
                    .map(|(_, proven)| {
                        let mut expressions = proven.normal_returns.clone();
                        if entry_premises_preserved {
                            expressions.extend_from_slice(&proven.entry);
                        }
                        expressions
                    })
                    .unwrap_or_default();
                if !expressions.is_empty() {
                    // A signature's op-slot expression has a different handle
                    // from the authored guarantee. Match through the existing
                    // exact conformance law, and independently require every
                    // supporting authored conjunct to have been proved.
                    let inherited =
                        validation::matched_machine_law_guarantees(self.program, machine_symbol)
                            .into_iter()
                            .filter(|matching| {
                                matching.machine == machine_symbol
                                    && matching
                                        .source_expressions
                                        .iter()
                                        .all(|source| expressions.contains(source))
                            })
                            .map(|matching| matching.expression)
                            .collect::<Vec<_>>();
                    expressions.extend(inherited);
                }
                self.machines.push(MachineEntailmentOutcome {
                    machine: machine_symbol,
                    expressions,
                    entry_premises_preserved,
                });
                self.machines.len() - 1
            });
        &self.machines[position]
    }
}

/// Integral reflexivity is independent of parameter substitution. An
/// inherited signature keeps its own formal symbol, not the implementing
/// machine's parameter identity. Floating equality is deliberately excluded.
pub(super) fn integral_parameter_reflexivity(
    program: &dyn ::validation::CheckingInput,
    fact: &facts::Fact,
) -> bool {
    use typed_trees::expression::BinaryOperator;
    use typed_trees::types::TypeReferenceNode;
    let facts::FactPayload::ContractBooleanExpression { expression, .. } = fact.payload else {
        return false;
    };
    let Some(comparison) = program.expression_table.binary_node(expression) else {
        return false;
    };
    if comparison.operator != BinaryOperator::Equal {
        return false;
    }
    let Some(left) = program.expression_table.name_path_node(comparison.left) else {
        return false;
    };
    let Some(right) = program.expression_table.name_path_node(comparison.right) else {
        return false;
    };
    if !left.symbol.is_valid()
        || left.symbol != right.symbol
        || left.head_symbol != right.head_symbol
        || program
            .expression_table
            .name_path_members(left.members)
            .len()
            != 1
        || program
            .expression_table
            .name_path_members(right.members)
            .len()
            != 1
    {
        return false;
    }
    program.state_parameters.iter().any(|(_, parameter)| {
        parameter.symbol == left.symbol
            && match program
                .type_reference_table
                .type_reference(parameter.type_reference)
            {
                TypeReferenceNode::Named { symbol, .. } => program
                    .symbols
                    .builtin_type_atom(*symbol)
                    .is_some_and(|atom| atom.is_fixed_integer()),
                _ => false,
            }
    })
}

pub(super) fn transparent_proposition_proves_exit(
    program: &dyn ::validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    outcome: &MachineEntailmentOutcome,
    state_flow: &checked_trees::FlowStateFact,
    fact: &facts::Fact,
) -> bool {
    if outcome.machine != state_flow.machine_symbol || !outcome.entry_premises_preserved {
        return false;
    }
    let facts::FactPayload::ContractPropositionApplication { fact: source, .. } = fact.payload
    else {
        return false;
    };
    let Some(application) = program.proof_facts.get(source).as_proposition() else {
        return false;
    };
    let Some(machine) = program.machines().iter().find(|machine| {
        machine.symbol == state_flow.machine_symbol
            && machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
    }) else {
        return false;
    };
    let Some(state) = validation::find_state_in_machine(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
    ) else {
        return false;
    };
    // A requirement-authored fact migrated onto this state names the
    // requirement's proposition parameter and parameters; its conformance
    // edge selects the family and the satisfier's terms.
    let inherited = facts
        .proof
        .contract_facts
        .iter()
        .find(|(_, contract)| {
            contract.fact == source
                && contract.owner
                    == checked_trees::ContractProofFactOwner::MachineState {
                        machine_symbol: state_flow.machine_symbol,
                        state_symbol: state_flow.state_symbol,
                    }
        })
        .and_then(|(_, contract)| {
            crate::semantic::facts::inherited_contract_scope_parts(program, &facts.proof, contract)
        })
        .map(
            |(trait_definition, requirement, _, scope)| validation::InheritedRequirementEdge {
                trait_definition,
                requirement,
                trait_arguments: &scope.trait_arguments,
            },
        );
    validation::transparent_proposition_application_entailed(
        program,
        machine,
        state,
        application,
        inherited,
    )
}

/// Entry-context proofs do not establish a mutable subject's exit revision.
/// Complete empty checked frames exclude transitive writes. Direct owned
/// parameter assignments must also be absent: they are not outward effects.
fn entry_premises_are_preserved(
    program: &dyn ::validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    machine_symbol: SymbolHandle,
    classification: &typed_trees::proof_only::ProofOnlyClassification,
    resolver: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let Some(machine) = program.machine_by_symbol(machine_symbol) else {
        return false;
    };
    if resolver.is_some_and(|resolver| {
        isolated_proof_values(program, facts, machine, classification, resolver)
    }) {
        return true;
    }
    let Some(mutation) = facts.mutation.for_machine(machine_symbol) else {
        return false;
    };
    !program.machine_states(machine).is_empty()
        && program.machine_states(machine).iter().all(|state| {
            !program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .any(|statement| {
                    matches!(
                        statement,
                        typed_trees::statement::StatementNode::Assignment(_)
                    )
                })
                && mutation.state_write_frames.iter().any(|frame| {
                    frame.state == state.symbol
                        && frame.frame.is_complete()
                        && frame.frame.paths().is_empty()
                })
        })
}

/// Structural induction and call formation share one proof language, but not
/// one observation point. Ask for the exact call entry, never reuse an exit
/// judgment or import an unproved callee guarantee.
pub(super) fn structural_call_requirement(
    program: &dyn ::validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    state_flow: &checked_trees::FlowStateFact,
    call_flow: &checked_trees::FlowCallFact,
    expression: ExpressionHandle,
    resolver: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let classification = validation::proof_only_classification(program);
    let Some((machine, state)) = validation::machine_state_by_symbols(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
    ) else {
        return false;
    };
    let Some(call_site) = validation::find_call_site(
        program,
        machine.symbol,
        state.symbol,
        call_flow.statement_index,
        call_flow.call_ordinal,
    ) else {
        return false;
    };
    // Expression calls can follow earlier sibling operands in this statement.
    // Their exact normal-call traversal owns those effects and capture epochs;
    // starting a fresh argument-only entry here would skip the siblings.
    if matches!(call_site, validation::CallSite::Expression { .. }) {
        return validation::selected_value_call_requirement_entailed(
            program,
            machine,
            state,
            call_flow.statement_index,
            call_site,
            expression,
        );
    }
    let Some(callee) = validation::call_target_state(program, call_flow.target_symbol) else {
        return false;
    };
    let callee_machine = program.machine_holding_state(callee.symbol);
    let runtime_values = !classification.is_proof_machine(program, machine)
        || callee_machine.is_none_or(|callee| !classification.is_proof_machine(program, callee));
    let case_facts_only = runtime_values
        && callee_machine.is_some_and(|machine| {
            case_preservation::is_case_requirement(program, machine, callee, expression)
        });
    let preserved = if case_facts_only {
        case_preservation::body_preserves_case_tags(program, facts, machine)
    } else if runtime_values {
        // The runtime path owner checks each executed prefix frame, including
        // argument evaluation, before retaining its observations. Writes after
        // this occurrence and recursive successors cannot invalidate its entry.
        true
    } else {
        entry_premises_are_preserved(program, facts, machine.symbol, &classification, resolver)
    };
    if !preserved {
        return false;
    }
    let arguments = validation::call_site_argument_expressions(program, &call_site);
    let in_transition_target = validation::transition_call_target(
        program,
        machine,
        state,
        call_flow.statement_index,
        call_flow.call_ordinal,
    )
    .is_some_and(|target| target.is_valid());
    validation::structural_call_requirement_entailed(
        program,
        machine,
        state.symbol,
        call_flow.statement_index,
        in_transition_target,
        callee,
        arguments,
        expression,
        case_facts_only,
    )
}

fn isolated_proof_values(
    program: &dyn ::validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    machine: &typed_trees::machine::Machine,
    classification: &typed_trees::proof_only::ProofOnlyClassification,
    resolver: &validation::CallFrameResolver<'_>,
) -> bool {
    if !classification.is_proof_machine(program, machine) {
        return false;
    }
    let isolated_parameters = |state: &typed_trees::state::State| {
        program.state_parameters(state).iter().all(|parameter| {
            !parameter.is_mutable
                && resolver.proof_value_is_caller_isolated(parameter.type_reference)
        })
    };
    program.machine_states(machine).iter().all(|state| {
        isolated_parameters(state)
            && program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .all(|statement| match statement {
                    typed_trees::statement::StatementNode::Assignment(_) => false,
                    typed_trees::statement::StatementNode::LocalData(local) => {
                        if validation::is_arm_pattern_marker(statement) {
                            // The marker's Unit type is validation metadata.
                            // Resolve its real subject through the same place
                            // law as destructure validation, then inspect the
                            // complete carrier, including every payload field.
                            validation::declared_place_type_raw(
                                program,
                                machine,
                                Some(state),
                                local.initial_value,
                            )
                            .is_some_and(|reference| {
                                resolver.proof_value_is_caller_isolated(reference)
                            })
                        } else {
                            resolver.proof_value_is_caller_isolated(local.type_reference)
                        }
                    }
                    _ => true,
                })
    }) && facts
        .flow
        .control
        .states
        .iter()
        .filter(|(_, state)| state.machine_symbol == machine.symbol)
        .all(|(_, state)| {
            facts
                .flow
                .control
                .calls
                .span_or_empty(state.calls)
                .iter()
                .all(|call| {
                    program
                        .state_by_symbol(call.target_symbol)
                        .map(isolated_parameters)
                        .or_else(|| {
                            program
                                .machine_parameter_signature_in(machine, call.target_symbol)
                                .map(|signature| {
                                    program.state_signature_parameters(signature).iter().all(
                                        |parameter| {
                                            !parameter.is_mutable
                                                && resolver.proof_value_is_caller_isolated(
                                                    parameter.type_reference,
                                                )
                                        },
                                    )
                                })
                        })
                        .unwrap_or(false)
                })
        })
}
