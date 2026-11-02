//! The logical normal-return slot contract at one authenticated invocation.
//! Checked bodies retain their complete exit relation; opaque completions use
//! the same applied provider-output frontier as loan attribution. Neither is a
//! performed-call permission. Flow must separately join its exact completed
//! BorrowCallFact before publishing an output namespace.
use super::*;

pub enum ReferenceSlotContract<'program> {
    CheckedBody {
        machine: &'program Machine,
        state: &'program State,
    },
    Opaque {
        parameters: &'program [StateParameter],
        effects: Vec<StoredReferenceEffect>,
    },
}

pub(in super::super) fn invocation_contract<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &'program Machine,
    state: &'program State,
    statement_index: usize,
    call_ordinal: usize,
    target: SymbolHandle,
    expression: ExpressionHandle,
) -> Option<ReferenceSlotContract<'program>> {
    // Preserve the existing pure checked-body summary and its memo: it does
    // not need a fresh source-occurrence walk. Only an opaque applied contract
    // needs declaration-level call selection; actual authority is checked later.
    if let Some((callee, selected)) = super::super::machine_state_by_symbol(program, target) {
        if callee.body_is_present && callee.supply_mode.is_checked_body() {
            return Some(ReferenceSlotContract::CheckedBody {
                machine: callee,
                state: selected,
            });
        }
    }
    let site =
        crate::find_call_site_in_state(program, machine, state, statement_index, call_ordinal)?;
    if site.target_symbol(program) != target {
        return None;
    }
    let write_site = match site {
        crate::CallSite::Statement(call) if !expression.is_valid() => {
            super::super::caller_aliases::CallerWriteSite::Call(call)
        }
        crate::CallSite::Expression {
            expression: actual, ..
        } if actual == expression => {
            super::super::caller_aliases::CallerWriteSite::Expression(expression)
        }
        _ => return None,
    };
    let parameters = site.parameters(program, machine.symbol)?;
    let opaque = if let Some((callee, _)) = super::super::machine_state_by_symbol(program, target) {
        if matches!(
            callee.supply_mode,
            language_semantics::MachineSupplyMode::Boundary
                | language_semantics::MachineSupplyMode::AdmissionClaim
                | language_semantics::MachineSupplyMode::ExternalRealization { .. }
        ) {
            true
        } else {
            return None;
        }
    } else {
        crate::selected_requirement_signature(program, target)
            .is_some_and(|(definition, _)| definition.is_boundary)
    };
    if !opaque {
        return None;
    }
    let effects = super::opaque_outputs::formal_effects(
        program, machine, parameters, write_site, target, expression,
    )?;
    Some(ReferenceSlotContract::Opaque {
        parameters,
        effects,
    })
}
