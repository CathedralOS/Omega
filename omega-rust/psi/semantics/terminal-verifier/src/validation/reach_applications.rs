//! Replay the finite relation between an original projected dependency, closed
//! selections, and actual emitted calls. The projection is an ordinary semantic
//! input. Its provenance digests alone do not prove source correspondence; a
//! coherently different projection defines a different semantic product.
//!
//! Operation-side markers and application-side consumers are checked in both
//! directions so deleting either half cannot silently turn a retained callback
//! into an unrelated ordinary call. Complete original-source coverage still
//! belongs to producer correspondence, not reconstruction from unmarked calls.

use super::*;
use terminal_psi::{ClosedReachMachineBinding, ClosedReachParameter};

pub(super) fn validate_closed_reach_applications(
    module: &TerminalModule,
) -> Result<(), ModuleError> {
    let services = module
        .services
        .iter()
        .map(|service| (service.id, service))
        .collect::<BTreeMap<_, _>>();
    let mut selected_contracts: Vec<&ClosedReachMachineBinding> = Vec::new();
    for owner in &module.machines {
        let invalid = |reason| ModuleError::InvalidClosedReachApplication {
            machine: owner.id,
            reason,
        };
        let operations = || owner.blocks.iter().flat_map(|block| &block.operations);
        let Some(application) = &owner.closed_reach_application else {
            if operations().any(|operation| operation.static_reach_binding.is_some()) {
                return Err(invalid("call marker has no closed reach application"));
            }
            continue;
        };
        if application.template_identity.is_empty()
            || application.template_commitment == [0; 32]
            || application.specialization_commitment == [0; 32]
            || application.telescope.is_empty()
        {
            return Err(invalid("closed reach origin or telescope is empty"));
        }
        let validate_row = |row: &[ServiceId]| {
            foundation::validate_service_ceiling(
                row,
                &services,
                ServiceCeilingOwner::Machine(owner.id),
            )
        };
        validate_row(&application.fixed)?;
        for parameter in &application.telescope {
            let binding = match parameter {
                ClosedReachParameter::Type { argument }
                | ClosedReachParameter::Const { argument }
                | ClosedReachParameter::Proposition { argument } => {
                    if argument.is_empty() {
                        return Err(invalid("closed telescope argument is empty"));
                    }
                    continue;
                }
                ClosedReachParameter::Machine(binding) => binding,
            };
            if binding.selected_identity.is_empty()
                || binding.selected_contract_commitment == [0; 32]
                || binding
                    .nominal_requirement
                    .as_ref()
                    .is_some_and(String::is_empty)
            {
                return Err(invalid(
                    "selected callable or nominal requirement identity is empty",
                ));
            }
            validate_row(&binding.upper_bound)?;
            validate_row(&binding.selected_reach)?;
            if binding
                .selected_reach
                .iter()
                .any(|service| !binding.upper_bound.contains(service))
            {
                return Err(invalid(
                    "selected contract exceeds its requirement reach bound",
                ));
            }
            if let Some(schema) = &binding.schema {
                // Selected identities already encode the exact normalized
                // machine owner followed by its selected state. Check the
                // owner without guessing an entry name or splitting overload
                // payloads on punctuation that they may themselves contain.
                let selected_state = binding
                    .selected_identity
                    .strip_prefix(schema.template_identity.as_str())
                    .and_then(|suffix| suffix.strip_prefix("|selected="));
                if schema.template_identity.is_empty()
                    || schema.template_commitment == [0; 32]
                    || binding.callee.is_some()
                    || selected_state.is_none_or(str::is_empty)
                {
                    return Err(invalid(
                        "selected schema has no exact template or claims one closed callee",
                    ));
                }
            }
            if let Some(callee) = binding.callee {
                let mut targets = module
                    .machines
                    .iter()
                    .filter(|machine| machine.id == callee);
                let target = targets
                    .next()
                    .ok_or_else(|| invalid("selected callable is absent"))?;
                if targets.next().is_some()
                    || target.published_service_ceiling != binding.selected_reach
                {
                    return Err(invalid(
                        "selected reach differs from its exact callable public contract",
                    ));
                }
            }
            for previous in &selected_contracts {
                if previous.selected_identity == binding.selected_identity {
                    if previous.selected_contract_commitment != binding.selected_contract_commitment
                        || previous.selected_reach != binding.selected_reach
                        || previous.schema != binding.schema
                        || matches!((previous.callee, binding.callee), (Some(left), Some(right)) if left != right)
                    {
                        return Err(invalid(
                            "selected identity has inconsistent contract or callable joins",
                        ));
                    }
                } else if binding.callee.is_some() && previous.callee == binding.callee {
                    return Err(invalid(
                        "one emitted callable has conflicting selected identities",
                    ));
                }
            }
            selected_contracts.push(binding);
        }
        if application
            .dependencies
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(invalid(
                "reach dependency positions are not strictly ordered",
            ));
        }
        let mut substituted = application.fixed.iter().copied().collect::<BTreeSet<_>>();
        for ordinal in &application.dependencies {
            let Some(ClosedReachParameter::Machine(binding)) =
                application.telescope.get(*ordinal as usize)
            else {
                return Err(invalid(
                    "reach dependency is not a machine telescope position",
                ));
            };
            let has_callable = if binding.schema.is_some() {
                has_schema_application_in_call_closure(module, owner.id, binding)
            } else {
                binding.callee.is_some()
            };
            if binding.nominal_requirement.is_none() || !has_callable {
                return Err(invalid(
                    "reach dependency has no nominal requirement or emitted callable",
                ));
            }
            substituted.extend(binding.selected_reach.iter().copied());
        }
        // All input rows are already parent-closed; their union therefore is
        // the ordinary normalized finite service row, including inert promises.
        if substituted.into_iter().collect::<Vec<_>>() != owner.published_service_ceiling {
            return Err(invalid(
                "closed reach substitution differs from the owner public row",
            ));
        }
        if application
            .calls
            .windows(2)
            .any(|pair| pair[0].operation >= pair[1].operation)
        {
            return Err(invalid("closed reach call roster is not strictly ordered"));
        }
        for call in &application.calls {
            let Some(ClosedReachParameter::Machine(binding)) =
                application.telescope.get(call.binder as usize)
            else {
                return Err(invalid("callback consumer has no machine binder"));
            };
            if binding.nominal_requirement.is_some() {
                if !application.dependencies.contains(&call.binder) {
                    return Err(invalid(
                        "nominal callback consumer is missing its reach dependency",
                    ));
                }
            } else if binding
                .upper_bound
                .iter()
                .any(|service| !application.fixed.contains(service))
            {
                return Err(invalid(
                    "structural callback consumer is missing its fixed requirement row",
                ));
            }
            let mut matching = operations().filter(|operation| operation.id == call.operation);
            let operation = matching
                .next()
                .ok_or_else(|| invalid("callback consumer operation is absent from its owner"))?;
            if matching.next().is_some() || operation.static_reach_binding != Some(call.binder) {
                return Err(invalid(
                    "callback consumer differs from its operation binder marker",
                ));
            }
            let callee = match operation.kind {
                OperationKind::Call { callee, .. }
                | OperationKind::CallUnit { callee, .. }
                | OperationKind::CallStructuralScalar { callee, .. }
                | OperationKind::CallStructural { callee, .. }
                | OperationKind::CallStructuralWithScalarArguments { callee, .. } => callee,
                _ => {
                    return Err(invalid(
                        "callback consumer is not an ordinary concrete call",
                    ));
                }
            };
            match (&binding.schema, &call.application) {
                (None, None) => {
                    if binding.callee != Some(callee) {
                        return Err(invalid(
                            "callback consumer targets a different selected callable",
                        ));
                    }
                }
                (Some(schema), Some(selected)) => {
                    if selected.callee != callee {
                        return Err(invalid(
                            "schema consumer targets a different closed application",
                        ));
                    }
                    let mut targets = module
                        .machines
                        .iter()
                        .filter(|machine| machine.id == callee);
                    let target = targets
                        .next()
                        .ok_or_else(|| invalid("schema consumer's closed callable is absent"))?;
                    if targets.next().is_some() {
                        return Err(invalid("schema consumer's closed callable is ambiguous"));
                    }
                    let target_application =
                        target.closed_reach_application.as_ref().ok_or_else(|| {
                            invalid("schema consumer's callable has no retained closed application")
                        })?;
                    if target_application.template_identity != schema.template_identity
                        || target_application.template_commitment != schema.template_commitment
                        || target_application.specialization_commitment
                            != selected.specialization_commitment
                    {
                        return Err(invalid(
                            "schema consumer's closed application has a different template or selection",
                        ));
                    }
                    // The commitment identifies a selected application but is
                    // not an opening of its telescope. Compare the complete
                    // expected argument selections with the retained callee
                    // parameters so a tuple change cannot pass on a stale hash.
                    if selected.arguments.len() != target_application.telescope.len()
                        || selected
                            .arguments
                            .iter()
                            .zip(&target_application.telescope)
                            .any(|(argument, parameter)| !argument.matches_parameter(parameter))
                    {
                        return Err(invalid(
                            "schema consumer's argument tuple differs from its closed callable",
                        ));
                    }
                    // A schema retains its universal public contract. Each
                    // application may have a smaller row; no one observed
                    // specialization replaces the schema's selected promise.
                    if target
                        .published_service_ceiling
                        .iter()
                        .any(|service| !binding.selected_reach.contains(service))
                    {
                        return Err(invalid(
                            "schema consumer's specialized reach exceeds its selected contract",
                        ));
                    }
                }
                (Some(_), None) => {
                    return Err(invalid(
                        "schema consumer has no closed application selection",
                    ));
                }
                (None, Some(_)) => {
                    return Err(invalid(
                        "ordinary callback consumer carries a schema application",
                    ));
                }
            }
        }
        for operation in operations().filter(|operation| operation.static_reach_binding.is_some()) {
            if !application.calls.iter().any(|call| {
                call.operation == operation.id
                    && Some(call.binder) == operation.static_reach_binding
            }) {
                return Err(invalid(
                    "callback operation marker has no exact application consumer",
                ));
            }
        }
    }
    Ok(())
}

/// A nominal dependency may be invoked by a helper that receives the selected
/// schema. Follow actual ordinary calls, retaining exact selected identity
/// (including the source state), rather than requiring a direct call in the
/// dependency owner or accepting an unrelated application elsewhere in a module.
/// This is a presence-only query, also used while the producer prunes incomplete
/// projections. It is not standalone validation: the main validation pass must
/// check every visited owner's application, operation, and argument joins.
pub fn has_schema_application_in_call_closure(
    module: &TerminalModule,
    owner: MachineId,
    selected: &ClosedReachMachineBinding,
) -> bool {
    let mut reachable = vec![owner];
    let mut next_machine_position = 0;
    while let Some(machine_id) = reachable.get(next_machine_position).copied() {
        next_machine_position += 1;
        let Some(machine) = module
            .machines
            .iter()
            .find(|machine| machine.id == machine_id)
        else {
            continue;
        };
        if let Some(application) = &machine.closed_reach_application {
            for call in &application.calls {
                let Some(ClosedReachParameter::Machine(binding)) =
                    application.telescope.get(call.binder as usize)
                else {
                    continue;
                };
                if call.application.is_some()
                    && binding.selected_identity == selected.selected_identity
                    && binding.selected_contract_commitment == selected.selected_contract_commitment
                    && binding.selected_reach == selected.selected_reach
                    && binding.schema == selected.schema
                {
                    return true;
                }
            }
        }
        for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
            let callee = match operation.kind {
                OperationKind::Call { callee, .. }
                | OperationKind::CallUnit { callee, .. }
                | OperationKind::CallStructuralScalar { callee, .. }
                | OperationKind::CallStructural { callee, .. }
                | OperationKind::CallStructuralWithScalarArguments { callee, .. } => callee,
                _ => continue,
            };
            if !reachable.contains(&callee) {
                reachable.push(callee);
            }
        }
    }
    false
}
