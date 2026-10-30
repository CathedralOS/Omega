//! Strict decrease judgments at the ordered structural prefix.
//!
//! The same path owner used by call-premise checking retains captured subjects,
//! selected cases, state actuals and completed citations. Reconstructing just a
//! predecessor guard loses a computed subject's local binding and can mistake
//! same-spelled state formals for entry parameters. The prefix stops before the
//! queried call: its conclusion and later citations cannot license its descent.
//! The legacy Nat/monus bridge still selects its relation from exact authored
//! contract occurrences, never from a program-wide spelling search.

use super::citations::machine_requires_facts;
use super::law_conformance::collect_equality_conjuncts;
use super::structural_case_arms::recognize_structural_case_arms;
use super::structural_judgment::{StructuralJudge, StructuralJudgment, StructuralTerm};
use super::structural_prefix::{
    PrefixObservation, StructuralPrefixSite, StructuralPrefixVisitor,
    expression_is_transition_target,
};
use super::structural_terms::{selected_application_machine, structural_term};
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::machine::Machine;
use typed_trees::statement::StatementNode;
use typed_trees::types::TypeReferenceNode;

/// Judge the selected Nat predecessor obligation on every path to this edge.
/// Unsupported prefixes and missing observations prove nothing. The operation
/// remains the existing monus bridge; this changes its premise transport, not
/// the supported ranking relation or the authority of a recursive conclusion.
pub(crate) fn proof_edge_strict_decrease_judged(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    edge_argument: ExpressionHandle,
    measure: &typed_trees::signature::StateParameter,
) -> bool {
    proof_interval_strict_decrease_judged(
        program,
        machine,
        state,
        statement_index,
        edge_argument,
        measure,
        &[],
    )
    .is_some()
}

/// Compare an exact call argument with the rank at the interval's entry.
/// Intermediate ordinary calls carry actuals and guards through the shared
/// prefix walker. Other ranked entries close the interval; their bodies and
/// recursive conclusions cannot establish this decrease. Retain the exact
/// selected order relation so a component cannot mix unrelated orders.
pub(crate) fn proof_interval_strict_decrease_judged(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    edge_argument: ExpressionHandle,
    measure: &typed_trees::signature::StateParameter,
    ranked_entries: &[SymbolHandle],
) -> Option<SymbolHandle> {
    let Some(entry) = program.machine_states(machine).first() else {
        return None;
    };
    // Entry-relative premises describe all arrivals only on an acyclic state
    // graph. The shared prefix walker does not replay a recurrent arrival;
    // accepting its first visit would donate stale premises to that cycle.
    if !crate::machine_calls::calls::named_state_transition_subgraph_is_acyclic(
        program, machine, entry,
    ) {
        return None;
    }
    let Some(relation) = selected_decrease_relation(program, machine, state, measure) else {
        return None;
    };
    let requires = machine_requires_facts(program, machine);
    let judge = StructuralJudge::from_requires(program, machine, &requires, true);
    let mut edge = StrictDecrease {
        site: StructuralPrefixSite {
            state: state.symbol,
            statement_index,
            in_transition_target: expression_is_transition_target(
                program,
                state,
                statement_index,
                edge_argument,
            ),
        },
        argument: edge_argument,
        relation,
        measure: measure.name.as_str().to_owned(),
        ranked_entries,
        seen: false,
        proven: true,
    };
    let mut diagnostics = Vec::new();
    let recognized = recognize_structural_case_arms(
        program,
        machine,
        &judge,
        &crate::proof_only_classification(program),
        &mut diagnostics,
        Some(&mut edge),
    );
    (recognized.is_some() && diagnostics.is_empty() && edge.seen && edge.proven).then_some(relation)
}

struct StrictDecrease<'a> {
    site: StructuralPrefixSite,
    argument: ExpressionHandle,
    relation: SymbolHandle,
    measure: String,
    ranked_entries: &'a [SymbolHandle],
    seen: bool,
    proven: bool,
}

impl StructuralPrefixVisitor for StrictDecrease<'_> {
    fn site(&self) -> StructuralPrefixSite {
        self.site
    }

    fn stops_at_machine(&self, machine: SymbolHandle) -> bool {
        self.ranked_entries.contains(&machine)
    }

    fn entered(&mut self) {
        self.seen = true;
    }

    fn unresolved(&mut self, _machine: &Machine) {
        self.proven = false;
    }

    fn observe<'program>(&mut self, observation: PrefixObservation<'_, 'program>) {
        let (program, machine, state, judge, environment) = observation;
        let Some(argument) = structural_term(
            program,
            Some((machine, state, Default::default())),
            None,
            self.argument,
            None,
        ) else {
            self.proven = false;
            return;
        };
        let left = StructuralTerm::Application {
            result_type: crate::call_target_return_type(program, self.relation).unwrap_or_default(),
            target: self.relation,
            selections: Vec::new(),
            machine: "saturating_sub".to_owned(),
            arguments: vec![
                StructuralTerm::Constructor {
                    data: "Nat".to_owned(),
                    case: "Succ".to_owned(),
                    fields: vec![(
                        "prev".to_owned(),
                        StructuralJudge::substitute_term(&argument, environment),
                    )],
                },
                StructuralTerm::Variable(self.measure.clone()),
            ],
        };
        let right = StructuralTerm::Constructor {
            data: "Nat".to_owned(),
            case: "Zero".to_owned(),
            fields: Vec::new(),
        };
        self.proven &= matches!(
            judge.judge_equation(judge.resolve(left), judge.resolve(right), 0),
            StructuralJudgment::Proven
        );
    }
}

/// Recover the existing Nat order bridge from authored selections. The legacy
/// spelling restricts the bridge's supported relation; it cannot select a
/// declaration or rescue a missing symbol. Multiple occurrences of one selected
/// relation agree; conflicting relations remain unsupported.
fn selected_decrease_relation(
    program: &dyn crate::CheckingInput,
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
                let Some(call) = program.expression_table.call_node(local.initial_value) else {
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
                let Some(expression) = fact.as_expression() else {
                    continue;
                };
                conjuncts.clear();
                collect_equality_conjuncts(program, *expression, &mut conjuncts);
                for expression in &conjuncts {
                    let Some(equality) = program.expression_table.binary_node(*expression) else {
                        continue;
                    };
                    if equality.operator != BinaryOperator::Equal {
                        continue;
                    }
                    for operand in [equality.left, equality.right] {
                        let Some(call) = program.expression_table.call_node(operand) else {
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
