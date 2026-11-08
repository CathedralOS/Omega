/*
    Structural control and boundary-machine construction.

    This file owns the control plan entry point. `boundary_machine.rs` builds
    boundary machines and their static requirements, `checked_machine.rs`
    builds a checked machine (`construction_trace.rs` records where it stopped
    when it declines a body) and `call_results.rs` binds call results to unit
    locals.
*/

mod boundary_contracts {
    /*
        Boundary scalar contracts retain source obligations separately from structural
        qualification identity. The published parameter roster decides which lane
        owns a membership; a scalar theory must project completely or fail closed.
    */
    use symbols::SymbolHandle;

    /*
        Publish scalar obligations in the declaration's formal namespace. Only an
        exact structural qualification restatement belongs to the structural lane;
        scalar membership retains its complete predicate, including implicit entry
        clauses introduced by parameter qualifications. Provider bodies do not
        choose which part of the published contract survives.
    */
    pub(super) fn boundary_scalar_requires(
        program: &dyn ::validation::CheckingInput,
        operators: &checked_trees::CheckedOperatorFacts,
        owner: SymbolHandle,
        parameters: &[typed_trees::signature::StateParameter],
        structural_parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
        scalar_parameters: &[checked_trees::CheckedStructuralScalarParameterPlan],
        domain_requirements: &[checked_trees::CheckedUnitStructuralDomainRequirementPlan],
        scalar_domain_requirements: &[checked_trees::CheckedBoundaryScalarDomainRequirementPlan],
        result_type: typed_trees::types::TypeReferenceHandle,
        contracts: &[typed_trees::signature::SignatureContract],
        entry_contracts: &[typed_trees::signature::SignatureContract],
        closed: &checked_trees::ClosedScalarValueContractPlan,
    ) -> Option<Vec<checked_trees::ClosedScalarContractValue>> {
        use checked_trees::ClosedScalarContractValue;
        use typed_trees::domain::ProofFact;
        use typed_trees::signature::SignatureContractKind;

        let authored = contracts
            .iter()
            .filter(|clause| {
                clause.kind == SignatureContractKind::Requires && clause.binding.is_none()
            })
            .collect::<Vec<_>>();
        if authored.len() != closed.authored_requires().len() {
            return None;
        }
        let mut rows = Vec::new();
        let entries = entry_contracts
            .iter()
            .filter(|clause| clause.kind == SignatureContractKind::Requires);
        for (clause, row) in authored
            .into_iter()
            .zip(closed.authored_requires().iter().map(Option::as_ref))
            .chain(entries.map(|clause| (clause, None)))
        {
            if clause.binding.is_some() {
                return None;
            }
            if let Some(
                value @ (ClosedScalarContractValue::Predicate(_)
                | ClosedScalarContractValue::Boolean(_)
                | ClosedScalarContractValue::Integer(_)),
            ) = row
            {
                rows.push(value.clone());
                continue;
            }
            let mut remaining = 4096;
            let mut conjunction = None;
            for fact in program.proof_facts.span_or_empty(clause.facts) {
                let predicate = match fact {
                    ProofFact::Membership(membership) => {
                        if let Some(valid) = super::super::types::signature_progress_requirement(
                            program, parameters, membership,
                        ) {
                            if !valid {
                                return None;
                            }
                            continue;
                        }
                        let structural =
                            validation::single_segment_name_symbol(program, membership.value)
                                .and_then(|symbol| {
                                    parameters
                                        .iter()
                                        .position(|parameter| parameter.symbol == symbol)
                                })
                                .is_some_and(|position| {
                                    structural_parameters
                                        .iter()
                                        .position(|parameter| {
                                            parameter.position as usize == position
                                        })
                                        .is_some_and(|argument_index| {
                                            domain_requirements.iter().any(|requirement| {
                                                requirement.argument_index as usize
                                                    == argument_index
                                                    && requirement.domain
                                                        == membership.semantic_domain
                                            })
                                        })
                                });
                        if structural
                            && validation::scalar_membership_value_carrier(program, membership)
                                .is_none()
                        {
                            continue;
                        }
                        let retained_nominal = structural
                            || validation::single_segment_name_symbol(program, membership.value)
                                .and_then(|symbol| {
                                    parameters
                                        .iter()
                                        .position(|parameter| parameter.symbol == symbol)
                                })
                                .is_some_and(|position| {
                                    scalar_parameters
                                        .iter()
                                        .position(|parameter| {
                                            parameter.source_position as usize == position
                                        })
                                        .is_some_and(|scalar_position| {
                                            scalar_domain_requirements.iter().any(|requirement| {
                                                requirement.argument_index as usize
                                                    == scalar_position
                                                    && requirement.domain
                                                        == membership.semantic_domain
                                                    && requirement.domain_symbol
                                                        == membership.domain_symbol
                                            })
                                        })
                                });
                        let project = if retained_nominal {
                            crate::values::scalar::result_contract::lower_callable_scalar_membership_value
                        } else {
                            crate::values::scalar::result_contract::lower_callable_scalar_membership
                        };
                        project(
                            program,
                            operators,
                            owner,
                            parameters,
                            membership,
                            &mut remaining,
                        )?
                    }
                    ProofFact::Expression(expression) => {
                        crate::values::lower_callable_scalar_contract_predicate(
                            program,
                            operators,
                            owner,
                            parameters,
                            result_type,
                            *expression,
                            false,
                            &mut remaining,
                        )?
                    }
                    ProofFact::Proposition(_) => return None,
                };
                conjunction = Some(match conjunction {
                    None => predicate,
                    Some(left) => checked_trees::CheckedBooleanExpression::And {
                        left: Box::new(left),
                        right: Box::new(predicate),
                    },
                });
            }
            if let Some(predicate) = conjunction {
                rows.push(ClosedScalarContractValue::Predicate(predicate));
            }
        }
        for row in &closed.requires()[closed.authored_requires().len()..] {
            match row {
                Some(value @ ClosedScalarContractValue::Predicate(_)) => rows.push(value.clone()),
                Some(ClosedScalarContractValue::FloatRange(_)) => {}
                _ => return None,
            }
        }
        Some(rows)
    }

    /*
        Preserve the whole source contract while selected endpoints remain symbolic.
        Final rebuilding must lower every clause normally; these rows grant no proof.
    */
    pub(super) fn pending_recipe_requires(
        program: &dyn ::validation::CheckingInput,
        contracts: &[typed_trees::signature::SignatureContract],
        entry_contracts: &[typed_trees::signature::SignatureContract],
    ) -> Option<(
        Vec<typed_trees::signature::SignatureContract>,
        Vec<typed_trees::constant_recipes::ConstantRecipeHandle>,
    )> {
        let clauses = contracts
            .iter()
            .chain(entry_contracts)
            .filter(|clause| clause.kind == typed_trees::signature::SignatureContractKind::Requires)
            .cloned()
            .collect::<Vec<_>>();
        let mut pending = clauses
            .iter()
            .flat_map(|clause| program.proof_facts.span_or_empty(clause.facts))
            .filter_map(|fact| fact.as_expression().copied())
            .collect::<Vec<_>>();
        let mut recipes = Vec::new();
        let mut visited = Vec::new();
        while let Some(expression) = pending.pop() {
            if visited.contains(&expression) {
                continue;
            }
            visited.push(expression);
            let recipe = program.constant_recipe_for_contract_expression(expression);
            if recipe.is_valid()
                && program.closed_constant_recipe_value(recipe).is_none()
                && !recipes.contains(&recipe)
            {
                recipes.push(recipe);
            }
            validation::expression_traversal::children(
                program,
                program.expression_table.expression(expression),
                |child| pending.push(child),
            );
        }
        (!recipes.is_empty()).then_some((clauses, recipes))
    }
}
mod boundary_machine {
    /*
        Building boundary machines and their static boundary requirements.

        Source checking owns authored public contracts and provider refinement.
        Publication retains their exact checked commitment, runtime signature and
        custody; it does not recheck their source arrangement. Qualifying a result
        still requires its separately checked authorization route.
    */
    use super::super::calls::free_structural_scalar_entry_signature_traced;
    use super::boundary_contracts::boundary_scalar_requires;
    use super::boundary_contracts::pending_recipe_requires;
    use super::call_results::checked_structural_result_type;
    use checked_trees::CheckedBoundaryExecutionPlan;

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

    /*
        Select the execution owner while the declaration is still being checked.
        An application with the same name or shape never acquires a core route.
    */
    fn boundary_execution(
        program: &typed_trees::TypedTrees,
        machine: SymbolHandle,
    ) -> Result<CheckedBoundaryExecutionPlan, &'static str> {
        use typed_trees::typed_trees::{
            AccessExecutionDeclaration, AccessLifecycleOperation, EmptyRunReleaseDeclaration,
        };
        if let Some(declaration) = AccessExecutionDeclaration::authenticate(program, machine)
            .map_err(|_| "placement primitive declaration is malformed")?
        {
            return Ok(match declaration.operation() {
                AccessLifecycleOperation::Initialize => {
                    CheckedBoundaryExecutionPlan::AccessInitialize
                }
                AccessLifecycleOperation::MoveOut => CheckedBoundaryExecutionPlan::AccessMoveOut,
                AccessLifecycleOperation::View => CheckedBoundaryExecutionPlan::AccessView,
                AccessLifecycleOperation::Relocate => CheckedBoundaryExecutionPlan::AccessRelocate,
            });
        }
        if EmptyRunReleaseDeclaration::authenticate(program, machine)
            .map_err(|_| "empty-run release declaration is malformed")?
            .is_some()
        {
            return Ok(CheckedBoundaryExecutionPlan::EmptyRunRelease);
        }
        Ok(CheckedBoundaryExecutionPlan::Provider)
    }

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
        for (_, operation) in program.access_lifecycle_plans.iter() {
            if matches!(operation, typed_trees::typed_trees::AccessLifecyclePlan::Evaluated { callable, .. }
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
        if !crate::flow::state_statements(program, state.statement_nodes).is_empty() {
            return Err("boundary signature state contains executable statements");
        }
        let signature = if machine.target_family_interface {
            free_structural_scalar_entry_signature_traced(
                program,
                shapes,
                state,
                &binders,
                &LocalConstructionTrace::default(),
            )
        } else {
            free_structural_scalar_signature(program, shapes, state, &binders)
        };
        let (structural_parameters, scalar_parameters) =
            signature.ok_or("boundary explicit parameters have no runtime signature")?;
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
        );
        let (scalar_requires, pending_scalar_requires, pending_scalar_recipes) =
            match scalar_requires {
                Some(rows) => (rows, Vec::new(), Vec::new()),
                None => {
                    let (clauses, recipes) = pending_recipe_requires(
                        program,
                        program.machine_contracts(machine),
                        program.state_contracts(state),
                    )
                    .ok_or("boundary scalar requirements have no retained plan")?;
                    (Vec::new(), clauses, recipes)
                }
            };
        let machine_reach = facts
            .service_reaches
            .for_machine(machine.symbol)
            .ok_or("boundary declaration has no checked service reach")?;

        let checked_family = program.target_machine_families.iter().any(|family| {
            family.canonical == machine.symbol
                && !family.members.is_empty()
                && family.members.iter().all(|member| {
                    program
                        .machine_by_symbol(member.machine)
                        .is_some_and(|member| member.supplies_checked_body())
                })
        });
        let operation =
            validation::compiler_operations::static_interrupt_declaration(program, machine)?;
        let preserves_descriptors = operation.is_some_and(|operation| {
            operation.entry == state.symbol && operation.preserves_parameter_descriptors()
        });
        let reference_outputs = if checked_family || preserves_descriptors {
            Vec::new()
        } else {
            reference_outputs(program, program.state_parameters(state), &[])?
        };
        let result_reference_sources =
            match validation::reference_result_custody::installed_context_sources(
                program,
                state.symbol,
            )? {
                Some(sources) => sources,
                None => reference_result_sources(
                    program,
                    program.state_parameters(state),
                    state.return_type,
                    &[],
                )?,
            };
        Ok(CheckedBoundaryMachinePlan {
            execution: match operation {
                Some(operation) => {
                    CheckedBoundaryExecutionPlan::StaticInterrupt(operation.operation)
                }
                None => boundary_execution(program.typed_program(), machine.symbol)?,
            },
            reference_outputs,
            result_reference_sources,
            machine: machine.symbol,
            kind: semantic_vocabulary::BoundaryRequirementKind::Machine,
            state: state.symbol,
            contract_owner: machine.symbol,
            owning_service: None,
            structural_parameters,
            scalar_parameters,
            result,
            domain_requirements: domain_requirements.structural,
            normal_qualifications: domain_requirements.normal,
            scalar_domain_requirements: domain_requirements.scalar,
            scalar_ensures: validation::derive_scalar_boundary_guarantees(
                program, facts, machine, contract,
            ),
            scalar_requires,
            pending_scalar_requires,
            pending_scalar_recipes,
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

    /*
        Project the narrow static boundary-trait surface used by checked-adapter
        dispatch. A trait requirement is not an attached machine and therefore
        contributes no provider value or structural attachment.
        The call shape of every instantiated selector-call slot
        (`monomorphization::dynamic_slots`). Every implementation a selector can
        reach is a checked body, so the slot's concrete formals take an ordinary
        checked callee's parameter classification, not a foreign boundary
        presentation; a formal or result the planners cannot present leaves the
        slot unplanned, and calls naming it decline at their own plan.
    */
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
            let Some(result) = boundary_result_plan(program, shapes, slot.return_type, &[], &[])
            else {
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
            let Some(result_claim_paths) =
                validation::linear_claim_frontier(program, slot.return_type)
                    .iter()
                    .map(|claim| {
                        validation::structural_claim_path(program, slot.return_type, &claim.path)
                            .ok()
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
            let family_tuple = if let Some(application) =
                program.closed_requirement_application(slot.call_requirement)
            {
                let Some(tuple) = application
                    .local_arguments
                    .iter()
                    .map(|argument| {
                        argument.static_operand().and_then(|reference| {
                            program.static_const_type_reference_identity(reference)
                        })
                    })
                    .collect::<Option<Vec<_>>>()
                else {
                    continue;
                };
                tuple
            } else {
                Vec::new()
            };
            plans.push(checked_trees::CheckedDynamicSlotPlan {
                slot: slot.symbol,
                interface_identity: slot.interface_identity,
                declaring_trait: slot.declaring_trait,
                requirement: slot.requirement,
                requirement_identity: program
                    .trait_requirement_callable_identity(definition, requirement),
                family_tuple,
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

    /*
        One `Machine`-site planning gap for a boundary requirement, carrying the
        reason the requirement could not be published.
    */
    fn machine_gap(
        program: &dyn ::validation::CheckingInput,
        signature: &typed_trees::signature::StateSignature,
        message: &'static str,
    ) -> checked_trees::CheckedUnitPlanGap {
        crate::execution::terminal_unit::plan_gaps::plan_gap(
            program,
            signature.symbol,
            crate::execution::terminal_unit::plan_gaps::GapSite::Machine,
            checked_trees::CheckedUnitPlanGapReason::Unsupported(std::borrow::Cow::Borrowed(
                message,
            )),
        )
    }

    pub(crate) fn build_static_boundary_requirements(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        boundary_signature_reaches: &[(SymbolHandle, language_semantics::ServiceReachRowId)],
        gaps: &mut Vec<checked_trees::CheckedUnitPlanGap>,
        shapes: &mut ShapeCollector<'_>,
    ) -> Vec<CheckedBoundaryMachinePlan> {
        let mut plans = Vec::new();
        for definition in program.traits().iter() {
            let owning_service = if definition.is_boundary {
                program.service_reaches.id_for_symbol(definition.symbol)
            } else {
                None
            };
            for signature in program.executable_trait_machine_instances(definition) {
                if definition.is_boundary && owning_service.is_none() {
                    gaps.push(machine_gap(
                        program,
                        signature,
                        "boundary trait has no service identity",
                    ));
                    continue;
                }
                if !program.trait_type_parameters(definition).is_empty()
                    && !program
                        .closed_requirement_application(signature.symbol)
                        .is_some_and(|application| application.declaring_trait == definition.symbol)
                {
                    gaps.push(machine_gap(
                        program,
                        signature,
                        "boundary requirement: open trait application",
                    ));
                    continue;
                }
                let type_parameters = program.state_signature_type_parameters(signature);
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
                let substitutions = if type_parameters.is_empty() || callback_telescope {
                    Vec::new()
                } else {
                    let Some(substitutions) = specialized_signature_substitutions(
                        program,
                        facts,
                        signature,
                        type_parameters,
                    ) else {
                        gaps.push(machine_gap(
                            program,
                            signature,
                            "boundary requirement: unresolved invocation telescope",
                        ));
                        continue;
                    };
                    substitutions
                };
                let Some(result) = boundary_result_plan(
                    program,
                    shapes,
                    signature.return_type,
                    &[],
                    &substitutions,
                )
                .and_then(|result| {
                    fold_boundary_result_domains(
                        program,
                        shapes,
                        signature,
                        signature.return_type,
                        &[],
                        result,
                    )
                }) else {
                    gaps.push(machine_gap(
                        program,
                        signature,
                        "boundary requirement: result shape or domain authorization",
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
                    gaps.push(machine_gap(
                        program,
                        signature,
                        "boundary requirement: parameter signature",
                    ));
                    continue;
                };
                let Some(capsule) = facts
                    .contract_plans
                    .crash_capsule(definition.symbol, signature.symbol)
                else {
                    gaps.push(machine_gap(
                        program,
                        signature,
                        "boundary requirement: checked contract commitment",
                    ));
                    continue;
                };
                let Some((_, published_reach)) = boundary_signature_reaches
                    .iter()
                    .find(|(requirement, _)| *requirement == signature.symbol)
                else {
                    gaps.push(machine_gap(
                        program,
                        signature,
                        "boundary requirement: published service reach",
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
                    gaps.push(machine_gap(
                        program,
                        signature,
                        "boundary requirement: structural precondition representation",
                    ));
                    continue;
                };
                let closed = crate::published::facts::build_callable_scalar_value_contract_plan(
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
                    gaps.push(machine_gap(
                        program,
                        signature,
                        "boundary requirement: scalar precondition representation",
                    ));
                    continue;
                };
                let scalar_ensures = closed
                    .ensures()
                    .iter()
                    .flatten()
                    .filter_map(|clause| match clause {
                        checked_trees::ClosedScalarContractValue::Predicate(predicate) => {
                            Some(checked_trees::CheckedBoundaryScalarGuarantee::Predicate(
                                predicate.clone(),
                            ))
                        }
                        _ => None,
                    })
                    .collect();
                plans.push(CheckedBoundaryMachinePlan {
                    execution: CheckedBoundaryExecutionPlan::Provider,
                    result_reference_sources: match reference_result_sources(
                        program,
                        program.state_signature_parameters(signature),
                        signature.return_type,
                        &substitutions,
                    ) {
                        Ok(sources) => sources,
                        Err(reason) => {
                            gaps.push(machine_gap(program, signature, reason));
                            continue;
                        }
                    },
                    reference_outputs: match reference_outputs(
                        program,
                        program.state_signature_parameters(signature),
                        &substitutions,
                    ) {
                        Ok(outputs) => outputs,
                        Err(reason) => {
                            gaps.push(machine_gap(program, signature, reason));
                            continue;
                        }
                    },
                    machine: signature.symbol,
                    kind: semantic_vocabulary::BoundaryRequirementKind::Trait,
                    state: signature.symbol,
                    contract_owner: definition.symbol,
                    owning_service,
                    structural_parameters,
                    scalar_parameters,
                    result,
                    domain_requirements: domain_requirements.structural,
                    normal_qualifications: domain_requirements.normal,
                    scalar_domain_requirements: domain_requirements.scalar,
                    scalar_ensures,
                    scalar_requires,
                    pending_scalar_requires: Vec::new(),
                    pending_scalar_recipes: Vec::new(),
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

    /*
        Legacy operator syntax declares the same portable requirement as a
        bodyless call. Its authored symbol owns the signature and contract; no
        synthetic machine or selected provider may replace that identity.
    */
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
        let closed = crate::published::facts::build_callable_scalar_value_contract_plan(
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
        let service_reach = language_semantics::ServiceReachSummary {
            direct: language_semantics::ServiceReachRowTable::EMPTY_ROW,
            transitive: language_semantics::ServiceReachRowTable::EMPTY_ROW,
        };
        Some(CheckedBoundaryMachinePlan {
            execution: CheckedBoundaryExecutionPlan::Provider,
            result_reference_sources: reference_result_sources(
                program,
                program.operator_parameters(operator),
                operator.return_type,
                &[],
            )
            .ok()?,
            reference_outputs: reference_outputs(
                program,
                program.operator_parameters(operator),
                &[],
            )
            .ok()?,
            machine: operator.symbol,
            kind: semantic_vocabulary::BoundaryRequirementKind::Operator,
            state: operator.symbol,
            contract_owner: operator.symbol,
            owning_service: None,
            structural_parameters,
            scalar_parameters,
            result,
            domain_requirements: domain_requirements.structural,
            normal_qualifications: domain_requirements.normal,
            scalar_domain_requirements: domain_requirements.scalar,
            scalar_ensures: Vec::new(),
            scalar_requires,
            pending_scalar_requires: Vec::new(),
            pending_scalar_recipes: Vec::new(),
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

    /*
        A boundary retains the same public returned-borrow relation as a dynamic
        requirement. Its actual provider must still supply a captured descriptor.
    */
    fn reference_result_sources(
        program: &dyn validation::CheckingInput,
        parameters: &[typed_trees::signature::StateParameter],
        return_type: typed_trees::types::TypeReferenceHandle,
        substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
    ) -> Result<Vec<checked_trees::CheckedReturnedReferencePlan>, &'static str> {
        let parameters = parameters
            .iter()
            .map(|parameter| {
                let mut parameter = parameter.clone();
                parameter.type_reference =
                    substituted_formal_type(program, parameter.type_reference, substitutions);
                parameter
            })
            .collect::<Vec<_>>();
        validation::reference_result_custody::declared_return_sources(
            program,
            &parameters,
            substituted_formal_type(program, return_type, substitutions),
        )
    }

    /*
        Retain admitted completion separately from ordinary reference-origin
        relations. In particular, an input lifetime does not locate provider backing.
    */
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
                validation::borrow_contracts::ViewReturnAmbiguity::IncompleteStructure {
                    ..
                } => "boundary reference completion has an incomplete type or writable frontier",
                validation::borrow_contracts::ViewReturnAmbiguity::ElidedMultipleInputs {
                    ..
                } => "boundary reference completion has ambiguous elided lifetime inputs",
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

    /*
        Substitute a generic requirement's `Type` formals with the actuals its
        checked calls admitted. Every flow call that names this requirement must
        retain a `requirement_call_specializations` row at its authored site that
        resolves the whole telescope: each `Type` binder carries its derived
        actual and each `machine` binder's retained selection is a concrete
        selected machine — exactly the authored static argument at that ordinal,
        mirroring the coverage `build_call_operation` requires before it plans
        the call. A forwarded in-scope binder or a missing row leaves the
        requirement unplanned. One boundary plan serves every call site, so the
        retained `Type` bindings must agree; selections may differ because the
        plan records no provider identity.
    */
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
                let authored_machine_arguments = match &call_site {
                    validation::CallSite::Statement(call) => call.machine_arguments.as_ref(),
                    validation::CallSite::Expression { call, .. } => {
                        call.machine_arguments.as_ref()
                    }
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
                                        .is_some_and(|argument| {
                                            argument.symbol == selection.selected
                                        })
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
        if is_reference(program, type_reference) {
            let carrier = checked_structural_result_type(program, shapes, type_reference, binders)?;
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
        Some(CheckedBoundaryMachineResultPlan::Structural {
            type_identity: shapes.add_type(type_reference, binders, substitutions)?,
            multiplicity: validation::reference_result_custody::result_multiplicity(
                program,
                type_reference,
            ),
            qualifications: parameter_qualifications(program, shapes, type_reference, binders)?,
            projected_qualifications: projected_result_qualifications(
                program,
                shapes,
                type_reference,
                binders,
            )?,
        })
    }
}
mod call_occurrences {

    /*
        Scalar operand calls belong to computation roots; structural operands retain
        their own result producers within the same authored statement.
        Assignment selectors and the stored value own their calls through those
        exact roots, independently of the destination's spelling. This roster only
        accounts for the calls: the store planner and emitter still establish the
        destination and evaluate selectors before the value, once in authored order.
    */
    use super::{CheckFacts, StatementNode, SymbolHandle};
    use crate::execution::terminal_unit::types::{ShapeCollector, is_unit, machine_binders};

    use crate::execution::terminal_unit::control::LocalConstructionTrace;
    use crate::execution::terminal_unit::control::call_results::checked_structural_result_type;

    use checked_trees::CheckedScalarComputationHandle;

    struct ComputationVisit {
        computation: CheckedScalarComputationHandle,
        completed: bool,
    }

    /*
        All consumers of one statement share exact computation occurrence custody.
        A retained value can be reached through several roles without executing its
        child twice. Different computations still cannot consume one captured call.
    */
    #[derive(Default)]
    struct CallOccurrenceLedger<'prefix> {
        prefix: Option<&'prefix CallOccurrencePrefix>,
        calls: Vec<arena::Handle<checked_trees::FlowCallFact>>,
        accounted_computations: Vec<AccountedComputation>,
        pending_minima: Vec<Option<u32>>,
    }

    /// Immutable accounting of the common subject/pattern prefix. Branch-local
    /// operands borrow it without copying the statement's call inventory.
    struct CallOccurrencePrefix {
        calls: Vec<arena::Handle<checked_trees::FlowCallFact>>,
        call_keys: std::collections::BTreeSet<(u32, u32)>,
        computations: std::collections::BTreeMap<(u32, u32, usize, u32, u32), Option<u32>>,
    }

    struct AccountedComputation {
        state: SymbolHandle,
        statement: usize,
        computation: CheckedScalarComputationHandle,
        /*
            Smallest actual Source-call ordinal in this exact immutable subtree.
            Pure computations have no call ordinal to constrain a consumer.
        */
        minimum_call_ordinal: Option<u32>,
    }

    impl CallOccurrenceLedger<'_> {
        fn record_minimum(&mut self, minimum: Option<u32>) {
            if let Some(minimum) = minimum {
                for pending in &mut self.pending_minima {
                    *pending = Some(pending.map_or(minimum, |prior| prior.min(minimum)));
                }
            }
        }
        fn contains(&self, call: &arena::Handle<checked_trees::FlowCallFact>) -> bool {
            self.calls.contains(call)
                || self.prefix.as_ref().is_some_and(|prefix| {
                    prefix
                        .call_keys
                        .contains(&(call.arena_index(), call.generation()))
                })
        }
        fn push(&mut self, call: arena::Handle<checked_trees::FlowCallFact>) {
            self.calls.push(call);
        }
        /*
            Record one call occurrence once: a walk that reaches the same authored
            call twice records it at its first position and drops the repeat.
        */
        fn record_once(&mut self, call: arena::Handle<checked_trees::FlowCallFact>) {
            if !self.contains(&call) {
                self.calls.push(call);
            }
        }
        fn iter(&self) -> impl Iterator<Item = &arena::Handle<checked_trees::FlowCallFact>> {
            self.prefix
                .iter()
                .flat_map(|prefix| prefix.calls.iter())
                .chain(self.calls.iter())
        }
        fn accounted_minimum(
            &self,
            state: SymbolHandle,
            statement: usize,
            computation: CheckedScalarComputationHandle,
        ) -> Option<Option<u32>> {
            self.accounted_computations
                .iter()
                .find_map(|entry| {
                    (entry.state == state
                        && entry.statement == statement
                        && entry.computation == computation)
                        .then_some(entry.minimum_call_ordinal)
                })
                .or_else(|| {
                    self.prefix
                        .as_ref()?
                        .computations
                        .get(&(
                            state.arena_index(),
                            state.generation(),
                            statement,
                            computation.arena_index(),
                            computation.generation(),
                        ))
                        .copied()
                })
        }
    }

    /*
        An unconditional machine invocation completes this body after
        its ordinary call operation. Internal state destinations remain control
        edges; guarded invocations must not be hoisted into this linear sequence.
        Return the authored identity to join captured occurrences; the call builder
        separately resolves the callee's executable entry and signature.
    */
    mod collecting {

        /*
            Which calls a statement's expressions contain: the recursive collector and
            its per-kind operand walks, plus the dispatch-subject and
            statement-computation ownership questions the walk answers.
        */
        use super::super::{
            CheckFacts, CheckedScalarExpressionRole as ExpressionRole, StatementNode, SymbolHandle,
        };
        use super::ComputationVisit;
        use crate::execution::terminal_unit::control::structural_operands;

        use checked_trees::{
            CheckedScalarComputationHandle, CheckedScalarComputationKind as ComputationKind,
        };

        /*
            Assignment targets and scalar values own their computation calls before
            the separately scheduled whole-value call. Both the statement sequencer
            and that call's operand owner consume the same exact occurrence ledger.
        */
        pub(super) fn collect_assignment_calls(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            machine: SymbolHandle,
            state: &typed_trees::state::State,
            statement_index: usize,
            assignment: &typed_trees::statement::TableAssignment,
            calls: &[checked_trees::FlowCallFact],
            consumed: &mut super::CallOccurrenceLedger<'_>,
        ) -> Option<()> {
            let owner = program.machine_by_symbol(machine)?;
            let plans = &facts.values.scalar_computations;
            let selectors = validation::assignment_target_selectors(program, assignment.target);
            let values =
                if let Some(carrier) = validation::atomic_assignment_carrier(program, assignment) {
                    carrier
                        .operands
                        .into_iter()
                        .enumerate()
                        .map(|(ordinal, operand)| {
                            Some((
                                ExpressionRole::AtomicOperand {
                                    operand_ordinal: u32::try_from(ordinal).ok()?,
                                },
                                operand,
                            ))
                        })
                        .collect::<Option<Vec<_>>>()?
                } else {
                    vec![(ExpressionRole::AssignmentValue, assignment.value)]
                };
            let operands = selectors
                .into_iter()
                .enumerate()
                .map(|(depth, expression)| {
                    Some((
                        ExpressionRole::AssignmentIndex {
                            depth: u32::try_from(depth).ok()?,
                        },
                        expression,
                    ))
                })
                .chain(values.into_iter().map(Some));
            for operand in operands {
                let (role, expression) = operand?;
                let Some(root) =
                    plans.root_at(state.symbol, u32::try_from(statement_index).ok()?, role)
                else {
                    continue;
                };
                let primitive = match role {
                    ExpressionRole::AssignmentIndex { .. } => {
                        typed_trees::types::PrimitiveType::U64
                    }
                    ExpressionRole::AssignmentValue | ExpressionRole::AtomicOperand { .. } => {
                        let reference = validation::declared_place_type_raw(
                            program,
                            owner,
                            Some(state),
                            assignment.target,
                        )?;
                        crate::values::assignment_target_primitive_type(program, reference)?
                    }
                    _ => return None,
                };
                if !plans.is_authored_root(root, machine, expression, primitive) {
                    return None;
                }
                collect_root_calls(
                    program,
                    facts,
                    state.symbol,
                    statement_index,
                    root.root,
                    calls,
                    0,
                    consumed,
                )?;
            }

            Some(())
        }

        /*
            Constructors retain their scalar evaluation roots independently of the
            surrounding schedule. Ordinary statements and selected call arms consume
            the same exact element, field, subject and pattern occurrence custody.
        */
        #[allow(clippy::too_many_arguments)]
        pub(super) fn collect_constructor_calls(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            machine: SymbolHandle,
            state: &typed_trees::state::State,
            statement_index: usize,
            statement: &StatementNode,
            calls: &[checked_trees::FlowCallFact],
            consumed: &mut super::CallOccurrenceLedger<'_>,
        ) -> Option<()> {
            let construction_destination = match statement {
                StatementNode::LocalData(local) if !local.is_mutable => {
                    Some((local.initial_value, local.type_reference))
                }
                StatementNode::Expression(expression) => Some((*expression, state.return_type)),
                _ => None,
            };
            let statement_ordinal = u32::try_from(statement_index).ok()?;
            for root in facts
                .values
                .scalar_computations
                .roots_for_statement(state.symbol, statement_ordinal)
                .into_iter()
                .filter(|root| {
                    matches!(
                        root.role,
                        ExpressionRole::StructuralValueField { .. }
                            | ExpressionRole::RecordField { .. }
                            | ExpressionRole::StructuralArrayElement { .. }
                            | ExpressionRole::StructuralScalarValue { .. }
                            | ExpressionRole::StructuralValueSubject { .. }
                            | ExpressionRole::StructuralValuePattern { .. }
                    )
                })
            {
                collect_owned_root_calls(
                    program,
                    facts,
                    machine,
                    state.symbol,
                    statement_index,
                    root,
                    calls,
                    consumed,
                )?;
            }
            let constructions =
                construction_destination
                    .into_iter()
                    .map(|(expression, expected)| {
                        (
                            checked_trees::CheckedArrayConstructionSource::Statement,
                            expression,
                            expected,
                        )
                    });
            for (source, expression, expected) in constructions {
                let Some(elements) =
                    validation::scalar_array_elements(program, machine, expression, expected)
                else {
                    continue;
                };
                let plans = &facts.values.scalar_computations;
                for (element_index, element) in elements.elements.into_iter().enumerate() {
                    let (expression, primitive_type) = match element {
                        validation::ScalarArrayElement::Authored {
                            expression,
                            primitive_type,
                        }
                        | validation::ScalarArrayElement::Projected {
                            row: expression,
                            primitive_type,
                            ..
                        } => (expression, primitive_type),
                    };
                    let role = ExpressionRole::ArrayElement {
                        source,
                        element_ordinal: u32::try_from(element_index).ok()?,
                    };
                    let Some(root) =
                        plans.root_at(state.symbol, u32::try_from(statement_index).ok()?, role)
                    else {
                        continue;
                    };
                    if !plans.is_authored_root(root, machine, expression, primitive_type) {
                        return None;
                    }
                    collect_root_calls(
                        program,
                        facts,
                        state.symbol,
                        statement_index,
                        root.root,
                        calls,
                        0,
                        consumed,
                    )?;
                }
            }
            Some(())
        }

        pub(super) fn statement_computation_prefix(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            machine: SymbolHandle,
            state: SymbolHandle,
            statement_index: usize,
            calls: &[checked_trees::FlowCallFact],
        ) -> Option<(super::CallOccurrenceLedger<'static>, Option<usize>)> {
            let mut consumed = super::CallOccurrenceLedger::default();
            let owner = program.machine_by_symbol(machine)?;
            let source_state = program
                .machine_states(owner)
                .iter()
                .find(|source| source.symbol == state)?;
            let mut guard_maximum = None;
            if let Some(StatementNode::Transition(transition)) = program
                .statement_table
                .statement_at(source_state, statement_index)
                && let typed_trees::statement::TransitionGuardNode::When(expression) =
                    transition.guard
            {
                let plans = &facts.values.scalar_computations;
                let statement_ordinal = u32::try_from(statement_index).ok()?;
                let mut roots =
                    plans.roots_for_statement_role(state, statement_ordinal, ExpressionRole::Guard);
                if let Some(root) = roots.next() {
                    if roots.next().is_some()
                        || !plans.is_authored_root(
                            root,
                            machine,
                            expression,
                            typed_trees::types::PrimitiveType::Bool,
                        )
                    {
                        return None;
                    }
                    collect_root_calls(
                        program,
                        facts,
                        state,
                        statement_index,
                        root.root,
                        calls,
                        0,
                        &mut consumed,
                    )?;
                    if consumed.iter().any(|handle| {
                        let guard_call = facts.flow.control.calls.get(*handle);
                        validation::transition_call_target(
                            program,
                            owner,
                            source_state,
                            statement_index,
                            guard_call.call_ordinal,
                        )
                        .is_none_or(|target| target.is_valid())
                    }) {
                        return None;
                    }
                    guard_maximum = consumed
                        .iter()
                        .map(|handle| facts.flow.control.calls.get(*handle).call_ordinal)
                        .max();
                }
            }
            let statement = program
                .statement_table
                .statement_at(source_state, statement_index)?;
            collect_constructor_calls(
                program,
                facts,
                machine,
                source_state,
                statement_index,
                statement,
                calls,
                &mut consumed,
            )?;
            if let Some(assignment) = statement.as_assignment() {
                collect_assignment_calls(
                    program,
                    facts,
                    machine,
                    source_state,
                    statement_index,
                    assignment,
                    calls,
                    &mut consumed,
                )?;
            }
            Some((consumed, guard_maximum))
        }

        /// A separately scheduled invocation owns its scalar operand calls through
        /// the same computation roots as an invocation in the statement sequence.
        /// A transition guard retains its own earlier computation. The two schedules
        /// share an occurrence ledger so neither can claim a call the other executes.
        /// Remaining occurrences must belong to this invocation's arm: a sibling
        /// arm's calls run only when this arm does not, and a structural argument
        /// owns the calls nested under its value, including a qualification cast.
        pub(in crate::execution::terminal_unit) fn call_owns_statement_computations(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            machine: SymbolHandle,
            state: SymbolHandle,
            call: &checked_trees::FlowCallFact,
            calls: &[checked_trees::FlowCallFact],
            predecessors: &[checked_trees::CheckedUnitEffectOperationPlan],
            selected: Option<&mut super::SelectedInvocationAccounting>,
        ) -> Option<()> {
            if facts.flow.control.is_retired(state, call)
                || !match selected.as_deref() {
                    Some(selected) => selected.contains_leaf(facts, machine, state, call, calls),
                    None => calls.iter().any(|candidate| std::ptr::eq(candidate, call)),
                }
            {
                return None;
            }
            let site = validation::find_call_site(
                program,
                machine,
                state,
                call.statement_index,
                call.call_ordinal,
            )?;
            let arguments = validation::call_site_argument_expressions(program, &site);
            let owner = program.machine_by_symbol(machine)?;
            let source_state = program.state_in_machine(owner, state)?;
            let (mut consumed, guard_maximum) = match selected.as_deref() {
                Some(selected) => selected.operand_ledger(),
                None => statement_computation_prefix(
                    program,
                    facts,
                    machine,
                    state,
                    call.statement_index,
                    calls,
                )?,
            };
            if guard_maximum.is_some_and(|ordinal| ordinal >= call.call_ordinal) {
                return None;
            }
            for nested in structural_operands::for_call(program, facts, owner, source_state, call)?
            {
                if !match selected.as_deref() {
                    Some(selected) => selected.contains_call(facts, nested),
                    None => calls.iter().any(|source| std::ptr::eq(source, nested)),
                } {
                    return None;
                }
                let handle = facts.flow.control.calls.handle_of(nested)?;
                if nested.statement_index != call.statement_index
                    || nested.call_ordinal <= call.call_ordinal
                    || facts.flow.control.is_retired(state, nested)
                    || consumed.contains(&handle)
                {
                    return None;
                }
                consumed.push(handle);
                let nested_site = validation::find_call_site(
                    program,
                    machine,
                    state,
                    nested.statement_index,
                    nested.call_ordinal,
                )?;
                collect_argument_calls(
                    program,
                    facts,
                    machine,
                    state,
                    nested,
                    validation::call_site_argument_expressions(program, &nested_site),
                    calls,
                    &mut consumed,
                )?;
            }
            collect_argument_calls(
                program,
                facts,
                machine,
                state,
                call,
                arguments,
                calls,
                &mut consumed,
            )?;
            consume_operand_value_calls(
                program,
                facts,
                machine,
                state,
                call,
                arguments,
                calls,
                &mut consumed,
            )?;
            if selected.is_some() {
                if selected.as_deref()?.operands_reschedule_producer(
                    program,
                    facts,
                    call,
                    &consumed.calls,
                    predecessors,
                )? {
                    return None;
                }
                // Release the borrowed common prefix before joining branch-local work.
                let calls = consumed.calls;
                return selected?.account(facts, call, &calls);
            }
            let scheduled_subject = |candidate: &checked_trees::FlowCallFact| {
                scheduled_dispatch_subject(
                    program,
                    facts,
                    owner,
                    source_state,
                    call,
                    candidate,
                    calls,
                    predecessors,
                )
            };
            if consumed
                .iter()
                .any(|handle| scheduled_subject(facts.flow.control.calls.get(*handle)))
            {
                return None;
            }
            let active_arm = validation::transition_call_target(
                program,
                owner,
                source_state,
                call.statement_index,
                call.call_ordinal,
            );
            calls
                .iter()
                .filter(|candidate| candidate.statement_index == call.statement_index)
                .all(|candidate| {
                    std::ptr::eq(candidate, call)
                        || scheduled_subject(candidate)
                        || scheduled_by_active_arm(candidate, predecessors)
                        || other_transition_arm(program, owner, source_state, active_arm, candidate)
                        || facts.flow.control.is_retired(state, candidate)
                        || program
                            .symbols
                            .builtin_function_for_symbol(candidate.target_symbol)
                            .is_some_and(|function| function.has_empty_write_frame())
                        || consumed.iter().any(|handle| {
                            std::ptr::eq(facts.flow.control.calls.get(*handle), candidate)
                        })
                })
                .then_some(())
        }

        /*
            Structural arguments own the calls inside their value, not only a call
            written as the argument expression itself. A qualification cast is a
            whole-value projection over the operand call; that call still executes
            on this arm.
        */
        fn consume_operand_value_calls(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            machine: SymbolHandle,
            state: SymbolHandle,
            call: &checked_trees::FlowCallFact,
            arguments: &[typed_trees::expression::ExpressionHandle],
            calls: &[checked_trees::FlowCallFact],
            consumed: &mut super::CallOccurrenceLedger<'_>,
        ) -> Option<()> {
            let statement_ordinal = u32::try_from(call.statement_index).ok()?;
            let minimum = u32::try_from(call.call_ordinal.checked_add(1)?).ok()?;
            let plans = &facts.values.structural_values;
            for argument in arguments {
                let Some(root) = plans.root_for_expression(state, statement_ordinal, *argument)
                else {
                    continue;
                };
                if root.machine != machine || !plans.nodes.is_valid(root.root) {
                    return None;
                }
                if !operand_value_nests_calls(&plans.nodes.get(root.root).kind) {
                    continue;
                }
                super::structural_values::collect(
                    program,
                    facts,
                    machine,
                    state,
                    call.statement_index,
                    root.root,
                    calls,
                    minimum,
                    consumed,
                    &mut Vec::new(),
                )?;
            }
            Some(())
        }

        /*
            Kinds whose children can carry a call this argument evaluates. A place
            or copied leaf has no nested invocation; its selectors stay on the scalar
            argument roster.
        */
        fn operand_value_nests_calls(kind: &checked_trees::CheckedStructuralValueKind) -> bool {
            match kind {
                checked_trees::CheckedStructuralValueKind::Reference { source_value, .. } => {
                    source_value.is_valid()
                }
                checked_trees::CheckedStructuralValueKind::Projection { .. }
                | checked_trees::CheckedStructuralValueKind::QuotientAdaptation { .. }
                | checked_trees::CheckedStructuralValueKind::QuotientCall { .. }
                | checked_trees::CheckedStructuralValueKind::Record { .. }
                | checked_trees::CheckedStructuralValueKind::StructuralCase { .. }
                | checked_trees::CheckedStructuralValueKind::FixedArray { .. }
                | checked_trees::CheckedStructuralValueKind::Dispatch { .. }
                | checked_trees::CheckedStructuralValueKind::ViewSubslice { .. }
                | checked_trees::CheckedStructuralValueKind::Call { .. } => true,
                _ => false,
            }
        }

        /// The active arm's preparation already scheduled this call, including a
        /// call retained inside a structural value rather than as its own operation.
        pub(super) fn scheduled_by_active_arm(
            candidate: &checked_trees::FlowCallFact,
            predecessors: &[checked_trees::CheckedUnitEffectOperationPlan],
        ) -> bool {
            predecessors.iter().any(|operation| {
                operation.with_value_calls().any(|scheduled| {
                    scheduled.call().is_some_and(|planned| {
                        planned.coordinate.statement_index as usize == candidate.statement_index
                            && planned.coordinate.call_ordinal as usize == candidate.call_ordinal
                    })
                })
            })
        }

        /// A call that completes or evaluates a different arm of this transition.
        /// Guard calls keep the zero target and stay on every arm's roster.
        pub(super) fn other_transition_arm(
            program: &dyn ::validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            active_arm: Option<typed_trees::statement::TransitionTargetHandle>,
            candidate: &checked_trees::FlowCallFact,
        ) -> bool {
            let Some(active_arm) = active_arm else {
                return false;
            };
            if !active_arm.is_valid() {
                return false;
            }
            validation::transition_call_target(
                program,
                machine,
                state,
                candidate.statement_index,
                candidate.call_ordinal,
            )
            .is_some_and(|arm| arm.is_valid() && arm != active_arm)
        }

        /*
            Join a saved dispatch call or operand read to its one already scheduled
            producer. Exact selection occurrence custody distinguishes parser copies
            from independently authored calls with identical targets and arguments.
        */
        pub(super) fn scheduled_dispatch_subject(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            invocation: &checked_trees::FlowCallFact,
            candidate: &checked_trees::FlowCallFact,
            calls: &[checked_trees::FlowCallFact],
            predecessors: &[checked_trees::CheckedUnitEffectOperationPlan],
        ) -> bool {
            super::dispatch_producers::authenticated_producer(
                program,
                facts,
                state,
                candidate,
                |statement, expression| {
                    let mut producers = calls.iter().filter(|producer| {
                        producer.statement_index == statement
                            && producer.authored_expression == expression
                    });
                    let producer = producers.next()?;
                    producers.next().is_none().then_some(producer)
                },
                |producer| {
                    scheduled_owner_count(
                        program,
                        facts,
                        machine,
                        state,
                        producer,
                        calls,
                        predecessors,
                    )
                },
            )
            .is_some_and(|producer| {
                producer < (invocation.statement_index, invocation.call_ordinal)
            })
        }

        fn scheduled_owner_count(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            producer: &checked_trees::FlowCallFact,
            calls: &[checked_trees::FlowCallFact],
            predecessors: &[checked_trees::CheckedUnitEffectOperationPlan],
        ) -> Option<usize> {
            let mut owners = 0;
            for operation in predecessors {
                let Some(planned) = operation.call() else {
                    continue;
                };
                let mut occurrences = calls.iter().filter(|candidate| {
                    candidate.statement_index == planned.coordinate.statement_index as usize
                        && candidate.call_ordinal == planned.coordinate.call_ordinal as usize
                });
                let prior = occurrences.next()?;
                if occurrences.next().is_some()
                    || facts.flow.control.is_retired(state.symbol, prior)
                {
                    return None;
                }
                if std::ptr::eq(prior, producer) {
                    owners += 1;
                    continue;
                }
                if prior.statement_index != producer.statement_index {
                    continue;
                }
                // Scalar operand invocations execute in the preceding call's retained
                // computation roots rather than as separate structural operations.
                let site = validation::find_call_site(
                    program,
                    machine.symbol,
                    state.symbol,
                    prior.statement_index,
                    prior.call_ordinal,
                )?;
                let mut consumed = super::CallOccurrenceLedger::default();
                collect_argument_calls(
                    program,
                    facts,
                    machine.symbol,
                    state.symbol,
                    prior,
                    validation::call_site_argument_expressions(program, &site),
                    calls,
                    &mut consumed,
                )?;
                if consumed
                    .iter()
                    .any(|handle| std::ptr::eq(facts.flow.control.calls.get(*handle), producer))
                {
                    owners += 1;
                }
            }
            Some(owners)
        }

        pub(super) fn collect_argument_calls(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            machine: SymbolHandle,
            state: SymbolHandle,
            call: &checked_trees::FlowCallFact,
            arguments: &[typed_trees::expression::ExpressionHandle],
            calls: &[checked_trees::FlowCallFact],
            consumed: &mut super::CallOccurrenceLedger<'_>,
        ) -> Option<()> {
            let register_operation = program
                .symbols
                .builtin_function_for_symbol(call.target_symbol)
                .and_then(|builtin| {
                    language_core::inline_assembly::AsmRegisterOperation::from_intrinsic_name(
                        builtin.name(),
                    )
                });
            let (scalar_arguments, structural_arguments) = if let Some(operation) =
                register_operation
            {
                if arguments.len() != operation.input_carriers().len() {
                    return None;
                }
                let scalar = arguments
                    .iter()
                    .zip(operation.input_carriers())
                    .map(|(argument, name)| {
                        Some((
                            *argument,
                            typed_trees::types::PrimitiveType::from_name(name.name())?,
                        ))
                    })
                    .collect::<Option<Vec<_>>>()?;
                (scalar, Vec::new())
            } else {
                if structural_operands::has_register_operands_only(program, call.target_symbol) {
                    return Some(());
                }
                let parameters = validation::call_target_parameters(program, call.target_symbol)?;
                let formals = parameters.iter().collect::<Vec<_>>();
                if arguments.len() != formals.len() {
                    return None;
                }
                let mut structural_ordinal = 0u32;
                let mut structural_arguments = Vec::new();
                for (argument, parameter) in arguments.iter().zip(formals.iter()) {
                    if crate::execution::terminal_unit::strips_erased_parameter(parameter)?
                        || program
                            .primitive_type_reference(parameter.type_reference)
                            .is_some()
                    {
                        continue;
                    }
                    structural_arguments.push(*argument);
                    collect_range_calls(
                        program,
                        facts,
                        machine,
                        state,
                        call.statement_index,
                        *argument,
                        checked_trees::CheckedSubsliceSite::CallArgument {
                            call_ordinal: u32::try_from(call.call_ordinal).ok()?,
                            argument_ordinal: structural_ordinal,
                        },
                        calls,
                        u32::try_from(call.call_ordinal.checked_add(1)?).ok()?,
                        consumed,
                    )?;
                    structural_ordinal = structural_ordinal.checked_add(1)?;
                }
                let scalar_arguments = arguments
                    .iter()
                    .zip(formals)
                    .filter_map(|(argument, parameter)| {
                        program
                            .primitive_type_reference(parameter.type_reference)
                            .map(|primitive| (*argument, primitive))
                    })
                    .collect::<Vec<_>>();
                (scalar_arguments, structural_arguments)
            };
            let mut roles = Vec::new();
            let mut visits = Vec::new();
            let statement_ordinal = u32::try_from(call.statement_index).ok()?;
            for root in facts
                .values
                .scalar_computations
                .roots_for_statement(state, statement_ordinal)
            {
                let plans = &facts.values.scalar_computations;
                let (expression, primitive) = match root.role {
                    ExpressionRole::BoundaryCallArgument {
                        call_ordinal,
                        argument_ordinal,
                    }
                    | ExpressionRole::UnitCallArgument {
                        call_ordinal,
                        argument_ordinal,
                    } if call_ordinal as usize == call.call_ordinal => {
                        *scalar_arguments.get(argument_ordinal as usize)?
                    }
                    ExpressionRole::CallSelector {
                        call_ordinal,
                        argument_ordinal,
                        expression,
                    } if call_ordinal as usize == call.call_ordinal => {
                        let argument = *structural_arguments.get(argument_ordinal as usize)?;
                        if !crate::values::authored_projection_selectors(program, argument)?
                            .contains(&expression)
                            || !plans.nodes.is_valid(root.root)
                        {
                            return None;
                        }
                        let primitive = plans.nodes.get(root.root).primitive_type;
                        if !primitive.is_fixed_integer() {
                            return None;
                        }
                        (expression, primitive)
                    }
                    _ => continue,
                };
                if root.machine != machine || roles.contains(&root.role) {
                    return None;
                }
                roles.push(root.role);
                if !plans.nodes.is_valid(root.root)
                    || plans.nodes.get(root.root).authored_root != expression
                    || plans.nodes.get(root.root).primitive_type != primitive
                {
                    return None;
                }
                collect(
                    program,
                    facts,
                    state,
                    call.statement_index,
                    root.root,
                    calls,
                    u32::try_from(call.call_ordinal.checked_add(1)?).ok()?,
                    &mut visits,
                    consumed,
                )?;
            }
            Some(())
        }

        /*
            A destination or invocation selects one endpoint site. Other coordinates
            naming the same authored range are not additional executions.
        */
        #[allow(clippy::too_many_arguments)]
        pub(super) fn collect_range_calls(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            machine: SymbolHandle,
            state: SymbolHandle,
            statement: usize,
            expression: typed_trees::expression::ExpressionHandle,
            site: checked_trees::CheckedSubsliceSite,
            calls: &[checked_trees::FlowCallFact],
            minimum_call_ordinal: u32,
            consumed: &mut super::CallOccurrenceLedger<'_>,
        ) -> Option<bool> {
            let plans = &facts.values.scalar_computations;
            let statement_ordinal = u32::try_from(statement).ok()?;
            let mut computed = false;
            for (endpoint, role, ()) in crate::values::subslice_endpoints(
                program,
                &facts.operators,
                expression,
                site,
                |_| Some(()),
            ) {
                let mut roots = plans.roots_for_statement_role(state, statement_ordinal, role);
                let Some(root) = roots.next() else { continue };
                if roots.next().is_some()
                    || !plans.is_authored_root(
                        root,
                        machine,
                        endpoint,
                        typed_trees::types::PrimitiveType::U64,
                    )
                {
                    return None;
                }
                collect_root_calls(
                    program,
                    facts,
                    state,
                    statement,
                    root.root,
                    calls,
                    minimum_call_ordinal,
                    consumed,
                )?;
                computed = true;
            }
            Some(computed)
        }

        /*
            A structural source carries its evaluated scalar dependencies directly;
            call accounting follows those operands, not parallel fact coordinates.
        */
        #[allow(clippy::too_many_arguments)]
        pub(super) fn collect_structural_argument_calls(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            state: SymbolHandle,
            statement: usize,
            argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
            calls: &[checked_trees::FlowCallFact],
            minimum_call_ordinal: u32,
            visits: &mut Vec<ComputationVisit>,
            consumed: &mut super::CallOccurrenceLedger<'_>,
        ) -> Option<()> {
            for handle in argument.computation_operands(&facts.values.scalar_computations.operands)
            {
                collect(
                    program,
                    facts,
                    state,
                    statement,
                    handle,
                    calls,
                    minimum_call_ordinal,
                    visits,
                    consumed,
                )?;
            }
            Some(())
        }

        /*
            The calls owned by one computation root in this machine — `None` when the
            root belongs to another machine or the walk cannot consume them.
        */
        pub(super) fn collect_owned_root_calls(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            machine: SymbolHandle,
            state: SymbolHandle,
            statement_index: usize,
            root: &checked_trees::CheckedScalarComputationRoot,
            calls: &[checked_trees::FlowCallFact],
            consumed: &mut super::CallOccurrenceLedger<'_>,
        ) -> Option<()> {
            if root.machine != machine {
                return None;
            }
            collect_root_calls(
                program,
                facts,
                state,
                statement_index,
                root.root,
                calls,
                0,
                consumed,
            )
        }

        /* Collect a scalar-computation root's calls with a fresh visit list. */
        pub(super) fn collect_root_calls(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            state: SymbolHandle,
            statement: usize,
            root: CheckedScalarComputationHandle,
            calls: &[checked_trees::FlowCallFact],
            minimum_call_ordinal: u32,
            consumed: &mut super::CallOccurrenceLedger<'_>,
        ) -> Option<()> {
            collect(
                program,
                facts,
                state,
                statement,
                root,
                calls,
                minimum_call_ordinal,
                &mut Vec::new(),
                consumed,
            )
        }

        pub(super) fn collect(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            state: SymbolHandle,
            statement: usize,
            handle: CheckedScalarComputationHandle,
            calls: &[checked_trees::FlowCallFact],
            minimum_call_ordinal: u32,
            visits: &mut Vec<ComputationVisit>,
            consumed: &mut super::CallOccurrenceLedger<'_>,
        ) -> Option<()> {
            let plans = &facts.values.scalar_computations;
            if !plans.nodes.is_valid(handle) {
                return None;
            }
            if let Some(minimum) = consumed.accounted_minimum(state, statement, handle) {
                if minimum.is_some_and(|ordinal| ordinal < minimum_call_ordinal) {
                    return None;
                }
                consumed.record_minimum(minimum);
                return Some(());
            }
            if let Some(visit) = visits.iter().find(|visit| visit.computation == handle) {
                return visit.completed.then_some(());
            }
            let visit_index = visits.len();
            visits.push(ComputationVisit {
                computation: handle,
                completed: false,
            });
            let pending_index = consumed.pending_minima.len();
            consumed.pending_minima.push(None);
            macro_rules! visit_computation {
                ($child:expr) => {
                    collect(
                        program,
                        facts,
                        state,
                        statement,
                        $child,
                        calls,
                        minimum_call_ordinal,
                        visits,
                        consumed,
                    )
                };
            }
            macro_rules! collect_subject {
                ($subject:expr $(,)?) => {
                    collect_structural_argument_calls(
                        program,
                        facts,
                        state,
                        statement,
                        $subject,
                        calls,
                        minimum_call_ordinal,
                        visits,
                        consumed,
                    )
                };
            }
            match &plans.nodes.get(handle).kind {
                ComputationKind::CaseMembership {
                    subject:
                        checked_trees::CheckedScalarComputationStructuralArgument::Place(subject),
                    ..
                }
                | ComputationKind::StorageAddress { subject, .. }
                | ComputationKind::ByteSequenceLiteralComparison { subject, .. } => {
                    collect_subject!(subject)?;
                }
                ComputationKind::CaseMembership {
                    subject:
                        checked_trees::CheckedScalarComputationStructuralArgument::Case(subject),
                    ..
                } => {
                    for field in plans.case_fields.span(subject.fields)? {
                        visit_computation!(field.value)?;
                    }
                }
                ComputationKind::SelectedComparison { left, right, .. } => {
                    visit_computation!(*left)?;
                    visit_computation!(*right)?;
                }
                checked_trees::cast_or_qualification_triple_pattern!(operand, ..) => {
                    visit_computation!(*operand)?;
                }
                ComputationKind::Dispatch { subject, arms, .. } => {
                    for operand in subject.scalar_operands(&plans.operands) {
                        visit_computation!(operand)?;
                    }
                    for arm in plans.dispatch_arms.span(*arms)? {
                        if let Some(pattern) = arm.pattern.computation() {
                            visit_computation!(pattern)?;
                        }
                        visit_computation!(arm.value)?;
                    }
                }
                ComputationKind::RetainedConstant { .. } | ComputationKind::Value(_) => {}
                ComputationKind::StructuralField {
                    subject,
                    receiver_value,
                    ..
                } => {
                    collect_subject!(subject)?;
                    if receiver_value.is_valid() {
                        let machine = facts.flow.control.state_by_symbol(state)?.machine_symbol;
                        super::structural_values::collect(
                            program,
                            facts,
                            machine,
                            state,
                            statement,
                            *receiver_value,
                            calls,
                            minimum_call_ordinal,
                            consumed,
                            &mut Vec::new(),
                        )?;
                    }
                }
                ComputationKind::ByteSequencePredicate {
                    predicate,
                    source_expression,
                    subject,
                    ..
                } => {
                    collect_subject!(subject)?;
                    let (selected, _) = typed_trees::byte_predicates::byte_predicate_call(
                        program,
                        *source_expression,
                    )?;
                    if selected != *predicate {
                        return None;
                    }
                    let authored = program.expression_table.call_node(*source_expression)?;
                    let mut occurrences = calls
                        .iter()
                        .filter(|call| call.authored_expression == *source_expression)
                        .filter_map(|call| Some((facts.flow.control.calls.handle_of(call)?, call)));
                    if let Some((handle, call)) = occurrences.next() {
                        if occurrences.next().is_some()
                            || call.statement_index != statement
                            || call.call_ordinal < minimum_call_ordinal as usize
                            || call.target_symbol
                                != validation::call_target_entry_symbol(
                                    program,
                                    authored.target_symbol,
                                )
                            || consumed.contains(&handle)
                            || facts.flow.control.is_retired(state, call)
                        {
                            return None;
                        }
                        consumed.record_minimum(Some(u32::try_from(call.call_ordinal).ok()?));
                        consumed.push(handle);
                    }
                }
                ComputationKind::Call {
                    source_call,
                    call_ordinal,
                    callee,
                    arguments,
                    structural_arguments,
                    ..
                } => {
                    let source_call = source_call.statement()?;
                    if !facts.flow.control.calls.is_valid(source_call)
                        || consumed.contains(&source_call)
                    {
                        return None;
                    }
                    let call = facts.flow.control.calls.get(source_call);
                    if *call_ordinal < minimum_call_ordinal
                        || call.call_ordinal != *call_ordinal as usize
                        || call.statement_index != statement
                        || call.target_symbol != callee.called_signature()
                        || !call.authored_expression.is_valid()
                        || !calls.iter().any(|candidate| std::ptr::eq(candidate, call))
                        || facts.flow.control.is_retired(state, call)
                    {
                        return None;
                    }
                    consumed.record_minimum(Some(*call_ordinal));
                    consumed.push(source_call);
                    for operand in callee
                        .selector()
                        .into_iter()
                        .chain(plans.operands.span(*arguments)?.iter().copied())
                    {
                        visit_computation!(operand)?;
                    }
                    for argument in plans.structural_arguments.span(*structural_arguments)? {
                        if let checked_trees::CheckedScalarComputationStructuralArgument::Place(
                            source,
                        ) = argument
                        {
                            collect_subject!(source)?;
                        }
                        if let checked_trees::CheckedScalarComputationStructuralArgument::Case(
                            subject,
                        ) = argument
                        {
                            for field in plans.case_fields.span(subject.fields)? {
                                visit_computation!(field.value)?;
                            }
                        }
                    }
                }
                ComputationKind::WithOperands {
                    operands, operand, ..
                } => {
                    for dependency in plans.evaluation_operands.span(*operands)? {
                        if let checked_trees::CheckedScalarEvaluationOperand::Scalar(value) =
                            dependency
                        {
                            visit_computation!(*value)?;
                        }
                        if let checked_trees::CheckedScalarEvaluationOperand::Structural(value) =
                            dependency
                        {
                            let machine = facts.flow.control.state_by_symbol(state)?.machine_symbol;
                            super::structural_values::collect(
                                program,
                                facts,
                                machine,
                                state,
                                statement,
                                *value,
                                calls,
                                minimum_call_ordinal,
                                consumed,
                                &mut Vec::new(),
                            )?;
                        }
                    }
                    visit_computation!(*operand)?;
                }
                ComputationKind::Apply { operands, .. }
                | ComputationKind::StorageObservation { operands, .. } => {
                    for operand in plans.operands.span(*operands)? {
                        visit_computation!(*operand)?;
                    }
                }
                ComputationKind::Select {
                    condition,
                    when_true,
                    when_false,
                    ..
                } => {
                    for operand in [condition, when_true, when_false] {
                        visit_computation!(*operand)?;
                    }
                }
            }
            visits[visit_index].completed = true;
            let minimum = consumed.pending_minima.pop()?;
            if consumed.pending_minima.len() != pending_index {
                return None;
            }
            consumed
                .accounted_computations
                .push(super::AccountedComputation {
                    state,
                    statement,
                    computation: handle,
                    minimum_call_ordinal: minimum,
                });
            Some(())
        }
    }
    mod dispatch_producers {
        //! Resolve each saved call to its unique already scheduled producer once.
        //! The invocation-order comparison stays at the actual consuming leaf.

        use super::{CallOccurrenceLedger, CheckFacts};
        use arena::Handle;
        use checked_trees::{CheckedUnitEffectOperationPlan, FlowCallFact};
        use std::collections::{BTreeMap, BTreeSet};
        use typed_trees::{machine::Machine, state::State};

        type CallKey = (u32, u32);
        type CallOrder = (usize, usize);

        fn key(handle: Handle<FlowCallFact>) -> CallKey {
            (handle.arena_index(), handle.generation())
        }

        pub(super) fn authenticated_producer<'a>(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            state: &State,
            candidate: &FlowCallFact,
            find: impl FnOnce(
                usize,
                typed_trees::expression::ExpressionHandle,
            ) -> Option<&'a FlowCallFact>,
            owners: impl FnOnce(&FlowCallFact) -> Option<usize>,
        ) -> Option<CallOrder> {
            let producer = source_producer(program, facts, state, candidate, find)?;
            (owners(producer)? == 1).then_some((producer.statement_index, producer.call_ordinal))
        }

        fn source_producer<'a>(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            state: &State,
            candidate: &FlowCallFact,
            find: impl FnOnce(
                usize,
                typed_trees::expression::ExpressionHandle,
            ) -> Option<&'a FlowCallFact>,
        ) -> Option<&'a FlowCallFact> {
            let (statement, expression) = validation::saved_dispatch_call(
                program,
                state,
                candidate.statement_index,
                candidate.authored_expression,
            )?;
            let producer = find(statement, expression)?;
            (!facts.flow.control.is_retired(state.symbol, producer)
                && producer.target_symbol == candidate.target_symbol)
                .then_some(producer)
        }

        #[derive(Clone, Copy)]
        struct Producer {
            key: CallKey,
            order: CallOrder,
        }

        #[derive(Default)]
        struct Owners {
            counts: BTreeMap<CallKey, usize>,
            unknown_operands: BTreeMap<usize, BTreeSet<CallKey>>,
            invalid: bool,
        }

        impl Owners {
            fn count(&self, producer: Producer) -> Option<usize> {
                // A direct owner skips its own operand query. All other preceding
                // calls at this statement must supply complete operand inventories.
                if self.invalid
                    || self
                        .unknown_operands
                        .get(&producer.order.0)
                        .is_some_and(|unknown| {
                            unknown.len() != 1 || !unknown.contains(&producer.key)
                        })
                {
                    return None;
                }
                Some(self.counts.get(&producer.key).copied().unwrap_or(0))
            }
        }

        pub(super) struct DispatchProducers {
            by_call: BTreeMap<CallKey, Producer>,
            orders: BTreeMap<CallKey, CallOrder>,
            coordinates: BTreeMap<CallOrder, Option<Handle<FlowCallFact>>>,
            common: Owners,
            predecessor_count: usize,
        }

        /// Prefix candidates retain their producer identities independently of who
        /// scheduled them. A leaf can add an owner or make an earlier owner ambiguous.
        pub(super) struct PrefixProducers {
            candidates: BTreeSet<CallKey>,
            common: BTreeMap<usize, BTreeSet<(usize, CallKey)>>,
        }

        impl DispatchProducers {
            pub(super) fn prepare(
                program: &dyn validation::CheckingInput,
                facts: &CheckFacts,
                machine: &Machine,
                state: &State,
                calls: &[FlowCallFact],
                statement: usize,
                predecessors: &[CheckedUnitEffectOperationPlan],
            ) -> Self {
                let mut expressions = BTreeMap::new();
                let mut coordinates = BTreeMap::new();
                let mut orders = BTreeMap::new();
                for call in calls {
                    let handle = facts.flow.control.calls.handle_of(call);
                    expressions
                        .entry((
                            call.statement_index,
                            call.authored_expression.arena_index(),
                            call.authored_expression.generation(),
                        ))
                        .and_modify(|prior| *prior = None)
                        .or_insert(handle);
                    coordinates
                        .entry((call.statement_index, call.call_ordinal))
                        .and_modify(|prior| *prior = None)
                        .or_insert(handle);
                    if let Some(handle) = handle {
                        orders.insert(key(handle), (call.statement_index, call.call_ordinal));
                    }
                }
                let mut by_call = BTreeMap::new();
                for candidate in calls
                    .iter()
                    .filter(|call| call.statement_index == statement)
                {
                    let producer = source_producer(
                        program,
                        facts,
                        state,
                        candidate,
                        |statement, expression| {
                            expressions
                                .get(&(
                                    statement,
                                    expression.arena_index(),
                                    expression.generation(),
                                ))
                                .copied()
                                .flatten()
                                .map(|handle| facts.flow.control.calls.get(handle))
                        },
                    );
                    if let Some(producer) = producer
                        && let Some(producer_handle) = facts.flow.control.calls.handle_of(producer)
                        && let Some(handle) = facts.flow.control.calls.handle_of(candidate)
                    {
                        by_call.insert(
                            key(handle),
                            Producer {
                                key: key(producer_handle),
                                order: (producer.statement_index, producer.call_ordinal),
                            },
                        );
                    }
                }
                let mut result = Self {
                    by_call,
                    orders,
                    coordinates,
                    common: Owners::default(),
                    predecessor_count: predecessors.len(),
                };
                result.common = result.owners(program, facts, machine, state, calls, predecessors);
                result
            }

            fn owners(
                &self,
                program: &dyn validation::CheckingInput,
                facts: &CheckFacts,
                machine: &Machine,
                state: &State,
                calls: &[FlowCallFact],
                predecessors: &[CheckedUnitEffectOperationPlan],
            ) -> Owners {
                let mut owners = Owners::default();
                for operation in predecessors {
                    let Some(planned) = operation.call() else {
                        continue;
                    };
                    let Some(prior_handle) = self
                        .coordinates
                        .get(&(
                            planned.coordinate.statement_index as usize,
                            planned.coordinate.call_ordinal as usize,
                        ))
                        .copied()
                        .flatten()
                    else {
                        owners.invalid = true;
                        break;
                    };
                    let prior = facts.flow.control.calls.get(prior_handle);
                    if facts.flow.control.is_retired(state.symbol, prior) {
                        owners.invalid = true;
                        break;
                    }
                    let prior_key = key(prior_handle);
                    *owners.counts.entry(prior_key).or_default() += 1;
                    let mut consumed = CallOccurrenceLedger::default();
                    let complete = validation::find_call_site(
                        program,
                        machine.symbol,
                        state.symbol,
                        prior.statement_index,
                        prior.call_ordinal,
                    )
                    .and_then(|site| {
                        super::collecting::collect_argument_calls(
                            program,
                            facts,
                            machine.symbol,
                            state.symbol,
                            prior,
                            validation::call_site_argument_expressions(program, &site),
                            calls,
                            &mut consumed,
                        )
                    })
                    .is_some();
                    if !complete {
                        owners
                            .unknown_operands
                            .entry(prior.statement_index)
                            .or_default()
                            .insert(prior_key);
                        continue;
                    }
                    // Several roles in one prior operation still constitute one owner.
                    let owned = consumed
                        .iter()
                        .filter_map(|handle| {
                            let row = facts.flow.control.calls.get(*handle);
                            (row.statement_index == prior.statement_index
                                && key(*handle) != prior_key)
                                .then_some(key(*handle))
                        })
                        .collect::<BTreeSet<_>>();
                    for producer in owned {
                        *owners.counts.entry(producer).or_default() += 1;
                    }
                }
                owners
            }

            pub(super) fn local_owners(
                &self,
                program: &dyn validation::CheckingInput,
                facts: &CheckFacts,
                machine: &Machine,
                state: &State,
                calls: &[FlowCallFact],
                predecessors: &[CheckedUnitEffectOperationPlan],
            ) -> Option<LocalProducers> {
                let local = self.owners(
                    program,
                    facts,
                    machine,
                    state,
                    calls,
                    predecessors.get(self.predecessor_count..)?,
                );
                Some(LocalProducers { owners: local })
            }

            pub(super) fn prefix(&self, calls: &[Handle<FlowCallFact>]) -> PrefixProducers {
                let mut prefix = PrefixProducers {
                    candidates: BTreeSet::new(),
                    common: BTreeMap::new(),
                };
                for handle in calls {
                    if let Some(producer) = self.by_call.get(&key(*handle)).copied() {
                        prefix.candidates.insert(producer.key);
                        if self.common.count(producer) == Some(1) {
                            prefix
                                .common
                                .entry(producer.order.0)
                                .or_default()
                                .insert((producer.order.1, producer.key));
                        }
                    }
                }
                prefix
            }

            pub(super) fn before(
                &self,
                handle: Handle<FlowCallFact>,
                invocation: &FlowCallFact,
            ) -> bool {
                self.by_call.get(&key(handle)).is_some_and(|producer| {
                    producer.order < (invocation.statement_index, invocation.call_ordinal)
                        && self.common.count(*producer) == Some(1)
                })
            }
        }

        pub(super) struct LocalProducers {
            owners: Owners,
        }

        impl LocalProducers {
            fn owns(&self, prepared: &DispatchProducers, producer: Producer) -> bool {
                prepared
                    .common
                    .count(producer)
                    .zip(self.owners.count(producer))
                    .is_some_and(|(common, local)| common + local == 1)
            }

            pub(super) fn rescheduled(
                &self,
                prepared: &DispatchProducers,
                prefix: &PrefixProducers,
                operands: &[Handle<FlowCallFact>],
                before: CallOrder,
            ) -> bool {
                if !self.owners.invalid {
                    for (statement, candidates) in &prefix.common {
                        if *statement > before.0 {
                            break;
                        }
                        // An unknown operand owner invalidates every other producer at
                        // this statement. Its own producer already has a local owner,
                        // which would duplicate the one required by this common set.
                        if self.owners.unknown_operands.contains_key(statement) {
                            continue;
                        }
                        for (ordinal, producer) in candidates {
                            if (*statement, *ordinal) >= before {
                                break;
                            }
                            if self.owners.counts.get(producer).copied().unwrap_or(0) == 0 {
                                return true;
                            }
                            // Every skipped candidate has a new local owner, so this
                            // walk is bounded by the new work rather than prefix size.
                        }
                    }
                }
                // A producer absent from the common schedule can be owned by this
                // leaf's preparation. Only locally touched identities need this join.
                for producer in self
                    .owners
                    .counts
                    .keys()
                    .filter(|key| prefix.candidates.contains(key))
                {
                    if let Some(order) = prepared.orders.get(producer).copied()
                        && order < before
                        && self.owns(
                            prepared,
                            Producer {
                                key: *producer,
                                order,
                            },
                        )
                    {
                        return true;
                    }
                }
                operands.iter().any(|handle| {
                    prepared.by_call.get(&key(*handle)).is_some_and(|producer| {
                        producer.order < before && self.owns(prepared, *producer)
                    })
                })
            }
        }

        #[cfg(test)]
        mod tests {

            use super::*;

            #[test]
            fn prefix_owner_index_matches_complete_join() {
                let first = (11, 3);
                let second = (12, 3);
                let third = (13, 4);
                let orders = BTreeMap::from([(first, (1, 1)), (second, (1, 2)), (third, (2, 1))]);
                let prepared = DispatchProducers {
                    by_call: BTreeMap::new(),
                    orders,
                    coordinates: BTreeMap::new(),
                    common: Owners {
                        counts: BTreeMap::from([(first, 1), (third, 1)]),
                        ..Default::default()
                    },
                    predecessor_count: 0,
                };
                let prefix = PrefixProducers {
                    candidates: BTreeSet::from([first, second, third]),
                    common: BTreeMap::from([
                        (1, BTreeSet::from([(1, first)])),
                        (2, BTreeSet::from([(1, third)])),
                    ]),
                };
                let locals = [
                    Owners::default(),
                    Owners {
                        counts: BTreeMap::from([(second, 1)]),
                        ..Default::default()
                    },
                    Owners {
                        counts: BTreeMap::from([(first, 1), (third, 1)]),
                        ..Default::default()
                    },
                    Owners {
                        counts: BTreeMap::from([(first, 1)]),
                        unknown_operands: BTreeMap::from([(1, BTreeSet::from([first]))]),
                        ..Default::default()
                    },
                    Owners {
                        counts: BTreeMap::from([(second, 1)]),
                        unknown_operands: BTreeMap::from([(1, BTreeSet::from([first, second]))]),
                        ..Default::default()
                    },
                    Owners {
                        invalid: true,
                        ..Default::default()
                    },
                ];
                for owners in locals {
                    let local = LocalProducers { owners };
                    for before in [(1, 1), (1, 3), (2, 1), (3, 0)] {
                        let linear = prefix.candidates.iter().any(|key| {
                            let order = prepared.orders[key];
                            order < before && local.owns(&prepared, Producer { key: *key, order })
                        });
                        assert_eq!(local.rescheduled(&prepared, &prefix, &[], before), linear);
                    }
                }
            }
        }
    }
    mod selected_calls {
        //! Account a selected invocation tree without treating its other leaves as
        //! unscheduled work on the current branch. Each leaf still proves its complete
        //! operand custody. Only the final all-leaf join can cover the statement.

        use super::dispatch_producers::{DispatchProducers, PrefixProducers};
        use super::{CallOccurrenceLedger, CallOccurrencePrefix, CheckFacts, SymbolHandle};
        use arena::{Handle, HandleSpan};
        use checked_trees::{CheckedCompletionExpressionKind as Kind, FlowCallFact};
        use std::collections::{BTreeMap, BTreeSet};

        type CallKey = (u32, u32);

        pub(in crate::execution::terminal_unit) struct SelectedInvocationAccounting {
            machine: SymbolHandle,
            state: SymbolHandle,
            statement: usize,
            calls: HandleSpan<FlowCallFact>,
            leaves: BTreeSet<CallKey>,
            bound: BTreeSet<CallKey>,
            accounted: BTreeSet<CallKey>,
            prefix: CallOccurrencePrefix,
            guard_maximum: Option<usize>,
            producers: DispatchProducers,
            prefix_producers: PrefixProducers,
            first: Handle<FlowCallFact>,
            expression_calls: BTreeMap<(usize, u32, u32), Option<Handle<FlowCallFact>>>,
        }

        fn key(call: Handle<FlowCallFact>) -> CallKey {
            (call.arena_index(), call.generation())
        }

        fn in_span(span: HandleSpan<FlowCallFact>, handle: Handle<FlowCallFact>) -> bool {
            handle
                .arena_index()
                .checked_sub(span.start().arena_index())
                .and_then(|offset| span.handles().nth(offset as usize))
                == Some(handle)
        }

        impl SelectedInvocationAccounting {
            pub(in crate::execution::terminal_unit) fn new(
                program: &dyn validation::CheckingInput,
                facts: &CheckFacts,
                root: &checked_trees::CheckedCompletionRoot,
                predecessors: &[checked_trees::CheckedUnitEffectOperationPlan],
            ) -> Option<Self> {
                if root.refusal.is_some() {
                    return None;
                }
                let calls = facts.flow.control.state(root.machine, root.state)?.calls;
                let roster = facts.flow.control.calls.span_or_empty(calls);
                let mut expression_calls = BTreeMap::new();
                for (handle, call) in calls.handles().zip(roster) {
                    if call.statement_index != root.statement_ordinal as usize {
                        continue;
                    }
                    let coordinate = (
                        call.call_ordinal,
                        call.authored_expression.arena_index(),
                        call.authored_expression.generation(),
                    );
                    expression_calls
                        .entry(coordinate)
                        .and_modify(|prior| *prior = None)
                        .or_insert(Some(handle));
                }
                let plans = &facts.values.completions;
                let mut pending = vec![(root.root, false)];
                let mut visited = BTreeMap::new();
                let mut leaves = BTreeSet::new();
                let mut first = None;
                while let Some((handle, completed)) = pending.pop() {
                    if !plans.nodes.is_valid(handle) {
                        return None;
                    }
                    let coordinate = (handle.arena_index(), handle.generation());
                    if completed {
                        visited.insert(coordinate, false);
                        continue;
                    }
                    match visited.get(&coordinate) {
                        Some(true) => return None,
                        Some(false) => continue,
                        None => {}
                    }
                    visited.insert(coordinate, true);
                    let node = plans.nodes.get(handle);
                    match &node.kind {
                        Kind::Missing => return None,
                        Kind::Invocation { source_call } => {
                            if !in_span(calls, *source_call) {
                                return None;
                            }
                            let call = facts.flow.control.calls.get(*source_call);
                            if call.statement_index != root.statement_ordinal as usize
                                || call.authored_expression != node.expression
                            {
                                return None;
                            }
                            leaves.insert(key(*source_call));
                            first.get_or_insert(*source_call);
                            visited.insert(coordinate, false);
                        }
                        Kind::Dispatch { arms, .. } => {
                            let arms = plans.arms.span(*arms)?;
                            if arms.is_empty() {
                                return None;
                            }
                            pending.push((handle, true));
                            pending.extend(arms.iter().rev().map(|arm| (arm.completion, false)));
                        }
                    }
                }
                let first = first?;
                let (prefix, guard_maximum) = super::collecting::statement_computation_prefix(
                    program,
                    facts,
                    root.machine,
                    root.state,
                    root.statement_ordinal as usize,
                    roster,
                )?;
                if !prefix.pending_minima.is_empty()
                    || prefix.prefix.is_some()
                    || prefix.calls.iter().any(|call| leaves.contains(&key(*call)))
                {
                    return None;
                }
                let machine = program.machine_by_symbol(root.machine)?;
                let state = program.state_in_machine(machine, root.state)?;
                let producers = DispatchProducers::prepare(
                    program,
                    facts,
                    machine,
                    state,
                    roster,
                    root.statement_ordinal as usize,
                    predecessors,
                );
                let prefix_producers = producers.prefix(&prefix.calls);
                let accounted = prefix.calls.iter().copied().map(key).collect();
                let computations = prefix
                    .accounted_computations
                    .into_iter()
                    .map(|entry| {
                        (
                            (
                                entry.state.arena_index(),
                                entry.state.generation(),
                                entry.statement,
                                entry.computation.arena_index(),
                                entry.computation.generation(),
                            ),
                            entry.minimum_call_ordinal,
                        )
                    })
                    .collect();
                Some(Self {
                    machine: root.machine,
                    state: root.state,
                    statement: root.statement_ordinal as usize,
                    calls,
                    leaves,
                    bound: BTreeSet::new(),
                    accounted,
                    prefix: CallOccurrencePrefix {
                        call_keys: prefix.calls.iter().copied().map(key).collect(),
                        calls: prefix.calls,
                        computations,
                    },
                    guard_maximum,
                    producers,
                    prefix_producers,
                    first,
                    expression_calls,
                })
            }

            pub(super) fn contains_call(&self, facts: &CheckFacts, call: &FlowCallFact) -> bool {
                facts
                    .flow
                    .control
                    .calls
                    .handle_of(call)
                    .is_some_and(|handle| in_span(self.calls, handle))
            }

            pub(super) fn contains_leaf(
                &self,
                facts: &CheckFacts,
                machine: SymbolHandle,
                state: SymbolHandle,
                call: &FlowCallFact,
                calls: &[FlowCallFact],
            ) -> bool {
                machine == self.machine
                    && state == self.state
                    && call.statement_index == self.statement
                    && std::ptr::eq(calls, facts.flow.control.calls.span_or_empty(self.calls))
                    && facts
                        .flow
                        .control
                        .calls
                        .handle_of(call)
                        .is_some_and(|handle| self.leaves.contains(&key(handle)))
            }

            pub(super) fn operand_ledger(&self) -> (CallOccurrenceLedger<'_>, Option<usize>) {
                (
                    CallOccurrenceLedger {
                        prefix: Some(&self.prefix),
                        ..Default::default()
                    },
                    self.guard_maximum,
                )
            }

            pub(super) fn operands_reschedule_producer(
                &self,
                program: &dyn validation::CheckingInput,
                facts: &CheckFacts,
                call: &FlowCallFact,
                operands: &[Handle<FlowCallFact>],
                predecessors: &[checked_trees::CheckedUnitEffectOperationPlan],
            ) -> Option<bool> {
                let machine = program.machine_by_symbol(self.machine)?;
                let state = program.state_in_machine(machine, self.state)?;
                let local = self.producers.local_owners(
                    program,
                    facts,
                    machine,
                    state,
                    facts.flow.control.calls.span_or_empty(self.calls),
                    predecessors,
                )?;
                Some(local.rescheduled(
                    &self.producers,
                    &self.prefix_producers,
                    operands,
                    (call.statement_index, call.call_ordinal),
                ))
            }

            pub(super) fn account(
                &mut self,
                facts: &CheckFacts,
                call: &FlowCallFact,
                consumed: &[Handle<FlowCallFact>],
            ) -> Option<()> {
                let call = key(facts.flow.control.calls.handle_of(call)?);
                if !self.leaves.contains(&call)
                    || consumed
                        .iter()
                        .any(|handle| self.leaves.contains(&key(*handle)))
                {
                    return None;
                }
                self.accounted.extend(consumed.iter().copied().map(key));
                self.accounted.insert(call);
                self.bound.insert(call);
                Some(())
            }

            /// A checked operand may stop before its callee is invoked. Retain only
            /// calls actually owned by that completed prefix, plus the leaf's known
            /// stopped disposition. Unknown or unrelated source rows remain debt.
            pub(in crate::execution::terminal_unit) fn stopped_prefix(
                &mut self,
                facts: &CheckFacts,
                call: &FlowCallFact,
                operations: &[checked_trees::CheckedUnitEffectOperationPlan],
            ) -> Option<()> {
                let leaf = key(facts.flow.control.calls.handle_of(call)?);
                if !self.leaves.contains(&leaf) || call.statement_index != self.statement {
                    return None;
                }
                let stop = operations
                    .iter()
                    .position(|operation| !operation.has_normal_continuation())?;
                let mut accounted = BTreeSet::new();
                for operation in &operations[..=stop] {
                    for operation in operation.with_value_calls() {
                        let Some(planned) = operation.call() else {
                            continue;
                        };
                        let Some(checked_trees::NominalMachineUseSite::Expression(expression)) =
                            planned.source_site()
                        else {
                            return None;
                        };
                        if planned.coordinate.statement_index as usize != self.statement {
                            return None;
                        }
                        let handle = self
                            .expression_calls
                            .get(&(
                                planned.coordinate.call_ordinal as usize,
                                expression.arena_index(),
                                expression.generation(),
                            ))
                            .copied()
                            .flatten()?;
                        if self.leaves.contains(&key(handle)) {
                            return None;
                        }
                        accounted.insert(key(handle));
                    }
                }
                // This accounts for source ownership, not execution of the callee.
                self.accounted.extend(accounted);
                self.accounted.insert(leaf);
                self.bound.insert(leaf);
                Some(())
            }

            pub(in crate::execution::terminal_unit) fn finish(
                &self,
                program: &dyn validation::CheckingInput,
                facts: &CheckFacts,
                predecessors: &[checked_trees::CheckedUnitEffectOperationPlan],
            ) -> Option<()> {
                if self.bound != self.leaves {
                    return None;
                }
                let machine = program.machine_by_symbol(self.machine)?;
                let state = program.state_in_machine(machine, self.state)?;
                let first = facts.flow.control.calls.get(self.first);
                let calls = facts.flow.control.calls.span_or_empty(self.calls);
                let active_arm = validation::transition_call_target(
                    program,
                    machine,
                    state,
                    self.statement,
                    first.call_ordinal,
                );
                calls
                    .iter()
                    .filter(|call| call.statement_index == self.statement)
                    .all(|call| {
                        facts
                            .flow
                            .control
                            .calls
                            .handle_of(call)
                            .is_some_and(|handle| self.accounted.contains(&key(handle)))
                            || facts
                                .flow
                                .control
                                .calls
                                .handle_of(call)
                                .is_some_and(|handle| self.producers.before(handle, first))
                            || super::collecting::scheduled_by_active_arm(call, predecessors)
                            || super::collecting::other_transition_arm(
                                program, machine, state, active_arm, call,
                            )
                            || facts.flow.control.is_retired(self.state, call)
                            || program
                                .symbols
                                .builtin_function_for_symbol(call.target_symbol)
                                .is_some_and(|function| function.has_empty_write_frame())
                    })
                    .then_some(())
            }
        }
    }
    mod structural_values {

        /*
            Shared occurrence accounting for an explicitly retained structural value.

            An ordinary statement and a scalar receiver dependency read the same value
            graph. Source calls are counted once by their captured identity; discovering
            another root or projection does not schedule another invocation. Selected
            callers keep this walk inside the selected operand's own ledger.
        */
        use super::collecting::collect_structural_argument_calls;
        use super::{CheckFacts, SymbolHandle};
        use crate::flow::expressions;

        pub(super) fn collect<'facts>(
            program: &dyn validation::CheckingInput,
            facts: &'facts CheckFacts,
            machine: SymbolHandle,
            state: SymbolHandle,
            statement_index: usize,
            value: checked_trees::CheckedStructuralValueHandle,
            calls: &'facts [checked_trees::FlowCallFact],
            minimum_call_ordinal: u32,
            consumed: &mut super::CallOccurrenceLedger<'_>,
            structural: &mut Vec<&'facts checked_trees::FlowCallFact>,
        ) -> Option<()> {
            let mut pending = vec![value];
            let mut visited = Vec::new();
            while let Some(value) = pending.pop() {
                let plans = &facts.values.structural_values;
                if !plans.nodes.is_valid(value) {
                    return None;
                }
                if visited.contains(&value) {
                    continue;
                }
                visited.push(value);
                let node = plans.nodes.get(value);
                if matches!(
                    node.kind,
                    checked_trees::CheckedStructuralValueKind::Reference {
                        storage: checked_trees::CheckedReferenceStorage::Access
                            | checked_trees::CheckedReferenceStorage::ResidentAccess { .. },
                        ..
                    }
                ) {
                    let formation =
                        validation::reference_formation_source(program, node.expression)?;
                    if !formation.interpretation.is_access_view() {
                        return None;
                    }
                    let authored = program.expression_table.call_node(node.expression)?;
                    let mut occurrences = calls
                        .iter()
                        .filter(|call| call.authored_expression == node.expression)
                        .filter_map(|call| Some((facts.flow.control.calls.handle_of(call)?, call)));
                    let (Some((handle, call)), None) = (occurrences.next(), occurrences.next())
                    else {
                        return None;
                    };
                    if call.statement_index != statement_index
                        || call.call_ordinal < minimum_call_ordinal as usize
                        || call.target_symbol
                            != validation::call_target_entry_symbol(program, authored.target_symbol)
                        || facts.flow.control.is_retired(state, call)
                    {
                        return None;
                    }
                    consumed.record_minimum(Some(u32::try_from(call.call_ordinal).ok()?));
                    consumed.record_once(handle);
                }
                match node.kind {
                    checked_trees::CheckedStructuralValueKind::NativeCodeLayout { .. } => {}
                    checked_trees::CheckedStructuralValueKind::QuotientCall {
                        request,
                        ref arguments,
                        ..
                    } => {
                        let mut occurrences = calls
                            .iter()
                            .filter(|call| call.authored_expression == request)
                            .filter_map(|call| {
                                Some((facts.flow.control.calls.handle_of(call)?, call))
                            });
                        let (handle, call) = occurrences.next()?;
                        if occurrences.next().is_some()
                            || call.statement_index != statement_index
                            || call.call_ordinal < minimum_call_ordinal as usize
                            || facts.flow.control.is_retired(state, call)
                            || expressions(program)
                                .call_node(request)?
                                .quotient_operation
                                .is_none()
                        {
                            return None;
                        }
                        consumed.record_minimum(Some(u32::try_from(call.call_ordinal).ok()?));
                        consumed.record_once(handle);
                        pending.extend(arguments.iter().rev().copied());
                    }
                    checked_trees::CheckedStructuralValueKind::Call { source_call } => {
                        if !facts.flow.control.calls.is_valid(source_call) {
                            return None;
                        }
                        let call = facts.flow.control.calls.get(source_call);
                        if call.authored_expression != node.expression
                            || call.statement_index != statement_index
                        {
                            return None;
                        }
                        if consumed.contains(&source_call) {
                            continue;
                        }
                        if call.call_ordinal < minimum_call_ordinal as usize
                            || facts.flow.control.is_retired(state, call)
                        {
                            return None;
                        }
                        consumed.record_minimum(Some(u32::try_from(call.call_ordinal).ok()?));
                        consumed.push(source_call);
                        structural.push(call);
                        let site = validation::find_call_site(
                            program,
                            machine,
                            state,
                            call.statement_index,
                            call.call_ordinal,
                        )?;
                        super::collecting::collect_argument_calls(
                            program,
                            facts,
                            machine,
                            state,
                            call,
                            validation::call_site_argument_expressions(program, &site),
                            calls,
                            consumed,
                        )?;
                        for operand in call.argument_operands().handles() {
                            let operand = facts.flow.control.operands.get(operand);
                            if let Some(root) = facts.values.structural_values.root_for_expression(
                                state,
                                u32::try_from(statement_index).ok()?,
                                operand.expression,
                            ) {
                                if root.machine != machine {
                                    return None;
                                }
                                pending.push(root.root);
                            }
                        }
                    }
                    checked_trees::CheckedStructuralValueKind::Record { fields, .. }
                    | checked_trees::CheckedStructuralValueKind::StructuralCase {
                        fields, ..
                    } => {
                        for field in plans.record_fields.span(fields)? {
                            if let checked_trees::CheckedStructuralRecordFieldValue::Structural(
                                child,
                            ) = field.value
                            {
                                pending.push(child);
                            }
                        }
                    }
                    checked_trees::CheckedStructuralValueKind::Dispatch { arms, .. } => {
                        pending.extend(plans.dispatch_arms.span(arms)?.iter().map(|arm| arm.value));
                    }
                    checked_trees::CheckedStructuralValueKind::ViewSubslice {
                        source,
                        ref start,
                        ref end,
                        ..
                    } => {
                        pending.push(source);
                        let mut visits = Vec::new();
                        for value in start.iter().chain(end.iter()) {
                            if let checked_trees::CheckedCallScalarArgument::Computation(root) =
                                value
                            {
                                super::collecting::collect(
                                    program,
                                    facts,
                                    state,
                                    statement_index,
                                    *root,
                                    calls,
                                    minimum_call_ordinal,
                                    &mut visits,
                                    consumed,
                                )?;
                            }
                        }
                    }
                    checked_trees::CheckedStructuralValueKind::Projection {
                        source,
                        ref path,
                        ref projection_evaluations,
                        ..
                    } => {
                        for root in
                            checked_trees::CheckedUnitStructuralArgumentPlan::projection_computation_operands(
                                path, *projection_evaluations, &facts.values.scalar_computations.operands,
                            )
                        {
                            super::collecting::collect_root_calls(
                                program,
                                facts,
                                state,
                                statement_index,
                                root,
                                calls,
                                minimum_call_ordinal,
                                consumed,
                            )?;
                        }
                        pending.push(source);
                    }
                    checked_trees::CheckedStructuralValueKind::QuotientAdaptation {
                        source,
                        ..
                    } => {
                        pending.push(source);
                    }
                    checked_trees::CheckedStructuralValueKind::FixedArray { ref elements } => {
                        pending.extend(elements.iter().copied());
                    }
                    checked_trees::CheckedStructuralValueKind::Reference {
                        source_value,
                        selectors,
                        ..
                    } => {
                        if source_value.is_valid() {
                            pending.push(source_value);
                        }
                        let mut visits = Vec::new();
                        for root in facts.values.scalar_computations.operands.span(selectors)? {
                            super::collecting::collect(
                                program,
                                facts,
                                state,
                                statement_index,
                                *root,
                                calls,
                                minimum_call_ordinal,
                                &mut visits,
                                consumed,
                            )?;
                        }
                    }
                    checked_trees::CheckedStructuralValueKind::BorrowedSliceView {
                        ref source,
                        ..
                    }
                    | checked_trees::CheckedStructuralValueKind::MutableSliceView {
                        ref source,
                        ..
                    }
                    | checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace {
                        ref source,
                        ..
                    }
                    | checked_trees::CheckedStructuralValueKind::Place(ref source) => {
                        collect_structural_argument_calls(
                            program,
                            facts,
                            state,
                            statement_index,
                            source,
                            calls,
                            minimum_call_ordinal,
                            &mut Vec::new(),
                            consumed,
                        )?;
                    }
                    checked_trees::CheckedStructuralValueKind::Case(_)
                    | checked_trees::CheckedStructuralValueKind::ZeroedScalarArray { .. }
                    | checked_trees::CheckedStructuralValueKind::ScalarArray { .. }
                    | checked_trees::CheckedStructuralValueKind::Scalar { .. }
                    | checked_trees::CheckedStructuralValueKind::ZeroedStructural { .. }
                    | checked_trees::CheckedStructuralValueKind::ByteLiteralView { .. } => {}
                }
            }
            Some(())
        }
    }
    mod traced {

        /*
            Discover each statement's call owners before composing its operation sequence.
            Flow inventory is in completion order, while occurrence ordinals identify
            source preorder. Discovery follows the latter so enclosing operand roots can
            claim nested producers; retained operand plans still own execution order.
            Exact call sites, not destination types or statement-family recognizers,
            supply argument ownership. Selected exits keep their own control authority.
            The operation prefix is selected in execution order; discovery retains
            source preorder within that membership, never scheduling a skipped call.
        */
        use super::super::{CheckFacts, CheckedScalarExpressionRole, StatementNode, SymbolHandle};
        use super::collecting::{
            collect_argument_calls, collect_assignment_calls, collect_constructor_calls,
            collect_owned_root_calls, collect_range_calls, collect_root_calls,
        };
        use super::{is_terminal_call_occurrence, ordered_statement_call};
        use crate::execution::terminal_unit::calls::boundary_authority_refusal;
        use crate::execution::terminal_unit::control::{statement_sequence, structural_operands};
        use crate::flow::indexed_statements;
        use crate::flow::state_statements;

        use crate::execution::terminal_unit::control::LocalConstructionTrace;

        /*
            Sequence only the common body. Selected exit operands retain their own
            computation/value roots and must not consume the body's call roster.
            Trace the exact admission step and statement when occurrence custody fails.
        */
        pub(in crate::execution::terminal_unit) fn outer_calls_in_prefix_traced<'a>(
            program: &dyn ::validation::CheckingInput,
            facts: &'a CheckFacts,
            machine: SymbolHandle,
            state: &typed_trees::state::State,
            calls: &'a [checked_trees::FlowCallFact],
            statement_end: std::ops::Bound<usize>,
            trace: &LocalConstructionTrace,
        ) -> Option<Vec<&'a checked_trees::FlowCallFact>> {
            trace.phase("outer calls: statement window");
            let statements = program.statement_table.statements(state.statement_nodes);
            let erased_statement = |statement: &StatementNode| matches!(statement, StatementNode::LocalData(local) if local.relevance.is_erased());
            let omitted = |call: &checked_trees::FlowCallFact| {
                facts.flow.control.is_retired(state.symbol, call)
                    || statements
                        .get(call.statement_index)
                        .is_some_and(erased_statement)
            };
            let scalar_intrinsic = |call: &checked_trees::FlowCallFact| {
                program
                    .symbols
                    .builtin_function_for_symbol(call.target_symbol)
                    .is_some_and(|function| function.has_empty_write_frame())
            };
            let execution_order = program
                .statement_table
                .admitted_execution_order(state.statement_nodes);
            let end_position = execution_order.prefix_end(statement_end)?;
            let in_prefix = |source| {
                execution_order
                    .position(source)
                    .is_some_and(|position| position < end_position)
            };
            let mut consumed = super::CallOccurrenceLedger::default();
            let mut structural = Vec::<&checked_trees::FlowCallFact>::new();
            let mut outer = Vec::new();
            let owner = program.machine_by_symbol(machine)?;
            let mut scalar_local_count = 0u32;
            let control_prefix = statement_sequence::scalar_control(program, facts, owner, state)
                .map(|(_, prefix)| prefix);
            for (statement_index, statement) in indexed_statements(program, state.statement_nodes) {
                if erased_statement(statement) {
                    continue;
                }
                let scalar_binding_ordinal = scalar_local_count;
                if let Some(local) = statement.as_local_data()
                    && !local.is_mutable
                    && program
                        .primitive_type_reference(local.type_reference)
                        .is_some()
                    && !control_prefix.is_some_and(|prefix| statement_index >= prefix)
                {
                    scalar_local_count = scalar_local_count.checked_add(1)?;
                }
                if !in_prefix(statement_index) {
                    continue;
                }
                let local_range = if let Some(local) = statement.as_local_data()
                    && !local.is_mutable
                    && collect_range_calls(
                        program,
                        facts,
                        machine,
                        state.symbol,
                        statement_index,
                        local.initial_value,
                        checked_trees::CheckedSubsliceSite::LocalBinding,
                        calls,
                        0,
                        &mut consumed,
                    )? {
                    local.initial_value
                } else {
                    typed_trees::expression::ExpressionHandle::invalid()
                };
                let statement_ordinal = u32::try_from(statement_index).ok()?;
                for root in facts
                    .values
                    .structural_values
                    .roots_for_statement(state.symbol, statement_ordinal)
                {
                    if root.machine != machine {
                        return None;
                    }
                    if local_range.is_valid() && root.expression == local_range {
                        continue;
                    }
                    super::structural_values::collect(
                        program,
                        facts,
                        machine,
                        state.symbol,
                        statement_index,
                        root.root,
                        calls,
                        0,
                        &mut consumed,
                        &mut structural,
                    )?;
                }
                collect_constructor_calls(
                    program,
                    facts,
                    machine,
                    state,
                    statement_index,
                    statement,
                    calls,
                    &mut consumed,
                )?;
                if control_prefix.is_some_and(|prefix| statement_index >= prefix) {
                    for root in facts
                        .values
                        .scalar_computations
                        .roots_for_statement(state.symbol, statement_ordinal)
                        .into_iter()
                        .filter(|root| {
                            matches!(
                                root.role,
                                CheckedScalarExpressionRole::Guard
                                    | CheckedScalarExpressionRole::Return
                                    | CheckedScalarExpressionRole::ContinuationReturn
                            )
                        })
                    {
                        collect_owned_root_calls(
                            program,
                            facts,
                            machine,
                            state.symbol,
                            statement_index,
                            root,
                            calls,
                            &mut consumed,
                        )?;
                    }
                    continue;
                }
                if let Some(local) = statement.as_local_data()
                    && let Some(carrier) =
                        validation::atomic_once_carrier(program, local.initial_value)
                {
                    let primitive = validation::declared_place_type_raw(
                        program,
                        owner,
                        Some(state),
                        carrier.place,
                    )
                    .and_then(|cell| program.primitive_type_reference(cell))?;
                    let plans = &facts.values.scalar_computations;
                    for (ordinal, expression) in carrier.operands.into_iter().enumerate() {
                        let role = CheckedScalarExpressionRole::AtomicOperand {
                            operand_ordinal: u32::try_from(ordinal).ok()?,
                        };
                        if let Some(root) =
                            plans.root_at(state.symbol, u32::try_from(statement_index).ok()?, role)
                        {
                            if !plans.is_authored_root(root, machine, expression, primitive) {
                                return None;
                            }
                            collect_root_calls(
                                program,
                                facts,
                                state.symbol,
                                statement_index,
                                root.root,
                                calls,
                                0,
                                &mut consumed,
                            )?;
                        }
                    }
                }
                if let Some(local) = statement.as_local_data()
                    && let Some(primitive_type) =
                        program.primitive_type_reference(local.type_reference)
                {
                    let role = if local.is_mutable {
                        CheckedScalarExpressionRole::StorageInitializer
                    } else {
                        CheckedScalarExpressionRole::LocalInitializer {
                            binding_ordinal: scalar_binding_ordinal,
                        }
                    };
                    if let Some(root) = facts.values.scalar_computations.root_at(
                        state.symbol,
                        u32::try_from(statement_index).ok()?,
                        role,
                    ) {
                        let plans = &facts.values.scalar_computations;
                        if !plans.is_authored_root(
                            root,
                            machine,
                            local.initial_value,
                            primitive_type,
                        ) {
                            return None;
                        }
                        collect_root_calls(
                            program,
                            facts,
                            state.symbol,
                            statement_index,
                            root.root,
                            calls,
                            0,
                            &mut consumed,
                        )?;
                    }
                }
                if statement_index.checked_add(1)?
                    == state_statements(program, state.statement_nodes).len()
                    && let Some(expression) =
                        super::completion_expression(program, owner, state, statement_index)
                    && let Some(root) = facts.values.scalar_computations.root_at(
                        state.symbol,
                        u32::try_from(statement_index).ok()?,
                        CheckedScalarExpressionRole::Return,
                    )
                {
                    let primitive = program.primitive_type_reference(state.return_type)?;
                    if !facts
                        .values
                        .scalar_computations
                        .is_authored_root(root, machine, expression, primitive)
                    {
                        return None;
                    }
                    collect_root_calls(
                        program,
                        facts,
                        state.symbol,
                        statement_index,
                        root.root,
                        calls,
                        0,
                        &mut consumed,
                    )?;
                }
                let Some(assignment) = statement.as_assignment() else {
                    continue;
                };
                collect_assignment_calls(
                    program,
                    facts,
                    machine,
                    state,
                    statement_index,
                    assignment,
                    calls,
                    &mut consumed,
                )?;
            }
            let mut occurrence_order = calls
                .iter()
                .filter(|call| {
                    in_prefix(call.statement_index) && !scalar_intrinsic(call) && !omitted(call)
                })
                .collect::<Vec<_>>();
            occurrence_order.sort_by_key(|call| (call.statement_index, call.call_ordinal));
            if occurrence_order.windows(2).any(|pair| {
                (pair[0].statement_index, pair[0].call_ordinal)
                    == (pair[1].statement_index, pair[1].call_ordinal)
            }) {
                return None;
            }
            let mut claimed_structural = 0;
            let expands =
                |call: &checked_trees::FlowCallFact| !scalar_intrinsic(call) && !omitted(call);
            for call in occurrence_order {
                claim_structural_operand_calls(
                    program,
                    facts,
                    owner,
                    state,
                    calls,
                    &mut structural,
                    &mut claimed_structural,
                    &mut consumed,
                    &expands,
                    trace,
                )?;
                if consumed
                    .iter()
                    .any(|handle| std::ptr::eq(facts.flow.control.calls.get(*handle), call))
                    || structural.iter().any(|owned| std::ptr::eq(*owned, call))
                {
                    continue;
                }
                trace.phase("outer calls: duplicate statement call");
                trace.statement(u32::try_from(call.statement_index).ok());
                if outer.iter().any(|prior: &&checked_trees::FlowCallFact| {
                    prior.statement_index == call.statement_index
                }) {
                    for unclaimed in outer.iter().copied().chain(std::iter::once(call)) {
                        if let Some(reason) =
                            boundary_authority_refusal(program, machine, state, unclaimed)
                        {
                            trace.phase(reason);
                            break;
                        }
                    }
                    return None;
                }
                outer.push(call);
                trace.phase("outer calls: structural operands");
                trace.statement(u32::try_from(call.statement_index).ok());
                for nested in structural_operands::for_call(program, facts, owner, state, call)? {
                    if structural.iter().any(|prior| std::ptr::eq(*prior, nested)) {
                        continue;
                    }
                    structural.push(nested);
                }
                trace.phase("outer calls: ordered statement call");

                trace.statement(u32::try_from(call.statement_index).ok());
                let statement = program
                    .statement_table
                    .statement_at(state, call.statement_index)?;
                if matches!(statement, StatementNode::Expression(_)) {
                    let (expression, authored) =
                        ordered_statement_call(program, owner, state, call.statement_index)?;
                    if call.authored_expression != expression
                        || call.target_symbol
                            != validation::call_target_entry_symbol(
                                program,
                                validation::call_target_symbol(program, authored),
                            )
                    {
                        return None;
                    }
                }
                let result_expression = match statement {
                    StatementNode::LocalData(local) => Some(local.initial_value),
                    StatementNode::Assignment(assignment) => Some(assignment.value),
                    _ => None,
                };
                if result_expression
                    .is_some_and(|expression| call.authored_expression != expression)
                {
                    return None;
                }
                if let StatementNode::Transition(transition) = statement
                    && !is_terminal_call_occurrence(program, owner, state, call)
                {
                    if matches!(
                        program.statement_table.transition_target(transition.target),
                        typed_trees::statement::TransitionTargetNode::Named { .. }
                    ) {
                        continue;
                    }
                    return None;
                }
                trace.phase("outer calls: exact call occurrence");
                trace.statement(u32::try_from(call.statement_index).ok());
                let site = validation::find_call_site_in_state(
                    program,
                    owner,
                    state,
                    call.statement_index,
                    call.call_ordinal,
                )?;
                let terminal_completion = is_terminal_call_occurrence(program, owner, state, call);
                if !terminal_completion {
                    validation::call_is_unconditional(
                        program,
                        machine,
                        state.symbol,
                        call.statement_index,
                        call.call_ordinal,
                    )?;
                }
                let target = match &site {
                    validation::CallSite::Expression {
                        expression,
                        call: authored,
                    } => {
                        if *expression != call.authored_expression {
                            return None;
                        }
                        validation::call_target_symbol(program, authored)
                    }
                    validation::CallSite::Statement(authored) => authored.target_symbol,
                    validation::CallSite::TransitionNamed { path, .. } => {
                        validation::terminal_callable_target(program, owner, path.symbol)
                            .map_or(path.symbol, |(callable, _)| callable)
                    }
                };
                if validation::call_target_entry_symbol(program, target) != call.target_symbol {
                    return None;
                }
                let arguments = validation::call_site_argument_expressions(program, &site);
                trace.phase("outer calls: argument calls");
                trace.statement(u32::try_from(call.statement_index).ok());
                collect_argument_calls(
                    program,
                    facts,
                    machine,
                    state.symbol,
                    call,
                    arguments,
                    calls,
                    &mut consumed,
                )?;
            }
            claim_structural_operand_calls(
                program,
                facts,
                owner,
                state,
                calls,
                &mut structural,
                &mut claimed_structural,
                &mut consumed,
                &expands,
                trace,
            )?;
            trace.phase("outer calls: unconsumed nested calls");
            trace.statement(None);
            if let Some(unconsumed) = calls
                .iter()
                .filter(|call| {
                    in_prefix(call.statement_index) && !scalar_intrinsic(call) && !omitted(call)
                })
                .find(|call| {
                    !consumed
                        .iter()
                        .any(|handle| std::ptr::eq(facts.flow.control.calls.get(*handle), *call))
                        && !structural.iter().any(|nested| std::ptr::eq(*nested, *call))
                        && !outer
                            .iter()
                            .any(|scheduled| std::ptr::eq(*scheduled, *call))
                })
            {
                trace.phase(match statements.get(unconsumed.statement_index) {
                    Some(StatementNode::Assignment(_)) => {
                        "outer calls: unconsumed nested call in an assignment"
                    }
                    Some(StatementNode::LocalData(_)) => {
                        "outer calls: unconsumed nested call in a local initializer"
                    }
                    Some(StatementNode::Call(_)) => {
                        "outer calls: unconsumed nested call in a call statement"
                    }
                    Some(StatementNode::Expression(_)) => {
                        "outer calls: unconsumed nested call in an expression statement"
                    }
                    Some(StatementNode::Transition(_)) => {
                        "outer calls: unconsumed nested call in a transition"
                    }
                    Some(StatementNode::Assembly(_)) => {
                        "outer calls: control metadata contains no call"
                    }
                    Some(StatementNode::AssemblyFact(_)) => {
                        "outer calls: unconsumed nested call in an assembly fact"
                    }
                    None => "outer calls: unconsumed nested call past the statement window",
                });
                trace.statement(u32::try_from(unconsumed.statement_index).ok());
                return None;
            }
            Some(outer)
        }

        /*
            Calls reached through the structural value walk (a record or case
            initializer holding a call node) and the structural operands of scheduled
            calls skip the outer-call loop's operand gathering. Claim each pending
            one's structural operand calls into `structural` and its argument calls
            into `consumed`, so transitively nested calls are owned before the loop
            visits them and are not reported unconsumed. In
            `combine(produce(&mut count), tick(&mut count), produce(&mut count)).value`
            the value-walk call `combine` must claim `tick` before preorder reaches it.
        */
        #[allow(clippy::too_many_arguments)]
        fn claim_structural_operand_calls<'a>(
            program: &dyn ::validation::CheckingInput,
            facts: &'a CheckFacts,
            owner: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            calls: &'a [checked_trees::FlowCallFact],
            structural: &mut Vec<&'a checked_trees::FlowCallFact>,
            claimed: &mut usize,
            consumed: &mut super::CallOccurrenceLedger<'_>,
            expands: &dyn Fn(&checked_trees::FlowCallFact) -> bool,
            trace: &LocalConstructionTrace,
        ) -> Option<()> {
            while let Some(call) = structural.get(*claimed).copied() {
                *claimed += 1;
                trace.phase("outer calls: structural operand expansion");
                trace.statement(u32::try_from(call.statement_index).ok());
                if expands(call) {
                    for nested in structural_operands::for_call(program, facts, owner, state, call)?
                    {
                        if !structural.iter().any(|prior| std::ptr::eq(*prior, nested)) {
                            structural.push(nested);
                        }
                    }
                }
                trace.phase("outer calls: structural argument site");
                trace.statement(u32::try_from(call.statement_index).ok());
                let site = validation::find_call_site(
                    program,
                    owner.symbol,
                    state.symbol,
                    call.statement_index,
                    call.call_ordinal,
                )?;
                let arguments = validation::call_site_argument_expressions(program, &site);
                trace.phase("outer calls: structural argument calls");
                collect_argument_calls(
                    program,
                    facts,
                    owner.symbol,
                    state.symbol,
                    call,
                    arguments,
                    calls,
                    consumed,
                )?;
            }
            Some(())
        }
    }
    pub(in crate::execution::terminal_unit) use selected_calls::SelectedInvocationAccounting;

    pub(in crate::execution::terminal_unit) use collecting::call_owns_statement_computations;
    pub(in crate::execution::terminal_unit) use traced::outer_calls_in_prefix_traced;

    pub(in crate::execution::terminal_unit) fn terminal_invocation(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
    ) -> Option<SymbolHandle> {
        use typed_trees::statement::{TransitionExit, TransitionGuardNode};
        let statements = typed_trees::statement::reachable_state_statements(
            program.statement_table.statements(state.statement_nodes),
        );
        if statement_index.checked_add(1)? != statements.len() {
            return None;
        }
        let transition = statements.get(statement_index)?.as_transition()?;
        if transition.guard != TransitionGuardNode::Always
            || transition.continuation.is_valid()
            || transition.exit != TransitionExit::Ordinary
            || !program
                .statement_table
                .transition_target_is_valid(transition.target)
        {
            return None;
        }
        if statements[..statement_index]
            .iter()
            .any(|statement| matches!(statement, StatementNode::Transition(_)))
        {
            return None;
        }
        let invocation = validation::completing_transition_invocation(program, transition.target)?;
        let (callable, return_type) =
            validation::terminal_callable_target(program, machine, invocation.target)?;
        (program.normalized_type_identity(return_type)
            == program.normalized_type_identity(state.return_type))
        .then_some(callable)
    }

    /*
        The same final value may be a bare expression or a normalized terminal
        transition. Both call accounting and statement sequencing must give its
        retained value graph the invocation, rather than schedule it a second time.
    */
    pub(in crate::execution::terminal_unit) fn completion_expression(
        program: &dyn validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
    ) -> Option<typed_trees::expression::ExpressionHandle> {
        match program
            .statement_table
            .statement_at(state, statement_index)?
        {
            StatementNode::Expression(expression) => Some(*expression),
            StatementNode::Transition(transition)
                if terminal_invocation(program, machine, state, statement_index).is_some() =>
            {
                match program.statement_table.transition_target(transition.target) {
                    typed_trees::statement::TransitionTargetNode::Value(expression) => {
                        Some(*expression)
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    pub(in crate::execution::terminal_unit) fn tail_call<'a>(
        program: &'a dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        statement_index: usize,
    ) -> Option<(
        typed_trees::expression::ExpressionHandle,
        &'a typed_trees::expression::TableCallExpression,
    )> {
        let statements = program.statement_table.statements(state.statement_nodes);
        if !is_unit(program, state.return_type)
            || statement_index.checked_add(1)? != statements.len()
        {
            return None;
        }
        let expression = statements.get(statement_index)?.as_expression()?;
        if !program.expression_table.expression_is_valid(*expression) {
            return None;
        }
        let call = program.expression_table.call_node(*expression)?;
        Some((*expression, call))
    }

    /*
        A Unit-result call can occupy a statement without being the state's tail.
        Tail consumers retain their existing final-statement check above.
    */
    pub(in crate::execution::terminal_unit) fn unit_statement_call<'a>(
        program: &'a dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
    ) -> Option<(
        typed_trees::expression::ExpressionHandle,
        &'a typed_trees::expression::TableCallExpression,
    )> {
        if let Some(call) = tail_call(program, state, statement_index) {
            return Some(call);
        }
        let expression = program
            .statement_table
            .statement_at(state, statement_index)?
            .as_expression()?;
        if !program.expression_table.expression_is_valid(*expression) {
            return None;
        }
        let call = program.expression_table.call_node(*expression)?;
        validation::unit_statement_call_is_admitted(program, machine, state, *expression)
            .then_some((*expression, call))
    }

    pub(super) fn ordered_statement_call<'program>(
        program: &'program dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
    ) -> Option<(
        typed_trees::expression::ExpressionHandle,
        &'program typed_trees::expression::TableCallExpression,
    )> {
        if let Some(call) = unit_statement_call(program, machine, state, statement_index) {
            return Some(call);
        }
        let expression = program
            .statement_table
            .statement_at(state, statement_index)?
            .as_expression()?;
        let call = program.expression_table.call_node(*expression)?;
        if !crate::values::is_scalar_return_call(program, state, *expression)
            && (statement_index.checked_add(1)?
                != crate::flow::state_statements(program, state.statement_nodes).len()
                || program.normalized_type_identity(validation::call_target_return_type(
                    program,
                    validation::call_target_symbol(program, call),
                )?) != program.normalized_type_identity(state.return_type)
                || checked_structural_result_type(
                    program,
                    &mut ShapeCollector::new(program),
                    state.return_type,
                    &machine_binders(program, machine),
                )
                .is_none())
        {
            return None;
        }
        Some((*expression, call))
    }

    /* `outer_calls_in_prefix` over the whole body, tracing where admission stopped. */
    pub(in crate::execution::terminal_unit) fn outer_calls_traced<'a>(
        program: &dyn ::validation::CheckingInput,
        facts: &'a CheckFacts,
        machine: SymbolHandle,
        state: &typed_trees::state::State,
        calls: &'a [checked_trees::FlowCallFact],
        trace: &LocalConstructionTrace,
    ) -> Option<Vec<&'a checked_trees::FlowCallFact>> {
        outer_calls_in_prefix_traced(
            program,
            facts,
            machine,
            state,
            calls,
            std::ops::Bound::Unbounded,
            trace,
        )
    }

    /*
        Authenticate the outer invocation of an unconditional terminal call. Nested
        operands can share its declaration but never inherit its control position.
    */
    pub(in crate::execution::terminal_unit) fn is_terminal_call_occurrence(
        program: &dyn validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        call: &checked_trees::FlowCallFact,
    ) -> bool {
        let Some(expected) = terminal_invocation(program, machine, state, call.statement_index)
        else {
            return false;
        };
        if validation::terminal_callable_target(program, machine, call.target_symbol)
            .is_none_or(|(target, _)| target != expected)
        {
            return false;
        }
        let Some(transition) = program
            .statement_table
            .statement_at(state, call.statement_index)
            .and_then(StatementNode::as_transition)
        else {
            return false;
        };
        validation::find_call_site_in_state(
            program,
            machine,
            state,
            call.statement_index,
            call.call_ordinal,
        )
        .is_some_and(|site| site.completes_transition(program, transition.target))
    }
}
mod call_results {
    /*
        Binding scalar and structural call results to checked unit locals.

        Ordinary completion transfers an already established value, including byte
        predicates represented in its field types. Fresh construction does not gain
        that authority from its declaration: it must still establish each member.
        Both routes retain the exact carrier for the receiving custody replay.
    */
    use crate::checks::type_multiplicity;
    use crate::execution::terminal_unit::types::borrowed_slice_result;
    use crate::execution::terminal_unit::types::substituted_formal_type;
    use crate::execution::terminal_unit::types::{
        ShapeCollector, is_unit, parameter_qualifications,
    };
    use checked_trees::{CheckedUnitCallPlan, CheckedUnitCallResult, CheckedUnitCallRoute};

    use crate::execution::terminal_unit::{
        CheckFacts, CheckedUnitEffectOperationPlan, CheckedUnitScalarResultBindingPlan,
        CheckedUnitStructuralResultBindingPlan, Multiplicity, SymbolHandle, TypeReferenceHandle,
        is_reference,
    };

    /*
        Rebind a Unit call's result to the scalar local its statement declares.
        A boundary call keeps its completion receipts; an ordinary call gains its
        target's contract commitment. Boundary targets need `allow_boundary`.
    */
    pub(crate) fn bind_scalar_call_result(
        facts: &CheckFacts,
        operation: CheckedUnitEffectOperationPlan,
        result: CheckedUnitScalarResultBindingPlan,
        allow_boundary: bool,
    ) -> Option<CheckedUnitEffectOperationPlan> {
        let operation = match operation {
            CheckedUnitEffectOperationPlan::RegisterAccess {
                coordinate,
                operation,
                arguments,
                service_reach,
                result: None,
            } if operation.produces_value()
                && operation.result_carrier().is_some_and(|carrier| {
                    typed_trees::types::PrimitiveType::from_name(carrier.name())
                        == Some(result.primitive_type)
                }) =>
            {
                return Some(CheckedUnitEffectOperationPlan::RegisterAccess {
                    coordinate,
                    operation,
                    arguments,
                    service_reach,
                    result: Some(result),
                });
            }
            operation => operation,
        };
        let CheckedUnitEffectOperationPlan::Call(call) = operation else {
            return None;
        };
        if *call.result() == CheckedUnitCallResult::NoNormal {
            return Some(CheckedUnitEffectOperationPlan::Call(call));
        }
        let mut parts = call.into_parts();
        if parts.result != CheckedUnitCallResult::Unit {
            return None;
        }
        match &mut parts.route {
            CheckedUnitCallRoute::ActivationStorage { .. } => return None,
            CheckedUnitCallRoute::DescriptorTable { result_type, .. } => {
                if result.primitive_type != *result_type {
                    return None;
                }
            }
            CheckedUnitCallRoute::Boundary if !allow_boundary => return None,
            CheckedUnitCallRoute::Boundary | CheckedUnitCallRoute::Dynamic { .. } => {}
            CheckedUnitCallRoute::Module {
                contract_commitment,
                ..
            } => {
                *contract_commitment = Some(
                    facts
                        .contract_plans
                        .for_machine(parts.target.machine)?
                        .commitment,
                );
            }
        }
        parts.result = CheckedUnitCallResult::Scalar(result);
        CheckedUnitCallPlan::new(parts).map(CheckedUnitEffectOperationPlan::Call)
    }

    /*
        Keep the checked call's exact target, source coordinate, operands and
        completion receipts while attaching its independently checked result local.
        An ordinary structural call already carries its result and custody.
    */
    pub(crate) fn bind_structural_call_result(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        caller_machine: SymbolHandle,
        caller_state: SymbolHandle,
        operation: CheckedUnitEffectOperationPlan,
        result: CheckedUnitStructuralResultBindingPlan,
    ) -> Option<CheckedUnitEffectOperationPlan> {
        let CheckedUnitEffectOperationPlan::Call(call) = operation else {
            return None;
        };
        if *call.result() == CheckedUnitCallResult::NoNormal {
            return Some(CheckedUnitEffectOperationPlan::Call(call));
        }
        match (call.route(), call.result()) {
            (
                CheckedUnitCallRoute::Module { .. }
                | CheckedUnitCallRoute::Dynamic { .. }
                | CheckedUnitCallRoute::ActivationStorage { .. },
                CheckedUnitCallResult::Structural { .. },
            ) => Some(CheckedUnitEffectOperationPlan::Call(call)),
            (CheckedUnitCallRoute::Boundary, CheckedUnitCallResult::Unit) => {
                let mut parts = call.into_parts();
                let occurrence = facts.flow.control.call_at(
                    caller_state,
                    parts.coordinate.statement_index as usize,
                    parts.coordinate.call_ordinal as usize,
                )?;
                let return_type = validation::call_target_return_type(program, parts.target.state)?;
                let substitutions = parts
                    .source_site
                    .and_then(|site| {
                        facts
                            .requirement_call_specializations
                            .for_site(site, occurrence.target_symbol)
                    })
                    .map(|specialization| {
                        specialization
                            .type_bindings
                            .iter()
                            .map(|binding| (binding.parameter, binding.actual))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let return_type = substituted_formal_type(program, return_type, &substitutions);
                let result_claims = validation::structural_call_result_claims(
                    program,
                    facts,
                    caller_machine,
                    caller_state,
                    occurrence,
                    return_type,
                )
                .ok()?;
                let checked_trees::CheckedUnitCallTransfers::CompletionReceipts(claim_transfers) =
                    parts.transfers
                else {
                    return None;
                };
                parts.transfers = checked_trees::CheckedUnitCallTransfers::StructuralCustody(
                    checked_trees::CheckedStructuralCallCustodyPlan {
                        claim_transfers,
                        result_claims,
                        ..Default::default()
                    },
                );
                parts.result = CheckedUnitCallResult::Structural {
                    discard_result_on_return: result.multiplicity == Multiplicity::Affine,
                    binding: result,
                };
                CheckedUnitCallPlan::new(parts).map(CheckedUnitEffectOperationPlan::Call)
            }
            _ => None,
        }
    }

    /*
        An independently replayed copy preserves the established leaf's complete
        represented contents, including bounded byte predicates and live lengths.
        The shared construction admission below cannot grant that authority: a
        fresh initializer still owes its member proofs. This binding only
        classifies storage; the copied value plan must separately rejoin its
        readable source and exact leaf type.
    */
    pub(crate) fn checked_copied_result_type(
        program: &dyn ::validation::CheckingInput,
        shapes: &mut ShapeCollector<'_>,
        result_type: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
    ) -> Option<CheckedUnitStructuralResultBindingPlan> {
        if let Some(result) = checked_structural_result_type(program, shapes, result_type, binders)
        {
            return Some(result);
        }
        if type_multiplicity(program, result_type) != Multiplicity::Unrestricted
            || !validation::has_contents(
                program,
                result_type,
                &[],
                validation::ContentsRequirement::RepresentedOwned,
            )
            || !parameter_qualifications(program, shapes, result_type, binders)?.is_empty()
        {
            return None;
        }
        Some(CheckedUnitStructuralResultBindingPlan {
            statement_index: 0,
            binding_ordinal: 0,
            type_identity: shapes.add_type(result_type, binders, &[])?,
            multiplicity: Multiplicity::Unrestricted,
            local: Default::default(),
        })
    }

    /*
        Result shape is independent of whether the source binds or discards it,
        and of whether a literal constructs the value here or a completed producer
        hands it over. Both answer to one storage judgment: borrow-free owned
        contents, independent of the cleanup they require. A scalar member under a plain domain whose
        complete predicate is represented, and a bounded byte member retaining its
        primitive predicates, are as carriable when a call returns the record as
        when this body builds it -- the producer discharged the member obligation
        either way, and classifying storage never establishes one.

        Linear value classification does not establish custody: the ordinary call
        producer must retain the independently reconstructed transfer/return record.
        Nominal cleanup still requires its checked settlement plan.
        Primitive arrays use their complete-shape classifier, including dimensions
        with no leaves; the older owned-storage classifier excludes empty arrays.
    */
    pub(crate) fn checked_structural_result_type(
        program: &dyn ::validation::CheckingInput,
        shapes: &mut ShapeCollector<'_>,
        result_type: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
    ) -> Option<CheckedUnitStructuralResultBindingPlan> {
        if validation::reference_result_custody::stored_loan_parts(program, result_type).is_some() {
            return Some(CheckedUnitStructuralResultBindingPlan {
                statement_index: 0,
                binding_ordinal: 0,
                type_identity: shapes.add_reference_type(result_type, binders)?,
                multiplicity: Multiplicity::Affine,
                local: Default::default(),
            });
        }
        if borrowed_slice_result(program, result_type) {
            return Some(CheckedUnitStructuralResultBindingPlan {
                statement_index: 0,
                binding_ordinal: 0,
                type_identity: shapes.add_stored_type(result_type, binders, &[])?,
                multiplicity: validation::reference_result_custody::result_multiplicity(
                    program,
                    result_type,
                ),
                local: Default::default(),
            });
        }
        let multiplicity = type_multiplicity(program, result_type);
        parameter_qualifications(program, shapes, result_type, binders)?;
        if multiplicity == Multiplicity::Unrestricted
            && validation::bounded_byte_buffer_capacity(program, result_type).is_some()
            && validation::has_contents(
                program,
                result_type,
                &[],
                validation::ContentsRequirement::RepresentedOwned,
            )
        {
            return Some(CheckedUnitStructuralResultBindingPlan {
                statement_index: 0,
                binding_ordinal: 0,
                type_identity: shapes.add_type(result_type, binders, &[])?,
                multiplicity,
                local: Default::default(),
            });
        }
        if is_unit(program, result_type)
            || program.primitive_type_reference(result_type).is_some()
            || is_reference(program, result_type)
            || (multiplicity != Multiplicity::Linear
                && (!(validation::has_contents(
                    program,
                    result_type,
                    &[],
                    validation::ContentsRequirement::BorrowFree,
                ) || validation::is_closed_primitive_array_type(program, result_type)
                    || validation::reference_result_custody::is_reference_record(
                        program,
                        result_type,
                    )
                    || validation::has_contents(
                        program,
                        result_type,
                        &[],
                        validation::ContentsRequirement::OwnedOrSharedView,
                    ))))
        {
            return None;
        }
        Some(CheckedUnitStructuralResultBindingPlan {
            statement_index: 0,
            binding_ordinal: 0,
            type_identity: shapes.add_type(result_type, binders, &[])?,
            multiplicity: validation::reference_result_custody::result_multiplicity(
                program,
                result_type,
            ),
            local: Default::default(),
        })
    }
}
mod checked_machine {
    /*
        Building one checked machine from its typed states and statements.
    */
    use crate::execution::terminal_unit::ScalarCalleeClosure;
    use crate::execution::terminal_unit::control::statement_sequence;

    use crate::execution::terminal_unit::calls::{
        build_affine_array_construction_prefix, entry_claims,
        free_structural_scalar_signature_traced,
    };
    use crate::execution::terminal_unit::control::LocalConstructionTrace;
    use crate::execution::terminal_unit::control::call_occurrences;
    use crate::execution::terminal_unit::control::statement_sequence::StatementSequence;
    use crate::execution::terminal_unit::types::{
        ShapeCollector, checked_state_contracts_have_plan, is_unit, machine_binders,
        parameter_storage_plans, return_unit_affine_discards, state_flow,
    };
    use crate::execution::terminal_unit::{
        BTreeSet, CheckFacts, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan,
        CheckedUnitPartialAffineDiscardPlan,
    };

    /*
        Build the machine while retaining the exit edge's residual
        discards alongside the machine plan. The ordinary Unit roster publishes
        only machines whose argument custody is whole-root; a body that moved an
        owned parameter through a projection is republished by the partial-affine
        cleanup lane instead — residual-bearing returns lower to
        `ReturnUnitPartialAffine` and fully consumed roots keep `ReturnUnit`.
    */
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn build_checked_machine_residual_parts(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        scalar_callees: ScalarCalleeClosure,
        shapes: &mut ShapeCollector<'_>,
        prepared_calls: &mut crate::execution::terminal_unit::structural_value_calls::PreparedCalls,
        machine: &typed_trees::machine::Machine,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
        trace: &LocalConstructionTrace,
    ) -> Option<(
        CheckedUnitEffectMachinePlan,
        Vec<CheckedUnitPartialAffineDiscardPlan>,
        bool,
    )> {
        trace.phase("single-state body");
        let [state] = program.machine_states(machine) else {
            return None;
        };
        trace.phase("result type");
        if !is_unit(program, state.return_type)
            && validation::reference_result_custody::parts(program, state.return_type).is_none()
            && validation::reference_result_custody::mutable_borrowed_parts(
                program,
                state.return_type,
            )
            .is_none()
            && !validation::reference_result_custody::is_reference_record(
                program,
                state.return_type,
            )
            && !crate::execution::terminal_unit::types::borrowed_slice_result(
                program,
                state.return_type,
            )
            && !crate::execution::terminal_unit::types::borrowed_named_view(
                program,
                state.return_type,
            )
            && !(matches!(
                crate::execution::terminal_unit::types::structural_access_for_type_reference(
                    program,
                    state.return_type,
                ),
                Some(checked_trees::CheckedStructuralAccess::Owned)
            ) && validation::has_contents(
                program,
                state.return_type,
                &[],
                validation::ContentsRequirement::OwnedOrSharedView,
            ))
            && !validation::is_closed_primitive_array_type(program, state.return_type)
            && !validation::has_contents(
                program,
                state.return_type,
                &[],
                validation::ContentsRequirement::StructuralConstruction,
            )
            && !(!validation::linear_claim_frontier(program, state.return_type).is_empty()
                && validation::has_contents(
                    program,
                    state.return_type,
                    &[],
                    validation::ContentsRequirement::LinearOwned,
                ))
            && program
                .primitive_type_reference(state.return_type)
                .is_none()
        {
            return None;
        }
        trace.phase("result contract");
        if program
            .primitive_type_reference(state.return_type)
            .is_some()
            && (!crate::execution::terminal_unit::types::checked_machine_signature_has_plan(
                program, facts, machine, state,
            ) || !validation::scalar_result_contract_is_represented(program, state.return_type))
        {
            return None;
        }
        trace.phase("signature");
        let statements = program.statement_table.statements(state.statement_nodes);
        let binders = machine_binders(program, machine);
        let (mut structural_parameters, scalar_parameters) =
            free_structural_scalar_signature_traced(program, shapes, state, &binders, trace)?;
        trace.phase("signature: authored result domains");
        let result = super::super::types::declared_result_plan(program, shapes, machine)?;
        trace.phase("signature: parameter storage");
        let parameter_storage =
            parameter_storage_plans(program, shapes, state, &binders, &scalar_parameters)?;
        trace.phase("state contracts");
        if !checked_state_contracts_have_plan(
            program,
            facts,
            machine.symbol,
            state,
            &structural_parameters,
        ) {
            return None;
        }
        trace.phase("entry claims");
        let entry_claims = entry_claims(
            program,
            facts,
            machine.symbol,
            state.symbol,
            &structural_parameters,
            program.state_parameters(state),
        )?;
        trace.phase("state flow");
        let state_flow = state_flow(facts, machine.symbol, state.symbol)?;
        let source_calls = facts.flow.control.calls.span_or_empty(state_flow.calls);
        trace.phase("outer calls");
        let calls = call_occurrences::outer_calls_traced(
            program,
            facts,
            machine.symbol,
            state,
            source_calls,
            trace,
        )?;
        trace.phase("trivial affine locals");
        let (local_rows, prefix_statement_count) = match build_affine_array_construction_prefix(
            program, facts, shapes, machine, state, &binders, statements,
        ) {
            Some((rows, declaration_count)) => {
                let count = declaration_count.checked_add(rows.len())?;
                (rows, count)
            }
            None => (Vec::new(), 0),
        };
        trace.phase("statement sequence");
        let mut sequence = statement_sequence::build(
            program,
            facts,
            scalar_callees,
            shapes,
            prepared_calls,
            machine,
            state,
            &mut structural_parameters,
            &scalar_parameters,
            &parameter_storage,
            &entry_claims,
            &calls,
            &local_rows,
            prefix_statement_count,
            std::ops::Bound::Unbounded,
            call_frames,
            trace,
        )?;
        let returned = sequence.structural_result.clone();
        sequence.finish_reference_retirement(
            program,
            facts,
            prepared_calls,
            shapes,
            &structural_parameters,
            &local_rows,
            &entry_claims,
            machine,
            state,
            returned.as_ref(),
            None,
            trace,
        )?;
        trace.phase("statement sequence: borrowed windows closed");
        if !sequence.windows.is_closed() {
            return None;
        }
        let trivial_affine_locals = local_rows
            .iter()
            .map(|(plan, _)| plan.clone())
            .collect::<Vec<_>>();
        let mut admitted_local_symbols = local_rows
            .iter()
            .map(|(_, symbol)| *symbol)
            .collect::<Vec<_>>();
        admitted_local_symbols.extend(&sequence.structural_local_symbols);

        let mut operations = trivial_affine_locals
            .iter()
            .map(|local| {
                CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::TrivialAffine {
                        statement_index: local
                            .construction
                            .as_ref()
                            .and_then(|element| u32::try_from(element.index).ok())
                            .and_then(|index| index.checked_add(1))
                            .unwrap_or(local.declaration_ordinal),
                        declaration_ordinal: local.declaration_ordinal,
                        type_identity: local.type_identity,
                    },
                )
            })
            .collect::<Vec<_>>();
        trace.phase("result ownership");
        let StatementSequence {
            scalar_result,
            scalar_control,
            structural_result,
            operations: sequence_operations,
            place_loans,
            ..
        } = sequence;
        let no_normal = sequence_operations
            .iter()
            .any(|operation| !operation.has_normal_continuation());
        if !is_unit(program, state.return_type)
            && structural_result.is_none()
            && scalar_result.is_none()
            && scalar_control.is_none()
            && !no_normal
            && statement_sequence::returning_terminator(program, facts, machine, state).is_none()
        {
            return None;
        }
        operations.extend(sequence_operations);
        trace.phase("completion");
        let transferred_local_ordinals = operations
            .iter()
            .flat_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::Call(call) => call
                    .structural_arguments
                    .iter()
                    .filter_map(|argument| argument.source_local_declaration_ordinal())
                    .collect::<Vec<_>>(),
                CheckedUnitEffectOperationPlan::Store(checked_trees::CheckedStorePlan {
                    destination:
                        checked_trees::CheckedStoreDestination {
                            leaf: checked_trees::CheckedStoreLeaf::SumField,
                            ..
                        },
                    value: checked_trees::CheckedStoreValue::Place(value),
                    ..
                }) => value
                    .source_local_declaration_ordinal()
                    .into_iter()
                    .collect(),
                CheckedUnitEffectOperationPlan::RegisterAccess { .. }
                | CheckedUnitEffectOperationPlan::GlobalDescriptorTable { .. }
                | CheckedUnitEffectOperationPlan::EvaluateStructuralValue { .. }
                | CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::ScalarArray { .. },
                )
                | checked_trees::reference_hold_pattern!()
                | checked_trees::byte_sequence_establishment_pattern!()
                | CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { .. },
                )
                | CheckedUnitEffectOperationPlan::Store(_)
                | CheckedUnitEffectOperationPlan::AtomicFence { .. }
                | CheckedUnitEffectOperationPlan::AtomicAccess(_)
                | checked_trees::structural_field_write_pattern!()
                | CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::TrivialAffine { .. },
                )
                | CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::Structural { .. },
                )
                | CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::Primitive { .. },
                )
                | CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::Scalar { .. },
                )
                | CheckedUnitEffectOperationPlan::ContinuationCleanup { .. }
                | CheckedUnitEffectOperationPlan::Complete { .. } => Vec::new(),
            })
            .collect::<BTreeSet<_>>();
        trace.phase("completion: parameter cleanup");
        let (trivial_affine_discards, residual_affine_discards, has_projected_parameter_moves) =
            if no_normal {
                (Vec::new(), Vec::new(), false)
            } else {
                return_unit_affine_discards(
                    program,
                    facts,
                    prepared_calls,
                    machine.symbol,
                    state.symbol,
                    &structural_parameters,
                    program.state_parameters(state),
                    &operations,
                    &admitted_local_symbols,
                    &shapes.types,
                )?
            };
        if !no_normal {
            operations.push(CheckedUnitEffectOperationPlan::Complete {
                statement_index: u32::try_from(statements.len()).ok()?,
                trivial_affine_local_discard_ordinals: (0..trivial_affine_locals.len())
                    .rev()
                    .map(|ordinal| u32::try_from(ordinal).ok())
                    .collect::<Option<Vec<_>>>()?
                    .into_iter()
                    .filter(|ordinal| !transferred_local_ordinals.contains(ordinal))
                    .collect(),
                trivial_affine_discards,
            });
        }

        trace.phase("contract plan");
        let contract = facts.contract_plans.for_machine(machine.symbol)?;
        let body_qualifications =
            super::super::types::body_structural_qualifications(program, facts, machine.symbol);

        trace.phase("service reach");
        let erased_scalar_parameters =
            crate::execution::terminal_unit::types::erased_scalar_parameter_plans(program, state)?;
        let erased_proof_parameters =
            crate::execution::terminal_unit::types::erased_proof_parameter_plans(program, state)?;
        Some((
            CheckedUnitEffectMachinePlan {
                result,
                normal_exit_cleanups: Default::default(),
                scalar_result,
                scalar_control,
                structural_result,
                machine: machine.symbol,
                state: state.symbol,
                structural_parameters,
                scalar_parameters,
                parameter_storage,
                erased_scalar_parameters,
                erased_proof_parameters,
                trivial_affine_locals,
                entry_claims,
                body_qualifications,
                contract_report_fingerprint: contract.report_fingerprint,
                contract_commitment: contract.commitment,
                contract_service_reach: facts.service_reaches.plan_for_machine(machine.symbol)?,
                service_reach: state_flow.service_reach,
                operations,
                statement_count: u32::try_from(
                    crate::flow::state_statements(program, state.statement_nodes).len(),
                )
                .ok()?,
                place_loans,
            },
            residual_affine_discards,
            has_projected_parameter_moves,
        ))
    }
}
mod construction_trace {

    /*
        Where the ordinary checked-machine builder stopped.

        The builder returns `None` from dozens of guards. Rather than threading a
        result through every helper, the builder marks each phase as it enters it
        and the call statement it is planning; when it returns `None`, the last
        marks name the requirement family that failed and the authored state and
        statement it was planning. The trace is diagnostic only: it never changes
        which bodies are admitted.
    */
    use std::cell::Cell;

    use symbols::SymbolHandle;

    use crate::execution::terminal_unit::plan_gaps::{GapSite, plan_gap};
    use checked_trees::{CheckedUnitPlanGap, CheckedUnitPlanGapReason};

    #[derive(Debug, Default)]
    pub(crate) struct LocalConstructionTrace {
        phase: Cell<&'static str>,
        state: Cell<Option<SymbolHandle>>,
        statement_index: Cell<Option<u32>>,
        declined_step: Cell<Option<DeclinedStep>>,
        result_custody: Cell<Option<ResultCustodyFailure>>,
    }

    /*
        One position of the trace, so a builder that set a statement aside for a
        later owner can return to where it was.
    */
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct TraceMark {
        phase: &'static str,
        statement_index: Option<u32>,
        result_custody: Option<ResultCustodyFailure>,
    }

    /// Counts from the actual failed result join, not a second custody analysis.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct ResultCustodyFailure {
        pub(crate) binding: u32,
        pub(crate) local: SymbolHandle,
        pub(crate) producer_operation: usize,
        /// Implicit exit, call, cleanup, construction, reference and view owners.
        pub(crate) dispositions: [usize; 6],
        pub(crate) normal_caller_completion: bool,
    }

    /* A declined attempt belongs to one complete authored trace position. */
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct DeclineMark {
        position: TraceMark,
        state: Option<SymbolHandle>,
    }

    #[derive(Debug, Clone, Copy)]
    struct DeclinedStep {
        at: DeclineMark,
        phase: &'static str,
    }

    /// Preserve a specific attempted plan's refusal if every alternative fails.
    /// Successful alternatives explicitly discard the note; early refusal through
    /// `?` keeps the original statement and owner instead of the last fallback.
    pub(crate) struct DeferredFailure<'a> {
        trace: &'a LocalConstructionTrace,
        at: Option<DeclineMark>,
    }

    impl DeferredFailure<'_> {
        pub(crate) fn retain(&mut self) {
            self.at = Some(self.trace.decline_mark());
        }

        pub(crate) fn succeeded(&mut self) {
            self.at = None;
        }
    }

    impl Drop for DeferredFailure<'_> {
        fn drop(&mut self) {
            if let Some(at) = self.at {
                self.trace.restore(&at.position);
                self.trace.state.set(at.state);
            }
        }
    }

    impl LocalConstructionTrace {
        pub(crate) fn deferred_failure(&self) -> DeferredFailure<'_> {
            DeferredFailure {
                trace: self,
                at: None,
            }
        }

        /// Enter a phase; the statement position resets with it.
        pub(crate) fn phase(&self, phase: &'static str) {
            self.declined_step.set(None);
            self.result_custody.set(None);
            self.phase.set(phase);
            self.statement_index.set(None);
        }

        /* The current position, to return to. */
        pub(crate) fn mark(&self) -> TraceMark {
            TraceMark {
                phase: self.phase.get(),
                statement_index: self.statement_index.get(),
                result_custody: self.result_custody.get(),
            }
        }

        /* Return to a marked position. */
        pub(crate) fn restore(&self, mark: &TraceMark) {
            self.declined_step.set(None);
            self.phase.set(mark.phase);
            self.statement_index.set(mark.statement_index);
            self.result_custody.set(mark.result_custody);
        }

        pub(crate) fn decline_mark(&self) -> DeclineMark {
            DeclineMark {
                position: self.mark(),
                state: self.state.get(),
            }
        }

        /*
            Keep the primary refusal site while naming the last attempted step.
            A later builder clears this note even when it enters the same phase.
        */
        pub(crate) fn restore_with_declined_step(&self, mark: &DeclineMark) {
            let phase = self.phase.get();
            self.restore(&mark.position);
            if self.state.get() == mark.state && phase != mark.position.phase {
                self.declined_step
                    .set(Some(DeclinedStep { at: *mark, phase }));
            }
        }

        /* The call statement the current phase is planning. */
        pub(crate) fn statement(&self, index: Option<u32>) {
            self.declined_step.set(None);
            self.statement_index.set(index);
            self.result_custody.set(None);
        }

        /*
            The authored state a multi-state builder is planning; it persists
            across that state's phases until the builder moves on. A fused graph's
            state may belong to another machine of its recursive component.
        */
        pub(crate) fn state(&self, state: Option<SymbolHandle>) {
            self.declined_step.set(None);
            self.state.set(state);
            self.result_custody.set(None);
        }

        pub(crate) fn result_custody_failure(&self, failure: ResultCustodyFailure) {
            self.result_custody.set(Some(failure));
        }

        /*
            The gap at the last marked position. A statement mark without a state
            belongs to the single-state builder's only state.
        */
        pub(crate) fn gap(
            &self,
            program: &dyn ::validation::CheckingInput,
            machine: SymbolHandle,
        ) -> CheckedUnitPlanGap {
            let state = self.state.get().or_else(|| {
                program
                    .machine_by_symbol(machine)
                    .and_then(|owner| program.machine_states(owner).first())
                    .map(|state| state.symbol)
            });
            let site = match (state, self.statement_index.get()) {
                (Some(state), Some(index)) => GapSite::Statement { state, index },
                (Some(state), None) if self.state.get().is_some() => GapSite::State(state),
                _ => GapSite::Machine,
            };
            let phase =
                match self.declined_step.get() {
                    Some(step) if step.at == self.decline_mark() => std::borrow::Cow::Owned(
                        format!("{} (last declined step: {})", self.phase.get(), step.phase,),
                    ),
                    _ => std::borrow::Cow::Borrowed(self.phase.get()),
                };
            let phase = match self.result_custody.get() {
                Some(failure) => {
                    let [implicit, call, cleanup, construction, reference, view] =
                        failure.dispositions;
                    std::borrow::Cow::Owned(format!(
                        "{phase}; result home {} local {:?}, producer operation {}, normal caller completion {}; dispositions: implicit={implicit}, call={call}, cleanup={cleanup}, construction={construction}, reference={reference}, view={view}",
                        failure.binding,
                        failure.local,
                        failure.producer_operation,
                        failure.normal_caller_completion,
                    ))
                }
                None => phase,
            };
            plan_gap(
                program,
                machine,
                site,
                CheckedUnitPlanGapReason::Unsupported(phase),
            )
        }
    }
}
pub(super) mod reference_captures {
    /*
        Local reference establishments consume the common occurrence source capture.
        Local binding eligibility is separate from the captured source, storage
        interpretation and selectors used by every reference-value position.
    */
    pub(in crate::execution::terminal_unit) use crate::values::reference_sources::ReferenceCapture;

    pub(in crate::execution::terminal_unit) fn plan(
        program: &dyn validation::CheckingInput,
        facts: &checked_trees::CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
        local: &typed_trees::statement::TableLocalData,
    ) -> Option<ReferenceCapture> {
        if local.is_mutable || local.relevance.is_erased() {
            return None;
        }
        if let Some(root) = facts.values.structural_values.root_for_expression(
            state.symbol,
            u32::try_from(statement_index).ok()?,
            local.initial_value,
        ) && let checked_trees::CheckedStructuralValueKind::Reference { selectors, .. } =
            facts.values.structural_values.nodes.get(root.root).kind
            && !selectors.is_empty()
        {
            return None;
        }
        let selector = |expression| {
            let reference = validation::expression_type_reference_in_state(
                program,
                state.symbol,
                statement_index,
                expression,
            )?;
            let primitive = program.primitive_type_reference(reference)?;
            if !primitive.is_fixed_integer() {
                return None;
            }
            let checked_trees::CheckedScalarExpression::Local { position, .. } =
                crate::values::lower_unit_scalar_argument(
                    program,
                    &facts.operators,
                    state,
                    statement_index,
                    expression,
                    primitive,
                )?
            else {
                return None;
            };
            Some(
                checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(
                    checked_trees::CheckedRuntimeIndex::Local {
                        position: u32::try_from(position).ok()?,
                    },
                ),
            )
        };
        let capture = crate::values::reference_sources::capture(
            program,
            machine,
            state,
            statement_index,
            local.initial_value,
            local.type_reference,
            &selector,
        )?;
        if matches!(
            capture.source.source,
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { .. }
        ) {
            return None;
        }
        Some(capture)
    }
}
pub(in crate::execution::terminal_unit) mod scalar_arrays {
    /*
        Array operands use the same checked pure values and computations as calls.
        Each element retains a statement-local role and exact primitive destination;
        selected constant projections still retain every indexing selection.
    */
    use super::{CheckFacts, CheckedScalarExpressionRole, SymbolHandle, TypeReferenceHandle};
    use typed_trees::expression::ExpressionHandle;

    pub(in crate::execution::terminal_unit) fn elements(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement_ordinal: u32,
        source: checked_trees::CheckedArrayConstructionSource,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<Vec<checked_trees::CheckedCallScalarArgument>> {
        let leaves = validation::scalar_array_elements(program, machine, expression, expected)?;
        for projection in leaves.projections {
            if let Some(selected) = facts.operators.expression_use(projection)
                && !selected
                    .resolved_through_builtin_fallback(language_core::OperatorSpelling::Index)
            {
                return None;
            }
        }
        leaves
            .elements
            .into_iter()
            .enumerate()
            .map(|(element_index, element)| {
                let role = CheckedScalarExpressionRole::ArrayElement {
                    source,
                    element_ordinal: u32::try_from(element_index).ok()?,
                };
                let (expression, primitive_type) = match element {
                    validation::ScalarArrayElement::Authored {
                        expression,
                        primitive_type,
                    }
                    | validation::ScalarArrayElement::Projected {
                        row: expression,
                        primitive_type,
                        ..
                    } => (expression, primitive_type),
                };
                let computations = &facts.values.scalar_computations;
                let roots = computations
                    .roots
                    .iter()
                    .map(|(_, root)| root)
                    .filter(|root| {
                        root.state == state
                            && root.statement_ordinal == statement_ordinal
                            && root.role == role
                    })
                    .collect::<Vec<_>>();
                if let [root] = roots.as_slice() {
                    if !computations.is_authored_root(root, machine, expression, primitive_type)
                        || facts
                            .values
                            .scalar_expressions
                            .expressions
                            .iter()
                            .any(|value| {
                                value.state == state
                                    && value.statement_ordinal == statement_ordinal
                                    && value.role == role
                            })
                    {
                        return None;
                    }
                    return Some(checked_trees::CheckedCallScalarArgument::Computation(
                        root.root,
                    ));
                }
                if !roots.is_empty() {
                    return None;
                }
                let (binding, value) = facts.values.scalar_expressions.bound_expression_at(
                    state,
                    statement_ordinal,
                    role,
                )?;
                if binding.expression != expression
                    || crate::values::scalar_expression_type(value) != Some(primitive_type)
                {
                    return None;
                }
                Some(checked_trees::CheckedCallScalarArgument::Pure(
                    value.clone(),
                ))
            })
            .collect()
    }

    /*
        A whole closed primitive array assignment is one value establishment before
        any receiving storage changes. Keep selected operators and qualifications
        at their existing owners; only builtin literal array construction lands here.
    */
    pub(in crate::execution::terminal_unit) fn assignment_type(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        assignment: &typed_trees::statement::TableAssignment,
    ) -> Option<TypeReferenceHandle> {
        if !matches!(
            program.expression_table.expression(assignment.value),
            typed_trees::expression::ExpressionNode::ArrayLiteral(_)
        ) || !validation::place_has_builtin_coordinates(
            program,
            machine,
            Some(state),
            assignment.target,
        ) {
            return None;
        }
        let expected = validation::closed_array_store_type(
            program,
            validation::assignment_stored_value_type(program, machine, state, assignment)?,
        )?;
        validation::scalar_array_preserves_memberships(program, expected).then_some(expected)
    }
}
pub(in crate::execution::terminal_unit) mod selected_copies {
    //! Associate selected returns with their completed payload reads once. Both
    //! scalar and structural consumers use the same checked target and copy list.

    use super::CheckFacts;
    use checked_trees::{CheckedScalarBranchDestination, CheckedScalarReturnCopies};
    use std::collections::BTreeMap;
    use typed_trees::statement::{StatementNode, TransitionExit, TransitionTargetNode};

    pub(in crate::execution::terminal_unit) struct SelectedCopies<'a> {
        program: &'a dyn validation::CheckingInput,
        state: &'a typed_trees::state::State,
        outgoing:
            BTreeMap<(usize, u32, u32), Option<Vec<checked_trees::CheckedSelectedPayloadCopy>>>,
    }

    impl<'a> SelectedCopies<'a> {
        pub(in crate::execution::terminal_unit) fn new(
            program: &'a dyn validation::CheckingInput,
            facts: &'a CheckFacts,
            machine: symbols::SymbolHandle,
            state: &'a typed_trees::state::State,
        ) -> Option<Self> {
            let flow = super::super::types::state_flow(facts, machine, state.symbol)?;
            let mut outgoing = BTreeMap::new();
            for row in facts.flow.control.outgoing.span(flow.outgoing)? {
                if row.machine_symbol != machine || row.state_symbol != state.symbol {
                    return None;
                }
                let target = row.transition_target;
                let mut values = Vec::new();
                for copy in facts.flow.control.operands.span(row.copied_payloads)? {
                    let checked_trees::FlowOperandCompletion::Copied { copy: identity, .. } =
                        copy.completion
                    else {
                        return None;
                    };
                    if copy.state_symbol != state.symbol
                        || copy.statement_index != row.statement_index
                        || copy.evaluation
                            != checked_trees::FlowPermissionEvaluation::TransitionTarget(target)
                    {
                        return None;
                    }
                    if program
                        .primitive_type_reference(copy.type_reference)
                        .is_some()
                        && validation::reference_shell(program, copy.type_reference).is_none()
                    {
                        continue;
                    }
                    let root = facts.values.structural_values.root_for_captured_copy(
                        state.symbol,
                        u32::try_from(row.statement_index).ok()?,
                        identity,
                    )?;
                    if root.machine != machine
                        || root.state != state.symbol
                        || root.type_reference != copy.type_reference
                        || root.statement_ordinal as usize != row.statement_index
                        || !validation::type_satisfies_declared_property(
                            program,
                            &[],
                            root.type_reference,
                            validation::DeclaredPropertyRequirement::Copy,
                        )
                        || !validation::has_contents(
                            program,
                            root.type_reference,
                            &[],
                            validation::ContentsRequirement::DescriptorCleanup,
                        )
                    {
                        return None;
                    }
                    values.push(checked_trees::CheckedSelectedPayloadCopy {
                        value: root.root,
                        type_identity: validation::stored_structural_type_identity(
                            program,
                            root.type_reference,
                            &[],
                            &[],
                        ),
                    });
                }
                outgoing
                    .entry((
                        row.statement_index,
                        target.arena_index(),
                        target.generation(),
                    ))
                    .and_modify(|captured| *captured = None)
                    .or_insert(Some(values));
            }
            Some(Self {
                program,
                state,
                outgoing,
            })
        }

        pub(in crate::execution::terminal_unit) fn into_rows(
            self,
        ) -> Option<Vec<checked_trees::CheckedSelectedPayloadCopies>> {
            self.outgoing
                .into_iter()
                .map(|((statement, index, generation), values)| {
                    Some(checked_trees::CheckedSelectedPayloadCopies {
                        statement_ordinal: u32::try_from(statement).ok()?,
                        transition_target: arena::Handle::from_parts(index, generation),
                        values: values?,
                    })
                })
                .collect()
        }

        /// Several decision paths may enter one authored return. Publish its birth
        /// and cleanup coordinate once, in first-use order; disagreement refuses.
        pub(in crate::execution::terminal_unit) fn for_destinations<'b>(
            &self,
            destinations: impl IntoIterator<Item = &'b CheckedScalarBranchDestination>,
        ) -> Option<Vec<CheckedScalarReturnCopies>> {
            let mut positions = BTreeMap::new();
            let mut selected = Vec::new();
            for destination in destinations {
                let (_, row) = self.for_destination(destination)?;
                let key = (row.statement_ordinal, row.is_continuation);
                if let Some(&position) = positions.get(&key) {
                    if selected.get(position) != Some(&row) {
                        return None;
                    }
                } else {
                    positions.insert(key, selected.len());
                    selected.push(row);
                }
            }
            Some(selected)
        }

        pub(in crate::execution::terminal_unit) fn for_destination(
            &self,
            destination: &CheckedScalarBranchDestination,
        ) -> Option<(
            typed_trees::expression::ExpressionHandle,
            CheckedScalarReturnCopies,
        )> {
            let CheckedScalarBranchDestination::Return {
                statement_ordinal,
                is_continuation,
            } = destination
            else {
                return None;
            };
            let (expression, target, copied_payloads) = match self
                .program
                .statement_table
                .statement_at(self.state, *statement_ordinal as usize)?
            {
                StatementNode::Expression(expression) if !is_continuation => {
                    (*expression, Default::default(), Vec::new())
                }
                StatementNode::Transition(transition)
                    if transition.exit == TransitionExit::Ordinary =>
                {
                    let target = if *is_continuation {
                        transition.continuation
                    } else {
                        transition.target
                    };
                    let TransitionTargetNode::Value(expression) =
                        self.program.statement_table.transition_target(target)
                    else {
                        return None;
                    };
                    let copies = self
                        .outgoing
                        .get(&(
                            *statement_ordinal as usize,
                            target.arena_index(),
                            target.generation(),
                        ))
                        .cloned()
                        .flatten()?;
                    (*expression, target, copies)
                }
                _ => return None,
            };
            Some((
                expression,
                CheckedScalarReturnCopies {
                    statement_ordinal: *statement_ordinal,
                    is_continuation: *is_continuation,
                    transition_target: target,
                    copied_payloads,
                },
            ))
        }
    }
}
#[path = "execution_terminal_unit_control_statement_sequence.rs"]
pub(super) mod statement_sequence;
pub(super) mod structural_operands {
    /*
        Direct structural operands execute before their enclosing call, retaining
        captured preorder coordinates rather than renumbering by execution order.
        Recursive collection retains each value's consuming call and formal slot;
        flattening evaluation order must not relabel a nested operand as an outer one.
        A borrowed owned actual uses the same extraction ledger as a named move.
        Its checked expression retains detached custody while the original place
        retains repair debt. Reaccess planning runs after this preparation, so
        it never invents a second displaced value. Terminal's shared argument
        scheduler interleaves these producers with scalar operands in authored order.
    */
    use super::super::{
        CheckedStructuralResultPlan, CheckedTrivialAffineStructuralLocalPlan,
        CheckedUnitEntryClaimPlan,
    };
    use super::{
        CheckFacts, CheckedBoundaryMachineResultPlan, CheckedUnitEffectOperationPlan,
        CheckedUnitStructuralParameterPlan, CheckedUnitStructuralResultBindingPlan, Multiplicity,
        SymbolHandle,
    };
    use crate::checks::type_reference_is_reference;
    use crate::execution::terminal_unit::calls::{ExpectedCallValueResult, build_call_operation};
    use crate::execution::terminal_unit::types::{
        ShapeCollector, machine_binders, parameter_qualifications, state_flow,
    };
    use crate::published::places::place_at;
    use crate::published::semantic::calls::shared_argument_referent;
    use crate::values::nested_structural_call_return_type;
    use crate::values::reference_sources::formed_reference_type;
    use crate::values::reference_sources::transported_value_type;

    use crate::execution::terminal_unit::ScalarCalleeClosure;

    use crate::execution::terminal_unit::control::LocalConstructionTrace;
    use crate::execution::terminal_unit::control::boundary_machine::boundary_result_plan;
    use crate::execution::terminal_unit::control::call_results::bind_structural_call_result;
    use crate::execution::terminal_unit::control::call_results::checked_structural_result_type;

    pub(in crate::execution::terminal_unit) enum Operand<'facts> {
        Call(&'facts checked_trees::FlowCallFact),
        /*
            Existing storage transferred to an owned formal. Borrowed referents
            first materialize their value and retain exact restoration debt.
        */
        Storage {
            expression: typed_trees::expression::ExpressionHandle,
            type_reference: typed_trees::types::TypeReferenceHandle,
        },
        /*
            A construction or copied place retained by the checker as a
            structural value rooted at the authored argument expression. The statement
            sequence establishes it as a state-local operand before the
            consuming call, giving the argument the same binding-ordinal source
            an anonymous call result carries.
        */
        Value {
            root: &'facts checked_trees::CheckedStructuralValueRoot,
            source: checked_trees::CheckedArrayConstructionSource,
            access: checked_trees::CheckedStructuralAccess,
        },
    }

    /*
        Rejoin the checked value at one exact argument occurrence. Shared formals
        lend the completed referent home; slice formals retain their descriptor.
        This classification establishes neither a new loan nor temporary cleanup:
        the consuming call or state edge still owns those independent obligations.
    */
    pub(in crate::execution::terminal_unit) fn evaluated_argument_root<'a>(
        program: &dyn ::validation::CheckingInput,
        facts: &'a CheckFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement_index: usize,
        argument: typed_trees::expression::ExpressionHandle,
        parameter: &typed_trees::signature::StateParameter,
    ) -> Option<&'a checked_trees::CheckedStructuralValueRoot> {
        if parameter.is_const
            || parameter.relevance.is_erased()
            || program
                .primitive_type_reference(parameter.type_reference)
                .is_some()
        {
            return None;
        }
        let (argument, expected) = match program
            .type_reference_table
            .type_reference(parameter.type_reference)
        {
            typed_trees::types::TypeReferenceNode::Reference { .. }
                if validation::reference_result_custody::stored_loan_parts(
                    program,
                    parameter.type_reference,
                )
                .is_some()
                    && validation::reference_formation_source(program, argument).is_some() =>
            {
                let machine = program.machine_by_symbol(machine)?;
                let state = program.state_by_symbol(state)?;
                (
                    argument,
                    formed_reference_type(
                        program,
                        machine,
                        state,
                        argument,
                        parameter.type_reference,
                    )?,
                )
            }
            typed_trees::types::TypeReferenceNode::Reference { .. }
                if validation::slice_view_shell(program, parameter.type_reference, &[])
                    .is_some() =>
            {
                (argument, parameter.type_reference)
            }
            typed_trees::types::TypeReferenceNode::Reference { .. }
                if let Some(completed) = facts
                    .values
                    .structural_values
                    .root_for_expression(state, u32::try_from(statement_index).ok()?, argument)
                    .filter(|root| {
                        facts.values.structural_values.nodes.is_valid(root.root)
                            && matches!(
                                facts.values.structural_values.nodes.get(root.root).kind,
                                checked_trees::CheckedStructuralValueKind::Projection {
                                    copies_selected_value: true,
                                    ..
                                }
                            )
                            && validation::reference_result_custody::stored_loan_parts(
                                program,
                                root.type_reference,
                            )
                            .is_some()
                    }) =>
            {
                (argument, completed.type_reference)
            }
            typed_trees::types::TypeReferenceNode::Reference { .. }
                if let Some(actual) = transported_value_type(
                    program,
                    program.machine_by_symbol(machine)?,
                    program.state_by_symbol(state)?,
                    statement_index,
                    argument,
                ) =>
            {
                (argument, actual)
            }
            typed_trees::types::TypeReferenceNode::Reference { .. }
                if validation::reference_formation_source(program, argument).is_some() =>
            {
                (argument, parameter.type_reference)
            }
            typed_trees::types::TypeReferenceNode::Reference {
                access: language_semantics::ReferenceAccess::Shared,
                ..
            } => {
                let referent =
                    shared_argument_referent(program, argument, parameter.type_reference)?;
                if place_at(program, state, statement_index, referent.0)
                    .is_some_and(|place| place.root.as_symbol().is_some())
                {
                    return None;
                }
                referent
            }
            typed_trees::types::TypeReferenceNode::Reference { .. } => return None,
            _ => (argument, parameter.type_reference),
        };
        let root = facts.values.structural_values.root_for_expression(
            state,
            u32::try_from(statement_index).ok()?,
            argument,
        )?;
        if root.machine != machine
            || program.normalized_type_identity(root.type_reference)
                != program.normalized_type_identity(expected)
            || !facts.values.structural_values.nodes.is_valid(root.root)
            || facts
                .values
                .structural_values
                .nodes
                .get(root.root)
                .expression
                != argument
        {
            return None;
        }
        Some(root)
    }

    /*
        Ordinary calls collect a call through its existing producer route, rather
        than establishing a second whole operand. State-edge
        establishment can consume the same authenticated root with its own ordered
        producer/cleanup roster.
    */
    fn evaluated_value_root<'a>(
        program: &dyn ::validation::CheckingInput,
        facts: &'a CheckFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement_index: usize,
        argument: typed_trees::expression::ExpressionHandle,
        parameter: &typed_trees::signature::StateParameter,
    ) -> Option<&'a checked_trees::CheckedStructuralValueRoot> {
        let mut root = evaluated_argument_root(
            program,
            facts,
            machine,
            state,
            statement_index,
            argument,
            parameter,
        )?;
        while let checked_trees::CheckedStructuralValueKind::Projection {
            source, ref path, ..
        } = facts.values.structural_values.nodes.get(root.root).kind
            && !path.is_empty()
        {
            if place_at(program, state, statement_index, root.expression)
                .is_some_and(|place| place.root.as_symbol().is_some())
            {
                return None;
            }
            root = facts.values.structural_values.root_for_expression(
                state,
                u32::try_from(statement_index).ok()?,
                facts.values.structural_values.nodes.get(source).expression,
            )?;
        }
        if matches!(
            facts.values.structural_values.nodes.get(root.root).kind,
            checked_trees::CheckedStructuralValueKind::Call { .. }
        ) {
            return None;
        }
        Some(root)
    }

    /*
        Retain ordinary call plans at their expression nodes. Their order here is
        catalog order only: the structural value evaluator invokes each call when
        that operand is reached, after all earlier authored field evaluations.

        Each call retains its own argument preparation. The enclosing value
        evaluator runs that sequence when it reaches the call, after earlier fields
        and only inside the selected arm. A whole call publishes the same sequence
        into its surrounding operation stream through `establish_value`. Earlier
        result bindings provide source custody without hoisting child evaluation.
    */
    #[allow(clippy::too_many_arguments)]
    pub(in crate::execution::terminal_unit) fn value_calls(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        scalar_callees: ScalarCalleeClosure,
        shapes: &mut ShapeCollector<'_>,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        parameters: &[CheckedUnitStructuralParameterPlan],
        trivial_locals: &[(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)],
        entry_claims: &[CheckedUnitEntryClaimPlan],
        results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
        preamble: &[CheckedUnitEffectOperationPlan],
        count: &mut usize,
        root: checked_trees::CheckedStructuralValueHandle,
        trace: &LocalConstructionTrace,
        windows: &mut super::super::borrowed_windows::OpenWindows,
    ) -> Option<(
        Vec<checked_trees::CheckedStructuralValueCall>,
        checked_trees::CheckedValueCompletion,
    )> {
        let caller_trace = trace.mark();
        let plans = &facts.values.structural_values;
        let binders = machine_binders(program, machine);
        let mut scoped_results = results.to_vec();
        enum Pending {
            Value(checked_trees::CheckedStructuralValueHandle),
            Sequence {
                children: Vec<checked_trees::CheckedStructuralValueHandle>,
                next: usize,
            },
        }
        let mut pending = vec![Pending::Value(root)];
        let mut visited = Vec::new();
        let mut output = Vec::new();
        let mut subject_completions = Vec::new();
        while let Some(step) = pending.pop() {
            let value = match step {
                Pending::Value(value) => value,
                Pending::Sequence { children, next } => {
                    if next > 0
                        && plans.completion_with(children[next - 1], &output, &|value| {
                            subject_completions.iter().find_map(|(root, completion)| {
                                (*root == value).then_some(*completion)
                            })
                        })? == checked_trees::CheckedValueCompletion::NoNormal
                    {
                        continue;
                    }
                    if let Some(child) = children.get(next).copied() {
                        pending.push(Pending::Sequence {
                            children,
                            next: next + 1,
                        });
                        pending.push(Pending::Value(child));
                    }
                    continue;
                }
            };
            if !plans.nodes.is_valid(value) || visited.contains(&value) {
                return None;
            }
            visited.push(value);
            shapes.add_stored_type(plans.nodes.get(value).type_reference, &binders, &[])?;
            match &plans.nodes.get(value).kind {
                checked_trees::CheckedStructuralValueKind::NativeCodeLayout { .. } => {}
                checked_trees::CheckedStructuralValueKind::QuotientCall { arguments, .. } => {
                    pending.push(Pending::Sequence {
                        children: arguments.clone(),
                        next: 0,
                    });
                }
                checked_trees::CheckedStructuralValueKind::Call { source_call } => {
                    if !facts.flow.control.calls.is_valid(*source_call) {
                        return None;
                    }
                    let call = facts.flow.control.calls.get(*source_call);
                    if call.authored_expression != plans.nodes.get(value).expression {
                        return None;
                    }
                    let mut preparation = preamble.to_vec();
                    let predecessor_count = preparation.len();
                    let completion = plan_call_operands(
                        program,
                        facts,
                        scalar_callees,
                        shapes,
                        machine,
                        state,
                        parameters,
                        trivial_locals,
                        entry_claims,
                        &mut scoped_results,
                        &mut preparation,
                        count,
                        call,
                        trace,
                        windows,
                    )?;
                    if completion == checked_trees::CheckedValueCompletion::NoNormal {
                        output.push(checked_trees::CheckedStructuralValueCall::stopped(
                            value,
                            checked_trees::CheckedOperandSequence::new(
                                preparation.split_off(predecessor_count),
                            ),
                        )?);
                        continue;
                    }
                    let reference =
                        validation::call_target_return_type(program, call.target_symbol)?;
                    let mut result = checked_structural_result_type(
                        program,
                        shapes,
                        reference,
                        &machine_binders(program, machine),
                    )?;
                    result.statement_index = u32::try_from(call.statement_index).ok()?;
                    result.binding_ordinal = u32::try_from(*count).ok()?;
                    let operation = build_call_operation(
                        program,
                        shapes,
                        facts,
                        Some(scalar_callees),
                        machine,
                        state,
                        parameters,
                        trivial_locals,
                        call,
                        Some(ExpectedCallValueResult::Structural(&result)),
                        &scoped_results,
                        &preparation,
                        trace,
                    )?;
                    let mut operation = if operation.has_normal_continuation() {
                        bind_structural_call_result(
                            program,
                            facts,
                            machine.symbol,
                            state.symbol,
                            operation,
                            result.clone(),
                        )?
                    } else {
                        operation
                    };
                    if let Some((_, discard_result_on_return)) =
                        operation.call_mut()?.structural_result_mut()
                    {
                        *discard_result_on_return = false;
                    }
                    let has_result = operation.structural_result_binding().is_some();
                    let mut preparation = preparation.split_off(predecessor_count);
                    super::statement_sequence::consume_retained_results(
                        &mut preparation,
                        &operation,
                    )?;
                    let completions = checked_trees::CheckedOperandSequence::new(
                        super::super::cleanup::anonymous::normal_call_completion(
                            program,
                            facts,
                            shapes,
                            machine,
                            state,
                            &scoped_results,
                            &mut preparation,
                            &operation,
                            trace,
                        )?,
                    );
                    trace.phase("structural value: retained call schedule");
                    trace.statement(Some(result.statement_index));
                    output.push(checked_trees::CheckedStructuralValueCall::with_operands(
                        value,
                        checked_trees::CheckedOperandSequence::new(preparation),
                        operation,
                        completions,
                    )?);
                    if has_result {
                        scoped_results.push((
                            result,
                            facts::PlaceRoot::Expression(call.authored_expression),
                        ));
                        *count = count.checked_add(1)?;
                    }
                }
                checked_trees::CheckedStructuralValueKind::Record { fields, .. }
                | checked_trees::CheckedStructuralValueKind::StructuralCase { fields, .. } => {
                    let mut children = Vec::new();
                    for field in plans.record_fields.span(*fields)? {
                        if let checked_trees::CheckedStructuralRecordFieldValue::Structural(value) =
                            field.value
                        {
                            if let checked_trees::CheckedStructuralValueKind::Place(argument) = &plans.nodes.get(value).kind
                                && matches!(argument.source, checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { .. })
                            {
                                let binders = machine_binders(program, machine);
                                if shapes.add_type(field.type_reference, &binders, &[])? != argument.type_identity {
                                    return None;
                                }
                                parameter_qualifications(program, shapes, field.type_reference, &binders)?;
                            }
                            children.push(value);
                        }
                    }
                    pending.push(Pending::Sequence { children, next: 0 });
                }
                checked_trees::CheckedStructuralValueKind::Dispatch { subject, arms } => {
                    let subject = subject.structural_value();
                    if subject.is_valid() {
                        let node = plans.nodes.get(subject);
                        let subject_root = plans.root_for_expression(
                            state.symbol,
                            node.statement_index,
                            node.expression,
                        )?;
                        if subject_root.root != subject || subject_root.machine != machine.symbol {
                            return None;
                        }
                        let (_, _, completion) =
                            super::super::structural_value_calls::prepare_root(
                                program,
                                facts,
                                shapes,
                                machine,
                                state,
                                parameters,
                                trivial_locals,
                                entry_claims,
                                preamble,
                                subject_root,
                            )?;
                        subject_completions.push((subject, completion));
                    }
                    pending.extend(
                        plans
                            .dispatch_arms
                            .span(*arms)?
                            .iter()
                            .rev()
                            .map(|arm| Pending::Value(arm.value)),
                    );
                }
                checked_trees::CheckedStructuralValueKind::ViewSubslice { source, .. } => {
                    let child = plans.nodes.get(*source);
                    shapes.add_stored_type(
                        child.type_reference,
                        &machine_binders(program, machine),
                        &[],
                    )?;
                    pending.push(Pending::Value(*source));
                }
                checked_trees::CheckedStructuralValueKind::Projection { source, .. }
                | checked_trees::CheckedStructuralValueKind::QuotientAdaptation {
                    source, ..
                } => {
                    pending.push(Pending::Value(*source));
                }
                checked_trees::CheckedStructuralValueKind::FixedArray { elements } => {
                    pending.push(Pending::Sequence {
                        children: elements.clone(),
                        next: 0,
                    });
                }
                checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace {
                    leaf, ..
                } => {
                    let binders = machine_binders(program, machine);
                    shapes.add_stored_type(*leaf, &binders, &[])?;
                }
                checked_trees::CheckedStructuralValueKind::Reference {
                    source,
                    source_value,
                    backing_type,
                    ..
                } => {
                    shapes.add_reference_backing(
                        *backing_type,
                        source.type_identity,
                        &machine_binders(program, machine),
                    )?;
                    if source_value.is_valid() {
                        pending.push(Pending::Value(*source_value));
                    }
                }
                checked_trees::CheckedStructuralValueKind::BorrowedSliceView { .. }
                | checked_trees::CheckedStructuralValueKind::MutableSliceView { .. }
                | checked_trees::CheckedStructuralValueKind::Case(_)
                | checked_trees::CheckedStructuralValueKind::ZeroedScalarArray { .. }
                | checked_trees::CheckedStructuralValueKind::ScalarArray { .. }
                | checked_trees::CheckedStructuralValueKind::Scalar { .. }
                | checked_trees::CheckedStructuralValueKind::ZeroedStructural { .. }
                | checked_trees::CheckedStructuralValueKind::ByteLiteralView { .. }
                | checked_trees::CheckedStructuralValueKind::Place(_) => {}
            }
        }
        trace.phase("structural value: completion");
        trace.statement(Some(plans.nodes.get(root).statement_index));
        let completion = plans.completion_with(root, &output, &|value| {
            subject_completions
                .iter()
                .find_map(|(root, completion)| (*root == value).then_some(*completion))
        });
        if completion.is_none() && ::validation::trace_flags::trace_completion() {
            eprintln!(
                "CHECKSITE completion-value-summary machine={} state={} root={:?} value={:?} calls={:?}",
                machine.name,
                state.name,
                root,
                plans.nodes.get(root),
                output,
            );
        }
        let completion = completion?;
        trace.restore(&caller_trace);
        Some((output, completion))
    }

    /*
        Publish the evaluated value through its ordinary binding owner. The value
        evaluator invokes each retained call once and transports its completed
        custody into the receiving declaration. A fresh call result may still need
        a distinct arrival when that declaration forgets a predicate membership.
    */
    pub(in crate::execution::terminal_unit) fn establish_value(
        facts: &CheckFacts,
        result: CheckedUnitStructuralResultBindingPlan,
        value: checked_trees::CheckedStructuralValueHandle,
        calls: Vec<checked_trees::CheckedStructuralValueCall>,
        operand_source: Option<checked_trees::CheckedArrayConstructionSource>,
        discard_result_on_return: bool,
        _operations: &mut Vec<CheckedUnitEffectOperationPlan>,
    ) -> Option<CheckedUnitEffectOperationPlan> {
        if facts.values.structural_values.completion(value, &calls)?
            == checked_trees::CheckedValueCompletion::NoNormal
        {
            return Some(CheckedUnitEffectOperationPlan::EvaluateStructuralValue {
                statement_index: result.statement_index,
                value,
                calls,
                operand_source,
            });
        }
        Some(CheckedUnitEffectOperationPlan::EstablishLocal(
            checked_trees::CheckedUnitLocalBindingPlan::StructuralValue {
                result,
                value,
                calls,
                operand_source,
                discard_result_on_return,
            },
        ))
    }

    /*
        An asm intrinsic owns no authored parameter roster: its instruction
        contract fixes every operand as a register value. Its call therefore owns
        no structural operand and no argument call; call planning still decides
        whether the instruction itself lowers.
    */
    pub(in crate::execution::terminal_unit) fn has_register_operands_only(
        program: &dyn ::validation::CheckingInput,
        target: SymbolHandle,
    ) -> bool {
        program
            .symbols
            .builtin_function_for_symbol(target)
            .is_some_and(|function| function.is_asm_intrinsic())
    }

    pub(in crate::execution::terminal_unit) fn for_call<'a>(
        program: &dyn ::validation::CheckingInput,
        facts: &'a CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        call: &checked_trees::FlowCallFact,
    ) -> Option<Vec<&'a checked_trees::FlowCallFact>> {
        Some(
            operations_for_call(program, facts, machine, state, call)?
                .into_iter()
                .filter_map(|operand| match operand {
                    Operand::Call(call) => Some(call),
                    Operand::Value { .. } | Operand::Storage { .. } => None,
                })
                .collect(),
        )
    }

    pub(in crate::execution::terminal_unit) fn operations_for_call<'a>(
        program: &dyn ::validation::CheckingInput,
        facts: &'a CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        call: &checked_trees::FlowCallFact,
    ) -> Option<Vec<Operand<'a>>> {
        let flow = state_flow(facts, machine.symbol, state.symbol)?;
        let calls = facts.flow.control.calls.span(flow.calls)?;
        let mut output = Vec::new();
        collect(
            program,
            facts,
            machine,
            state,
            calls,
            call,
            &mut Vec::new(),
            &mut output,
        )?;
        Some(output)
    }

    fn collect<'a>(
        program: &dyn ::validation::CheckingInput,
        facts: &'a CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        calls: &'a [checked_trees::FlowCallFact],
        call: &checked_trees::FlowCallFact,
        active: &mut Vec<typed_trees::expression::ExpressionHandle>,
        output: &mut Vec<Operand<'a>>,
    ) -> Option<()> {
        let site = validation::find_call_site(
            program,
            machine.symbol,
            state.symbol,
            call.statement_index,
            call.call_ordinal,
        )?;
        if has_register_operands_only(program, call.target_symbol) {
            return Some(());
        }
        let arguments = validation::call_site_argument_expressions(program, &site);
        let parameters = validation::call_target_parameters(program, call.target_symbol)?;
        let parameters = parameters.iter().enumerate().collect::<Vec<_>>();
        if parameters.len() != arguments.len() {
            return None;
        }
        for (argument, (position, parameter)) in arguments.iter().zip(&parameters) {
            if program
                .primitive_type_reference(parameter.type_reference)
                .is_some()
            {
                continue;
            }
            let source = checked_trees::CheckedArrayConstructionSource::CallArgument {
                call_ordinal: u32::try_from(call.call_ordinal).ok()?,
                parameter_position: u32::try_from(*position).ok()?,
            };
            if let Some(root) = evaluated_value_root(
                program,
                facts,
                machine.symbol,
                state.symbol,
                call.statement_index,
                *argument,
                parameter,
            ) {
                if output.iter().any(|operand| {
                    matches!(operand, Operand::Value { root: prior, .. } if prior.expression == root.expression)
                }) {
                    return None;
                }
                output.push(Operand::Value {
                    root,
                    source,
                    access: super::super::types::structural_access_for_type_reference(
                        program,
                        parameter.type_reference,
                    )?,
                });
                continue;
            }
            let place = place_at(program, state.symbol, call.statement_index, *argument);
            if place
                .as_ref()
                .is_some_and(|place| place.root.as_symbol().is_some())
                && !parameter.is_const
                && !parameter.relevance.is_erased()
                && !type_reference_is_reference(program, parameter.type_reference)
                && program.type_multiplicity(parameter.type_reference) != Multiplicity::Unrestricted
            {
                output.push(Operand::Storage {
                    expression: *argument,
                    type_reference: parameter.type_reference,
                });
                continue;
            }
            let expression = match place {
                Some(place)
                    if place.segments.iter().all(|segment| {
                        matches!(
                            segment,
                            facts::PlaceSegment::Field { .. }
                                | facts::PlaceSegment::FixedIndex { .. }
                        )
                    }) =>
                {
                    match place.root {
                        facts::PlaceRoot::Expression(expression) => expression,
                        _ => *argument,
                    }
                }
                _ => *argument,
            };
            let Some(authored) = program.expression_table.call_node(expression) else {
                continue;
            };
            result(
                program,
                machine.symbol,
                expression,
                &mut ShapeCollector::new(program),
            )?;
            if active.contains(&expression)
                || output
                    .iter()
                    .any(|prior| matches!(prior, Operand::Call(prior) if prior.authored_expression == expression))
            {
                return None;
            }
            let mut matching = calls.iter().filter(|nested| {
                nested.statement_index == call.statement_index
                    && nested.call_ordinal > call.call_ordinal
                    && nested.authored_expression == expression
                    && nested.target_symbol
                        == validation::call_target_entry_symbol(
                            program,
                            validation::call_target_symbol(program, authored),
                        )
            });
            let nested = matching.next()?;
            if matching.next().is_some() {
                return None;
            }
            active.push(expression);
            collect(
                program, facts, machine, state, calls, nested, active, output,
            )?;
            active.pop();
            output.push(Operand::Call(nested));
        }
        Some(())
    }

    /*
        Anonymous results use the same stored-owned shape as named boundary results.
        This selects only the source signature; build_call_operation still checks
        the complete target contract, arguments, and ownership events.
    */
    pub(in crate::execution::terminal_unit) fn result(
        program: &dyn ::validation::CheckingInput,
        caller: SymbolHandle,
        expression: typed_trees::expression::ExpressionHandle,
        shapes: &mut ShapeCollector<'_>,
    ) -> Option<CheckedStructuralResultPlan> {
        if !program.expression_table.expression_is_valid(expression) {
            return None;
        }
        let call = program.expression_table.call_node(expression)?;
        let return_type = nested_structural_call_return_type(program, caller, call)?;
        let binders = if let Some((owner, _)) =
            validation::find_machine_head(program, call.target_symbol)
        {
            if !owner.supply_mode.is_boundary_declaration() {
                if !(owner.supplies_checked_body() || owner.has_bodyless_signature())
                    || !machine_binders(program, owner).is_empty()
                {
                    return None;
                }
                let binding = checked_structural_result_type(program, shapes, return_type, &[])?;
                return Some(CheckedStructuralResultPlan {
                    type_identity: binding.type_identity,
                    multiplicity: binding.multiplicity,
                    qualifications: parameter_qualifications(program, shapes, return_type, &[])?,
                    projected_qualifications:
                        validation::structural_result_projected_qualifications(program, return_type)
                            .ok()?,
                });
            }
            machine_binders(program, owner)
        } else {
            Vec::new()
        };
        let CheckedBoundaryMachineResultPlan::Structural {
            type_identity,
            multiplicity,
            qualifications,
            projected_qualifications,
        } = boundary_result_plan(program, shapes, return_type, &binders, &[])?
        else {
            return None;
        };
        Some(CheckedStructuralResultPlan {
            type_identity,
            multiplicity,
            qualifications,
            projected_qualifications,
        })
    }

    /*
        Prepare structural operands through the same construction and result
        binding path for statement calls and calls inside selected values. The
        caller owns the operation sequence, so an edge retains these operations
        until selection instead of executing them in its predecessor.
    */
    #[allow(clippy::too_many_arguments)]
    pub(in crate::execution::terminal_unit) fn plan_call_operands(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        scalar_callees: ScalarCalleeClosure,
        shapes: &mut ShapeCollector<'_>,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        structural_parameters: &[CheckedUnitStructuralParameterPlan],
        trivial_affine_locals: &[(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)],
        entry_claims: &[CheckedUnitEntryClaimPlan],
        structural_results: &mut Vec<(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)>,
        operations: &mut Vec<CheckedUnitEffectOperationPlan>,
        structural_count: &mut usize,
        call: &checked_trees::FlowCallFact,
        trace: &LocalConstructionTrace,
        windows: &mut super::super::borrowed_windows::OpenWindows,
    ) -> Option<checked_trees::CheckedValueCompletion> {
        let statement_index = u32::try_from(call.statement_index).ok()?;
        let binders = machine_binders(program, machine);
        for operand in operations_for_call(program, facts, machine, state, call)? {
            let storage = match &operand {
                Operand::Storage {
                    expression,
                    type_reference,
                } => Some((*expression, *type_reference)),
                Operand::Value {
                    root,
                    access: checked_trees::CheckedStructuralAccess::Owned,
                    ..
                } if program.type_multiplicity(root.type_reference)
                    != Multiplicity::Unrestricted =>
                {
                    Some((root.expression, root.type_reference))
                }
                _ => None,
            };
            if let Some((expression, type_reference)) = storage
                && let Some((operation, result)) = windows.move_call_operand(
                    program,
                    facts,
                    shapes,
                    machine,
                    state,
                    structural_parameters,
                    statement_index,
                    expression,
                    type_reference,
                    u32::try_from(*structural_count).ok()?,
                )
            {
                *structural_count = structural_count.checked_add(1)?;
                structural_results.push((result, facts::PlaceRoot::Expression(expression)));
                operations.push(operation);
                continue;
            }
            let nested = match operand {
                Operand::Storage { .. } => continue,
                Operand::Value { root, source, .. } => {
                    if structural_results
                        .iter()
                        .any(|(_, source)| *source == facts::PlaceRoot::Expression(root.expression))
                    {
                        continue;
                    }
                    let calls = super::statement_sequence::prepare_value_operands(
                        program,
                        facts,
                        scalar_callees,
                        shapes,
                        machine,
                        state,
                        structural_parameters,
                        trivial_affine_locals,
                        entry_claims,
                        statement_index,
                        structural_results,
                        operations,
                        structural_count,
                        root.root,
                        trace,
                        windows,
                    )?;
                    let (calls, completion) = calls;
                    if completion == checked_trees::CheckedValueCompletion::NoNormal {
                        operations.push(CheckedUnitEffectOperationPlan::EvaluateStructuralValue {
                            statement_index,
                            value: root.root,
                            calls,
                            operand_source: Some(source),
                        });
                        return Some(completion);
                    }
                    let multiplicity = validation::reference_result_custody::result_multiplicity(
                        program,
                        root.type_reference,
                    );
                    let result = CheckedUnitStructuralResultBindingPlan {
                        statement_index,
                        binding_ordinal: u32::try_from(*structural_count).ok()?,
                        type_identity: shapes.add_stored_type(
                            root.type_reference,
                            &binders,
                            &[],
                        )?,
                        multiplicity,
                        local: Default::default(),
                    };
                    *structural_count = structural_count.checked_add(1)?;
                    structural_results.push((
                        result.clone(),
                        facts::PlaceRoot::Expression(root.expression),
                    ));
                    let establishment = establish_value(
                        facts,
                        result,
                        root.root,
                        calls,
                        Some(source),
                        multiplicity == Multiplicity::Affine,
                        operations,
                    )?;
                    operations.push(establishment);
                    continue;
                }
                Operand::Call(nested) => nested,
            };
            if structural_results.iter().any(|(_, root)| {
                matches!(root, facts::PlaceRoot::Expression(expression)
                    if *expression == nested.authored_expression)
            }) {
                continue;
            }
            let target = result(program, machine.symbol, nested.authored_expression, shapes)?;
            let result = CheckedUnitStructuralResultBindingPlan {
                statement_index,
                binding_ordinal: u32::try_from(*structural_count).ok()?,
                type_identity: target.type_identity,
                multiplicity: target.multiplicity,
                local: Default::default(),
            };
            let operation = build_call_operation(
                program,
                shapes,
                facts,
                Some(scalar_callees),
                machine,
                state,
                structural_parameters,
                trivial_affine_locals,
                nested,
                Some(ExpectedCallValueResult::Structural(&result)),
                structural_results,
                operations,
                trace,
            )?;
            let operation = if operation.has_normal_continuation() {
                bind_structural_call_result(
                    program,
                    facts,
                    machine.symbol,
                    state.symbol,
                    operation,
                    result.clone(),
                )?
            } else {
                operation
            };
            super::statement_sequence::consume_results(&mut *operations, &operation)?;
            let completions = super::super::cleanup::anonymous::normal_call_completion(
                program,
                facts,
                shapes,
                machine,
                state,
                structural_results,
                operations,
                &operation,
                trace,
            )?;
            let normal = operation.has_normal_continuation();
            operations.push(operation);
            if !normal {
                return Some(checked_trees::CheckedValueCompletion::NoNormal);
            }
            operations.extend(completions);
            structural_results.push((
                result,
                facts::PlaceRoot::Expression(nested.authored_expression),
            ));
            *structural_count = structural_count.checked_add(1)?;
        }
        Some(checked_trees::CheckedValueCompletion::Normal)
    }
}

pub(crate) use boundary_machine::{
    build_boundary_machine, build_dynamic_slot_plans, build_static_boundary_requirements,
};
pub(super) use call_occurrences::{
    SelectedInvocationAccounting, call_owns_statement_computations, completion_expression,
    outer_calls_in_prefix_traced, terminal_invocation,
};
pub(crate) use call_results::{
    bind_scalar_call_result, bind_structural_call_result, checked_structural_result_type,
};
pub(crate) use checked_machine::build_checked_machine_residual_parts;
pub(crate) use construction_trace::{LocalConstructionTrace, ResultCustodyFailure};

use super::{
    CheckFacts, CheckedBoundaryMachineResultPlan, CheckedScalarExpression,
    CheckedScalarExpressionRole, CheckedStructuralAccess, CheckedStructuralScalarParameterPlan,
    CheckedUnitEffectOperationPlan, CheckedUnitScalarResultBindingPlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralResultBindingPlan, ExpressionNode,
    Multiplicity, PermissionEventSource, StatementNode, SymbolHandle, TypeReferenceHandle,
};
