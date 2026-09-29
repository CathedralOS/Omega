//! Strict decrease judgments along proof edges.
//!
//! A Build and a product can carry distinct checked instances of the same
//! library. The legacy Nat/monus obligation therefore takes its operation from
//! resolved contract occurrences in this machine and its selected citations,
//! never from a program-wide spelling search. Relation discovery supplies no
//! hypotheses: ordered citation intake still proves every premise independently.

use super::law_conformance::collect_equality_conjuncts;
use super::structural_terms::selected_application_machine;
use crate::proof_contracts::contract_entailment::citations::{
    intake_citation_for_edge, intake_statement_citation_for_edge, machine_requires_facts,
};
use crate::proof_contracts::contract_entailment::structural_judgment::{
    StructuralJudge, StructuralJudgment, StructuralTerm,
};
use crate::proof_contracts::contract_entailment::structural_terms::structural_term;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use typed_trees::types::TypeReferenceNode;

/// Judge one computed Nat predecessor with ordinary structural entailment.
/// The machine's requires and its unique incoming arm supply initial facts,
/// including exact case-payload aliases. Statement and value-call citations
/// contribute their instantiated guarantees in source order, only after their
/// premises prove. The selected monus obligation
/// `saturating_sub(Succ(ARG), MEASURE) == Zero` must then follow from those facts.
pub(crate) fn proof_edge_strict_decrease_judged(
    program: &TypedTrees,
    machine: &Machine,
    state: &typed_trees::state::State,
    edge_argument: ExpressionHandle,
    measure: &typed_trees::signature::StateParameter,
) -> bool {
    let Some(relation) = selected_decrease_relation(program, machine, state, measure) else {
        return false;
    };
    let requires = machine_requires_facts(program, machine);
    let mut judge = StructuralJudge::from_requires(program, machine, &requires);
    let trace = std::env::var_os("OMEGA_EDGE_TRACE").is_some();

    // Incoming-arm hypotheses -- SOUND only when this state has exactly ONE
    // incoming edge (otherwise a second path could reach it without the
    // arm's case holding; conservative: intake nothing).
    let mut incoming = 0usize;
    for other in program.machine_states(machine) {
        for statement in program.statement_table.statements(other.statement_nodes) {
            let StatementNode::Transition(transition) = statement else {
                continue;
            };
            if !transition.target.is_valid() {
                continue;
            }
            if let TransitionTargetNode::Named { path, .. } =
                program.statement_table.transition_target(transition.target)
                && program
                    .statement_table
                    .name_path_members(path.members)
                    .last()
                    .is_some_and(|name| name.as_str() == state.name.as_str())
            {
                incoming += 1;
            }
        }
    }
    if incoming != 1 && trace {
        eprintln!(
            "EDGE {}: {} incoming edges -- arm facts skipped",
            state.name.as_str(),
            incoming
        );
    }
    for other in program.machine_states(machine) {
        if incoming != 1 {
            break;
        }
        for statement in program.statement_table.statements(other.statement_nodes) {
            let StatementNode::Transition(transition) = statement else {
                continue;
            };
            let TransitionGuardNode::When(guard) = transition.guard else {
                if trace {
                    eprintln!(
                        "EDGE incoming arm into {}: guard NOT When",
                        state.name.as_str()
                    );
                }
                continue;
            };
            if !transition.target.is_valid() {
                continue;
            }
            let TransitionTargetNode::Named {
                path, arguments, ..
            } = program.statement_table.transition_target(transition.target)
            else {
                continue;
            };
            let targets_state = program
                .statement_table
                .name_path_members(path.members)
                .last()
                .is_some_and(|name| name.as_str() == state.name.as_str());
            if !targets_state {
                continue;
            }
            // Materialize the payload alias for a tag-only case guard
            // BEFORE any raw intake: a fieldless `b == Nat::Succ`
            // substitution would win first and mask the payload (first
            // binding wins), leaving `b`'s prev unreachable.
            let ExpressionNode::Binary(comparison) = program.expression_table.expression(guard)
            else {
                judge.intake(program, guard);
                continue;
            };
            if !matches!(
                comparison.operator,
                BinaryOperator::Equal | BinaryOperator::CaseMembership
            ) {
                judge.intake(program, guard);
                continue;
            }
            let Some(subject_term) = structural_term(program, comparison.left) else {
                continue;
            };
            let Some((definition, variant)) = super::structural_terms::case_guard_classifier(
                program,
                machine,
                Some(other),
                guard,
            ) else {
                judge.intake(program, guard);
                continue;
            };
            let data = definition.name.as_str().to_owned();
            let case = variant.name.as_str().to_owned();
            let declared_fields = program
                .data_members(definition)
                .iter()
                .filter_map(|member| match member {
                    typed_trees::data::DataMember::Field(field) => {
                        Some(field.name.as_str().to_owned())
                    }
                    _ => None,
                })
                .chain(
                    program
                        .data_payload_fields(variant)
                        .iter()
                        .map(|field| field.name.as_str().to_owned()),
                )
                .collect::<Vec<_>>();
            if declared_fields.is_empty() {
                judge.intake(program, guard);
                continue;
            }
            // Payload reads in the incoming target arguments: an argument
            // `<subject>.field` delivered to sub-state param `p` aliases
            // the payload as `p`.
            let parameters = program.state_parameters(state);
            let argument_handles = program.statement_table.expression_handles(*arguments);
            if parameters.len() != argument_handles.len() {
                judge.intake(program, guard);
                continue;
            }
            let mut aliased: Vec<(String, StructuralTerm)> = Vec::new();
            for (parameter, argument) in parameters.iter().zip(argument_handles) {
                let ExpressionNode::Member(member) = program.expression_table.expression(*argument)
                else {
                    continue;
                };
                let Some(receiver_term) = structural_term(program, member.receiver) else {
                    continue;
                };
                if receiver_term != subject_term {
                    continue;
                }
                let member_name = member.member.as_str().to_owned();
                if declared_fields.contains(&member_name) {
                    aliased.push((
                        member_name,
                        StructuralTerm::Variable(parameter.name.as_str().to_owned()),
                    ));
                }
            }
            if trace {
                eprintln!(
                    "EDGE alias for {}: aliased {}/{} declared",
                    state.name.as_str(),
                    aliased.len(),
                    declared_fields.len()
                );
            }
            if aliased.len() == declared_fields.len()
                && declared_fields.iter().all(|declared| {
                    aliased.iter().filter(|(name, _)| name == declared).count() == 1
                })
            {
                aliased.sort_by(|(left, _), (right, _)| left.cmp(right));
                judge.intake_equation(
                    subject_term,
                    StructuralTerm::Constructor {
                        data,
                        case,
                        fields: aliased,
                    },
                    0,
                );
            } else {
                judge.intake(program, guard);
            }
        }
    }

    // Citations + local bindings, in statement order.
    for statement in program.statement_table.statements(state.statement_nodes) {
        match statement {
            StatementNode::LocalData(local) if local.initial_value.is_valid() => {
                if let ExpressionNode::Call(call) =
                    program.expression_table.expression(local.initial_value)
                {
                    intake_citation_for_edge(
                        program,
                        &mut judge,
                        call,
                        Some(local.name.as_str()),
                        local.initial_value,
                    );
                } else if let Some(term) = structural_term(program, local.initial_value) {
                    judge.intake_equation(
                        StructuralTerm::Variable(local.name.as_str().to_owned()),
                        term,
                        0,
                    );
                }
            }
            StatementNode::Call(call) => {
                let receiver_members = program.statement_table.name_path_members(call.receiver);
                if !receiver_members.is_empty() {
                    continue;
                }
                intake_statement_citation_for_edge(program, &mut judge, call);
            }
            _ => {}
        }
    }

    // The obligation.
    let Some(argument_term) = structural_term(program, edge_argument) else {
        return false;
    };
    let left = StructuralTerm::Application {
        target: relation,
        selections: Vec::new(),
        machine: "saturating_sub".to_owned(),
        arguments: vec![
            StructuralTerm::Constructor {
                data: "Nat".to_owned(),
                case: "Succ".to_owned(),
                fields: vec![("prev".to_owned(), argument_term)],
            },
            StructuralTerm::Variable(measure.name.as_str().to_owned()),
        ],
    };
    let right = StructuralTerm::Constructor {
        data: "Nat".to_owned(),
        case: "Zero".to_owned(),
        fields: Vec::new(),
    };
    let verdict = judge.judge_equation(judge.resolve(left.clone()), judge.resolve(right), 0);
    if trace {
        eprintln!(
            "EDGE obligation in {}: resolved LHS {:?} verdict {}",
            state.name.as_str(),
            judge.resolve(left),
            match verdict {
                StructuralJudgment::Proven => "Proven",
                StructuralJudgment::Refuted => "Refuted",
                StructuralJudgment::Unknown => "Unknown",
            }
        );
    }
    matches!(verdict, StructuralJudgment::Proven)
}

/// Recover the existing Nat order bridge from authored selections. The legacy
/// spelling restricts the bridge's supported relation; it cannot select a
/// declaration or rescue a missing symbol. Multiple occurrences of one selected
/// relation agree; conflicting relations remain unsupported.
fn selected_decrease_relation(
    program: &TypedTrees,
    machine: &Machine,
    state: &typed_trees::state::State,
    measure: &typed_trees::signature::StateParameter,
) -> Option<SymbolHandle> {
    let TypeReferenceNode::Named {
        symbol: carrier, ..
    } = program
        .type_reference_table
        .type_reference(measure.type_reference)
    else {
        return None;
    };
    let definition = program.data_definition_by_symbol(*carrier)?;
    if definition.name.as_str() != "Nat" {
        return None;
    }
    let is_carrier = |reference| {
        matches!(
            program.type_reference_table.type_reference(reference),
            TypeReferenceNode::Named { symbol, .. } if symbol == carrier
        )
    };
    let mut owners = vec![machine];
    for statement in program.statement_table.statements(state.statement_nodes) {
        let target = match statement {
            StatementNode::Call(call) if call.receiver.is_empty() => {
                if call.static_machine_parameter.is_valid()
                    || call.static_requirement_dispatch.is_some()
                    || !call.machine_arguments.is_empty()
                    || !call.evidence_arguments.is_empty()
                {
                    return None;
                }
                call.target_symbol
            }
            StatementNode::LocalData(local) if local.initial_value.is_valid() => {
                let ExpressionNode::Call(call) =
                    program.expression_table.expression(local.initial_value)
                else {
                    continue;
                };
                if call.receiver.is_valid() {
                    continue;
                }
                if call.static_machine_parameter.is_valid()
                    || call.static_requirement_dispatch.is_some()
                    || !call.machine_arguments.is_empty()
                    || !call.evidence_arguments.is_empty()
                {
                    return None;
                }
                call.target_symbol
            }
            _ => continue,
        };
        if let Some(callee) = selected_application_machine(program, target)
            && !owners.iter().any(|owner| owner.symbol == callee.symbol)
        {
            owners.push(callee);
        }
    }
    let mut selected = SymbolHandle::invalid();
    let mut conjuncts = Vec::new();
    for owner in owners {
        for contract in program.machine_contracts(owner) {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let typed_trees::domain::ProofFact::Expression(expression) = fact else {
                    continue;
                };
                conjuncts.clear();
                collect_equality_conjuncts(program, *expression, &mut conjuncts);
                for expression in &conjuncts {
                    let ExpressionNode::Binary(equality) =
                        program.expression_table.expression(*expression)
                    else {
                        continue;
                    };
                    if equality.operator != BinaryOperator::Equal {
                        continue;
                    }
                    for operand in [equality.left, equality.right] {
                        let ExpressionNode::Call(call) =
                            program.expression_table.expression(operand)
                        else {
                            continue;
                        };
                        let Some(relation) =
                            selected_application_machine(program, call.target_symbol)
                        else {
                            continue;
                        };
                        if relation.attached_data.is_some()
                            || relation.name.as_str() != "saturating_sub"
                        {
                            continue;
                        }
                        let entry = program.machine_states(relation).first()?;
                        let parameters = program.state_parameters(entry);
                        if call.receiver.is_valid()
                            || call.static_machine_parameter.is_valid()
                            || call.static_requirement_dispatch.is_some()
                            || !call.machine_arguments.is_empty()
                            || !call.evidence_arguments.is_empty()
                            || parameters.len() != 2
                            || parameters
                                .iter()
                                .any(|parameter| !is_carrier(parameter.type_reference))
                            || !is_carrier(entry.return_type)
                            || (selected.is_valid() && selected != call.target_symbol)
                        {
                            return None;
                        }
                        selected = call.target_symbol;
                    }
                }
            }
        }
    }
    selected.is_valid().then_some(selected)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn citation_program(premise: &str) -> TypedTrees {
        // This tests transport of already-checked contracts, not theorem-body
        // verification. The whole-program checker separately rejects false lemmas.
        crate::front_end::typed_program(&format!(
            r#"
            data Nat {{ case Zero; case Succ(prev: Nat); }}
            machine saturating_sub(left: Nat, right: Nat) -> Nat
            terminates by right;
            {{
                transition right {{
                    Nat::Zero -> left
                    Nat::Succ {{ prev }} -> (predecessor(saturating_sub(left, prev)))
                }}
            }}
            machine decoy_relation(left: Nat, right: Nat) -> Nat {{ left }}
            machine lemma(value: Nat, next: Nat)
            {premise}
            ensures saturating_sub(Nat::Succ {{ prev: next }}, value) == Nat::Zero {{}}
            machine decoy_lemma(value: Nat, next: Nat) {{}}
            machine caller(value: Nat, next: Nat) -> Nat {{
                lemma(value, next);
                transition {{ _ -> next }}
            }}
            machine predecessor(value: Nat) -> Nat {{
                transition value {{ Nat::Zero -> Nat::Zero Nat::Succ {{ prev }} -> prev }}
            }}
            "#,
        ))
    }

    fn decreases(program: &TypedTrees) -> bool {
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "caller")
            .unwrap();
        let state = &program.machine_states(machine)[0];
        let measure = &program.state_parameters(state)[0];
        let argument = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| {
                let StatementNode::Call(call) = statement else {
                    return None;
                };
                program
                    .statement_table
                    .expression_handles(call.arguments)
                    .get(1)
                    .copied()
            })
            .unwrap();
        proof_edge_strict_decrease_judged(program, machine, state, argument, measure)
    }

    #[test]
    fn selected_relation_survives_unselected_same_spelled_declarations() {
        let mut program = citation_program("");
        assert!(decreases(&program));
        let decoy = program
            .machines()
            .iter()
            .position(|machine| machine.name.as_str() == "decoy_relation")
            .unwrap();
        program.machines_mut()[decoy].name = "saturating_sub".into();
        assert!(decreases(&program));
        program.machines_mut().swap(0, decoy);
        assert!(
            decreases(&program),
            "declaration order must not select the relation"
        );
    }

    #[test]
    fn unselected_same_spelled_lemma_cannot_supply_decrease() {
        let mut program = citation_program("");
        let decoy = program
            .machines()
            .iter()
            .position(|machine| machine.name.as_str() == "decoy_lemma")
            .unwrap();
        let target = program.machine_states(&program.machines()[decoy])[0].symbol;
        program.machines_mut()[decoy].name = "lemma".into();
        let caller = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "caller")
            .unwrap();
        let statements = program.machine_states(caller)[0].statement_nodes;
        let StatementNode::Call(call) = &mut program.statement_table.statements_mut(statements)[0]
        else {
            panic!("citation");
        };
        call.target_symbol = target;
        assert!(!decreases(&program));
        program.machines_mut().swap(2, decoy);
        assert!(
            !decreases(&program),
            "the unselected valid lemma cannot rescue the call"
        );
    }

    #[test]
    fn discovering_a_relation_does_not_discharge_citation_premises() {
        let program = citation_program("requires value == Nat::Zero;");
        assert!(!decreases(&program));
    }

    #[test]
    fn a_wrapped_carrier_is_not_the_nominal_order_relation() {
        let mut program = citation_program("");
        let relation = program.machines()[0].clone();
        let carrier = program.machine_states(&relation)[0].return_type;
        let slice = program
            .type_reference_table
            .insert(TypeReferenceNode::Slice {
                element_type: carrier,
            });
        program.machine_states_mut(&relation)[0].return_type = slice;
        assert!(!decreases(&program));
    }

    #[test]
    fn static_citation_contracts_do_not_borrow_private_guarantees() {
        let mut program = citation_program("");
        let caller = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "caller")
            .unwrap();
        let statements = program.machine_states(caller)[0].statement_nodes;
        let symbol = caller.symbol;
        let StatementNode::Call(call) = &mut program.statement_table.statements_mut(statements)[0]
        else {
            panic!("citation");
        };
        call.static_machine_parameter = symbol;
        assert!(!decreases(&program));
    }
}
