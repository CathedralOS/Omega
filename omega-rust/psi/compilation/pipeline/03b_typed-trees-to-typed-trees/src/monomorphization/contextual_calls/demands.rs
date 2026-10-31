//! Prepare every portable member's demand against one determined provider tuple.
//!
//! Member telescopes correspond by position, not binder spelling. Determine
//! the provider once at the occurrence, then apply each member's own contract
//! without closing another provider or choosing another executable instance.
//! Immutable checking consumes these views and independently admits all demands.

use super::super::{Diagnostic, SymbolHandle, TypeReferenceHandle, TypedTrees};
use super::invocations::{MachineDemand, machine_demands};
use crate::tables::expressions_of;
use typed_trees::name::Identifier;
use typed_trees::static_applications::{
    StaticMachineApplicationHandle, StaticMachineLifetimeBinding,
};

pub(super) struct MemberDemands {
    requirements: Vec<MachineDemand>,
    operands: Vec<(SymbolHandle, TypeReferenceHandle)>,
    lifetimes: Vec<(Identifier, Identifier)>,
}

pub(super) fn members(
    program: &TypedTrees,
    target: SymbolHandle,
    operands: &[(SymbolHandle, TypeReferenceHandle)],
) -> Result<Vec<MemberDemands>, Vec<Diagnostic>> {
    let Some(head) = program
        .machines()
        .iter()
        .find(|machine| program.machine_or_state_symbol(machine, target))
    else {
        return Ok(Vec::new());
    };
    let Some(family) = program
        .target_machine_families
        .iter()
        .find(|family| family.canonical == head.symbol)
    else {
        return Ok(Vec::new());
    };
    let head_parameters = program.machine_type_parameters(head);
    let mut demands = Vec::with_capacity(family.members.len());
    for member in &family.members {
        let machine = program
            .machine_by_symbol(member.machine)
            .ok_or_else(|| failure("target-family demand lost its retained declaration"))?;
        let parameters = program.machine_type_parameters(machine);
        if machine.lifetime_parameters.len() != head.lifetime_parameters.len() {
            return Err(failure(
                "target-family demand lost its lifetime binder correspondence",
            ));
        }
        let mut member_operands = Vec::with_capacity(operands.len());
        for (parameter, actual) in operands {
            let corresponding = head_parameters
                .iter()
                .position(|binding| binding.symbol == *parameter)
                .and_then(|position| parameters.get(position))
                .ok_or_else(|| {
                    failure("target-family demand lost its generic binder correspondence")
                })?;
            member_operands.push((corresponding.symbol, *actual));
        }
        demands.push(MemberDemands {
            requirements: machine_demands(program, parameters),
            operands: member_operands,
            lifetimes: head
                .lifetime_parameters
                .iter()
                .cloned()
                .zip(machine.lifetime_parameters.iter().cloned())
                .collect(),
        });
    }
    Ok(demands)
}

pub(super) fn retain(
    program: &mut TypedTrees,
    invocation: StaticMachineApplicationHandle,
    ordinal: usize,
    selected: SymbolHandle,
    head: &MachineDemand,
    members: &[MemberDemands],
) -> Result<(), Vec<Diagnostic>> {
    if members.is_empty() {
        return Ok(());
    }
    let provider = program
        .applied_machine_callable(
            invocation,
            ordinal,
            selected,
            head.parameter,
            &head.signature,
        )
        .map(|view| view.provider);
    let Some(provider) = provider else {
        if program.has_applied_machine_selection(invocation, ordinal, selected) {
            return Err(failure(
                "portable selection lost its determined provider tuple",
            ));
        }
        // Universal requirements and forwarded binders use ordinary callable
        // refinement; they have no determined provider tuple to apply here.
        return Ok(());
    };
    let caller_lifetimes = expressions_of(program)
        .static_machine_application_lifetimes(invocation)
        .to_vec();
    let provider_bindings = expressions_of(program)
        .static_machine_application_bindings(provider)
        .to_vec();
    let provider_lifetimes = expressions_of(program)
        .static_machine_application_lifetimes(provider)
        .to_vec();
    for member in members {
        let demand = member.requirements.get(ordinal).ok_or_else(|| {
            failure("target-family demand lost its machine binder correspondence")
        })?;
        // Only captured outer binders change names. Arguments retain the
        // invocation's caller scope; requirement-local fresh binders remain
        // protected by the ordinary deep callable application.
        let mut lifetimes = Vec::with_capacity(caller_lifetimes.len());
        for binding in &caller_lifetimes {
            let (_, parameter) = member
                .lifetimes
                .iter()
                .find(|(head, _)| head == &binding.parameter)
                .ok_or_else(|| failure("target-family demand lost its applied lifetime binder"))?;
            lifetimes.push(StaticMachineLifetimeBinding {
                parameter: parameter.clone(),
                argument: binding.argument.clone(),
            });
        }
        if program.applied_machine_callable_matches(
            invocation,
            ordinal,
            selected,
            demand.parameter,
            &provider_bindings,
            &provider_lifetimes,
            &demand.signature,
            &member.operands,
            &lifetimes,
        ) {
            continue;
        }
        program
            .prepare_applied_machine_callable(
                invocation,
                ordinal,
                provider,
                selected,
                demand.parameter,
                &demand.signature,
                &member.operands,
                &lifetimes,
            )
            .map_err(failure)?;
    }
    Ok(())
}

fn failure(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(message)]
}
