use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::ExpressionHandle;

#[cfg(test)]
mod tests;

/// Transient, program-local results of positive entailment judgments. The
/// runtime place prover need not duplicate structural or integer proof rules,
/// but absence of a validation error is not evidence for any postcondition.
pub(super) struct ProvenExitExpressions<'program> {
    program: &'program TypedTrees,
    classification: &'program psi_typed_trees::proof_only::ProofOnlyClassification,
    resolver: Option<psi_validation::CallFrameResolver<'program>>,
    machines: Vec<MachineEntailmentOutcome>,
}

pub(super) struct MachineEntailmentOutcome {
    machine: SymbolHandle,
    pub(super) expressions: Vec<ExpressionHandle>,
    entry_premises_preserved: bool,
}

impl<'program> ProvenExitExpressions<'program> {
    pub(super) fn new(
        program: &'program TypedTrees,
        classification: &'program psi_typed_trees::proof_only::ProofOnlyClassification,
    ) -> Self {
        Self {
            program,
            classification,
            resolver: psi_validation::CallFrameResolver::new(program),
            machines: Vec::new(),
        }
    }

    pub(super) fn for_machine(
        &mut self,
        facts: &psi_checked_trees::CheckFacts,
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
                    self.resolver.as_ref(),
                );
                let mut expressions = if entry_premises_preserved {
                    psi_validation::proven_machine_contract_expressions(
                        self.program,
                        machine_symbol,
                    )
                } else {
                    Vec::new()
                };
                if !expressions.is_empty() {
                    // A signature's op-slot expression has a different handle
                    // from the authored guarantee. Match through the existing
                    // exact conformance law, and independently require every
                    // supporting authored conjunct to have been proved.
                    let inherited = psi_validation::matched_machine_law_guarantees(
                        self.program,
                        machine_symbol,
                    )
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
pub(super) fn integral_parameter_reflexivity(program: &TypedTrees, fact: &psi_facts::Fact) -> bool {
    use psi_typed_trees::expression::{BinaryOperator, ExpressionNode};
    use psi_typed_trees::types::TypeReferenceNode;
    let psi_facts::FactPayload::ContractBooleanExpression { expression, .. } = fact.payload else {
        return false;
    };
    let ExpressionNode::Binary(comparison) = program.expression_table.expression(expression) else {
        return false;
    };
    if comparison.operator != BinaryOperator::Equal {
        return false;
    }
    let ExpressionNode::Name(left) = program.expression_table.expression(comparison.left) else {
        return false;
    };
    let ExpressionNode::Name(right) = program.expression_table.expression(comparison.right) else {
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
                TypeReferenceNode::Named { symbol, .. } => matches!(
                    program.symbols.builtin_type_atom(*symbol),
                    Some(
                        psi_symbols::BuiltinTypeAtom::I8
                            | psi_symbols::BuiltinTypeAtom::I16
                            | psi_symbols::BuiltinTypeAtom::I32
                            | psi_symbols::BuiltinTypeAtom::I64
                            | psi_symbols::BuiltinTypeAtom::U8
                            | psi_symbols::BuiltinTypeAtom::U16
                            | psi_symbols::BuiltinTypeAtom::U32
                            | psi_symbols::BuiltinTypeAtom::U64
                    )
                ),
                _ => false,
            }
    })
}

pub(super) fn transparent_proposition_proves_exit(
    program: &TypedTrees,
    outcome: &MachineEntailmentOutcome,
    state_flow: &psi_checked_trees::FlowStateFact,
    fact: &psi_facts::Fact,
) -> bool {
    if outcome.machine != state_flow.machine_symbol || !outcome.entry_premises_preserved {
        return false;
    }
    let psi_facts::FactPayload::ContractPropositionApplication { fact: source, .. } = fact.payload
    else {
        return false;
    };
    let psi_typed_trees::domain::ProofFact::Proposition(application) =
        program.proof_facts.get(source)
    else {
        return false;
    };
    let Some(machine) = program.machines().iter().find(|machine| {
        machine.symbol == state_flow.machine_symbol
            && machine.supply_mode == psi_language_semantics::MachineSupplyMode::CheckedBody
    }) else {
        return false;
    };
    let Some(state) = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_flow.state_symbol)
    else {
        return false;
    };
    psi_validation::transparent_proposition_application_entailed(
        program,
        machine,
        state,
        application,
    )
}

/// Entry-context proofs do not establish a mutable subject's exit revision.
/// Complete empty checked frames exclude transitive writes. Direct owned
/// parameter assignments must also be absent: they are not outward effects.
fn entry_premises_are_preserved(
    program: &TypedTrees,
    facts: &psi_checked_trees::CheckFacts,
    machine_symbol: SymbolHandle,
    classification: &psi_typed_trees::proof_only::ProofOnlyClassification,
    resolver: Option<&psi_validation::CallFrameResolver<'_>>,
) -> bool {
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
    else {
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
                        psi_typed_trees::statement::StatementNode::Assignment(_)
                    )
                })
                && mutation.state_write_frames.iter().any(|frame| {
                    frame.state == state.symbol
                        && frame.frame.is_complete()
                        && frame.frame.paths().is_empty()
                })
        })
}

fn isolated_proof_values(
    program: &TypedTrees,
    facts: &psi_checked_trees::CheckFacts,
    machine: &psi_typed_trees::machine::Machine,
    classification: &psi_typed_trees::proof_only::ProofOnlyClassification,
    resolver: &psi_validation::CallFrameResolver<'_>,
) -> bool {
    if !classification.is_proof_machine(program, machine) {
        return false;
    }
    let isolated_parameters = |state: &psi_typed_trees::state::State| {
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
                    psi_typed_trees::statement::StatementNode::Assignment(_) => false,
                    psi_typed_trees::statement::StatementNode::LocalData(local) => {
                        if psi_validation::is_arm_pattern_marker(statement) {
                            // The marker's Unit type is validation metadata.
                            // Resolve its real subject through the same place
                            // law as destructure validation, then inspect the
                            // complete carrier, including every payload field.
                            psi_validation::declared_place_type_raw(
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
                        .machines()
                        .iter()
                        .flat_map(|machine| program.machine_states(machine))
                        .find(|target| target.symbol == call.target_symbol)
                        .is_some_and(isolated_parameters)
                })
        })
}
