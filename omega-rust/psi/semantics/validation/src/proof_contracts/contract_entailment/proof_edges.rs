//! Strict decrease judgments along proof edges.

use crate::proof_contracts::contract_entailment::citations::{
    intake_citation_for_edge, intake_statement_citation_for_edge, machine_requires_facts,
};
use crate::proof_contracts::contract_entailment::structural_judgment::{
    StructuralJudge, StructuralJudgment, StructuralTerm,
};
use crate::proof_contracts::contract_entailment::structural_terms::structural_term;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

/// Resolve and gate ONE citation, pushing the callee's ensures conjuncts
/// instantiated at `argument_terms` (the call's arguments ALREADY converted
/// to terms in the consumer's frame: machine-level intake reads them raw,
/// per-arm intake converts under the arm environment first). `result` maps
/// to the application at these operands.
/// N4 slice a3 (gcd): judge a recursive proof machine edge's STRICT-DECREASE
/// obligation through the structural judge -- the general route when the
/// syntactic citation match cannot see through arm destructuring. The judge
/// starts from the machine's requires, gains the source state's INCOMING-ARM
/// hypotheses (guard equations, plus the MATERIALIZED payload alias
/// `subject == Case { field: param }` recovered from a tag-only guard, the
/// data declaration, and the incoming transition's payload-read target
/// arguments), then intakes the source state's citations IN ORDER --
/// statement calls and `let`-bound call initializers alike -- each with its
/// requires judged Proven first (skipped otherwise; over-refusal safe) and
/// its ensures instantiated with `result` mapped to the call term. The
/// obligation `saturating_sub(Succ(ARG), MEASURE) == Zero` then judges under the
/// accumulated hypotheses.
pub(crate) fn proof_edge_strict_decrease_judged(
    program: &TypedTrees,
    machine: &Machine,
    state: &typed_trees::state::State,
    edge_argument: ExpressionHandle,
    measure_name: &str,
) -> bool {
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
    // This legacy synthetic Nat obligation has no authored call occurrence.
    // Resolve its declaration once, refusing ambiguity; ordinary applications
    // never recover a missing selected identity from this spelling.
    let mut candidates = program.machines().iter().filter(|candidate| {
        candidate.attached_data.is_none() && candidate.name.as_str() == "saturating_sub"
    });
    let Some(selected) = candidates.next() else {
        return false;
    };
    if candidates.next().is_some() {
        return false;
    }
    let Some(selected_entry) = program.machine_states(selected).first() else {
        return false;
    };
    let left = StructuralTerm::Application {
        target: selected_entry.symbol,
        selections: Vec::new(),
        machine: "saturating_sub".to_owned(),
        arguments: vec![
            StructuralTerm::Constructor {
                data: "Nat".to_owned(),
                case: "Succ".to_owned(),
                fields: vec![("prev".to_owned(), argument_term)],
            },
            StructuralTerm::Variable(measure_name.to_owned()),
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
