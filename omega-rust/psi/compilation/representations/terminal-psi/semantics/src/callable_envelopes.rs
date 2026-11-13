//! A selection must satisfy its complete published operational contract.
//!
//! Named envelopes belong to the selected declaration, independently of any
//! descriptor. Compare parameters by their ordered signature positions.

use std::collections::BTreeSet;

use semantic_vocabulary::{ScalarType, ServiceId};
use terminal_psi::{
    BoundaryParameterKind, StructuralParameterDeclaration, TerminalCallableEnvelope,
    TerminalCallableInvocation, TerminalCallableTermination, TerminalModule, ValueDeclaration,
};

fn validate_envelope(
    module: &TerminalModule,
    services: &BTreeSet<ServiceId>,
    envelope: &TerminalCallableEnvelope,
    order: &[BoundaryParameterKind],
    scalars: &[ValueDeclaration],
    structures: &[StructuralParameterDeclaration],
) -> Result<(), &'static str> {
    if order
        .iter()
        .filter(|kind| **kind == BoundaryParameterKind::Scalar)
        .count()
        != scalars.len()
        || order
            .iter()
            .filter(|kind| **kind == BoundaryParameterKind::Structural)
            .count()
            != structures.len()
    {
        return Err("callable envelope has no complete ordered formal namespace");
    }
    if envelope
        .service_reach
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
        || envelope
            .service_reach
            .iter()
            .any(|service| !services.contains(service))
        || envelope
            .invocations
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err("callable envelope has a repeated, unordered or unknown authority");
    }
    for invocation in &envelope.invocations {
        match invocation {
            TerminalCallableInvocation::Service(service) if !services.contains(service) => {
                return Err("callable invocation ceiling names an unknown service");
            }
            TerminalCallableInvocation::Parameter(position) => {
                let position = usize::try_from(*position)
                    .map_err(|_| "callable invocation position does not fit its namespace")?;
                match order.get(position) {
                    Some(BoundaryParameterKind::Structural) => {}
                    Some(BoundaryParameterKind::Scalar) => {
                        let ordinal = order[..position]
                            .iter()
                            .filter(|kind| **kind == BoundaryParameterKind::Scalar)
                            .count();
                        if !matches!(
                            scalars.get(ordinal).map(|formal| formal.scalar_type),
                            Some(ScalarType::Callable(_))
                        ) {
                            return Err(
                                "callable invocation ceiling names a non-callable scalar formal",
                            );
                        }
                    }
                    None => return Err("callable invocation ceiling escapes its formal namespace"),
                }
            }
            TerminalCallableInvocation::Service(_) => {}
        }
    }
    if let TerminalCallableTermination::Terminates { premises } = &envelope.termination {
        for (ordinal, premise) in premises.iter().enumerate() {
            if premises[..ordinal].contains(premise) {
                return Err("callable termination ceiling repeats a premise");
            }
            let position = usize::try_from(premise.parameter)
                .map_err(|_| "callable progress position does not fit its namespace")?;
            if order.get(position) != Some(&BoundaryParameterKind::Structural) {
                return Err("callable progress premise names no structural formal");
            }
            let structural = order[..position]
                .iter()
                .filter(|kind| **kind == BoundaryParameterKind::Structural)
                .count();
            let formal = structures
                .get(structural)
                .ok_or("callable progress premise lost its exact formal")?;
            let domain = module
                .structural_domain(premise.domain)
                .ok_or("callable progress premise names an unknown domain")?;
            if crate::qualification_path_carrier(
                module.structural_types.iter(),
                formal.structural_type,
                &premise.path,
            ) != Some(domain.carrier)
            {
                return Err("callable progress premise has a different projected carrier");
            }
        }
    }
    Ok(())
}

pub fn validate_callable_operational_refinements(
    module: &TerminalModule,
) -> Result<(), &'static str> {
    let services = module.services.iter().map(|service| service.id).collect();
    for contract in &module.callables.contracts {
        validate_envelope(
            module,
            &services,
            &contract.envelope,
            &contract.parameter_order,
            &contract.scalar_parameters,
            &contract.structural_parameters,
        )?;
    }
    for row in module.callables.machine_envelopes.iter().flatten() {
        let machine = module
            .machine(row.machine)
            .filter(|machine| machine.declaration.is_some())
            .ok_or("named callable envelope has no exact checked declaration")?;
        validate_envelope(
            module,
            &services,
            &row.envelope,
            &machine.parameter_order,
            &machine.parameters,
            &machine.structural_parameters,
        )?;
        let declaration = machine
            .declaration
            .as_ref()
            .ok_or("named callable envelope lost its checked declaration")?;
        if row.envelope.service_reach != machine.published_service_ceiling
            || row.envelope.suspends != declaration.published_may_suspend
            || row.envelope.blocks != declaration.published_may_block
        {
            return Err("named callable operational envelope differs from its checked publication");
        }
    }
    for descriptor in &module.callables.descriptors {
        let required = module
            .callables
            .contract(descriptor.contract)
            .ok_or("selected machine has no callable operational contract")?;
        let machine = module
            .machine(descriptor.machine)
            .ok_or("selected callable machine is absent")?;
        let selected = module
            .callables
            .machine_envelope(descriptor.machine)
            .ok_or("selected callable machine has no published operational envelope")?;
        if required.parameter_order != machine.parameter_order {
            return Err("selected callable operational formals have another order");
        }
        callable_envelope_refines(&required.envelope, selected)?;
    }
    Ok(())
}

/// Validate a standalone published header before using its permissions.
pub fn validate_callable_envelope(
    module: &TerminalModule,
    envelope: &TerminalCallableEnvelope,
    order: &[BoundaryParameterKind],
    scalars: &[ValueDeclaration],
    structures: &[StructuralParameterDeclaration],
) -> Result<(), &'static str> {
    let services = module.services.iter().map(|service| service.id).collect();
    validate_envelope(module, &services, envelope, order, scalars, structures)
}

/// A supplied contract may use only permissions the required contract grants.
pub fn callable_envelope_refines(
    required: &TerminalCallableEnvelope,
    selected: &TerminalCallableEnvelope,
) -> Result<(), &'static str> {
    if selected
        .service_reach
        .iter()
        .any(|service| !required.service_reach.contains(service))
    {
        return Err("selected machine exceeds the callable service ceiling");
    }
    callable_operational_refines(
        CallableOperationalView::from(required),
        CallableOperationalView::from(selected),
    )
}

/// The non-service axes shared by closed callables and open static schemas.
#[derive(Clone, Copy)]
pub struct CallableOperationalView<'a> {
    pub invocations: &'a [TerminalCallableInvocation],
    pub suspends: bool,
    pub blocks: bool,
    pub termination: &'a TerminalCallableTermination,
}
impl<'a> From<&'a TerminalCallableEnvelope> for CallableOperationalView<'a> {
    fn from(envelope: &'a TerminalCallableEnvelope) -> Self {
        Self {
            invocations: &envelope.invocations,
            suspends: envelope.suspends,
            blocks: envelope.blocks,
            termination: &envelope.termination,
        }
    }
}
pub fn callable_operational_refines(
    required: CallableOperationalView<'_>,
    selected: CallableOperationalView<'_>,
) -> Result<(), &'static str> {
    if selected
        .invocations
        .iter()
        .any(|invocation| !required.invocations.contains(invocation))
    {
        return Err("selected machine exceeds the callable invocation ceiling");
    }
    if selected.suspends && !required.suspends {
        return Err("selected machine may suspend outside the callable contract");
    }
    if selected.blocks && !required.blocks {
        return Err("selected machine may block outside the callable contract");
    }
    if let TerminalCallableTermination::Terminates {
        premises: required_premises,
    } = required.termination
    {
        let TerminalCallableTermination::Terminates {
            premises: selected_premises,
        } = selected.termination
        else {
            return Err("selected machine has no published callable termination guarantee");
        };
        if selected_premises
            .iter()
            .any(|premise| !required_premises.contains(premise))
        {
            return Err("selected machine needs an unpublished callable progress premise");
        }
    }
    Ok(())
}
