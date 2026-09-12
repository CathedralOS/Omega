//! Preserve closed substitution before the source companions are discarded.
//!
//! The original generic graph supplies the finite dependency; the selected
//! checked contracts supply its arguments. Neither the selected emitted bodies
//! nor subtraction of requirement bounds can recover the original fixed row.
//! This projection is ordinary producer-trusted semantics, not a commitment
//! opening or a portable proof of source inference. Exact source owners and
//! call occurrences nevertheless let the consumer check every retained join.

use checked_trees::CheckedTrees;
use checked_trees::data::{
    MachineParameterContract, MachineParameterContractView, TypeParameterKind,
};
use language_semantics::ServiceReachId;
use lowered_psi::LoweredSourceCallOccurrence;
use semantic_vocabulary::{MachineId, ServiceId};
use symbols::SymbolHandle;
use terminal_psi::{
    ClosedReachApplication, ClosedReachCall, ClosedReachMachineBinding, ClosedReachParameter,
    OperationKind, ServiceDeclaration, TerminalModule,
};

use crate::{LoweringError, unsupported};

pub(crate) fn retain_closed_reach_applications(
    checked: &CheckedTrees,
    source_machines: &[(SymbolHandle, MachineId)],
    occurrences: &[LoweredSourceCallOccurrence],
    module: &mut TerminalModule,
) -> Result<(), LoweringError> {
    if !checked.machine_specializations.iter().any(|application| {
        source_machines
            .iter()
            .any(|(source, _)| *source == application.instance)
    }) {
        return Ok(());
    }
    // One prepared graph serves the complete selected batch. The commitment
    // replay immediately preceding this phase checks retained template custody.
    let operational = validation::infer_operational_may(&checked.typed);
    let inferred = validation::infer_service_reaches(&checked.typed, &operational);
    for specialization in &checked.machine_specializations {
        let Some(owner) = exact_machine(source_machines, specialization.instance)? else {
            continue;
        };
        let parameters = checked
            .data_type_parameters
            .span_or_empty(specialization.template_parameters);
        if parameters.len() != specialization.template_parameters.len()
            || u32::try_from(parameters.len()).is_err()
        {
            return unsupported("closed reach application lost its complete portable telescope");
        }
        if !parameters
            .iter()
            .any(|parameter| matches!(parameter.kind, TypeParameterKind::Machine { .. }))
        {
            continue;
        }
        // Executable specialization currently retains no proposition arguments
        // or declaration-identity contract. Keep those existing routes outside
        // this closed application projection rather than fabricate positions.
        if parameters.iter().any(|parameter| {
            matches!(
                parameter.kind,
                TypeParameterKind::Proposition { .. }
                    | TypeParameterKind::Machine {
                        contract: MachineParameterContract::RequirementIdentity
                    }
            )
        }) {
            continue;
        }
        let template =
            inferred
                .for_machine(specialization.template)
                .ok_or(LoweringError::Unsupported(
                    "closed reach application lost the original template dependency",
                ))?;
        let selected_owner = checked
            .facts
            .service_reaches
            .for_machine(specialization.instance)
            .ok_or(LoweringError::Unsupported(
                "closed reach application lost its checked owner row",
            ))?;
        if !selected_owner.unresolved_installation_reaches.is_empty() {
            continue;
        }
        let mut dependencies = Vec::new();
        for dependency in inferred
            .dependency_parameters
            .span_or_empty(template.dependency.parameters)
        {
            let position = parameters
                .iter()
                .position(|parameter| parameter.symbol == *dependency)
                .ok_or(LoweringError::Unsupported(
                    "closed reach dependency is outside its full telescope",
                ))?;
            dependencies.push(u32::try_from(position).map_err(|_| {
                LoweringError::Unsupported("closed reach telescope exceeds portable positions")
            })?);
        }
        dependencies.sort_unstable();
        let original_service_count = module.services.len();
        let mut telescope = Vec::with_capacity(parameters.len());
        let mut types = specialization.type_argument_identities.iter();
        let mut constants = specialization.const_argument_identities.iter();
        let mut machines = specialization
            .machine_arguments
            .iter()
            .zip(&specialization.machine_argument_contract_commitments);
        let mut covered = true;
        for (position, parameter) in parameters.iter().enumerate() {
            telescope.push(match &parameter.kind {
                TypeParameterKind::Type => ClosedReachParameter::Type {
                    argument: types
                        .next()
                        .ok_or(LoweringError::Unsupported(
                            "closed reach telescope lost a type argument",
                        ))?
                        .clone(),
                },
                TypeParameterKind::Const { .. } => ClosedReachParameter::Const {
                    argument: constants
                        .next()
                        .ok_or(LoweringError::Unsupported(
                            "closed reach telescope lost a const argument",
                        ))?
                        .clone(),
                },
                TypeParameterKind::Machine { contract } => {
                    let (selected, commitment) =
                        machines.next().ok_or(LoweringError::Unsupported(
                            "closed reach telescope lost a selected machine contract",
                        ))?;
                    let (selected_machine, selected_state) = checked
                        .machines()
                        .iter()
                        .find_map(|machine| {
                            checked
                                .machine_states(machine)
                                .iter()
                                .find(|state| state.symbol == *selected)
                                .map(|state| (machine, state))
                        })
                        .ok_or(LoweringError::Unsupported(
                            "closed reach selection has no exact callable owner",
                        ))?;
                    let reach = checked
                        .facts
                        .service_reaches
                        .for_machine(selected_machine.symbol)
                        .ok_or(LoweringError::Unsupported(
                            "closed reach selection has no checked contract row",
                        ))?;
                    let callee = exact_machine(source_machines, selected_machine.symbol)?;
                    if !reach.unresolved_installation_reaches.is_empty()
                        || (dependencies.contains(&(position as u32)) && callee.is_none())
                        || !checked.machine_type_parameters(selected_machine).is_empty()
                    {
                        covered = false;
                        break;
                    }
                    let view = checked.machine_parameter_contract_view(contract).ok_or(
                        LoweringError::Unsupported(
                            "closed reach binder has no retained callable contract",
                        ),
                    )?;
                    let nominal_requirement = match view {
                        MachineParameterContractView::Structural(_) => None,
                        MachineParameterContractView::Nominal {
                            trait_definition,
                            requirement,
                        } => Some(format!(
                            "{}|{}",
                            declaration_identity(checked, trait_definition.symbol),
                            checked
                                .normalized_trait_requirement_overload_identity(
                                    trait_definition,
                                    requirement
                                )
                                .identity(),
                        )),
                    };
                    let overload = checked
                        .normalized_machine_overload_identity(selected_machine)
                        .ok_or(LoweringError::Unsupported(
                            "closed reach selected machine has no normalized identity",
                        ))?
                        .identity();
                    let upper_bound = retain_services(
                        checked,
                        checked
                            .service_reach_rows
                            .services(view.signature().service_reach_row),
                        module,
                    )?;
                    let selected_reach = retain_services(
                        checked,
                        checked.facts.service_reaches.rows.services(reach.effective),
                        module,
                    )?;
                    ClosedReachParameter::Machine(ClosedReachMachineBinding {
                        nominal_requirement,
                        upper_bound,
                        selected_identity: format!(
                            "{}|{}|selected={}",
                            declaration_identity(checked, selected_machine.symbol),
                            overload,
                            declaration_identity(checked, selected_state.symbol)
                        ),
                        selected_contract_commitment: *commitment,
                        selected_reach,
                        callee,
                    })
                }
                TypeParameterKind::Proposition { .. } => {
                    return unsupported(
                        "closed reach executable proposition argument is unavailable",
                    );
                }
            });
        }
        if !covered {
            module.services.truncate(original_service_count);
            continue;
        }
        if types.next().is_some() || constants.next().is_some() || machines.next().is_some() {
            return unsupported("closed reach arguments differ from the complete telescope");
        }
        let mut calls = Vec::new();
        let caller = operational
            .machines()
            .iter()
            .find(|machine| machine.symbol == specialization.instance)
            .ok_or(LoweringError::Unsupported(
                "closed reach application has no operational owner",
            ))?;
        for state in operational.states.span_or_empty(caller.states) {
            for call in operational.calls.span_or_empty(state.calls) {
                if !call.static_machine_parameter.is_valid() {
                    continue;
                }
                let binder = parameters
                    .iter()
                    .position(|parameter| parameter.symbol == call.static_machine_parameter)
                    .ok_or(LoweringError::Unsupported(
                        "closed reach call lost its original binder",
                    ))?;
                let mut matches = occurrences.iter().filter(|occurrence| {
                    occurrence.source_state == state.symbol
                        && occurrence.statement_index == call.statement_index
                        && occurrence.call_ordinal == call.call_ordinal
                        && occurrence.source_target == call.target_state_symbol
                });
                let Some(occurrence) = matches.next() else {
                    // Inlining routes have no retained direct operation. Their
                    // ordinary checking remains, but this projection is absent.
                    covered = false;
                    break;
                };
                if matches.next().is_some() {
                    return unsupported("closed reach call has ambiguous emitted occurrences");
                }
                calls.push(ClosedReachCall {
                    operation: occurrence.terminal_operation,
                    binder: binder as u32,
                });
            }
        }
        if !covered {
            module.services.truncate(original_service_count);
            continue;
        }
        calls.sort_by_key(|call| call.operation);
        let fixed = retain_services(
            checked,
            inferred.services(template.dependency.concrete),
            module,
        )?;
        let terminal_owner = module
            .machines
            .iter_mut()
            .find(|machine| machine.id == owner)
            .ok_or(LoweringError::Unsupported(
                "closed reach source owner has no Terminal machine",
            ))?;
        let mut row = fixed.clone();
        for dependency in &dependencies {
            let Some(ClosedReachParameter::Machine(binding)) = telescope.get(*dependency as usize)
            else {
                return unsupported("closed reach dependency is not a machine binder");
            };
            if binding.nominal_requirement.is_none() {
                return unsupported("closed reach dependency refers to a structural binder");
            }
            row.extend(&binding.selected_reach);
        }
        row.sort();
        row.dedup();
        if row != terminal_owner.published_service_ceiling {
            return unsupported(
                "closed reach substitution differs from its owner's published ceiling",
            );
        }
        for call in &calls {
            let Some(ClosedReachParameter::Machine(binding)) = telescope.get(call.binder as usize)
            else {
                return unsupported("closed reach consumer does not select a machine binder");
            };
            let operation = terminal_owner
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.operations)
                .find(|operation| operation.id == call.operation)
                .ok_or(LoweringError::Unsupported(
                    "closed reach consumer has no exact owner operation",
                ))?;
            let callee = match operation.kind {
                OperationKind::Call { callee, .. }
                | OperationKind::CallUnit { callee, .. }
                | OperationKind::CallStructuralScalar { callee, .. }
                | OperationKind::CallStructural { callee, .. }
                | OperationKind::CallStructuralWithScalarArguments { callee, .. } => callee,
                _ => return unsupported("closed reach consumer is not a direct machine call"),
            };
            if binding.callee != Some(callee) {
                return unsupported("closed reach consumer differs from its exact selected callee");
            }
            operation.static_reach_binding = Some(call.binder);
        }
        terminal_owner.closed_reach_application = Some(ClosedReachApplication {
            template_identity: specialization.normalized_template_identity.clone(),
            template_commitment: specialization.template_contract_commitment.as_bytes(),
            specialization_commitment: specialization.commitment.as_bytes(),
            telescope,
            fixed,
            dependencies,
            calls,
        });
    }
    Ok(())
}

fn exact_machine(
    sources: &[(SymbolHandle, MachineId)],
    source: SymbolHandle,
) -> Result<Option<MachineId>, LoweringError> {
    let mut matches = sources.iter().filter(|(candidate, _)| *candidate == source);
    let result = matches.next().map(|(_, machine)| *machine);
    if matches.next().is_some() {
        return unsupported("closed reach selection has ambiguous source owners");
    }
    Ok(result)
}

// Same package-qualified declaration vocabulary as specialization commitment
// replay. Unmanaged fixtures remain explicit; no arena address or fresh digest
// stands in for a declaration's identity.
fn declaration_identity(checked: &CheckedTrees, symbol: SymbolHandle) -> String {
    let path = checked.symbols.display_path(symbol, "::");
    match checked.symbols.symbol_package_identity(symbol) {
        Some(package) => {
            let owner = package
                .digest()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            format!("package:{owner}::{path}")
        }
        None => format!("unmanaged::{path}"),
    }
}

fn retain_services(
    checked: &CheckedTrees,
    services: &[ServiceReachId],
    module: &mut TerminalModule,
) -> Result<Vec<ServiceId>, LoweringError> {
    let mut result = Vec::with_capacity(services.len());
    for service in services {
        let definition = checked
            .facts
            .service_reaches
            .services
            .definition(*service)
            .ok_or(LoweringError::Unsupported(
                "closed reach contract has an unknown service",
            ))?;
        let existing = module
            .services
            .iter()
            .find(|service| service.identity == definition.name)
            .map(|service| service.id);
        let id = if let Some(id) = existing {
            id
        } else {
            // A bound or unused argument can name a service absent from the
            // executable closure. Append its parent closure without renumbering
            // existing operations, requirements, or service rows.
            let parents = retain_services(checked, &definition.parents, module)?;
            let next = module
                .services
                .iter()
                .map(|service| service.id.get())
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or(LoweringError::Unsupported(
                    "closed reach service identities are exhausted",
                ))?;
            let id = crate::service_id(next);
            module.services.push(ServiceDeclaration {
                id,
                identity: definition.name.clone(),
                parents,
            });
            id
        };
        result.push(id);
    }
    result.sort();
    result.dedup();
    Ok(result)
}
