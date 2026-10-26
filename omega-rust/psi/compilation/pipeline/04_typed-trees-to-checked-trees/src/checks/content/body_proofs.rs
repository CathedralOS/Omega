//! Prepare numeric law dependencies from the exact callable's completed exits.
//!
//! The machine-header validator already retains its whole-body candidates.
//! This reader covers state-local declarations through the same completed-value
//! observation owner. Internal transfers cannot masquerade as completed result
//! observations; their proof still belongs to partition composition. No entry
//! projection is rebound to a later state's arrival version.

use checked_trees::CheckFacts;
use language_semantics::content::ContentConservationOwnerKind;

pub(super) fn completed_source(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    source: &validation::ContentConservationSourcePlan,
) -> Option<Vec<validation::ContentConservationDependencies>> {
    if source.plan.owner_kind != ContentConservationOwnerKind::Machine {
        return None;
    }
    let machine = program.machine_by_symbol(source.plan.owner)?;
    if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !machine.body_is_present
    {
        return None;
    }
    let state = program.state_in_machine(machine, source.plan.callable)?;
    let outgoing = super::partition_wrappers::ordinary_returns(program, facts, state);
    if outgoing.is_empty() {
        return None;
    }
    let mut dependencies = Vec::new();
    for handle in outgoing {
        let returned = facts.flow.control.outgoing.get(handle);
        if returned.machine_symbol != machine.symbol
            || returned.state_symbol != state.symbol
            || !facts
                .flow
                .control
                .operands
                .is_valid(returned.returned_value)
        {
            return None;
        }
        let captured = facts.flow.control.operands.get(returned.returned_value);
        if captured.state_symbol != state.symbol
            || captured.statement_index != returned.statement_index
            || !captured.expression.is_valid()
            || !captured.type_reference.is_valid()
        {
            return None;
        }
        dependencies.extend(
            validation::completed_value_content_conservation_dependencies(
                program,
                machine,
                state,
                captured.statement_index,
                captured.expression,
                captured.type_reference,
                &source.plan,
                &facts.qualifications.content.plans,
            )?,
        );
    }
    Some(dependencies)
}
