//! Building boundary machines and their static boundary requirements.

use crate::execution::terminal_unit::types::{byte_sequence_carrier, substituted_formal_type};
use crate::execution::terminal_unit::{
    CheckFacts, CheckedBoundaryMachinePlan, CheckedBoundaryMachineResultPlan,
    CheckedStructuralScalarParameterPlan, CheckedUnitStructuralDomainRequirementPlan,
    CheckedUnitStructuralParameterPlan, ShapeCollector, SymbolHandle, TypedTrees,
    boundary_domain_requirements, exact_compiler_intrinsic_boundary_requirement, is_reference,
    is_unit, machine_binders, parameter_qualifications, projected_parameter_qualifications,
    shared_plain_affine_referent, signature_contracts_are_exact_parameter_qualifications,
    state_flow, structural_access_for_type_reference, structural_scalar_signature,
    type_graph_requires_nominal_drop,
};

pub(crate) fn build_boundary_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
) -> Option<CheckedBoundaryMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    let binders = machine_binders(program, machine);
    let result = boundary_result_plan(program, shapes, state.return_type, &binders, &[])?;
    if !program
        .statement_table
        .statements(state.statement_nodes)
        .is_empty()
    {
        return None;
    }
    let (attachment_type_identity, structural_parameters, scalar_parameters) =
        structural_scalar_signature(program, shapes, machine, state, &binders, false)?;
    let domain_requirements = boundary_domain_requirements(
        program,
        facts,
        shapes,
        machine,
        state,
        &structural_parameters,
        &binders,
    )?;
    let contract = facts.contract_plans.for_machine(machine.symbol)?;
    let state_flow = state_flow(facts, machine.symbol, state.symbol)?;

    Some(CheckedBoundaryMachinePlan {
        machine: machine.symbol,
        state: state.symbol,
        contract_owner: machine.symbol,
        attachment_type_identity: Some(attachment_type_identity),
        structural_parameters,
        scalar_parameters,
        result,
        domain_requirements,
        contract_report_fingerprint: contract.report_fingerprint,
        contract_commitment: contract.commitment,
        contract_service_reach: facts.service_reaches.plan_for_machine(machine.symbol)?,
        service_reach: state_flow.service_reach,
    })
}

/// Project the narrow static boundary-trait surface used by checked-adapter
/// dispatch. A trait requirement is not an attached machine and therefore
/// contributes no provider value or structural attachment.
pub(crate) fn build_static_boundary_requirements(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
) -> Vec<CheckedBoundaryMachinePlan> {
    let mut plans = Vec::new();
    let mut call_target_requirements = None;
    for definition in program.traits().iter().filter(|definition| {
        definition.is_boundary && program.trait_type_parameters(definition).is_empty()
    }) {
        for signature in program.trait_machine_signatures(definition) {
            let type_parameters = program.state_signature_type_parameters(signature);
            // A `machine` binder without an ABI `native callback` entry takes
            // its destination from a target-owned private slot: the cited
            // `PrivateCallbackSlot` conformance evaluated a boundary calling
            // plan, retained as `callback_placement` on each recorded use of
            // this registrar.
            let nominal_use_backs_binder =
                |ordinal: usize, parameter: &typed_trees::data::TypeParameter| {
                    let typed_trees::data::TypeParameterKind::Machine {
                        contract:
                            typed_trees::data::MachineParameterContract::Nominal {
                                trait_definition,
                                requirement,
                            },
                    } = parameter.kind
                    else {
                        return false;
                    };
                    facts.nominal_machine_uses.uses.iter().any(|nominal_use| {
                        nominal_use.registration_operation == signature.symbol
                            && usize::try_from(nominal_use.static_machine_ordinal).ok()
                                == Some(ordinal)
                            && nominal_use.satisfaction_trait == trait_definition
                            && nominal_use.satisfaction_requirement == requirement
                            && nominal_use.callback_placement.is_some()
                    })
                };
            let callback_telescope = signature.native_callback_parameters.len()
                <= type_parameters.len()
                && type_parameters
                    .iter()
                    .enumerate()
                    .all(|(ordinal, parameter)| {
                        match signature.native_callback_parameters.get(ordinal) {
                            Some(callback) => {
                                parameter.name == callback.binder
                                    && matches!(
                                    parameter.kind,
                                    typed_trees::data::TypeParameterKind::Machine {
                                        contract:
                                            typed_trees::data::MachineParameterContract::Nominal {
                                                ..
                                            }
                                    }
                                )
                            }
                            None => nominal_use_backs_binder(ordinal, parameter),
                        }
                    });
            // A suspending requirement parks its caller, which no synchronous
            // boundary plan expresses. A blocking one only occupies the
            // worker while it waits; its envelope is folded into the
            // contract commitment below, exactly as an attached `boundary
            // machine` declaration that blocks is planned above.
            if !signature_contracts_are_exact_parameter_qualifications(program, signature)
                || signature.suspends
            {
                continue;
            }
            // A generic telescope the callback path cannot reach is still
            // planned when every checked call naming this requirement retains
            // a specialization resolving the whole telescope: each `Type`
            // binder's derived actual substitutes the formals below, and each
            // `machine` binder's selected entry is already-admitted provider
            // evidence this pass replays rather than re-derives.
            let substitutions = if type_parameters.is_empty() || callback_telescope {
                Vec::new()
            } else {
                let Some(substitutions) =
                    specialized_signature_substitutions(program, facts, signature, type_parameters)
                else {
                    continue;
                };
                substitutions
            };
            let Some(result) =
                boundary_result_plan(program, shapes, signature.return_type, &[], &substitutions)
            else {
                continue;
            };
            let mut structural_parameters = Vec::new();
            let mut scalar_parameters = Vec::new();
            let mut supported = true;
            let mut abi_position = 0_usize;
            for parameter in program.state_signature_parameters(signature) {
                // A boundary-trait receiver selects the provider occurrence;
                // it is not an outbound ABI argument. Its progress premise is
                // closed by installation rather than materialized here.
                if parameter.is_self {
                    continue;
                }
                // Foreign ABI positions are agreed with the boundary; an
                // erased parameter has no such agreement yet.
                if parameter.relevance.is_erased() {
                    supported = false;
                    break;
                }
                let Some(source_position) = u32::try_from(abi_position).ok() else {
                    supported = false;
                    break;
                };
                abi_position += 1;
                // A specialization-resolved formal names its derived actual;
                // compound formals keep their own reference and identity-level
                // substitution covers the generic arguments inside them.
                let parameter_type =
                    substituted_formal_type(program, parameter.type_reference, &substitutions);
                // `is_mutable` is set by a `mut` binding and by an exclusive
                // borrow alike. An exclusive borrow is already carried exactly
                // by the parameter's structural access below. Owned mutable
                // primitive destinations need no storage inside this caller;
                // other owned mutable carriers remain unsupported.
                if parameter.is_const
                    || (parameter.is_mutable
                        && !is_reference(program, parameter_type)
                        && crate::values::mutable_scalar_parameter_type(program, parameter)
                            .is_none())
                {
                    supported = false;
                    break;
                }
                if let Some(primitive_type) = program.primitive_type_reference(parameter_type) {
                    scalar_parameters.push(CheckedStructuralScalarParameterPlan {
                        source_position,
                        primitive_type,
                    });
                    continue;
                }
                let Some(type_identity) = shapes.add_type(parameter_type, &[], &substitutions)
                else {
                    supported = false;
                    break;
                };
                let Some(qualifications) =
                    parameter_qualifications(program, shapes, parameter_type, &[])
                else {
                    supported = false;
                    break;
                };
                if is_reference(program, parameter_type)
                    && byte_sequence_carrier(program, parameter_type, &[])
                        != Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView)
                    && !(qualifications.is_empty()
                        && shared_plain_affine_referent(program, parameter_type).is_some())
                {
                    supported = false;
                    break;
                }
                let Some(access) = structural_access_for_type_reference(program, parameter_type)
                else {
                    supported = false;
                    break;
                };
                let Some(projected_qualifications) =
                    projected_parameter_qualifications(program, shapes, parameter_type, &[])
                else {
                    supported = false;
                    break;
                };
                structural_parameters.push(CheckedUnitStructuralParameterPlan {
                    position: source_position,
                    is_self: false,
                    type_identity,
                    multiplicity: crate::checks::type_multiplicity(program, parameter_type),
                    access,
                    qualifications,
                    projected_qualifications,
                    fused_service_erasure: None,
                });
            }
            if !supported {
                continue;
            }
            let Some(capsule) = facts
                .contract_plans
                .crash_capsule(definition.symbol, signature.symbol)
            else {
                continue;
            };
            // Target classification is invariant across boundary signatures.
            // Resolve each distinct target once, only if a supported boundary
            // actually needs call evidence. Keep every call's reach below.
            let requirements = call_target_requirements
                .get_or_insert_with(|| static_boundary_call_targets(program, facts));
            let signature_key = (
                signature.symbol.arena_index(),
                signature.symbol.generation(),
            );
            let first_target = requirements.partition_point(|(requirement, _)| {
                (requirement.arena_index(), requirement.generation()) < signature_key
            });
            let after_targets = requirements.partition_point(|(requirement, _)| {
                (requirement.arena_index(), requirement.generation()) <= signature_key
            });
            let targets = &requirements[first_target..after_targets];
            if targets.is_empty() {
                continue;
            }
            let call_reaches = facts
                .flow
                .control
                .calls
                .iter()
                .map(|(_, call)| call)
                .filter(|call| {
                    targets
                        .binary_search_by_key(
                            &(
                                call.target_symbol.arena_index(),
                                call.target_symbol.generation(),
                            ),
                            |(_, target)| (target.arena_index(), target.generation()),
                        )
                        .is_ok()
                })
                .map(|call| call.service_reach.transitive)
                .collect::<Vec<_>>();
            let [published_reach, rest @ ..] = call_reaches.as_slice() else {
                continue;
            };
            if rest.iter().any(|reach| reach != published_reach) {
                continue;
            }
            let service_reach = language_semantics::ServiceReachSummary {
                direct: *published_reach,
                transitive: *published_reach,
            };
            let domain_requirements = structural_parameters
                .iter()
                .enumerate()
                .flat_map(|(argument_index, parameter)| {
                    parameter.qualifications.iter().map(move |domain| {
                        CheckedUnitStructuralDomainRequirementPlan {
                            argument_index: u32::try_from(argument_index)
                                .expect("structural parameter count already fits source positions"),
                            domain: *domain,
                        }
                    })
                })
                .collect();
            plans.push(CheckedBoundaryMachinePlan {
                machine: signature.symbol,
                state: signature.symbol,
                contract_owner: definition.symbol,
                attachment_type_identity: None,
                structural_parameters,
                scalar_parameters,
                result,
                domain_requirements,
                contract_report_fingerprint: capsule.target_contract_report_fingerprint(),
                contract_commitment: capsule.target_contract_commitment(),
                contract_service_reach: language_semantics::ServiceReachPlan {
                    interface: language_semantics::ServiceReachInterface::PublishedCeiling(
                        *published_reach,
                    ),
                    checked_inferred: *published_reach,
                },
                service_reach,
            });
        }
    }
    plans.sort_by_key(|plan| (plan.machine.arena_index(), plan.machine.generation()));
    plans.dedup_by_key(|plan| plan.machine);
    plans
}

/// A target can denote its own requirement and project to another one. Retain
/// the union of exact identities, not a preferred classification. This sorted
/// scratch index belongs only to the current immutable program and call facts.
/// Nominal binder requirements are not selected execution targets; only a closed
/// specialization may replace a binder with a callable body or exact boundary.
fn static_boundary_call_targets(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> Vec<(SymbolHandle, SymbolHandle)> {
    let mut targets = facts
        .flow
        .control
        .calls
        .iter()
        .map(|(_, call)| call.target_symbol)
        .collect::<Vec<_>>();
    targets.sort_unstable_by_key(|target| (target.arena_index(), target.generation()));
    targets.dedup();
    let mut requirements = Vec::new();
    for target in targets {
        requirements.push((target, target));
        if let Some((requirement, _)) =
            exact_compiler_intrinsic_boundary_requirement(program, target)
        {
            requirements.push((requirement, target));
        }
    }
    requirements.sort_unstable_by_key(|(requirement, target)| {
        (
            requirement.arena_index(),
            requirement.generation(),
            target.arena_index(),
            target.generation(),
        )
    });
    requirements.dedup();
    requirements
}

/// Substitute a generic requirement's `Type` formals with the actuals its
/// checked calls admitted. Every flow call that names this requirement must
/// retain a `requirement_call_specializations` row at its authored site that
/// resolves the whole telescope: each `Type` binder carries its derived
/// actual and each `machine` binder's retained selection is a concrete
/// selected machine — exactly the authored static argument at that ordinal,
/// mirroring the coverage `build_call_operation` requires before it plans
/// the call. A forwarded in-scope binder or a missing row leaves the
/// requirement unplanned. One boundary plan serves every call site, so the
/// retained `Type` bindings must agree; selections may differ because the
/// plan records no provider identity.
fn specialized_signature_substitutions(
    program: &TypedTrees,
    facts: &CheckFacts,
    signature: &typed_trees::signature::StateSignature,
    type_parameters: &[typed_trees::data::TypeParameter],
) -> Option<Vec<(SymbolHandle, typed_trees::types::TypeReferenceHandle)>> {
    let mut resolved: Option<Vec<checked_trees::CheckedRequirementCallTypeBinding>> = None;
    for (_, flow_state) in facts.flow.control.states.iter() {
        for call in facts.flow.control.calls.span_or_empty(flow_state.calls) {
            if call.target_symbol != signature.symbol {
                continue;
            }
            let call_site = crate::semantic_calls::find_call_site(
                program,
                flow_state.machine_symbol,
                flow_state.state_symbol,
                call.statement_index,
                call.call_ordinal,
            )?;
            let site = match &call_site {
                crate::semantic_calls::CallSite::Statement(_) => {
                    let state = crate::semantic_calls::find_state_in_machine(
                        program,
                        flow_state.machine_symbol,
                        flow_state.state_symbol,
                    )?;
                    let offset = u32::try_from(call.statement_index).ok()?;
                    checked_trees::NominalMachineUseSite::Statement(arena::Handle::from_parts(
                        state
                            .statement_nodes
                            .start()
                            .arena_index()
                            .checked_add(offset)?,
                        state.statement_nodes.start().generation(),
                    ))
                }
                crate::semantic_calls::CallSite::Expression { expression, .. } => {
                    checked_trees::NominalMachineUseSite::Expression(*expression)
                }
                crate::semantic_calls::CallSite::TransitionNamed { .. } => return None,
            };
            let specialization = facts
                .requirement_call_specializations
                .for_site(site, signature.symbol)?;
            // The authored static machine arguments the retained ordinal
            // indexes: validation filtered the `<>` argument list to
            // machine-typed members before recording `static_machine_ordinal`.
            let authored_machine_arguments = match &call_site {
                crate::semantic_calls::CallSite::Statement(call) => call.machine_arguments.as_ref(),
                crate::semantic_calls::CallSite::Expression { call, .. } => {
                    call.machine_arguments.as_ref()
                }
                crate::semantic_calls::CallSite::TransitionNamed { .. } => &[],
            }
            .iter()
            .filter(|argument| {
                matches!(
                    program.symbols.get(argument.symbol).kind,
                    symbols::SymbolKind::State | symbols::SymbolKind::MachineParameter
                )
            })
            .collect::<Vec<_>>();
            if !type_parameters
                .iter()
                .all(|parameter| match &parameter.kind {
                    typed_trees::data::TypeParameterKind::Type => specialization
                        .type_bindings
                        .iter()
                        .any(|binding| binding.parameter == parameter.symbol),
                    typed_trees::data::TypeParameterKind::Machine { .. } => {
                        specialization.machine_selections.iter().any(|selection| {
                            selection.parameter == parameter.symbol
                                && selection.selected_machine.is_valid()
                                && usize::try_from(selection.static_machine_ordinal)
                                    .ok()
                                    .and_then(|ordinal| authored_machine_arguments.get(ordinal))
                                    .is_some_and(|argument| argument.symbol == selection.selected)
                        })
                    }
                    _ => false,
                })
            {
                return None;
            }
            match &resolved {
                None => resolved = Some(specialization.type_bindings.clone()),
                Some(existing) if *existing == specialization.type_bindings => {}
                Some(_) => return None,
            }
        }
    }
    resolved.map(|type_bindings| {
        type_bindings
            .iter()
            .map(|binding| (binding.parameter, binding.actual))
            .collect()
    })
}

pub(crate) fn boundary_result_plan(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    type_reference: typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
) -> Option<CheckedBoundaryMachineResultPlan> {
    let type_reference = substituted_formal_type(program, type_reference, substitutions);
    if is_unit(program, type_reference) {
        return Some(CheckedBoundaryMachineResultPlan::Unit);
    }
    if let Some(scalar) = program.primitive_type_reference(type_reference) {
        return Some(CheckedBoundaryMachineResultPlan::Scalar(scalar));
    }
    if is_reference(program, type_reference)
        || type_graph_requires_nominal_drop(program, type_reference)
    {
        return None;
    }
    Some(CheckedBoundaryMachineResultPlan::Structural {
        type_identity: shapes.add_type(type_reference, binders, substitutions)?,
        multiplicity: crate::checks::type_multiplicity(program, type_reference),
        qualifications: parameter_qualifications(program, shapes, type_reference, binders)?,
    })
}
