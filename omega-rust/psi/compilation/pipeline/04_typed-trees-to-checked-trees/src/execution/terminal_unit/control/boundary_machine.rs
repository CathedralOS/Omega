//! Building boundary machines and their static boundary requirements.
//!
//! Source checking owns authored public contracts and provider refinement.
//! Publication retains their exact checked commitment, runtime signature and
//! custody; it does not recheck their source arrangement. Qualifying a result
//! still requires its separately checked authorization route.

use super::boundary_contracts::boundary_scalar_requires;

use crate::execution::terminal_unit::calls::{
    free_structural_scalar_signature, parameter_signatures,
};
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::types::{
    ShapeCollector, boundary_domain_requirements, fold_boundary_result_domains, is_unit,
    machine_binders, parameter_qualifications, projected_result_qualifications,
    substituted_formal_type,
};
use crate::execution::terminal_unit::{
    CheckFacts, CheckedBoundaryMachinePlan, CheckedBoundaryMachineResultPlan, SymbolHandle,
    is_reference,
};

pub(crate) fn build_boundary_machine(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
) -> Result<CheckedBoundaryMachinePlan, &'static str> {
    let [state] = program.machine_states(machine) else {
        return Err("boundary declaration requires one signature state");
    };
    let binders = machine_binders(program, machine);
    for (_, operation) in program.placement_lifecycle_plans.iter() {
        if matches!(operation, typed_trees::typed_trees::PlacementLifecyclePlan::Evaluated { callable, .. }
            if *callable == machine.symbol || *callable == state.symbol)
        {
            for definition in operation.storage_definitions(program.typed_program())? {
                shapes
                    .add_attached_data(definition, &binders)
                    .ok_or("placement operation storage has no retained shape")?;
            }
        }
    }
    let result = boundary_result_plan(program, shapes, state.return_type, &binders, &[])
        .ok_or("boundary result has no runtime signature")?;
    if !program
        .statement_table
        .statements(state.statement_nodes)
        .is_empty()
    {
        return Err("boundary signature state contains executable statements");
    }
    // A static qualifier retains nominal ownership through the callable's
    // declaration identity, not a runtime data shape. Requiring a shape here
    // made `Opaque::operation(value)` demand a representation of Opaque even
    // when no argument or result contained it. All boundary storage and
    // custody come from the explicit signature, exactly as for checked bodies.
    let (structural_parameters, scalar_parameters) =
        free_structural_scalar_signature(program, shapes, state, &binders)
            .ok_or("boundary explicit parameters have no runtime signature")?;
    let domain_requirements = boundary_domain_requirements(
        program,
        shapes,
        program.state_parameters(state),
        program
            .machine_contracts(machine)
            .iter()
            .chain(program.state_contracts(state)),
        &structural_parameters,
        &scalar_parameters,
        &binders,
    )
    .ok_or("boundary domain requirements have no retained plan")?;
    let contract = facts
        .contract_plans
        .for_machine(machine.symbol)
        .ok_or("boundary declaration has no checked contract")?;
    let scalar_requires = boundary_scalar_requires(
        program,
        &facts.operators,
        machine.symbol,
        program.state_parameters(state),
        &structural_parameters,
        &scalar_parameters,
        &domain_requirements.structural,
        &domain_requirements.scalar,
        state.return_type,
        program.machine_contracts(machine),
        program.state_contracts(state),
        &contract.closed_scalar_values,
    )
    .ok_or("boundary scalar requirements have no retained plan")?;
    // The callable's checked reach can include target-family members. Its
    // empty interface state is not the machine's inferred execution closure.
    let machine_reach = facts
        .service_reaches
        .for_machine(machine.symbol)
        .ok_or("boundary declaration has no checked service reach")?;

    // An independent family containing only checked bodies is a portable
    // contract, not a foreign completion. Its selected member returns actual
    // checked custody through the ordinary call. Mixed/opaque families still
    // need the full host-output contract, independently of the selected target.
    let checked_family = program.target_machine_families.iter().any(|family| {
        family.canonical == machine.symbol
            && !family.members.is_empty()
            && family.members.iter().all(|member| {
                program
                    .machine_by_symbol(member.machine)
                    .is_some_and(|member| member.supplies_checked_body())
            })
    });
    let reference_outputs = if checked_family {
        Vec::new()
    } else {
        reference_outputs(program, program.state_parameters(state), &[])?
    };
    Ok(CheckedBoundaryMachinePlan {
        reference_outputs,
        machine: machine.symbol,
        kind: semantic_vocabulary::BoundaryRequirementKind::Machine,
        state: state.symbol,
        contract_owner: machine.symbol,
        attachment_type_identity: None,
        structural_parameters,
        scalar_parameters,
        result,
        domain_requirements: domain_requirements.structural,
        scalar_domain_requirements: domain_requirements.scalar,
        scalar_ensures: validation::derive_scalar_boundary_guarantees(
            program, facts, machine, contract,
        ),
        scalar_requires,
        contract_report_fingerprint: contract.report_fingerprint,
        contract_commitment: contract.commitment,
        contract_service_reach: facts
            .service_reaches
            .plan_for_machine(machine.symbol)
            .ok_or("boundary declaration has no retained service reach plan")?,
        service_reach: language_semantics::ServiceReachSummary {
            direct: machine_reach.inferred_direct,
            transitive: machine_reach.inferred_transitive,
        },
    })
}

/// Project the narrow static boundary-trait surface used by checked-adapter
/// dispatch. A trait requirement is not an attached machine and therefore
/// contributes no provider value or structural attachment.
/// The call shape of every instantiated selector-call slot
/// (`monomorphization::dynamic_slots`). Every implementation a selector can
/// reach is a checked body, so the slot's concrete formals take an ordinary
/// checked callee's parameter classification, not a foreign boundary
/// presentation; a formal or result the planners cannot present leaves the
/// slot unplanned, and calls naming it decline at their own plan.
pub(crate) fn build_dynamic_slot_plans(
    program: &dyn ::validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
) -> Vec<checked_trees::CheckedDynamicSlotPlan> {
    let mut plans = Vec::new();
    for slot in &program.dynamic_slots {
        let Some(definition) = program.trait_definition_by_symbol(slot.declaring_trait) else {
            continue;
        };
        let Some(requirement) =
            program.trait_machine_signature_by_symbol(definition, slot.requirement)
        else {
            continue;
        };
        let Some(result) = boundary_result_plan(program, shapes, slot.return_type, &[], &[]) else {
            continue;
        };
        let Some((structural_parameters, scalar_parameters)) =
            crate::execution::terminal_unit::calls::parameter_signatures(
                program,
                shapes,
                &slot.parameters,
                &[],
                &[],
                &crate::execution::terminal_unit::control::LocalConstructionTrace::default(),
            )
        else {
            continue;
        };
        // The instantiated requirement owns the complete disposal frontier.
        // An affine formal may contain linear descendants: its root
        // multiplicity alone cannot describe the claims every row receives.
        let entry_claim_paths = structural_parameters.iter().enumerate().try_fold(
            Vec::new(),
            |mut paths,
             (argument_index, parameter)|
             -> Option<Vec<checked_trees::CheckedDynamicClaimPath>> {
                if parameter.access != checked_trees::CheckedStructuralAccess::Owned {
                    return Some(paths);
                }
                let source = slot.parameters.get(parameter.position as usize)?;
                for claim in validation::linear_claim_frontier(program, source.type_reference) {
                    paths.push(checked_trees::CheckedDynamicClaimPath {
                        argument_index: u32::try_from(argument_index).ok()?,
                        path: validation::structural_claim_path(
                            program,
                            source.type_reference,
                            &claim.path,
                        )
                        .ok()?,
                    });
                }
                Some(paths)
            },
        );
        let Some(entry_claim_paths) = entry_claim_paths else {
            continue;
        };
        let Some(result_claim_paths) = validation::linear_claim_frontier(program, slot.return_type)
            .iter()
            .map(|claim| {
                validation::structural_claim_path(program, slot.return_type, &claim.path).ok()
            })
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        let Ok(result_reference_sources) =
            validation::reference_result_custody::declared_return_sources(
                program,
                &slot.parameters,
                slot.return_type,
            )
        else {
            continue;
        };
        plans.push(checked_trees::CheckedDynamicSlotPlan {
            slot: slot.symbol,
            interface_identity: slot.interface_identity.clone(),
            declaring_trait: slot.declaring_trait,
            requirement: slot.requirement,
            requirement_identity: program
                .trait_requirement_callable_identity(definition, requirement),
            structural_parameters,
            entry_claim_paths,
            scalar_parameters,
            result,
            result_claim_paths,
            result_reference_sources,
        });
    }
    plans
}

pub(crate) fn build_static_boundary_requirements(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    boundary_signature_reaches: &[(SymbolHandle, language_semantics::ServiceReachRowId)],
    gaps: &mut Vec<checked_trees::CheckedUnitPlanGap>,
    shapes: &mut ShapeCollector<'_>,
) -> Vec<CheckedBoundaryMachinePlan> {
    let mut plans = Vec::new();
    for definition in program
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
    {
        for signature in program.trait_machine_instances(definition) {
            // The declaration's outer telescope remains authored and open.
            // Prepared instances retain its exact application and substituted
            // formals; publish those instances without admitting the open head
            // or inferring arguments from a selected provider's ABI.
            if !program.trait_type_parameters(definition).is_empty()
                && !program
                    .closed_requirement_application(signature.symbol)
                    .is_some_and(|application| application.declaring_trait == definition.symbol)
            {
                gaps.push(crate::execution::terminal_unit::plan_gaps::plan_gap(
                    program,
                    signature.symbol,
                    crate::execution::terminal_unit::plan_gaps::GapSite::Machine,
                    checked_trees::CheckedUnitPlanGapReason::Unsupported(
                        "boundary requirement: open trait application",
                    ),
                ));
                continue;
            }
            let type_parameters = program.state_signature_type_parameters(signature);
            // A `machine` binder without an ABI `native callback` entry takes
            // its destination from a target-owned private slot. Psi retains
            // the source callback entry; the target calling policy supplies
            // and checks its placement in Omega.
            let nominal_use_backs_binder =
                |ordinal: usize, parameter: &typed_trees::data::TypeParameter| {
                    let typed_trees::data::TypeParameterKind::Machine {
                        contract:
                            typed_trees::data::MachineParameterContract::Nominal {
                                trait_definition,
                                requirement,
                                ..
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
                            && nominal_use.callback_entry.is_some()
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
            // Authored contracts were checked against the selected public
            // signature before executable planning. Publication retains that
            // exact contract commitment; it does not require every predicate
            // to be representable as a parameter qualification. Suspension and
            // blocking stay independent demands even when a provider is inline.
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
                    gaps.push(crate::execution::terminal_unit::plan_gaps::plan_gap(
                        program,
                        signature.symbol,
                        crate::execution::terminal_unit::plan_gaps::GapSite::Machine,
                        checked_trees::CheckedUnitPlanGapReason::Unsupported(
                            "boundary requirement: unresolved invocation telescope",
                        ),
                    ));
                    continue;
                };
                substitutions
            };
            let Some(result) =
                boundary_result_plan(program, shapes, signature.return_type, &[], &substitutions)
                    .and_then(|result| {
                        fold_boundary_result_domains(
                            program,
                            shapes,
                            signature,
                            signature.return_type,
                            &[],
                            result,
                        )
                    })
            else {
                gaps.push(crate::execution::terminal_unit::plan_gaps::plan_gap(
                    program,
                    signature.symbol,
                    crate::execution::terminal_unit::plan_gaps::GapSite::Machine,
                    checked_trees::CheckedUnitPlanGapReason::Unsupported(
                        "boundary requirement: result shape or domain authorization",
                    ),
                ));
                continue;
            };
            let Some((structural_parameters, scalar_parameters)) = parameter_signatures(
                program,
                shapes,
                program.state_signature_parameters(signature),
                &[],
                &substitutions,
                &LocalConstructionTrace::default(),
            ) else {
                gaps.push(crate::execution::terminal_unit::plan_gaps::plan_gap(
                    program,
                    signature.symbol,
                    crate::execution::terminal_unit::plan_gaps::GapSite::Machine,
                    checked_trees::CheckedUnitPlanGapReason::Unsupported(
                        "boundary requirement: parameter signature",
                    ),
                ));
                continue;
            };
            let Some(capsule) = facts
                .contract_plans
                .crash_capsule(definition.symbol, signature.symbol)
            else {
                gaps.push(crate::execution::terminal_unit::plan_gaps::plan_gap(
                    program,
                    signature.symbol,
                    crate::execution::terminal_unit::plan_gaps::GapSite::Machine,
                    checked_trees::CheckedUnitPlanGapReason::Unsupported(
                        "boundary requirement: checked contract commitment",
                    ),
                ));
                continue;
            };
            // A contract cannot depend on callers or selected provider reach.
            // The checking owner projected the declaration once, including its
            // nominal service and invocation promises, even when never called.
            let Some((_, published_reach)) = boundary_signature_reaches
                .iter()
                .find(|(requirement, _)| *requirement == signature.symbol)
            else {
                gaps.push(crate::execution::terminal_unit::plan_gaps::plan_gap(
                    program,
                    signature.symbol,
                    crate::execution::terminal_unit::plan_gaps::GapSite::Machine,
                    checked_trees::CheckedUnitPlanGapReason::Unsupported(
                        "boundary requirement: published service reach",
                    ),
                ));
                continue;
            };
            let service_reach = language_semantics::ServiceReachSummary {
                direct: *published_reach,
                transitive: *published_reach,
            };
            let Some(domain_requirements) = boundary_domain_requirements(
                program,
                shapes,
                program.state_signature_parameters(signature),
                program.state_signature_contracts(signature).iter(),
                &structural_parameters,
                &scalar_parameters,
                &[],
            ) else {
                gaps.push(crate::execution::terminal_unit::plan_gaps::plan_gap(
                    program,
                    signature.symbol,
                    crate::execution::terminal_unit::plan_gaps::GapSite::Machine,
                    checked_trees::CheckedUnitPlanGapReason::Unsupported(
                        "boundary requirement: structural precondition representation",
                    ),
                ));
                continue;
            };
            let closed = crate::facts::build_callable_scalar_value_contract_plan(
                program,
                signature.symbol,
                Some(program.state_signature_parameters(signature)),
                signature.return_type,
                program.state_signature_contracts(signature),
                &facts.operators,
            );
            let Some(scalar_requires) = boundary_scalar_requires(
                program,
                &facts.operators,
                signature.symbol,
                program.state_signature_parameters(signature),
                &structural_parameters,
                &scalar_parameters,
                &domain_requirements.structural,
                &domain_requirements.scalar,
                signature.return_type,
                program.state_signature_contracts(signature),
                &[],
                &closed,
            ) else {
                gaps.push(crate::execution::terminal_unit::plan_gaps::plan_gap(
                    program,
                    signature.symbol,
                    crate::execution::terminal_unit::plan_gaps::GapSite::Machine,
                    checked_trees::CheckedUnitPlanGapReason::Unsupported(
                        "boundary requirement: scalar precondition representation",
                    ),
                ));
                continue;
            };
            let scalar_ensures = closed
                .ensures()
                .iter()
                .flatten()
                .filter_map(|clause| match clause {
                    checked_trees::ClosedScalarContractValue::Predicate(predicate) => Some(
                        checked_trees::CheckedBoundaryScalarGuarantee::Predicate(predicate.clone()),
                    ),
                    // Source checking still establishes the complete guarantee.
                    // An unrepresentable consequence grants no Terminal fact.
                    _ => None,
                })
                .collect();
            plans.push(CheckedBoundaryMachinePlan {
                reference_outputs: match reference_outputs(
                    program,
                    program.state_signature_parameters(signature),
                    &substitutions,
                ) {
                    Ok(outputs) => outputs,
                    Err(reason) => {
                        gaps.push(crate::execution::terminal_unit::plan_gaps::plan_gap(
                            program,
                            signature.symbol,
                            crate::execution::terminal_unit::plan_gaps::GapSite::Machine,
                            checked_trees::CheckedUnitPlanGapReason::Unsupported(reason),
                        ));
                        continue;
                    }
                },
                machine: signature.symbol,
                kind: semantic_vocabulary::BoundaryRequirementKind::Trait,
                state: signature.symbol,
                contract_owner: definition.symbol,
                attachment_type_identity: None,
                structural_parameters,
                scalar_parameters,
                result,
                domain_requirements: domain_requirements.structural,
                scalar_domain_requirements: domain_requirements.scalar,
                scalar_ensures,
                scalar_requires,
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
    for operator in program
        .operators()
        .iter()
        .filter(|operator| operator.is_boundary)
    {
        if let Some(plan) = build_boundary_operator(program, facts, shapes, operator) {
            plans.push(plan);
        }
    }
    plans.sort_by_key(|plan| (plan.machine.arena_index(), plan.machine.generation()));
    plans.dedup_by_key(|plan| plan.machine);
    plans
}

/// Legacy operator syntax declares the same portable requirement as a
/// bodyless call. Its authored symbol owns the signature and contract; no
/// synthetic machine or selected provider may replace that identity.
fn build_boundary_operator(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    operator: &typed_trees::operator::OperatorDefinition,
) -> Option<CheckedBoundaryMachinePlan> {
    if !program.operator_type_parameters(operator).is_empty() {
        return None;
    }
    let capsule = facts
        .contract_plans
        .crash_capsule(operator.symbol, operator.symbol)?;
    let result = boundary_result_plan(program, shapes, operator.return_type, &[], &[])?;
    let (structural_parameters, scalar_parameters) = parameter_signatures(
        program,
        shapes,
        program.operator_parameters(operator),
        &[],
        &[],
        &LocalConstructionTrace::default(),
    )?;
    let domain_requirements = boundary_domain_requirements(
        program,
        shapes,
        program.operator_parameters(operator),
        program.operator_contracts(operator).iter(),
        &structural_parameters,
        &scalar_parameters,
        &[],
    )?;
    let closed = crate::facts::build_callable_scalar_value_contract_plan(
        program,
        operator.symbol,
        Some(program.operator_parameters(operator)),
        operator.return_type,
        program.operator_contracts(operator),
        &facts.operators,
    );
    let scalar_requires = boundary_scalar_requires(
        program,
        &facts.operators,
        operator.symbol,
        program.operator_parameters(operator),
        &structural_parameters,
        &scalar_parameters,
        &domain_requirements.structural,
        &domain_requirements.scalar,
        operator.return_type,
        program.operator_contracts(operator),
        &[],
        &closed,
    )?;
    // Operator declarations have no authored service-reach clause. A public
    // omission is an explicit empty ceiling, not private body inference.
    let service_reach = language_semantics::ServiceReachSummary {
        direct: language_semantics::ServiceReachRowTable::EMPTY_ROW,
        transitive: language_semantics::ServiceReachRowTable::EMPTY_ROW,
    };
    Some(CheckedBoundaryMachinePlan {
        reference_outputs: reference_outputs(program, program.operator_parameters(operator), &[])
            .ok()?,
        machine: operator.symbol,
        kind: semantic_vocabulary::BoundaryRequirementKind::Operator,
        state: operator.symbol,
        contract_owner: operator.symbol,
        attachment_type_identity: None,
        structural_parameters,
        scalar_parameters,
        result,
        domain_requirements: domain_requirements.structural,
        scalar_domain_requirements: domain_requirements.scalar,
        scalar_ensures: Vec::new(),
        scalar_requires,
        contract_report_fingerprint: capsule.target_contract_report_fingerprint(),
        contract_commitment: capsule.target_contract_commitment(),
        contract_service_reach: language_semantics::ServiceReachPlan {
            interface: language_semantics::ServiceReachInterface::PublishedCeiling(
                service_reach.transitive,
            ),
            checked_inferred: service_reach.transitive,
        },
        service_reach,
    })
}

/// Retain admitted completion separately from ordinary reference-origin
/// relations. In particular, an input lifetime does not locate provider backing.
fn reference_outputs(
    program: &dyn validation::CheckingInput,
    parameters: &[typed_trees::signature::StateParameter],
    substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
) -> Result<Vec<checked_trees::CheckedBoundaryReferenceOutput>, &'static str> {
    use checked_trees::{
        CheckedBoundaryLifetimeDependency as Dependency,
        CheckedBoundaryReferenceDestination as Destination, CheckedBoundaryReferenceOutput,
    };
    use validation::borrow_contracts::{ProviderOutputDestination, ViewReturnInputSource};
    let parameters = parameters
        .iter()
        .map(|parameter| {
            let mut parameter = parameter.clone();
            parameter.type_reference =
                substituted_formal_type(program, parameter.type_reference, substitutions);
            parameter
        })
        .collect::<Vec<_>>();
    let parameters = parameters.as_slice();
    let relation = validation::borrow_contracts::resolve_provider_outputs(program, parameters)
        .map_err(|error| match error {
            validation::borrow_contracts::ViewReturnAmbiguity::IncompleteStructure { .. } => {
                "boundary reference completion has an incomplete type or writable frontier"
            }
            validation::borrow_contracts::ViewReturnAmbiguity::ElidedMultipleInputs { .. } => {
                "boundary reference completion has ambiguous elided lifetime inputs"
            }
            validation::borrow_contracts::ViewReturnAmbiguity::LifetimeMatchesNoInput {
                ..
            } => "boundary reference completion lifetime has no matching input",
            validation::borrow_contracts::ViewReturnAmbiguity::IncompatibleSourceAccess {
                ..
            } => "boundary reference completion has incompatible lifetime access",
        })?;
    if relation.readiness != validation::borrow_contracts::BorrowRelationReadiness::Complete {
        return Err("boundary reference completion retains an open declaration frontier");
    }
    relation
        .outputs
        .into_iter()
        .map(|output| {
            let (destination, root) = match output.destination {
                ProviderOutputDestination::Parameter(position) => (
                    Destination::Parameter(
                        u32::try_from(position)
                            .map_err(|_| "boundary reference output position exceeds u32")?,
                    ),
                    validation::reference_shell(
                        program,
                        parameters
                            .get(position)
                            .ok_or("boundary reference output parameter is missing")?
                            .type_reference,
                    )
                    .ok_or("boundary reference output parameter has no reference shell")?
                    .referee,
                ),
            };
            let path = reference_output_path(program, root, &output.owner_path)?;
            let dependencies = output
                .dependencies
                .into_iter()
                .map(|dependency| {
                    let parameter_index = u32::try_from(dependency.parameter_index)
                        .map_err(|_| "boundary reference dependency position exceeds u32")?;
                    let access = dependency.access;
                    Ok(match dependency.source {
                        ViewReturnInputSource::Referent => {
                            if !dependency.source_path.is_empty() {
                                return Err(
                                    "boundary reference dependency has a projected outer referent",
                                );
                            }
                            Dependency::Referent {
                                parameter_index,
                                access,
                            }
                        }
                        ViewReturnInputSource::Stored => {
                            let root = validation::reference_shell(program, dependency.source_type)
                                .map_or(dependency.source_type, |shell| shell.referee);
                            Dependency::Stored {
                                parameter_index,
                                access,
                                path: reference_output_path(
                                    program,
                                    root,
                                    &dependency.source_path,
                                )?,
                            }
                        }
                    })
                })
                .collect::<Result<Vec<_>, &'static str>>()?;
            Ok(CheckedBoundaryReferenceOutput {
                destination,
                path,
                access: output.access,
                dependencies,
            })
        })
        .collect()
}

fn reference_output_path(
    program: &dyn validation::CheckingInput,
    root: typed_trees::types::TypeReferenceHandle,
    path: &[checked_trees::BorrowLoanOwnerSegment],
) -> Result<Vec<checked_trees::CheckedUnitStructuralPathSegment>, &'static str> {
    let segments = path
        .iter()
        .map(|segment| {
            Ok(match segment {
                checked_trees::BorrowLoanOwnerSegment::Field(symbol) => {
                    facts::PlaceSegment::Field { symbol: *symbol }
                }
                checked_trees::BorrowLoanOwnerSegment::Case(variant) => {
                    facts::PlaceSegment::Case { variant: *variant }
                }
                checked_trees::BorrowLoanOwnerSegment::FixedIndex(index) => {
                    facts::PlaceSegment::FixedIndex { index: *index }
                }
                checked_trees::BorrowLoanOwnerSegment::DynamicIndex => {
                    return Err("boundary reference output has an unresolved element path");
                }
            })
        })
        .collect::<Result<Vec<_>, &'static str>>()?;
    validation::structural_storage_path(program, root, &segments).map(|(path, _)| path)
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
    program: &dyn ::validation::CheckingInput,
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
            let call_site = validation::find_call_site(
                program,
                flow_state.machine_symbol,
                flow_state.state_symbol,
                call.statement_index,
                call.call_ordinal,
            )?;
            let site = match &call_site {
                validation::CallSite::Statement(_) => {
                    let state = validation::find_state_in_machine(
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
                validation::CallSite::Expression { expression, .. } => {
                    checked_trees::NominalMachineUseSite::Expression(*expression)
                }
                validation::CallSite::TransitionNamed { .. } => return None,
            };
            let specialization = facts
                .requirement_call_specializations
                .for_site(site, signature.symbol)?;
            // The authored static machine arguments the retained ordinal
            // indexes: validation filtered the `<>` argument list to
            // machine-typed members before recording `static_machine_ordinal`.
            let authored_machine_arguments = match &call_site {
                validation::CallSite::Statement(call) => call.machine_arguments.as_ref(),
                validation::CallSite::Expression { call, .. } => call.machine_arguments.as_ref(),
                validation::CallSite::TransitionNamed { .. } => &[],
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
    program: &dyn ::validation::CheckingInput,
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
    // A reference result publishes the same descriptor or permission carrier
    // as a checked-body result. The source call's captured lifetime relation
    // supplies loan custody; this signature neither creates backing storage
    // nor authorizes a provider. Keep carrier classification independent of
    // which admitted implementation will satisfy the requirement.
    if is_reference(program, type_reference) {
        let carrier = super::call_results::checked_structural_result_type(
            program,
            shapes,
            type_reference,
            binders,
        )?;
        return Some(CheckedBoundaryMachineResultPlan::Structural {
            type_identity: carrier.type_identity,
            multiplicity: carrier.multiplicity,
            qualifications: parameter_qualifications(program, shapes, type_reference, binders)?,
            projected_qualifications: projected_result_qualifications(
                program,
                shapes,
                type_reference,
                binders,
            )?,
        });
    }
    // Owned result storage is independent of its cleanup obligation. The
    // successful call transports the producer's actual claims; the receiving
    // body still owes their separately checked settlement.
    Some(CheckedBoundaryMachineResultPlan::Structural {
        type_identity: shapes.add_type(type_reference, binders, substitutions)?,
        multiplicity: crate::checks::type_multiplicity(program, type_reference),
        qualifications: parameter_qualifications(program, shapes, type_reference, binders)?,
        projected_qualifications: projected_result_qualifications(
            program,
            shapes,
            type_reference,
            binders,
        )?,
    })
}
