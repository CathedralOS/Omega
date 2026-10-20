//! Admit selected cleanup only after ordinary crash and progress refinement.
//!
//! Declaration shape and the completed operational graph have already ruled
//! out suspension, blocking and external abandonment. Private body inference
//! does not itself promise infallibility or termination: those are additional
//! obligations of an automatic disposition, including nested dispositions.

use checked_trees::CheckFacts;
use diagnostics::Diagnostic;
use language_semantics::{MachineSupplyMode, TerminationGuarantee, TerminationInterface};

pub(crate) fn validate_selected_cleanup_guarantees(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut linear_targets = Vec::new();
    // Disposition, not the hook's name or mere existence, selects this rule.
    // The current whole-linear plan carries no independent service-authority
    // evidence, so neither a published ceiling nor inferred reach may add an
    // external dependency to its owner's terminal disposition.
    for (_, disposition) in facts.contextual_cleanup.dispositions.iter() {
        let linear = facts.flow.ownership.permissions.iter().any(|(_, event)| {
            event.machine_symbol == disposition.machine
                && event.state_symbol == disposition.state
                && event.source == disposition.source
                && event.root == disposition.root
                && facts.flow.ownership.segments.span(event.segments)
                    == facts.flow.ownership.segments.span(disposition.path)
                && event.kind == language_semantics::PermissionEventKind::Consume
                && event.multiplicity == language_semantics::Multiplicity::Linear
                && event.access == language_semantics::PermissionAccess::Owned
        });
        if !linear {
            continue;
        }
        let Some(leaves) = crate::flow::selected_cleanup_leaves(program, facts, disposition) else {
            diagnostics.push(Diagnostic::error(
                "linear cleanup has no complete owner-authorized disposition",
            ));
            continue;
        };
        for leaf in leaves {
            if let crate::flow::CleanupLeaf::Nominal { machine, .. } = leaf
                && !linear_targets.contains(&machine)
            {
                linear_targets.push(machine);
            }
        }
    }
    let mut checked_targets = Vec::new();
    for (_, dependency) in facts.operational.cleanup_dependencies.iter() {
        let target = dependency.target_machine;
        if checked_targets.contains(&target) {
            continue;
        }
        checked_targets.push(target);
        let machine = program.machine_by_symbol(target);
        let contract = facts.contract_plans.for_machine(target);
        let termination = facts.termination.for_machine(target);
        let (Some(machine), Some(contract), Some(termination)) = (machine, contract, termination)
        else {
            diagnostics.push(Diagnostic::error(
                "selected cleanup has no complete checked crash and termination contract",
            ));
            continue;
        };

        if linear_targets.contains(&target)
            && !facts
                .service_reaches
                .for_machine(target)
                .is_some_and(|reach| {
                    reach.unresolved_installation_reaches.is_empty()
                        && facts
                            .service_reaches
                            .rows
                            .services(reach.effective)
                            .is_empty()
                        && facts
                            .service_reaches
                            .rows
                            .services(reach.inferred_transitive)
                            .is_empty()
                })
        {
            diagnostics.push(Diagnostic::error(format!(
                "selected linear cleanup machine `{}` requires runtime authority beyond its owned value",
                machine.name,
            )));
        }

        // Exact ordinary invocation refinement may eliminate a helper's
        // published crash route. Its mere declaration is not a surviving
        // effect of this hook. Direct crashes and trapping primitives retain
        // their own implementation sites independently of that call evidence.
        let crash = &contract.crash;
        let calls_complete = facts
            .flow
            .control
            .states
            .iter()
            .filter(|(_, state)| state.machine_symbol == target)
            .all(|(_, state)| {
                facts
                    .flow
                    .control
                    .calls
                    .span_or_empty(state.calls)
                    .iter()
                    .all(|call| {
                        let Some(site) = validation::find_call_site(
                            program,
                            target,
                            state.state_symbol,
                            call.statement_index,
                            call.call_ordinal,
                        ) else {
                            return false;
                        };
                        if matches!(site, validation::CallSite::TransitionNamed { .. }) {
                            // State arrivals remain inside this activation; their
                            // bodies already participate in this machine's plan.
                            return true;
                        }
                        let Some((target_machine, target_state)) =
                            crate::proof::contract_target_from_state_symbol(
                                program,
                                call.target_symbol,
                            )
                        else {
                            return false;
                        };
                        let (Ok(statement), Ok(ordinal)) = (
                            u32::try_from(call.statement_index),
                            u32::try_from(call.call_ordinal),
                        ) else {
                            return false;
                        };
                        crash
                            .checked_call_at(state.state_symbol, statement, ordinal)
                            .is_some_and(|row| {
                                row.target_machine() == target_machine
                                    && row.target_state() == target_state
                            })
                    })
            });
        if !calls_complete {
            diagnostics.push(Diagnostic::error(format!(
                "selected cleanup machine `{}` has an unexamined call in its checked crash contract",
                machine.name,
            )));
        }
        if !crash.published().is_empty()
            || !crash.checked_sites().is_empty()
            || !crash.trapping_sites().is_empty()
            || crash
                .checked_calls()
                .iter()
                .any(|call| !call.surviving_buckets().is_empty())
            || crash
                .checked_operators()
                .iter()
                .any(|operator| !operator.surviving().is_empty())
        {
            diagnostics.push(Diagnostic::error(format!(
                "selected cleanup machine `{}` must be infallible; its checked body retains a crash outcome",
                machine.name,
            )));
        }

        let guarantee = if machine.supply_mode == MachineSupplyMode::CheckedBody {
            &termination.checked_summary
        } else {
            match &termination.interface {
                TerminationInterface::Published(guarantee) => guarantee,
                TerminationInterface::InternalDerived => &TerminationGuarantee::NoGuarantee,
            }
        };
        let guaranteed = match guarantee {
            TerminationGuarantee::NoGuarantee => false,
            TerminationGuarantee::Terminates { premises } => {
                super::termination::entry_covers_progress_premises(program, facts, target, premises)
            }
        };
        if !guaranteed {
            diagnostics.push(Diagnostic::error(format!(
                "selected cleanup machine `{}` has no termination guarantee established by its own entry requirements",
                machine.name,
            )));
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
