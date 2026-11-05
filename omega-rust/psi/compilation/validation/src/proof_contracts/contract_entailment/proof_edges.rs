//! Strict decrease judgments at the ordered structural prefix.
//!
//! The same path owner used by call-premise checking retains captured subjects,
//! selected cases, state actuals and completed citations. Reconstructing just a
//! predecessor guard loses a computed subject's local binding and can mistake
//! same-spelled state formals for entry parameters. The prefix stops before the
//! queried call: its conclusion and later citations cannot license its descent.
//! An authored comparison can request descent checking, but only an independently
//! checked body theorem can connect that comparison to a well-founded order.

use super::citations::machine_requires_facts;
use super::structural_case_arms::recognize_structural_case_arms;
use super::structural_judgment::{StructuralJudge, StructuralJudgment, StructuralTerm};
use super::structural_prefix::{
    PrefixObservation, StructuralPrefixSite, StructuralPrefixVisitor,
    expression_is_transition_target,
};
use super::structural_terms::{constructed_term, selected_application_machine, structural_term};
use symbols::SymbolHandle;
use typed_trees::data::{DataDefinition, DataField, DataMember, DataVariant};
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;

/// Judge the certified comparison on every path to this edge. Unsupported
/// prefixes and missing observations prove nothing; a recursive conclusion
/// cannot establish the decrease required to admit that same call.
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
) -> Option<(
    SymbolHandle,
    arena::Handle<crate::mathematical_applications::PreparedComputedOrder>,
)> {
    let entry = program.machine_states(machine).first()?;
    // Entry-relative premises describe all arrivals only on an acyclic state
    // graph. The shared prefix walker does not replay a recurrent arrival;
    // accepting its first visit would donate stale premises to that cycle.
    if !crate::machine_calls::calls::named_state_transition_subgraph_is_acyclic(
        program, machine, entry,
    ) {
        return None;
    }
    let relation = selected_decrease_relation(program, machine, state, measure)?;
    if relation
        .evidence
        .dependencies
        .machines
        .iter()
        .any(|dependency| *dependency == machine.symbol || ranked_entries.contains(dependency))
    {
        return None;
    }
    let relation_symbol = relation.symbol;
    let order = relation.order;
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
    (recognized.is_some() && diagnostics.is_empty() && edge.seen && edge.proven)
        .then_some((relation_symbol, order))
}

struct StrictDecrease<'a> {
    site: StructuralPrefixSite,
    argument: ExpressionHandle,
    relation: SelectedDecreaseRelation<'a>,
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
            result_type: crate::call_target_return_type(program, self.relation.symbol)
                .unwrap_or_default(),
            target: self.relation.symbol,
            selections: Vec::new(),
            machine: self.relation.name.to_owned(),
            arguments: vec![
                constructed_term(
                    self.relation.carrier,
                    Some(self.relation.successor),
                    vec![(
                        self.relation.child.name.as_str().to_owned(),
                        StructuralJudge::substitute_term(&argument, environment),
                    )],
                ),
                StructuralTerm::Variable(self.measure.clone()),
            ],
        };
        let right = constructed_term(self.relation.carrier, Some(self.relation.zero), Vec::new());
        self.proven &= matches!(
            judge.judge_equation(judge.resolve(left), judge.resolve(right), 0),
            StructuralJudgment::Proven
        );
    }
}

/// Every selected relation retains its exact carrier, source body theorem and
/// structural dependency closure. Names remain diagnostic text only.
struct SelectedDecreaseRelation<'program> {
    symbol: SymbolHandle,
    name: &'program str,
    carrier: &'program DataDefinition,
    successor: &'program DataVariant,
    zero: &'program DataVariant,
    child: &'program DataField,
    evidence: &'program crate::mathematical_applications::PreparedComputedOrderEvidence,
    order: arena::Handle<crate::mathematical_applications::PreparedComputedOrder>,
}

fn selected_decrease_relation<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    measure: &typed_trees::signature::StateParameter,
) -> Option<SelectedDecreaseRelation<'program>> {
    let ordinal = super::computed_order::ordinal_carrier(program, measure.type_reference).ok()?;
    let definition = program.data_definition_by_symbol(ordinal.declaration)?;
    let view = program.prepared_mathematics()?;
    let mut selected = None;
    for request in
        super::computed_order::authored_order_requests(program, machine, state, &ordinal)?
    {
        let Some((order, row)) = view.computed_order(request.target, ordinal.identity) else {
            continue;
        };
        let Ok(evidence) = &row.judgment else {
            continue;
        };
        if selected
            .as_ref()
            .is_some_and(|(symbol, _, _, _)| *symbol != request.target)
        {
            return None;
        }
        let relation = selected_application_machine(program, request.target)?;
        selected = Some((request.target, relation.name.as_str(), evidence, order));
    }
    let (symbol, name, evidence, order) = selected?;
    let case = |symbol| {
        program
            .data_members(definition)
            .iter()
            .find_map(|member| match member {
                DataMember::Variant(variant) if variant.symbol == symbol => Some(variant),
                _ => None,
            })
    };
    let successor = case(ordinal.successor)?;
    let child = program
        .data_payload_fields(successor)
        .iter()
        .find(|field| field.symbol == ordinal.child)?;
    Some(SelectedDecreaseRelation {
        symbol,
        name,
        carrier: definition,
        successor,
        zero: case(ordinal.base)?,
        child,
        evidence,
        order,
    })
}
