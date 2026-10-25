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
    ClosedReachApplication, ClosedReachArgument, ClosedReachCall, ClosedReachCallApplication,
    ClosedReachMachineBinding, ClosedReachParameter, ClosedReachSchema, ServiceDeclaration,
    TerminalModule,
};

use crate::direct_calls::direct_call_parts;
use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;

/// Checked substitution scopes, retaining capture and invocation separately.
struct ReachApplicationView<'a> {
    template: SymbolHandle,
    instance: SymbolHandle,
    parameters: Vec<checked_trees::data::TypeParameter>,
    parameter_aliases: Vec<(SymbolHandle, SymbolHandle)>,
    invocation_start: usize,
    capture: Option<&'a typed_trees::typed_trees::CapturedMachineApplication>,
    normalized_template_identity: &'a str,
    type_argument_identities: Vec<String>,
    const_argument_identities: Vec<String>,
    machine_arguments: Vec<SymbolHandle>,
    machine_contracts: Vec<[u8; 32]>,
}

impl ReachApplicationView<'_> {
    fn position(&self, symbol: SymbolHandle) -> Option<usize> {
        let symbol = self
            .parameter_aliases
            .iter()
            .find_map(|(original, applied)| (*original == symbol).then_some(*applied))
            .unwrap_or(symbol);
        typed_trees::data::type_parameter_position(&self.parameters, symbol)
    }
}

fn capture_view<'a>(
    checked: &'a CheckedTrees,
    capture: &'a typed_trees::typed_trees::CapturedMachineApplication,
) -> Result<ReachApplicationView<'a>, LoweringError> {
    use typed_trees::static_applications::StaticMachineBindingValue;
    let owner = checked
        .conformance_by_symbol(capture.conformance_application.declaration)
        .ok_or(LoweringError::Unsupported(
            "closed capture lost its selecting declaration",
        ))?;
    let parameters = checked.conformance_type_parameters(owner);
    let bindings = checked
        .expression_table
        .static_machine_application_bindings(capture.binding_application);
    if bindings.len() != parameters.len() {
        return unsupported("closed capture lost its complete typed substitution");
    }
    let mut types = Vec::new();
    let mut constants = Vec::new();
    for parameter in parameters {
        let mut matches = bindings
            .iter()
            .filter(|binding| binding.parameter == parameter.symbol);
        let (Some(binding), None) = (matches.next(), matches.next()) else {
            return unsupported("closed capture has no unique exact parameter binding");
        };
        match (&parameter.kind, &binding.value) {
            (TypeParameterKind::Type, StaticMachineBindingValue::Type(reference)) => {
                types.push(checked.normalized_type_identity_text(*reference));
            }
            (
                TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. },
                StaticMachineBindingValue::Const(reference),
            ) => {
                constants.push(
                    checked
                        .static_const_type_reference_identity(*reference)
                        .ok_or(LoweringError::Unsupported(
                            "closed capture has an open const operand",
                        ))?,
                );
            }
            (TypeParameterKind::Machine { .. }, StaticMachineBindingValue::Machine(_)) => {}
            _ => return unsupported("closed capture argument differs from its declared kind"),
        }
    }
    let identity = checked
        .facts
        .specialization_identities
        .for_instance(capture.instance)
        .ok_or(LoweringError::Unsupported(
            "closed capture lost its checked identity",
        ))?;
    Ok(ReachApplicationView {
        template: capture.template,
        instance: capture.instance,
        parameters: parameters.to_vec(),
        parameter_aliases: capture.parameters.clone(),
        invocation_start: parameters.len(),
        capture: Some(capture),
        normalized_template_identity: &capture.normalized_template_identity,
        type_argument_identities: types,
        const_argument_identities: constants,
        machine_arguments: checked
            .expression_table
            .static_machine_application_selected_machines(capture.binding_application)
            .iter()
            .map(|argument| argument.symbol)
            .collect(),
        machine_contracts: identity.machine_argument_contract_commitments.to_vec(),
    })
}

fn application_views<'a>(
    checked: &'a CheckedTrees,
    sources: &[(SymbolHandle, MachineId)],
) -> Result<Vec<ReachApplicationView<'a>>, LoweringError> {
    let mut applications = Vec::with_capacity(
        checked.machine_specializations.len() + checked.captured_machine_applications.len(),
    );
    for application in &checked.machine_specializations {
        if exact_machine(sources, application.instance)?.is_none() {
            continue;
        }
        let parameters = checked
            .data_type_parameters
            .span(application.template_parameters)
            .ok_or(LoweringError::Unsupported(
                "closed invocation has a stale telescope",
            ))?;
        let identity = checked
            .facts
            .specialization_identities
            .for_instance(application.instance)
            .ok_or(LoweringError::Unsupported(
                "closed invocation lost its checked identity",
            ))?;
        let mut captures = checked
            .captured_machine_applications
            .iter()
            .filter(|(_, capture)| capture.instance == application.template)
            .map(|(_, capture)| capture);
        let mut view = if let Some(capture) = captures.next() {
            if captures.next().is_some() {
                return unsupported("closed invocation has ambiguous capture ownership");
            }
            capture_view(checked, capture)?
        } else {
            ReachApplicationView {
                template: application.template,
                instance: application.instance,
                parameters: Vec::new(),
                parameter_aliases: Vec::new(),
                invocation_start: 0,
                capture: None,
                normalized_template_identity: &application.normalized_template_identity,
                type_argument_identities: Vec::new(),
                const_argument_identities: Vec::new(),
                machine_arguments: Vec::new(),
                machine_contracts: Vec::new(),
            }
        };
        view.instance = application.instance;
        view.normalized_template_identity = &application.normalized_template_identity;
        view.parameters.extend_from_slice(parameters);
        view.type_argument_identities
            .extend_from_slice(&application.type_argument_identities);
        view.const_argument_identities
            .extend_from_slice(&application.const_argument_identities);
        view.machine_arguments
            .extend_from_slice(&application.machine_arguments);
        view.machine_contracts
            .extend_from_slice(&identity.machine_argument_contract_commitments);
        applications.push(view);
    }
    for (_, capture) in checked.captured_machine_applications.iter() {
        if exact_machine(sources, capture.instance)?.is_some() {
            applications.push(capture_view(checked, capture)?);
        }
    }
    Ok(applications)
}

fn parameter_kind(parameter: &checked_trees::data::TypeParameter) -> u8 {
    match &parameter.kind {
        TypeParameterKind::Type => 0,
        TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => 1,
        TypeParameterKind::Proposition { .. } => 2,
        TypeParameterKind::Machine {
            contract: MachineParameterContract::RequirementIdentity,
        } => 4,
        TypeParameterKind::Machine { .. } => 3,
        TypeParameterKind::Implementation => 5,
    }
}

fn requirement_arguments(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    application: &ReachApplicationView<'_>,
    contract: &MachineParameterContract,
) -> Result<(Vec<u8>, Vec<terminal_psi::TerminalIdentityIndex>), LoweringError> {
    let MachineParameterContract::Nominal {
        trait_definition,
        trait_arguments,
        ..
    } = contract
    else {
        return Ok((Vec::new(), Vec::new()));
    };
    let declaration = checked
        .trait_definition_by_symbol(*trait_definition)
        .ok_or(LoweringError::Unsupported(
            "closed reach requirement lost its exact trait",
        ))?;
    let parameters = checked.trait_type_parameters(declaration);
    if parameters.len() != trait_arguments.len() {
        return unsupported("closed reach requirement lost its full trait argument telescope");
    }
    let mut binders = application
        .parameters
        .iter()
        .enumerate()
        .map(|(position, parameter)| (parameter.symbol, format!("reach-slot({position})")))
        .collect::<Vec<_>>();
    if let Some(template) = checked.machine(application.template) {
        binders.extend(
            template
                .conformance_bounds
                .iter()
                .enumerate()
                .map(|(position, bound)| {
                    (bound.binder, format!("reach-evidence-slot({position})"))
                }),
        );
    }
    for (original, applied) in &application.parameter_aliases {
        if let Some((symbol, name)) = binders
            .iter()
            .find(|(symbol, _)| symbol == applied || symbol == original)
        {
            let other = if symbol == applied {
                *original
            } else {
                *applied
            };
            let name = name.clone();
            if !binders.iter().any(|(symbol, _)| *symbol == other) {
                binders.push((other, name));
            }
        }
    }
    let arguments = trait_arguments
        .iter()
        .map(|argument| {
            let text = checked.type_identity_request_text(
                typed_trees::type_identity::TypeIdentityRequest {
                    type_reference: *argument,
                    binders: &binders,
                    substitutions: &[],
                    qualification:
                        typed_trees::type_identity::TypeIdentityQualification::PackageQualified,
                },
            );
            if text.is_empty() {
                return unsupported("closed reach requirement has an empty argument identity");
            }
            Ok(identities.intern(&text))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    Ok((parameters.iter().map(parameter_kind).collect(), arguments))
}

pub(crate) fn retain_closed_reach_applications(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    source_machines: &[(SymbolHandle, MachineId)],
    occurrences: &[LoweredSourceCallOccurrence],
    module: &mut TerminalModule,
) -> Result<(), LoweringError> {
    // Invocation and conformance capture are distinct checked applications of
    // an authored member. Both own a substitution ledger; neither is inferred
    // again from the executable body's selected callbacks.
    let applications = application_views(checked, source_machines)?;
    let operational = &checked.facts.operational;
    let inferred = &checked.facts.service_reaches;
    for specialization in &applications {
        let checked_identity = checked
            .facts
            .specialization_identities
            .for_instance(specialization.instance)
            .ok_or(LoweringError::Unsupported(
                "closed reach application lost its checked specialization identity",
            ))?;
        let Some(owner) = exact_machine(source_machines, specialization.instance)? else {
            continue;
        };
        let parameters = &specialization.parameters;
        if u32::try_from(parameters.len()).is_err() {
            return unsupported("closed reach application lost its complete portable telescope");
        }
        // Type/const-only applications still specialize an original contract:
        // their fixed dependency may include private-helper reach. An empty
        // machine-binder set needs no alternate representation or replay rule.
        // Every admitted invocation preserves its declaration order; nested
        // trait telescopes retain their own proposition and identity slots.
        let template =
            inferred
                .for_machine(specialization.template)
                .ok_or(LoweringError::Unsupported(
                    "closed reach application lost the original template dependency",
                ))?;
        // An owner may retain unresolved installation reaches: their provider
        // bounds stay in the ordinary conservative rows this projection
        // replays, while the requirement structure itself remains in the
        // module's separate installation dependency axis.
        let instance =
            inferred
                .for_machine(specialization.instance)
                .ok_or(LoweringError::Unsupported(
                    "closed reach application lost its checked owner row",
                ))?;
        let mut dependencies = Vec::new();
        for dependency in inferred
            .dependency_parameters
            .span_or_empty(template.dependency.parameters)
        {
            let position = specialization.position(*dependency)
                .ok_or(LoweringError::Unsupported(
                    "closed reach dependency is outside its full telescope",
                ))?;
            dependencies.push(u32::try_from(position).map_err(|_| {
                LoweringError::Unsupported("closed reach telescope exceeds portable positions")
            })?);
        }
        dependencies.sort_unstable();
        let mut telescope = Vec::with_capacity(parameters.len());
        let mut types = specialization.type_argument_identities.iter();
        let mut constants = specialization.const_argument_identities.iter();
        let mut machines = specialization.machine_arguments.iter().zip(
            specialization.machine_contracts.iter(),
        );
        for parameter in parameters {
            telescope.push(match &parameter.kind {
                TypeParameterKind::Type => ClosedReachParameter::Type {
                    argument: types
                        .next()
                        .ok_or(LoweringError::Unsupported(
                            "closed reach telescope lost a type argument",
                        ))?
                        .clone(),
                },
                TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => {
                    ClosedReachParameter::Const {
                        argument: constants
                            .next()
                            .ok_or(LoweringError::Unsupported(
                                "closed reach telescope lost a const argument",
                            ))?
                            .clone(),
                    }
                }
                TypeParameterKind::Machine { contract } => {
                    let (selected, commitment) =
                        machines.next().ok_or(LoweringError::Unsupported(
                            "closed reach telescope lost a selected machine contract",
                        ))?;
                    let (selected_machine, _) = checked
                        .machines()
                        .iter()
                        .find_map(|machine| {
                            typed_trees::state::state_by_symbol(
                                checked.machine_states(machine),
                                *selected,
                            )
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
                    // A selected generic schema may produce several closed
                    // callees with different tuples/rows. One binder-level
                    // callee cannot stand in for those per-call applications.
                    let schema =
                        if checked.machine_type_parameters(selected_machine).is_empty() {
                            None
                        } else {
                            // An unused family has a selected public contract but
                            // no executable tuple. Retain that contract without
                            // manufacturing a body or a template opening. Search
                            // the whole retained module so used and unused bindings
                            // of the same selection agree on any available schema.
                            checked
                                .machine_specializations
                                .iter()
                                .find(|application| {
                                    application.template == selected_machine.symbol
                                        && source_machines
                                            .iter()
                                            .any(|(source, _)| *source == application.instance)
                                })
                                .map(|application| -> Result<ClosedReachSchema, LoweringError> {
                                    let identity = checked.facts.specialization_identities
                                    .for_instance(application.instance)
                                    .ok_or(LoweringError::Unsupported(
                                        "closed reach schema lost its checked template commitment",
                                    ))?;
                                    Ok(ClosedReachSchema {
                                        template_identity: identities
                                            .intern(&application.normalized_template_identity),
                                        template_commitment: identity
                                            .template_contract_commitment
                                            .as_bytes(),
                                    })
                                })
                                .transpose()?
                        };
                    // An unresolved installation selection keeps its effective
                    // row here: provider bounds are already part of the
                    // conservative contract rows, and the requirement axis is
                    // retained separately for installation substitution.
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
                            checked.trait_requirement_callable_identity_text(
                                trait_definition,
                                requirement
                            ),
                        )),
                    };
                    let upper_bound = retain_services(
                        identities,
                        checked,
                        checked
                            .service_reach_rows
                            .services(view.signature().service_reach_row),
                        module,
                    )?;
                    let selected_reach = retain_services(
                        identities,
                        checked,
                        checked.facts.service_reaches.rows.services(reach.effective),
                        module,
                    )?;
                    let (requirement_parameter_kinds, requirement_arguments) =
                        requirement_arguments(identities, checked, specialization, contract)?;
                    ClosedReachParameter::Machine(ClosedReachMachineBinding {
                        nominal_requirement,
                        requirement_parameter_kinds,
                        requirement_arguments,
                        upper_bound,
                        selected_identity: identities
                            .intern(&selected_static_machine_identity(checked, *selected)?),
                        selected_contract_commitment: *commitment,
                        selected_reach,
                        callee,
                        schema,
                    })
                }
                TypeParameterKind::Proposition { .. } => {
                    return unsupported(
                        "closed reach executable proposition argument is unavailable",
                    );
                }
                TypeParameterKind::Implementation => {
                    return unsupported(
                        "closed reach machine telescope retains a trait-only implementation binder",
                    );
                }
            });
        }
        if types.next().is_some() || constants.next().is_some() || machines.next().is_some() {
            return unsupported("closed reach arguments differ from the complete telescope");
        }
        let mut calls = Vec::new();
        let caller = operational
            .machine_by_symbol(specialization.instance)
            .ok_or(LoweringError::Unsupported(
                "closed reach application has no operational owner",
            ))?;
        for state in operational.states.span_or_empty(caller.states) {
            for call in operational.calls.span_or_empty(state.calls) {
                if !call.static_machine_parameter.is_valid() {
                    continue;
                }
                let binder = specialization.position(call.static_machine_parameter)
                .ok_or(LoweringError::Unsupported(
                    "closed reach call lost its original binder",
                ))?;
                let mut matches = occurrences.iter().filter(|occurrence| {
                    occurrence.source_state == state.symbol
                        && occurrence.statement_index == call.statement_index
                        && occurrence.call_ordinal == call.call_ordinal
                        && resolved_source_entry(checked, occurrence.source_target)
                            == Some(call.target_state_symbol)
                });
                let Some(occurrence) = matches.next() else {
                    // The complete owner substitution survives without an
                    // emitted direct operation.
                    continue;
                };
                if matches.next().is_some() {
                    return unsupported("closed reach call has ambiguous emitted occurrences");
                }
                let Some(operation) = occurrence.terminal_site.operation() else { continue; };
                let emitted = module.machine(owner)
                    .and_then(|machine| machine.operations().find(|emitted| emitted.id == operation))
                    .ok_or(LoweringError::Unsupported("closed reach call lost its emitted operation"))?;
                if emitted.kind.direct_callee().is_none() { continue; }
                calls.push(ClosedReachCall {
                    operation,
                    binder: binder as u32,
                    application: match &telescope[binder] {
                        ClosedReachParameter::Machine(binding) if binding.schema.is_some() => {
                            closed_call_application(
                                identities, checked, source_machines, call.target_state_symbol,
                            )?
                        }
                        _ => None,
                    },
                });
            }
        }
        calls.sort_by_key(|call| call.operation);
        // Type/const specialization selects actual custody leaves. Retain
        // their contractual contribution beside the template dependency; it
        // cannot be recovered from authored calls alone or by subtracting a
        // callback ceiling from the owner's flattened row.
        let mut fixed = retain_services(
            identities,
            checked,
            inferred.rows.services(template.dependency.concrete),
            module,
        )?;
        fixed.extend(retain_services(
            identities,
            checked,
            inferred.rows.services(instance.cleanup_contribution),
            module,
        )?);
        fixed.sort();
        fixed.dedup();
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
        for call in &calls {
            let Some(ClosedReachParameter::Machine(_)) = telescope.get(call.binder as usize) else {
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
            if direct_call_parts(&operation.kind).is_none() {
                return unsupported("closed reach consumer is not a direct machine call");
            }
            operation.static_reach_binding = Some(call.binder);
        }
        let capture = specialization
            .capture
            .map(|capture| {
                let receipt = checked
                    .facts
                    .specialization_identities
                    .for_instance(capture.instance)
                    .ok_or(LoweringError::Unsupported(
                        "closed capture lost its checked receipt",
                    ))?;
                let invocation = telescope.split_off(specialization.invocation_start);
                let capture_telescope = std::mem::replace(&mut telescope, invocation);
                Ok::<_, LoweringError>(terminal_psi::ClosedReachCapture {
                    template_identity: identities.intern(&capture.normalized_template_identity),
                    template_commitment: receipt.template_contract_commitment.as_bytes(),
                    specialization_commitment: receipt.commitment.as_bytes(),
                    parameter_kinds: specialization.parameters[..specialization.invocation_start]
                        .iter()
                        .map(parameter_kind)
                        .collect(),
                    telescope: capture_telescope,
                })
            })
            .transpose()?;
        terminal_owner.closed_reach_application = Some(ClosedReachApplication {
            template_identity: identities
                .intern(specialization.normalized_template_identity),
            template_commitment: checked_identity.template_contract_commitment.as_bytes(),
            specialization_commitment: checked_identity.commitment.as_bytes(),
            parameter_kinds: specialization.parameters[specialization.invocation_start..]
                .iter().map(parameter_kind).collect(),
            telescope,
            capture,
            fixed,
            dependencies,
            calls,
        });
    }
    Ok(())
}

// Source call custody may retain a machine declaration while operational
// inference names its resolved entry state. Normalize that exact owner join;
// neither a matching signature nor an arbitrary state of the owner suffices.
fn resolved_source_entry(checked: &CheckedTrees, target: SymbolHandle) -> Option<SymbolHandle> {
    checked.machines().iter().find_map(|machine| {
        let states = checked.machine_states(machine);
        if machine.symbol == target {
            states.first().map(|state| state.symbol)
        } else {
            typed_trees::state::state_by_symbol(states, target).map(|state| state.symbol)
        }
    })
}

fn closed_call_application(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    sources: &[(SymbolHandle, MachineId)],
    target_state: SymbolHandle,
) -> Result<Option<ClosedReachCallApplication>, LoweringError> {
    let source_machine = checked
        .machines()
        .iter()
        .find(|machine| {
            typed_trees::state::state_by_symbol(checked.machine_states(machine), target_state)
                .is_some()
        })
        .ok_or(LoweringError::Unsupported(
            "closed schema call lost its source owner",
        ))?;
    let Some(callee) = exact_machine(sources, source_machine.symbol)? else {
        return Ok(None);
    };
    let specialization = checked
        .machine_specialization(source_machine.symbol)
        .ok_or(LoweringError::Unsupported(
            "closed schema call lost its specialization",
        ))?;
    let mut types = specialization.type_argument_identities.iter();
    let checked_identity = checked
        .facts
        .specialization_identities
        .for_instance(specialization.instance)
        .ok_or(LoweringError::Unsupported(
            "closed schema call lost its checked specialization identity",
        ))?;
    let mut constants = specialization.const_argument_identities.iter();
    let mut machines = specialization.machine_arguments.iter().zip(
        checked_identity
            .machine_argument_contract_commitments
            .iter(),
    );
    let mut arguments = Vec::with_capacity(specialization.template_parameters.len());
    for parameter in checked
        .data_type_parameters
        .span_or_empty(specialization.template_parameters)
    {
        arguments.push(match parameter.kind {
            TypeParameterKind::Type => {
                ClosedReachArgument::Type(identities.intern(types.next().ok_or(
                    LoweringError::Unsupported("schema call lost a type argument"),
                )?))
            }
            TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => {
                ClosedReachArgument::Const(identities.intern(constants.next().ok_or(
                    LoweringError::Unsupported("schema call lost a const argument"),
                )?))
            }
            TypeParameterKind::Machine { .. } => {
                let (selected, commitment) = machines.next().ok_or(LoweringError::Unsupported(
                    "schema call lost a machine argument",
                ))?;
                ClosedReachArgument::Machine {
                    identity: identities
                        .intern(&selected_static_machine_identity(checked, *selected)?),
                    contract_commitment: *commitment,
                }
            }
            TypeParameterKind::Proposition { .. } => return Ok(None),
            TypeParameterKind::Implementation => {
                return unsupported(
                    "schema machine telescope retains a trait-only implementation binder",
                );
            }
        });
    }
    if types.next().is_some() || constants.next().is_some() || machines.next().is_some() {
        return unsupported("schema call arguments differ from its complete telescope");
    }
    Ok(Some(ClosedReachCallApplication {
        callee,
        specialization_commitment: checked_identity.commitment.as_bytes(),
        arguments,
    }))
}

/// A static machine operand names its exact checked owner, overload and entry.
/// Reach telescopes, nested schema tuples and conformance method tuples must
/// retain the same identity. A callable-only evidence identity omits the
/// selected entry and cannot join these independent invocation records.
pub(crate) fn selected_static_machine_identity(
    checked: &CheckedTrees,
    selected: SymbolHandle,
) -> Result<String, LoweringError> {
    let owner = checked
        .machine_holding_state(selected)
        .ok_or(LoweringError::Unsupported(
            "static machine argument lost its exact selected owner",
        ))?;
    let overload =
        checked
            .machine_callable_identity_text(owner)
            .ok_or(LoweringError::Unsupported(
                "static machine argument has no normalized callable identity",
            ))?;
    Ok(format!(
        "{}|{}|selected={}",
        declaration_identity(checked, owner.symbol),
        overload,
        declaration_identity(checked, selected),
    ))
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
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
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
        let identity = identities.intern(
            &crate::terminal_identities::service_declaration_identity(checked, definition.symbol)?,
        );
        let existing = module
            .services
            .iter()
            .find(|service| service.identity == identity)
            .map(|service| service.id);
        let id = if let Some(id) = existing {
            id
        } else {
            // A bound or unused argument can name a service absent from the
            // executable closure. Append its parent closure without renumbering
            // existing operations, requirements, or service rows.
            let parents = retain_services(identities, checked, &definition.parents, module)?;
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
            let id = crate::terminal_identities::service_id(next);
            module.services.push(ServiceDeclaration {
                id,
                identity,
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
