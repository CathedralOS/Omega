//! Retain the complete selected map and each exact executable application.
//!
//! Application owners are emitted machine bodies. Their selected realizations
//! are callables: an ordinary body or an opaque requirement, including a
//! portable target-family head. Keeping these coordinates distinct preserves
//! the public contract without choosing a target in Psi or borrowing a
//! provider body's identity. Both callable kinds retain their independent
//! checked declaration, result class and method tuple for artifact ingress.
use checked_trees::data::TypeParameterKind;
use checked_trees::{CheckedTrees, ClosedConformanceConstArgument};
use terminal_psi::{
    ClosedConformanceApplication, ClosedConformanceCallableResult,
    ClosedConformanceParameterBinding, ClosedConformanceParameterKind,
    ClosedConformanceRealizationCallable, ClosedConformanceRow, TargetMachineCallable,
    TerminalModule, closed_conformance_application_commitment,
    closed_conformance_application_report_fingerprint,
};

use super::LoweringError;

pub(crate) fn lower_closed_conformance_applications(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    source_machines: &[symbols::SymbolHandle],
    source_boundaries: &[(
        symbols::SymbolHandle,
        semantic_vocabulary::BoundaryMachineId,
    )],
    module: &mut TerminalModule,
) -> Result<(), LoweringError> {
    let carries_applications = checked
        .typed
        .closed_conformance_application_sites()
        .into_iter()
        .any(|(_, owner, _, _)| source_machines.contains(&owner));
    if !carries_applications {
        module.closed_conformance_applications.clear();
        return Ok(());
    }
    if source_machines.len() != module.machines.len() {
        return Err(LoweringError::Unsupported(
            "closed conformance application ownership does not match the terminal machine closure",
        ));
    }

    let owners = source_machines
        .iter()
        .zip(&module.machines)
        .map(|(source, terminal)| (*source, terminal.id, &terminal.result))
        .collect::<Vec<_>>();
    let mut callables = owners
        .iter()
        .map(|(source, machine, result)| {
            let result = match result {
                terminal_psi::TerminalMachineResult::Unit => ClosedConformanceCallableResult::Unit,
                terminal_psi::TerminalMachineResult::Scalar(result) => {
                    ClosedConformanceCallableResult::Scalar(result.scalar_type)
                }
                terminal_psi::TerminalMachineResult::Structural(result) => {
                    ClosedConformanceCallableResult::Structural(result.structural_type)
                }
            };
            (*source, TargetMachineCallable::Machine(*machine), result)
        })
        .collect::<Vec<_>>();
    for (source, boundary) in source_boundaries {
        let declaration = module
            .boundary_machine(*boundary)
            .ok_or(LoweringError::Unsupported(
                "static conformance boundary owner is absent",
            ))?;
        let result = match &declaration.result {
            terminal_psi::BoundaryMachineResult::Unit => ClosedConformanceCallableResult::Unit,
            terminal_psi::BoundaryMachineResult::Scalar(result) => {
                ClosedConformanceCallableResult::Scalar(*result)
            }
            terminal_psi::BoundaryMachineResult::Structural(result) => {
                ClosedConformanceCallableResult::Structural(result.structural_type)
            }
        };
        callables.push((*source, TargetMachineCallable::Boundary(*boundary), result));
    }
    module.closed_conformance_applications =
        collect_closed_conformance_applications(identities, checked, &owners, &callables, &[])?;
    Ok(())
}

fn collect_closed_conformance_applications(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    owners: &[(
        symbols::SymbolHandle,
        semantic_vocabulary::MachineId,
        &terminal_psi::TerminalMachineResult,
    )],
    emitted_callables: &[(
        symbols::SymbolHandle,
        TargetMachineCallable,
        ClosedConformanceCallableResult,
    )],
    excluded_sources: &[symbols::SymbolHandle],
) -> Result<Vec<ClosedConformanceApplication>, LoweringError> {
    let mut applications = Vec::new();
    let selected_applications = checked
        .typed
        .closed_conformance_application_sites()
        .into_iter()
        .map(|(_, owner, _, application)| (owner, application));
    for (source_owner, application) in selected_applications {
        if excluded_sources.contains(&source_owner) {
            continue;
        }
        let Some((_, owner, _)) = owners.iter().find(|(source, _, _)| *source == source_owner)
        else {
            continue;
        };
        let selected = checked
            .conformance_by_symbol(application.declaration)
            .ok_or(LoweringError::Unsupported(
                "closed conformance application lost its declaration",
            ))?;
        let selected_rows = selected.rows.as_slice();
        let realized_calls = checked_static_dispatches(checked, source_owner)?;
        let telescope = checked_application_telescope(checked, application)?;
        let mut selected_calls = Vec::new();
        for (invocation, dispatch) in &realized_calls {
            if dispatch.application_report_fingerprint != application.report_fingerprint
                || dispatch.application_commitment != application.commitment
            {
                continue;
            }
            // Canonical records combine equivalent tuples, but each executable
            // still rejoins its own exact Source occurrence. Raw type arenas
            // are not interchangeable between independently authored calls.
            let selected = checked.typed.selected_conformance_applications(
                source_owner,
                dispatch.selection_application,
                dispatch.requirement,
                dispatch.application_report_fingerprint,
                dispatch.application_commitment,
            );
            let [source_application] = selected.as_slice() else {
                return Err(LoweringError::Unsupported(
                    "static conformance call lost its unique source application",
                ));
            };
            if source_application.declaration != application.declaration
                || source_application.lifetime_arguments != application.lifetime_arguments
                || source_application.trait_definition != application.trait_definition
                || source_application.trait_lifetime_arguments
                    != application.trait_lifetime_arguments
                || source_application.trait_arguments != application.trait_arguments
                || source_application.rows != application.rows
                || checked_application_telescope(checked, source_application)? != telescope
            {
                continue;
            }
            selected_calls.push((invocation, dispatch, *source_application));
        }
        let realization_callables = selected_calls
            .into_iter()
            .map(|(invocation, dispatch, source_application)| {
                let selected_row = selected_rows
                    .iter()
                    .find(|row| {
                        row.declaring_trait == dispatch.declaring_trait
                            && row.requirement == dispatch.requirement
                            && row.realization_machine == dispatch.realization_machine
                            && row.realization_state == dispatch.realization_state
                    })
                    .ok_or(LoweringError::Unsupported(
                        "static conformance dispatch lost its normalized row",
                    ))?;
                let callable = emitted_callables
                    .iter()
                    .find_map(|(source, terminal, _)| {
                        (*source == invocation.machine).then_some(*terminal)
                    })
                    .ok_or(LoweringError::Unsupported(
                        "static conformance realization is absent from the Terminal closure",
                    ))?;
                let realization_state = checked
                    .typed
                    .machines()
                    .iter()
                    .flat_map(|machine| checked.typed.machine_states(machine))
                    .find(|state| state.symbol == invocation.state)
                    .ok_or(LoweringError::Unsupported(
                        "static conformance realization state is absent",
                    ))?;
                let requirements = checked
                    .typed
                    .trait_definition_by_symbol(dispatch.declaring_trait)
                    .map(|definition| {
                        checked
                            .typed
                            .trait_machine_signatures(definition)
                            .iter()
                            .filter(|requirement| requirement.symbol == dispatch.requirement)
                            .collect::<Vec<_>>()
                    })
                    .ok_or(LoweringError::Unsupported(
                        "static conformance requirement is absent",
                    ))?;
                let [requirement] = requirements.as_slice() else {
                    return Err(LoweringError::Unsupported(
                        "static conformance requirement is absent or ambiguous",
                    ));
                };
                let emitted_result = emitted_callables
                    .iter()
                    .find(|(source, _, _)| *source == invocation.machine)
                    .map(|(_, _, result)| *result)
                    .ok_or(LoweringError::Unsupported(
                        "static conformance lost its emitted result signature",
                    ))?;
                let classify = |return_type: checked_trees::types::TypeReferenceHandle| {
                    if !return_type.is_valid() {
                        return Ok(ClosedConformanceCallableResult::Unit);
                    }
                    if let Some(primitive) = checked.typed.primitive_type_reference(return_type) {
                        return super::terminal_scalar_type(primitive)
                            .map(ClosedConformanceCallableResult::Scalar);
                    }
                    match emitted_result {
                        ClosedConformanceCallableResult::Structural(result) => {
                            Ok(ClosedConformanceCallableResult::Structural(result))
                        }
                        _ => Err(LoweringError::Unsupported(
                            "structured conformance result has no emitted structural signature",
                        )),
                    }
                };
                let result = classify(realization_state.return_type)?;
                let public_signature = if dispatch.contract_signature.is_valid() {
                    &checked
                        .typed
                        .closed_requirement_application(dispatch.contract_signature)
                        .ok_or(LoweringError::Unsupported(
                            "static conformance lost its applied public signature",
                        ))?
                        .signature
                } else {
                    let scope = source_application
                        .trait_scope
                        .scope(&checked.typed)
                        .and_then(|scope| {
                            scope.declaring_scope(&checked.typed, dispatch.declaring_trait)
                        })
                        .ok_or(LoweringError::Unsupported(
                            "static conformance lost its exact declaring trait application",
                        ))?;
                    checked
                        .typed
                        .applied_callable_signature(&scope, dispatch.requirement)
                        .or_else(|| {
                            (scope.substitutions.is_empty()
                                && scope.declaration.lifetime_parameters.is_empty())
                            .then_some(*requirement)
                        })
                        .ok_or(LoweringError::Unsupported(
                            "static conformance lost its applied public callable signature",
                        ))?
                };
                let public_return =
                    checked_method_return_reference(checked, public_signature, invocation.machine)?;
                if classify(public_return)? != result {
                    return Err(LoweringError::Unsupported(
                        "static conformance requirement and realization result classes differ",
                    ));
                }
                let source_callable_identity = identities.intern(
                    &super::evidence_lowering::checked_evidence_machine_identity(
                        checked,
                        invocation.machine,
                    )?,
                );
                let applied = terminal_psi::ClosedConformanceStaticApplication {
                    public_requirement_identity: identities.intern(
                        &super::evidence_lowering::checked_evidence_requirement_identity(
                            checked,
                            dispatch.declaring_trait,
                            dispatch.requirement,
                        )?,
                    ),
                    realization_identity: identities.intern(
                        &checked
                            .symbols
                            .display_path(selected_row.realization_state, "::"),
                    ),
                    source_callable_identity,
                    callable,
                    application_commitment: checked
                        .facts
                        .specialization_identities
                        .for_instance(invocation.machine)
                        .map(|identity| identity.commitment.as_bytes()),
                    method_arguments: checked_method_arguments(
                        identities,
                        checked,
                        invocation.machine,
                    )?,
                };
                Ok((
                    ClosedConformanceRealizationCallable {
                        source_callable_identity: identities.intern(
                            &super::evidence_lowering::checked_evidence_machine_identity(
                                checked,
                                invocation.machine,
                            )?,
                        ),
                        callable,
                        result,
                    },
                    applied,
                ))
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let (mut realization_callables, mut static_applications): (Vec<_>, Vec<_>) =
            realization_callables.into_iter().unzip();
        let table = identities.snapshot();
        let mut keyed = static_applications
            .into_iter()
            .map(|row| {
                terminal_semantics::closed_conformances::static_application_order_key(&table, &row)
                    .map(|key| (key, row))
                    .map_err(LoweringError::Unsupported)
            })
            .collect::<Result<Vec<_>, _>>()?;
        keyed.sort_by(|left, right| left.0.cmp(&right.0));
        keyed.dedup_by(|left, right| left.0 == right.0);
        static_applications = keyed.into_iter().map(|(_, row)| row).collect();
        let mut schemas = application
            .rows
            .iter()
            .map(|row| {
                super::evidence_lowering::checked_requirement_schema(
                    identities,
                    checked,
                    row.declaring_trait,
                    row.requirement,
                    row.realization_state,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        schemas.sort_by(|left, right| {
            (
                identities.text(left.declaring_trait_identity),
                identities.text(left.public_requirement_identity),
            )
                .cmp(&(
                    identities.text(right.declaring_trait_identity),
                    identities.text(right.public_requirement_identity),
                ))
        });
        if schemas.windows(2).any(|pair| {
            pair[0].declaring_trait_identity == pair[1].declaring_trait_identity
                && pair[0].public_requirement_identity == pair[1].public_requirement_identity
        }) {
            return Err(LoweringError::Unsupported(
                "original conformance schema inventory repeats one requirement",
            ));
        }
        let table = identities.snapshot();
        realization_callables.sort_by(|left, right| {
            table
                .text(left.source_callable_identity)
                .cmp(&table.text(right.source_callable_identity))
        });
        realization_callables.dedup();
        let mut rows = Vec::new();
        for row in &application.rows {
            let selected_row = selected_rows
                .iter()
                .find(|candidate| {
                    candidate.declaring_trait == row.declaring_trait
                        && candidate.requirement == row.requirement
                        && candidate.realization_state == row.realization_state
                })
                .ok_or(LoweringError::Unsupported(
                    "closed conformance application row no longer matches its declaration",
                ))?;
            let declaring_trait_identity = checked
                .symbols
                .display_path(selected_row.declaring_trait, "::");
            let public_requirement_identity =
                super::evidence_lowering::checked_evidence_requirement_identity(
                    checked,
                    selected_row.declaring_trait,
                    selected_row.requirement,
                )?;
            let requirement_identity = checked.symbols.display_path(selected_row.requirement, "::");
            // A finite generic requirement contributes one row per
            // declared roster tuple, each naming the exact specialization
            // instance that tuple selects.
            for family_row in super::evidence_lowering::checked_static_conformance_rows(
                checked,
                selected_row.declaring_trait,
                selected_row.requirement,
                selected_row.realization_machine,
                selected_row.realization_state,
            )? {
                let selected_by_static_dispatch = checked
                    .facts
                    .proof
                    .proof_output_calls
                    .iter()
                    .filter_map(|(_, invocation)| {
                        (invocation.caller_machine_symbol == source_owner)
                            .then_some(invocation.static_requirement_dispatch?)
                    })
                    .any(|dispatch| {
                        dispatch.application_report_fingerprint == application.report_fingerprint
                            && dispatch.application_commitment == application.commitment
                            && dispatch.declaring_trait == selected_row.declaring_trait
                            && dispatch.requirement == selected_row.requirement
                            && dispatch.realization_machine == family_row.realization_machine
                            && dispatch.realization_state == family_row.realization_state
                    });
                let realization_callable_identity = selected_by_static_dispatch
                        .then(|| {
                            let callable_target = emitted_callables
                                .iter()
                                .find_map(|(source, terminal, _)| {
                                    (*source == family_row.realization_machine)
                                        .then_some(*terminal)
                                })
                                .ok_or(LoweringError::Unsupported(
                                    "static conformance realization is absent from the Terminal closure",
                                ))?;
                            let identity =
                                super::evidence_lowering::checked_evidence_machine_identity(
                                    checked,
                                    family_row.realization_machine,
                                )?;
                            realization_callables
                                .iter()
                                .any(|callable| {
                                    callable.source_callable_identity == identities.intern(&identity)
                                        && callable.callable == callable_target
                                })
                                .then_some(identity)
                                .ok_or(LoweringError::Unsupported(
                                    "static conformance row lost its independent callable registry entry",
                                ))
                        })
                        .transpose()?;
                rows.push(ClosedConformanceRow {
                    declaring_trait_identity: identities.intern(&declaring_trait_identity.clone()),
                    public_requirement_identity: identities
                        .intern(&public_requirement_identity.clone()),
                    family_tuple: family_row
                        .family_tuple
                        .iter()
                        .map(|identity| identities.intern(identity))
                        .collect(),
                    requirement_identity: identities.intern(&requirement_identity.clone()),
                    realization_identity: identities.intern(
                        &checked
                            .symbols
                            .display_path(family_row.realization_state, "::"),
                    ),
                    realization_callable_identity: realization_callable_identity
                        .as_ref()
                        .map(|identity| identities.intern(identity)),
                });
            }
        }
        let telescope = checked_application_telescope(checked, application)?;
        let mut lowered = ClosedConformanceApplication {
            owner: *owner,
            declaration_identity: identities
                .intern(&checked.symbols.display_path(application.declaration, "::")),
            telescope,
            trait_identity: identities.intern(
                &checked
                    .symbols
                    .display_path(application.trait_definition, "::"),
            ),
            trait_lifetime_arguments: application.trait_lifetime_arguments.clone(),
            trait_arguments: application.trait_arguments.clone(),
            realization_callables,
            schemas,
            static_applications,
            rows,
            report_fingerprint: 0,
            commitment: Default::default(),
        };
        lowered.report_fingerprint =
            closed_conformance_application_report_fingerprint(&identities.snapshot(), &lowered);
        lowered.commitment =
            closed_conformance_application_commitment(&identities.snapshot(), &lowered);
        if !applications.contains(&lowered) {
            applications.push(lowered);
        }
    }
    sort_applications(&identities.snapshot(), &mut applications);
    Ok(applications)
}

fn sort_applications(
    identities: &terminal_psi::TerminalIdentityTable,
    applications: &mut [ClosedConformanceApplication],
) {
    applications.sort_by(|left, right| {
        (
            left.owner,
            identities
                .text(left.declaration_identity)
                .unwrap_or_default(),
            left.report_fingerprint,
        )
            .cmp(&(
                right.owner,
                identities
                    .text(right.declaration_identity)
                    .unwrap_or_default(),
                right.report_fingerprint,
            ))
    });
}

/// Shared exact tuple replay for retention and proof-output joins.
pub(crate) fn checked_application_telescope(
    checked: &CheckedTrees,
    application: &typed_trees::typed_trees::ClosedConformanceApplication,
) -> Result<Vec<ClosedConformanceParameterBinding>, LoweringError> {
    let selected = checked
        .conformance_by_symbol(application.declaration)
        .ok_or(LoweringError::Unsupported(
            "closed conformance telescope lost its original declaration",
        ))?;
    if selected.lifetime_parameters.len() != application.lifetime_arguments.len() {
        return Err(LoweringError::Unsupported(
            "closed conformance application has an open lifetime telescope",
        ));
    }
    let mut telescope = selected
        .lifetime_parameters
        .iter()
        .zip(&application.lifetime_arguments)
        .map(|(parameter, argument)| ClosedConformanceParameterBinding {
            parameter: parameter.as_str().to_owned(),
            kind: ClosedConformanceParameterKind::Lifetime,
            argument: argument.clone(),
        })
        .collect::<Vec<_>>();
    let mut type_index = 0usize;
    let mut const_index = 0usize;
    let mut machine_index = 0usize;
    for parameter in checked.conformance_type_parameters(selected) {
        let (kind, argument) = match &parameter.kind {
            TypeParameterKind::Type => {
                let argument = application.type_arguments.get(type_index).cloned();
                type_index += 1;
                (ClosedConformanceParameterKind::Type, argument)
            }
            TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => {
                let argument = match application.const_arguments.get(const_index) {
                    Some(ClosedConformanceConstArgument::Evaluated { value, .. }) => Some(
                        language_semantics::const_value::CanonicalConstValue::new(
                            value.type_name.clone(),
                            value.encoding.clone(),
                            "",
                        )
                        .atom(),
                    ),
                    // Nothing substitutes a caller's const binder
                    // into a retained application, and Terminal's
                    // telescope names only evaluated values.
                    Some(ClosedConformanceConstArgument::CallerBinder { .. }) => {
                        return Err(LoweringError::Unsupported(
                            "closed conformance application has an open const binder argument",
                        ));
                    }
                    None => None,
                };
                const_index += 1;
                (ClosedConformanceParameterKind::Const, argument)
            }
            TypeParameterKind::Machine { .. } => {
                if !application.callable_application.is_valid() {
                    return Err(LoweringError::Unsupported(
                        "closed conformance Machine tuple lost its checked occurrence",
                    ));
                }
                let selected = checked
                    .typed
                    .closed_application_selected_machines(application)
                    .get(machine_index)
                    .ok_or(LoweringError::Unsupported(
                        "closed conformance Machine tuple lost its exact selected callback",
                    ))?;
                let owner = checked.typed.machine_holding_state(selected.symbol).ok_or(
                    LoweringError::Unsupported(
                        "closed conformance Machine tuple is not executable",
                    ),
                )?;
                let argument = Some(super::evidence_lowering::checked_evidence_machine_identity(
                    checked,
                    owner.symbol,
                )?);
                machine_index += 1;
                (ClosedConformanceParameterKind::Machine, argument)
            }
            TypeParameterKind::Proposition { .. } => {
                return Err(LoweringError::Unsupported(
                    "closed conformance application retains a proposition parameter",
                ));
            }
            TypeParameterKind::Implementation => {
                return Err(LoweringError::Unsupported(
                    "closed conformance telescope retains a trait-only implementation binder",
                ));
            }
        };
        telescope.push(ClosedConformanceParameterBinding {
            parameter: parameter.name.as_str().to_owned(),
            kind,
            argument: argument.ok_or(LoweringError::Unsupported(
                "closed conformance application has an open static telescope",
            ))?,
        });
    }
    if type_index != application.type_arguments.len()
        || const_index != application.const_arguments.len()
        || machine_index != application.machine_arguments.len()
    {
        return Err(LoweringError::Unsupported(
            "closed conformance application telescope has extra arguments",
        ));
    }
    Ok(telescope)
}

/// The method's invocation frame is independent of its selecting map capture.
pub(crate) fn checked_method_arguments(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<Vec<terminal_psi::ClosedReachArgument>, LoweringError> {
    let records = checked
        .machine_specializations
        .iter()
        .filter(|row| row.instance == machine)
        .collect::<Vec<_>>();
    let specialization = match records.as_slice() {
        [] => return Ok(Vec::new()),
        [record] => record,
        _ => {
            return Err(LoweringError::Unsupported(
                "static method application has duplicate invocation ownership",
            ));
        }
    };
    let identity = checked
        .facts
        .specialization_identities
        .for_instance(machine)
        .ok_or(LoweringError::Unsupported(
            "static method application lost its checked invocation commitment",
        ))?;
    let mut types = specialization.type_argument_identities.iter();
    let mut constants = specialization.const_argument_identities.iter();
    let mut machines = specialization
        .machine_arguments
        .iter()
        .zip(identity.machine_argument_contract_commitments.iter());
    let mut arguments = Vec::new();
    for parameter in checked
        .typed
        .data_type_parameters
        .span_or_empty(specialization.template_parameters)
    {
        arguments.push(match parameter.kind {
            TypeParameterKind::Type => terminal_psi::ClosedReachArgument::Type(identities.intern(types.next().ok_or(LoweringError::Unsupported("static method tuple lost a Type argument"))?)),
            TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => terminal_psi::ClosedReachArgument::Const(identities.intern(constants.next().ok_or(LoweringError::Unsupported("static method tuple lost a const argument"))?)),
            TypeParameterKind::Machine { .. } => {
                let (selected, commitment) = machines.next().ok_or(LoweringError::Unsupported("static method tuple lost a Machine argument"))?;
                terminal_psi::ClosedReachArgument::Machine { identity: identities.intern(&super::evidence_lowering::checked_evidence_machine_identity(checked, checked.typed.machine_holding_state(*selected).map_or(*selected, |machine| machine.symbol))?), contract_commitment: *commitment }
            }
            _ => return Err(LoweringError::Unsupported("static method tuple contains an unsupported proposition or implementation argument")),
        });
    }
    if types.next().is_some() || constants.next().is_some() || machines.next().is_some() {
        return Err(LoweringError::Unsupported(
            "static method tuple has extra invocation arguments",
        ));
    }
    Ok(arguments)
}

struct StaticExecutableCall {
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
}

/// Retain every checked ordinary occurrence, including scalar computation calls
/// with no erased proof outputs. Proof-lane presence does not select execution.
/// The occurrences are the owner's live flow call rows: each carries the
/// static selection the checker resolved, so no statement is walked again;
/// a row retired by selected execution names a call the body no longer makes.
fn checked_static_dispatches(
    checked: &CheckedTrees,
    owner: symbols::SymbolHandle,
) -> Result<
    Vec<(
        StaticExecutableCall,
        checked_trees::StaticRequirementDispatchFact,
    )>,
    LoweringError,
> {
    let control = &checked.facts.flow.control;
    let mut calls = Vec::new();
    for (_, state) in control
        .states
        .iter()
        .filter(|(_, state)| state.machine_symbol == owner)
    {
        for call in control.calls.span_or_empty(state.calls) {
            let Some(dispatch) = call.static_requirement_dispatch else {
                continue;
            };
            if control.is_retired(state.state_symbol, call) {
                continue;
            }
            let target = call.target_symbol;
            let realized =
                checked
                    .typed
                    .machine_holding_state(target)
                    .ok_or(LoweringError::Unsupported(
                        "static executable call lost its exact target owner",
                    ))?;
            calls.push((
                StaticExecutableCall {
                    machine: realized.symbol,
                    state: target,
                },
                dispatch,
            ));
        }
    }
    Ok(calls)
}

fn checked_method_return_reference(
    checked: &CheckedTrees,
    signature: &checked_trees::signature::StateSignature,
    target: symbols::SymbolHandle,
) -> Result<checked_trees::types::TypeReferenceHandle, LoweringError> {
    let result = signature.return_type;
    if !result.is_valid() {
        return Ok(result);
    }
    let checked_trees::types::TypeReferenceNode::Named { symbol, .. } =
        checked.typed.type_reference_table.type_reference(result)
    else {
        return Ok(result);
    };
    let parameters = checked.typed.state_signature_type_parameters(signature);
    let Some(position) = parameters
        .iter()
        .position(|parameter| parameter.symbol == *symbol)
    else {
        return Ok(result);
    };
    let specialization = checked
        .machine_specializations
        .iter()
        .find(|row| row.instance == target)
        .ok_or(LoweringError::Unsupported(
            "generic conformance result lost its exact method invocation",
        ))?;
    let private = checked
        .typed
        .data_type_parameters
        .span_or_empty(specialization.template_parameters)
        .get(position)
        .ok_or(LoweringError::Unsupported(
            "generic conformance result has no corresponding method binder",
        ))?;
    match checked
        .typed
        .specialization_binding(specialization, private.symbol)
    {
        Some(typed_trees::static_applications::StaticMachineBindingValue::Type(reference)) => {
            Ok(*reference)
        }
        Some(_) => Err(LoweringError::Unsupported(
            "generic conformance result binder is not a Type",
        )),
        None => Err(LoweringError::Unsupported(
            "generic conformance result has no unique applied Type",
        )),
    }
}
