//! Check callback substitutions against their full refinement questions, then
//! reconstruct reach from the actual lowered calls. A template declaration
//! records that function of its callbacks; it never supplies executable code.

use semantic_vocabulary::{InvocationSite, MachineId, ServiceId};
use terminal_check_tables::contracts::{Ceiling, StaticImplementation};
use terminal_check_tables::program::Elaboration;
use terminal_check_tables::{Checked, Construct, RefusalReason};

fn fault(machine: MachineId, detail: &str) -> terminal_check_tables::Refusal {
    crate::checking::refusal(
        RefusalReason::ForeignIdentity,
        machine,
        Construct::Machine,
        detail,
    )
}

pub(crate) fn validate(elaboration: &Elaboration) -> Checked<()> {
    let first = MachineId::new(1).expect("nonzero diagnostic coordinate");
    if elaboration
        .generic_callable_declarations
        .windows(2)
        .any(|pair| pair[0].machine >= pair[1].machine)
    {
        return Err(fault(
            first,
            "generic callable declarations are not strictly ordered",
        ));
    }
    let mut requirements = elaboration.requirements.clone();
    requirements.sort_unstable();
    if !elaboration.generic_binders.is_empty() {
        let mut undeclared = elaboration
            .undeclared
            .iter()
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        undeclared.sort_unstable();
        for machine in &elaboration.module.machines {
            if machine.declaration.is_none()
                && requirements.binary_search(&machine.id).is_err()
                && undeclared.binary_search(&machine.id).is_err()
                && elaboration
                    .generic_callable_declarations
                    .binary_search_by_key(&machine.id, |row| row.machine)
                    .is_err()
            {
                return Err(fault(
                    machine.id,
                    "generic body lost its complete source contract",
                ));
            }
        }
    }
    if elaboration.static_reach_parameters.is_empty()
        && elaboration.static_reach_applications.is_empty()
        && elaboration.static_reach_summaries.is_empty()
        && elaboration.generic_callable_declarations.is_empty()
    {
        return Ok(());
    }
    let declarations = crate::checking::static_contracts::requirement_declarations(elaboration)?;
    let requirement = |id| {
        declarations
            .get(id)
            .and_then(|row| *row)
            .and_then(|row| row.instance.as_ref())
    };
    for parameter in &elaboration.static_reach_parameters {
        if requirement(parameter.requirement).is_none() {
            return Err(fault(
                parameter.owner,
                "static reach parameter has no complete originating header",
            ));
        }
    }
    for application in &elaboration.static_reach_applications {
        let mut pending = application.arguments.iter().collect::<Vec<_>>();
        while let Some(argument) = pending.pop() {
            let source = declarations
                .get(argument.requirement)
                .and_then(|row| *row)
                .ok_or_else(|| {
                    fault(
                        application.caller,
                        "static callback has no originating declaration",
                    )
                })?;
            let instance = source.instance.as_ref().ok_or_else(|| {
                fault(
                    application.caller,
                    "static callback lost its applied header",
                )
            })?;
            let question = elaboration
                .static_conformances
                .get(argument.conformance as usize)
                .ok_or_else(|| {
                    fault(
                        application.caller,
                        "static callback lost its refinement question",
                    )
                })?;
            let mut required = question.required.clone();
            required.proof.id = instance.header.proof.id;
            if required != instance.header
                || question.envelope != source.envelope
                || question.declaration != source.declaration
                || question.requirement != source.declaration
                || !matches!(question.implementation, StaticImplementation::Machine(selected) if selected == argument.selected)
            {
                return Err(fault(
                    application.caller,
                    "static callback substitution differs from its complete refinement question",
                ));
            }
            pending.extend(&argument.arguments);
        }
    }
    if requirements
        .iter()
        .any(|id| elaboration.requirement_ceilings.get(*id).is_none())
    {
        return Err(fault(
            first,
            "static reach lost its abstract requirement bounds",
        ));
    }
    let derived = terminal_semantics::static_reach::derive(
        &elaboration.module,
        &elaboration.static_reach_parameters,
        &elaboration.static_reach_applications,
        |id| {
            requirements.binary_search(&id).ok().and_then(|_| {
                elaboration
                    .requirement_ceilings
                    .get(id)
                    .map(|row| match &row.reaches {
                        Ceiling::Inferred => None,
                        Ceiling::Authored(reach) => Some(reach.as_slice()),
                    })
            })
        },
    )
    .map_err(|reason| fault(first, reason))?;
    if derived != elaboration.static_reach_summaries {
        return Err(fault(
            first,
            "generic reach summary differs from its lowered calls",
        ));
    }
    for declaration in &elaboration.generic_callable_declarations {
        let id = declaration.machine;
        let position = elaboration
            .module
            .machines
            .binary_search_by_key(&id, |row| row.id)
            .map_err(|_| fault(id, "generic declaration has no exact body"))?;
        let machine = &elaboration.module.machines[position];
        if machine.blocks.is_empty()
            || machine.declaration.is_some()
            || elaboration.generic_binders.is_empty()
            || requirements.binary_search(&id).is_ok()
            || declaration.declaration.specialization.is_some()
            || elaboration.module.callables.machine_envelope(id).is_some()
        {
            return Err(fault(
                id,
                "generic checking contract overlaps executable or abstract declaration authority",
            ));
        }
        if declaration.declaration.published_may_suspend != machine.published_may_suspend
            || declaration.declaration.published_may_block != machine.published_may_block
        {
            return Err(fault(
                id,
                "generic declaration changes its operational bounds",
            ));
        }
        let summary = derived
            .binary_search_by_key(&id, |row| row.machine)
            .ok()
            .map(|position| &derived[position].term)
            .ok_or_else(|| fault(id, "generic declaration has no derived reach"))?;
        if summary != &declaration.reach
            || !declaration
                .projections
                .iter()
                .map(|row| row.requirement)
                .eq(summary.parameters.iter().copied())
        {
            return Err(fault(
                id,
                "generic declaration drops or changes a callback dependency",
            ));
        }
        for projection in &declaration.projections {
            let parameter = terminal_psi::static_reach::StaticReachParameter {
                owner: id,
                parameter: projection.parameter,
                requirement: projection.requirement,
            };
            let origin = requirement(projection.requirement).ok_or_else(|| {
                fault(
                    id,
                    "generic dependency lost its full originating requirement",
                )
            })?;
            if elaboration
                .static_reach_parameters
                .binary_search(&parameter)
                .is_err()
                || projection.member != origin.member
            {
                return Err(fault(
                    id,
                    "generic dependency changes its parameter or member identity",
                ));
            }
        }
        let projections = declaration
            .projections
            .iter()
            .map(|projection| {
                elaboration
                    .module
                    .identities
                    .text(projection.member)
                    .filter(|identity| !identity.is_empty())
                    .map(|identity| (projection.parameter, identity))
                    .ok_or_else(|| fault(id, "generic dependency has no portable member identity"))
            })
            .collect::<Checked<Vec<_>>>()?;
        let checking = &declaration.envelope;
        crate::checking::static_contracts::check_envelope(&elaboration.module, id, checking, None)?;
        if !checking
            .invocations
            .iter()
            .filter_map(|entry| match entry {
                terminal_psi::TerminalCallableInvocation::Service(service) => Some(*service),
                terminal_psi::TerminalCallableInvocation::Parameter(_) => None,
            })
            .eq(machine.published_synchronous_entries.iter().copied())
        {
            return Err(fault(
                id,
                "generic checking header changes its published invocation row",
            ));
        }
        if checking.suspends != machine.published_may_suspend
            || checking.blocks != machine.published_may_block
        {
            return Err(fault(
                id,
                "generic checking header changes its operational bounds",
            ));
        }
        let commitment = terminal_semantics::static_reach::template_commitment(
            declaration.base_contract,
            &summary.concrete,
            projections,
            &checking.invocations,
            |service| {
                elaboration
                    .module
                    .services
                    .binary_search_by_key(&service, |row| row.id)
                    .ok()
                    .and_then(|position| {
                        elaboration
                            .module
                            .identities
                            .text(elaboration.module.services[position].identity)
                    })
            },
        )
        .map_err(|reason| fault(id, reason))?;
        if commitment != declaration.declaration.contract {
            return Err(fault(
                id,
                "generic callable contract does not commit its complete reach dependency",
            ));
        }
    }
    Ok(())
}

pub(crate) struct ReachContribution {
    pub services: Vec<ServiceId>,
    pub open: bool,
}

/// Apply the selected published contracts before comparing a call with its
/// caller's ceiling. Unsubstituted callbacks still use their declared bounds.
pub(crate) fn call_reach(
    elaboration: &Elaboration,
    caller: MachineId,
    site: InvocationSite,
    inputs: crate::checking::call_contracts::Inputs<'_>,
) -> Checked<Option<ReachContribution>> {
    let terminal_check_tables::edges::Callee::Declared(
        semantic_vocabulary::TargetMachineCallable::Machine(callee),
    ) = inputs.callee
    else {
        return Ok(None);
    };
    if inputs.required_contract.is_some() {
        return Ok(None);
    }
    let unsubstituted = terminal_psi::static_reach::StaticReachApplication {
        caller,
        site,
        callee,
        arguments: Vec::new(),
    };
    let application = match elaboration
        .static_reach_applications
        .binary_search_by_key(&(caller, site), |row| (row.caller, row.site))
    {
        Ok(position) => &elaboration.static_reach_applications[position],
        Err(_)
            if elaboration
                .static_reach_summaries
                .binary_search_by_key(&callee, |row| row.machine)
                .is_ok() =>
        {
            &unsubstituted
        }
        Err(_) => return Ok(None),
    };
    if application.callee != callee {
        return Err(fault(
            caller,
            "static substitution changed its exact called machine",
        ));
    }
    let term = terminal_semantics::static_reach::applied_reach(
        &elaboration.static_reach_summaries,
        application,
    )
    .map_err(|reason| fault(caller, reason))?;
    let mut services = term.concrete;
    let mut open = false;
    for requirement in term.parameters {
        let bounds = elaboration
            .requirement_ceilings
            .get(requirement)
            .ok_or_else(|| fault(caller, "callback reach lost its originating bound"))?;
        match &bounds.reaches {
            Ceiling::Inferred => open = true,
            Ceiling::Authored(reach) => services.extend_from_slice(reach),
        }
    }
    services.sort_unstable();
    services.dedup();
    Ok(Some(ReachContribution { services, open }))
}
