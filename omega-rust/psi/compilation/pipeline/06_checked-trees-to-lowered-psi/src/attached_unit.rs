/*
Attached Unit closure assembly (`lower_unit_closure`, `lower_unit_effect_closure`): transitive call
catalogs, boundary and provider admission, signature and parameter allocation, ordinary machine
emission, and scalar/structural completions.
*/

pub(crate) use crate::unit::*;

pub(crate) mod inner {

    pub(crate) use super::{
        Block, BoundaryMachineDeclaration, BoundaryMachineResult,
        BoundaryStructuralResultDeclaration, CheckedBoundaryMachinePlan,
        CheckedBoundaryMachineResultPlan, CheckedScalarExpression, CheckedScalarExpressionRole,
        CheckedTrees, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan, LoweredPsi,
        LoweringError, MachineContract, MachineId, Multiplicity, Operation, OperationKind,
        OperationResult, PlaceId, ProviderCandidateConformance, ProviderParameterRefinement,
        ProviderRefinement, ProviderSignature, ProviderSignatureParameter, ScalarType,
        SemanticDomainId, ServiceReachSummary, StructuralDomainRequirement, StructuralMultiplicity,
        StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeId, StructuralTypeShape,
        TERMINAL_UNIT_CALL_OBLIGATION_BASE, TerminalMachine, TerminalMachineResult, TerminalModule,
        Terminator, ValueDeclaration, VocabularyMarker, allocate_dense, boundary_machine_id,
        content_conservation, contract_id, dense_identity,
        direct_expression_contains_short_circuit, edge_id, emit_direct_expression, lookup_claim_id,
        lookup_domain_id, lookup_machine_id, lookup_service_id, lookup_type_id,
        lower_access_view_input, lower_boundary_content_guarantees, lower_boundary_crash_routes,
        lower_checked_crash_route_buckets, lower_structural_crash_route_buckets, machine_id,
        obligation_id, place_id, terminal_scalar_type, unsupported,
        validate_direct_parameter_types, value_id,
    };
    pub(crate) use crate::emission::structural_byte_sequence_store::prepare_types;
    pub(crate) use crate::scalar_lowering::call_closure::callee::CheckedScalarCallee;
    pub(crate) use crate::scalar_lowering::contracts::indexed_theorems;
    pub(crate) use crate::shared::activation_storage::prepare_operation;
    pub(crate) use crate::shared::checked_facts::flow_facts;
    pub(crate) use crate::unit::attached_unit::call_closure::BoundaryIdentity;
    pub(crate) use crate::unit::dynamic_dispatch::DynamicDispatchIndex;
    pub(crate) use crate::unit_arguments::attached_unit::{argument_evaluation, argument_schedule};
    pub(crate) use crate::unit_composed_control::attached_unit::composed_control;
    pub(crate) use crate::unit_operation_frame::attached_unit::operation_frame;
    pub(crate) use crate::unit_structural_values::attached_unit::structural_values;

    pub(crate) mod admission {

        use super::super::{
            CheckedTrees, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan,
            LoweringError,
        };
        use crate::unit::attached_unit::call_closure::BoundaryIdentity;

        use super::bodies::{UnitBody, UnitPlans};
        use super::composed_control::{self, callable::CallableBody};
        use super::retain_unit_boundary;
        use checked_trees::{CheckedBoundaryMachinePlan, CheckedComposedUnitControlMachinePlan};
        use symbols::SymbolHandle;

        pub(crate) enum AdmittedBody<'a> {
            Ordinary(&'a CheckedUnitEffectMachinePlan),
            Composed {
                source: &'a CheckedComposedUnitControlMachinePlan,
                body: CallableBody<'a>,
            },
        }

        impl<'a> AdmittedBody<'a> {
            pub(crate) fn source(&self) -> UnitBody<'a> {
                match self {
                    Self::Ordinary(plan) => UnitBody::Ordinary(plan),
                    Self::Composed { source, .. } => UnitBody::Composed(source),
                }
            }
        }

        pub(crate) fn admit<'a>(
            checked: &'a CheckedTrees,
            plans: UnitPlans<'a>,
            closure: &[SymbolHandle],
            boundaries: &mut Vec<(&'a CheckedBoundaryMachinePlan, BoundaryIdentity)>,
        ) -> Result<Vec<AdmittedBody<'a>>, LoweringError> {
            let mut bodies = Vec::with_capacity(closure.len());
            for source in closure {
                let admitted = match UnitBody::find(plans, *source)? {
                    UnitBody::Composed(plan) => {
                        let body = composed_control::admit_callable(checked, plan)?;
                        for (boundary, _) in body.boundaries() {
                            retain_unit_boundary(checked, plans, boundaries, boundary.machine)?;
                        }
                        AdmittedBody::Composed { source: plan, body }
                    }
                    UnitBody::Ordinary(plan) => {
                        retain_call_boundaries(checked, plans, &plan.operations, boundaries)?;
                        AdmittedBody::Ordinary(plan)
                    }
                };
                bodies.push(admitted);
            }
            Ok(bodies)
        }

        pub(crate) fn retain_call_boundaries<'a, 'o>(
            checked: &CheckedTrees,
            plans: UnitPlans<'a>,
            operations: impl IntoIterator<Item = &'o CheckedUnitEffectOperationPlan>,
            boundaries: &mut Vec<(&'a CheckedBoundaryMachinePlan, BoundaryIdentity)>,
        ) -> Result<(), LoweringError> {
            for operation in operations {
                for call in operation
                    .with_value_calls()
                    .filter_map(CheckedUnitEffectOperationPlan::call)
                    .filter(|call| call.is_boundary())
                {
                    retain_unit_boundary(checked, plans, boundaries, call.target.machine)?;
                }
            }
            Ok(())
        }
    }
    pub(crate) mod boundaries {

        use super::super::{ServiceId, ServiceReachId};
        use super::operation_frame::BoundaryParameters;
        use super::{
            BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryStructuralResultDeclaration,
            CheckedBoundaryMachinePlan, CheckedBoundaryMachineResultPlan, CheckedTrees,
            LoweringError, StructuralDomainRequirement, StructuralTypeId, UnitPlans,
            boundary_machine_id, checked_unit_boundary_identity, dense_identity, lookup_type_id,
            lower_boundary_content_guarantees, lower_boundary_crash_routes,
            lower_fixed_boundary_service_reach, lower_published_service_ceiling,
            lower_runtime_parameter_order, lower_unit_parameters, terminal_scalar_type,
            unique_unit_boundary, unsupported,
        };
        use crate::emission::borrowed_window::terminal_multiplicity;
        use crate::expression_preparation::qualifications::signature_value_type;
        use crate::retention::conformance_applications::checked_method_arguments;
        use crate::scalar_lowering::contracts::proposition;
        use crate::shared::terminal_identities::lookup_service_id;
        use crate::shared::terminal_identities::value_id;
        use crate::shared::terminal_identities::{
            StructuralDomainBinding, lookup_domain_id_for_carrier,
        };
        use crate::unit::attached_unit::call_closure::BoundaryIdentity;
        use crate::unit::attached_unit::catalog::lower_program_local_root_introductions;
        use crate::unit::{Proposition, ScalarType, ValueDeclaration};

        pub(crate) mod reference_outputs {

            use crate::LoweringError;
            use crate::expression_preparation::bindings::structural_paths::lower_static_argument_path;
            use checked_trees::{
                CheckedBoundaryLifetimeDependency, CheckedBoundaryMachinePlan,
                CheckedBoundaryReferenceDestination,
            };
            use terminal_psi::{
                BoundaryLifetimeDependency, BoundaryReferenceDestination, BoundaryReferenceOutput,
                StructuralAccess, StructuralParameterDeclaration, StructuralTypeDeclaration,
                TerminalIdentityTableBuilder,
            };

            pub(crate) fn lower(
                identities: &mut TerminalIdentityTableBuilder,
                plan: &CheckedBoundaryMachinePlan,
                parameters: &[StructuralParameterDeclaration],
                types: &[StructuralTypeDeclaration],
            ) -> Result<Vec<BoundaryReferenceOutput>, LoweringError> {
                let parameter = |position| {
                    plan.structural_parameters
                        .iter()
                        .position(|parameter| parameter.position == position)
                        .and_then(|position| parameters.get(position))
                        .ok_or(LoweringError::Unsupported(
                            "boundary reference contract names an absent structural parameter",
                        ))
                };
                let mut outputs = Vec::with_capacity(plan.reference_outputs.len());
                for output in &plan.reference_outputs {
                    let (destination, root) = match output.destination {
                        CheckedBoundaryReferenceDestination::Parameter(position) => {
                            let parameter = parameter(position)?;
                            (
                                BoundaryReferenceDestination::Parameter(parameter.position),
                                parameter.structural_type,
                            )
                        }
                    };
                    let path = lower_static_argument_path(identities, &output.path, root, types)?;
                    let mut dependencies = Vec::with_capacity(output.dependencies.len());
                    for dependency in &output.dependencies {
                        dependencies.push(match dependency {
                            CheckedBoundaryLifetimeDependency::Referent {
                                parameter_index,
                                access,
                            } => BoundaryLifetimeDependency::Referent {
                                argument_index: parameter(*parameter_index)?.position,
                                access: lower_access(*access),
                            },
                            CheckedBoundaryLifetimeDependency::Stored {
                                parameter_index,
                                path,
                                access,
                            } => {
                                let parameter = parameter(*parameter_index)?;
                                BoundaryLifetimeDependency::Stored {
                                    argument_index: parameter.position,
                                    access: lower_access(*access),
                                    path: lower_static_argument_path(
                                        identities,
                                        path,
                                        parameter.structural_type,
                                        types,
                                    )?,
                                }
                            }
                        });
                    }
                    outputs.push(BoundaryReferenceOutput {
                        destination,
                        path,
                        access: lower_access(output.access),
                        dependencies,
                    });
                }
                Ok(outputs)
            }

            fn lower_access(access: language_semantics::ReferenceAccess) -> StructuralAccess {
                match access {
                    language_semantics::ReferenceAccess::Shared => StructuralAccess::SharedBorrow,
                    language_semantics::ReferenceAccess::Mutable => StructuralAccess::MutableBorrow,
                    language_semantics::ReferenceAccess::WriteOnly => {
                        StructuralAccess::WriteOnlyBorrow
                    }
                }
            }
        }

        pub(crate) fn lower_declarations(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            scalar_applications: &super::scalar_applications::SourceScalarApplications,
            exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
            boundaries: &[(&CheckedBoundaryMachinePlan, BoundaryIdentity)],
            scalar_qualifications: &crate::expression_preparation::qualifications::PreparedScalarQualifications,
            type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
            structural_types: &[terminal_psi::StructuralTypeDeclaration],
            domain_ids: &[StructuralDomainBinding],
            service_ids: &[(ServiceReachId, ServiceId)],
            next_place: &mut u64,
        ) -> Result<(Vec<BoundaryMachineDeclaration>, Vec<BoundaryParameters>), LoweringError>
        {
            let mut declarations = Vec::with_capacity(boundaries.len());
            let mut parameter_rosters = Vec::with_capacity(boundaries.len());
            for (index, (plan, identity)) in boundaries.iter().enumerate() {
                let parameters = lower_unit_parameters(
                    identities,
                    &plan.structural_parameters,
                    type_ids,
                    structural_types,
                    domain_ids,
                    next_place,
                )?;
                let scalar_parameters = plan
                    .scalar_parameters
                    .iter()
                    .map(|parameter| terminal_scalar_type(parameter.primitive_type))
                    .collect::<Result<Vec<_>, _>>()?;
                let requires = lower_requirements(
                    identities,
                    &plan.domain_requirements,
                    &parameters,
                    structural_types,
                    domain_ids,
                )?;
                let normal_qualifications = lower_requirements(
                    identities,
                    &plan.normal_qualifications,
                    &parameters,
                    structural_types,
                    domain_ids,
                )?;
                let predicate_parameters = plan
                    .structural_parameters
                    .iter()
                    .zip(&parameters)
                    .map(
                        |(source, terminal)| terminal_psi::StructuralParameterDeclaration {
                            position: source.position,
                            ..terminal.clone()
                        },
                    )
                    .collect::<Vec<_>>();
                let view_roster = predicate_parameters
                    .iter()
                    .map(|parameter| (parameter.position, parameter.clone()))
                    .collect::<Vec<_>>();
                let views = crate::scalar_lowering::contracts::ContractViewNamespace {
                    scalar_applications,
                    constant_values: &checked.typed.constant_recipe_closure.values,
                    parameters: &view_roster,
                    result: crate::scalar_lowering::contracts::ContractStructuralResult::Absent,
                    structural_types,
                };
                let scalar_requires =
                    lower_scalar_requires(identities, plan, &scalar_parameters, &views)?;
                let result = if plan.result.structural_identity().is_some() {
                    let signature = crate::shared::declarations::target_signature(
                        checked,
                        plan.machine,
                        plan.machine,
                    )?;
                    let position = u32::try_from(signature.parameters.len()).map_err(|_| {
                        LoweringError::Unsupported(
                            "boundary result coordinate exceeds the contract namespace",
                        )
                    })?;
                    crate::scalar_lowering::contracts::ContractStructuralResult::Unavailable {
                        position,
                    }
                } else {
                    crate::scalar_lowering::contracts::ContractStructuralResult::Absent
                };
                let scalar_ensures = lower_scalar_ensures(
                    identities,
                    plan,
                    &scalar_parameters,
                    &crate::scalar_lowering::contracts::ContractViewNamespace { result, ..views },
                )?;
                let crash_routes = lower_boundary_crash_routes(
                    identities,
                    checked,
                    plan,
                    &scalar_parameters,
                    &predicate_parameters,
                    structural_types,
                    &scalar_requires,
                )?;
                let requirement_count = scalar_requires.len();
                let id = boundary_machine_id(dense_identity(index)?);
                let mut scalar_domain_requirements = plan
                    .scalar_domain_requirements
                    .iter()
                    .map(|requirement| {
                        Ok(terminal_psi::ScalarDomainRequirement {
                            argument_index: requirement.argument_index,
                            domain: scalar_qualifications.boundary_domain_id(requirement)?,
                        })
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                scalar_domain_requirements.sort();
                let result = lower_boundary_result(
                    identities,
                    checked,
                    plan.machine,
                    scalar_qualifications.catalog(),
                    &plan.result,
                    type_ids,
                    structural_types,
                    domain_ids,
                )?;
                let result_reference_regions =
                    crate::unit::dynamic_dispatch::lower_result_reference_regions(
                        identities,
                        checked,
                        &plan.result_reference_sources,
                        &result,
                        &parameters
                            .iter()
                            .map(|p| (p.structural_type, p.access))
                            .collect::<Vec<_>>(),
                        type_ids,
                        structural_types,
                    )?;
                declarations.push(BoundaryMachineDeclaration {
                    numeric_intrinsic: checked.typed.numeric_intrinsic_execution(plan.machine),
                    scalar_domain_requirements,
                    normal_qualifications,
                    declaration: crate::machine_lowering::declarations::boundary_identity(
                        identities,
                        checked,
                        exact_toolchain_sources,
                        plan.machine,
                    )
                    .map_err(LoweringError::InvalidDeclarationIdentity)?
                    .map(|source| {
                        Ok::<_, LoweringError>(terminal_psi::CheckedBoundaryCallable {
                            source,
                            execution: match plan.execution {
                                checked_trees::CheckedBoundaryExecutionPlan::Provider => {
                                    terminal_psi::CheckedBoundaryExecution::Provider
                                }
                                checked_trees::CheckedBoundaryExecutionPlan::AccessInitialize => {
                                    terminal_psi::CheckedBoundaryExecution::AccessInitialize
                                }
                                checked_trees::CheckedBoundaryExecutionPlan::AccessMoveOut => {
                                    terminal_psi::CheckedBoundaryExecution::AccessMoveOut
                                }
                                checked_trees::CheckedBoundaryExecutionPlan::AccessRelocate => {
                                    terminal_psi::CheckedBoundaryExecution::AccessRelocate
                                }
                                checked_trees::CheckedBoundaryExecutionPlan::AccessView => {
                                    terminal_psi::CheckedBoundaryExecution::AccessView
                                }
                                checked_trees::CheckedBoundaryExecutionPlan::EmptyRunRelease => {
                                    terminal_psi::CheckedBoundaryExecution::EmptyRunRelease
                                }
                                checked_trees::CheckedBoundaryExecutionPlan::StaticInterrupt(
                                    operation,
                                ) => static_interrupt_execution(operation),
                            },
                            method_arguments: checked_method_arguments(
                                identities,
                                checked,
                                plan.machine,
                            )?,
                        })
                    })
                    .transpose()?,
                    role: terminal_psi::BoundaryDeclarationRole::Callable,
                    unconditional_termination: unconditional_termination(checked, plan)?,
                    completion: validation::boundary_requirement_completion(
                        &checked.typed,
                        plan.machine,
                    ),
                    kind: plan.kind,
                    scalar_requires,
                    scalar_ensures,
                    reference_outputs: reference_outputs::lower(
                        identities,
                        plan,
                        &parameters,
                        structural_types,
                    )?,
                    parameter_order: lower_runtime_parameter_order(
                        &plan.scalar_parameters,
                        &plan.structural_parameters,
                    )?,
                    id,
                    identity: identities.intern(&identity.render(checked)?),
                    owner: crate::machine_lowering::declarations::nominal(
                        identities,
                        checked,
                        exact_toolchain_sources,
                        plan.contract_owner,
                    )
                    .map_err(LoweringError::Unsupported)?,
                    attachment: None,
                    scalar_parameters: scalar_parameters.clone(),
                    crash_routes,
                    structural_parameters: parameters.clone(),
                    result: result.clone(),
                    result_reference_regions,
                    requires,
                    program_local_root_introductions: lower_program_local_root_introductions(
                        checked,
                        plan,
                        *identity,
                        &parameters,
                        domain_ids,
                    )?,
                    content_guarantees: lower_boundary_content_guarantees(
                        &checked.facts.qualifications.content.conservation_plans,
                        plan.state,
                    )?,
                    fixed_service_reach: lower_fixed_boundary_service_reach(
                        checked,
                        plan,
                        service_ids,
                    )?,
                    owning_service: plan
                        .owning_service
                        .map(|owner| lookup_service_id(service_ids, owner))
                        .transpose()?,
                    published_service_ceiling: lower_published_service_ceiling(
                        &checked.facts.service_reaches.rows,
                        plan.contract_service_reach,
                        plan.service_reach,
                        service_ids,
                    )?,
                });
                parameter_rosters.push(BoundaryParameters {
                    source: plan.machine,
                    id,
                    structural: parameters,
                    scalar: scalar_qualifications.boundary_signature_types(plan)?,
                    result,
                    requirement_count,
                });
            }
            Ok((declarations, parameter_rosters))
        }

        fn lower_scalar_requires(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            plan: &CheckedBoundaryMachinePlan,
            scalar_types: &[ScalarType],
            views: &crate::scalar_lowering::contracts::ContractViewNamespace<'_>,
        ) -> Result<Vec<Proposition>, LoweringError> {
            if !plan.pending_scalar_requires.is_empty() || !plan.pending_scalar_recipes.is_empty() {
                return unsupported("boundary scalar contract clauses remain pending");
            }
            lower_scalar_contract(identities, &plan.scalar_requires, scalar_types, views)
        }

        fn lower_scalar_ensures(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            plan: &CheckedBoundaryMachinePlan,
            scalar_types: &[ScalarType],
            views: &crate::scalar_lowering::contracts::ContractViewNamespace<'_>,
        ) -> Result<Vec<Proposition>, LoweringError> {
            let mut namespace = scalar_types.to_vec();
            if let Some(result) = plan.result.scalar() {
                namespace.push(terminal_scalar_type(result)?);
            }
            let formals = scalar_contract_formals(&namespace)?;
            let mut predicate = |value| proposition(identities, value, &formals, &[], views);
            let mut rows = Vec::new();
            for guarantee in &plan.scalar_ensures {
                use checked_trees::CheckedBoundaryScalarGuarantee;
                let row = match guarantee {
                    CheckedBoundaryScalarGuarantee::Predicate(value) => predicate(value),
                    CheckedBoundaryScalarGuarantee::Implication {
                        premise,
                        conclusion,
                    } => {
                        let premise = predicate(premise);
                        let conclusion = predicate(conclusion);
                        match (premise, conclusion) {
                            (Err(error), _)
                                if error != LoweringError::UnavailableContractResult =>
                            {
                                Err(error)
                            }
                            (_, Err(error))
                                if error != LoweringError::UnavailableContractResult =>
                            {
                                Err(error)
                            }
                            (Ok(premise), Ok(conclusion)) => Ok(Proposition::Implication {
                                premise: Box::new(premise),
                                conclusion: Box::new(conclusion),
                            }),
                            _ => Err(LoweringError::UnavailableContractResult),
                        }
                    }
                };
                match row {
                    Ok(row) => rows.push(row),
                    Err(LoweringError::UnavailableContractResult) => {}
                    Err(error) => return Err(error),
                }
            }
            canonical_scalar_contract_rows(rows)
        }

        fn lower_scalar_contract(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            clauses: &[checked_trees::ClosedScalarContractValue],
            scalar_types: &[ScalarType],
            views: &crate::scalar_lowering::contracts::ContractViewNamespace<'_>,
        ) -> Result<Vec<Proposition>, LoweringError> {
            if clauses.is_empty() {
                return Ok(Vec::new());
            }
            let formals = scalar_contract_formals(scalar_types)?;
            let rows = clauses
                .iter()
                .map(|row| {
                    crate::scalar_lowering::contracts::clauses(
                        identities,
                        std::slice::from_ref(&Some(row.clone())),
                        &formals,
                        &[],
                        views,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            canonical_scalar_contract_rows(rows)
        }

        fn scalar_contract_formals(
            scalar_types: &[ScalarType],
        ) -> Result<Vec<ValueDeclaration>, LoweringError> {
            scalar_types
                .iter()
                .enumerate()
                .map(|(position, scalar_type)| {
                    Ok(ValueDeclaration {
                        id: value_id(dense_identity(position)?),
                        scalar_type: *scalar_type,
                        qualifications: Default::default(),
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()
        }

        fn canonical_scalar_contract_rows(
            propositions: Vec<Proposition>,
        ) -> Result<Vec<Proposition>, LoweringError> {
            let mut pending = propositions.into_iter().rev().collect::<Vec<_>>();
            let mut rows = Vec::new();
            while let Some(row) = pending.pop() {
                match row {
                    Proposition::Truth => {}
                    Proposition::Conjunction(conjuncts) => {
                        pending.extend(conjuncts.into_iter().rev())
                    }
                    other => rows.push(other),
                }
            }
            crate::scalar_lowering::contracts::canonicalize_requires(&mut rows)?;
            Ok(rows)
        }

        fn lower_requirements(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            requirements: &[checked_trees::CheckedUnitStructuralDomainRequirementPlan],
            parameters: &[terminal_psi::StructuralParameterDeclaration],
            structural_types: &[terminal_psi::StructuralTypeDeclaration],
            domain_ids: &[StructuralDomainBinding],
        ) -> Result<Vec<StructuralDomainRequirement>, LoweringError> {
            let mut requires = requirements
                .iter()
                .map(|requirement| {
                    let parameter = parameters.get(requirement.argument_index as usize).ok_or(
                        LoweringError::Unsupported(
                            "boundary structural requirement has an invalid argument index",
                        ),
                    )?;
                    let (path, domain) = if requirement.path.is_empty() {
                        (
                            Vec::new(),
                            lookup_domain_id_for_carrier(
                                domain_ids,
                                requirement.domain,
                                parameter.structural_type,
                            )?,
                        )
                    } else {
                        let mut rows = super::parameters::lower_projected_qualifications(
                            identities,
                            &[checked_trees::CheckedStructuralPathQualification {
                                path: requirement.path.clone(),
                                domain: requirement.domain,
                            }],
                            domain_ids,
                            parameter.structural_type,
                            structural_types,
                        )?;
                        let row = rows.pop().ok_or(LoweringError::Unsupported(
                            "boundary normal qualification lost its exact subject",
                        ))?;
                        (row.path, row.domain)
                    };
                    Ok(StructuralDomainRequirement {
                        argument_index: requirement.argument_index,
                        path,
                        domain,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            requires.sort();
            let original_requirement_count = requires.len();
            requires.dedup();
            if requires.len() != original_requirement_count {
                return unsupported("boundary structural requirements contain duplicates");
            }
            Ok(requires)
        }

        pub(crate) fn lower_boundary_result(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            source: symbols::SymbolHandle,
            scalar_qualifications: &terminal_psi::ScalarQualificationCatalog,
            result: &CheckedBoundaryMachineResultPlan,
            type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
            structural_types: &[terminal_psi::StructuralTypeDeclaration],
            domain_ids: &[StructuralDomainBinding],
        ) -> Result<BoundaryMachineResult, LoweringError> {
            Ok(match result {
                CheckedBoundaryMachineResultPlan::Unit => BoundaryMachineResult::Unit,
                CheckedBoundaryMachineResultPlan::Scalar(_) => {
                    let signature =
                        crate::shared::declarations::target_signature(checked, source, source)?;
                    BoundaryMachineResult::Scalar(signature_value_type(
                        checked,
                        scalar_qualifications,
                        signature.return_type,
                    )?)
                }
                CheckedBoundaryMachineResultPlan::Structural {
                    type_identity,
                    multiplicity,
                    qualifications,
                    projected_qualifications,
                } => BoundaryMachineResult::Structural(BoundaryStructuralResultDeclaration {
                    structural_type: lookup_type_id(type_ids, type_identity)?,
                    multiplicity: terminal_multiplicity(*multiplicity),
                    qualifications: {
                        let mut lowered = qualifications
                            .iter()
                            .map(|domain| {
                                lookup_domain_id_for_carrier(
                                    domain_ids,
                                    *domain,
                                    lookup_type_id(type_ids, type_identity)?,
                                )
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        lowered.sort();
                        lowered.dedup();
                        lowered
                    },
                    projected_qualifications: super::parameters::lower_projected_qualifications(
                        identities,
                        projected_qualifications,
                        domain_ids,
                        lookup_type_id(type_ids, type_identity)?,
                        structural_types,
                    )?,
                }),
            })
        }

        pub(crate) fn retain_unit_boundary<'plans>(
            checked: &CheckedTrees,
            plans: UnitPlans<'plans>,
            boundaries: &mut Vec<(&'plans CheckedBoundaryMachinePlan, BoundaryIdentity)>,
            machine: symbols::SymbolHandle,
        ) -> Result<(), LoweringError> {
            let target = unique_unit_boundary(plans, machine)?;
            if !boundaries
                .iter()
                .any(|(candidate, _)| candidate.machine == target.machine)
            {
                boundaries.push((
                    target,
                    checked_unit_boundary_identity(checked, target.machine)?,
                ));
            }
            Ok(())
        }

        fn unconditional_termination(
            checked: &CheckedTrees,
            plan: &CheckedBoundaryMachinePlan,
        ) -> Result<bool, LoweringError> {
            use language_semantics::TerminationGuarantee;
            use semantic_vocabulary::BoundaryRequirementKind;
            let guarantee = match plan.kind {
                BoundaryRequirementKind::Machine => checked
                    .typed
                    .machine(plan.machine)
                    .and_then(|machine| machine.termination_plan.interface.published()),
                BoundaryRequirementKind::Trait => checked
                    .typed
                    .trait_definition_by_symbol(plan.contract_owner)
                    .and_then(|definition| {
                        checked
                            .typed
                            .trait_machine_signature_by_symbol(definition, plan.machine)
                    })
                    .map(|signature| &signature.termination_guarantee),
                BoundaryRequirementKind::Operator => None,
            };
            Ok(
                matches!(guarantee, Some(TerminationGuarantee::Terminates { premises }) if premises.is_empty()),
            )
        }

        fn static_interrupt_execution(
            operation: language_core::static_interrupts::StaticInterruptOperation,
        ) -> terminal_psi::CheckedBoundaryExecution {
            use language_core::static_interrupts::StaticInterruptOperation;
            use terminal_psi::CheckedBoundaryExecution;
            match operation {
                StaticInterruptOperation::Install => {
                    CheckedBoundaryExecution::StaticInterruptInstall
                }
                StaticInterruptOperation::Context => {
                    CheckedBoundaryExecution::StaticInterruptContext
                }
                StaticInterruptOperation::BeginCompletion => {
                    CheckedBoundaryExecution::StaticInterruptBeginCompletion
                }
                StaticInterruptOperation::FinishCompletion => {
                    CheckedBoundaryExecution::StaticInterruptFinishCompletion
                }
            }
        }
    }
    pub(crate) mod call_catalog {

        use super::bodies::UnitPlans;
        use super::{
            CheckedScalarCallee, CheckedTrees, CheckedUnitProviderCandidate, LoweringError,
            UnitBody, checked_scalar_call_closure_with_structural_roots,
            checked_unit_call_closure_including, checked_unit_provider_candidates, unsupported,
        };
        use crate::expression_preparation::computation_graph::boundary_targets;
        use crate::expression_preparation::computation_graph::call_targets;
        use crate::expression_preparation::computation_graph::static_realization_targets;
        use crate::expression_preparation::computation_graph::structural_call_targets;
        use crate::expression_preparation::computation_graph::value_call_operations;
        use crate::shared::checked_facts::value_facts;
        use crate::shared::lowering_error::unit_machine_plan_error;
        pub(crate) struct CheckedCallCatalog {
            pub(crate) operations: Vec<symbols::SymbolHandle>,
            pub(crate) source_machines: Vec<symbols::SymbolHandle>,
            pub(crate) providers: Vec<CheckedUnitProviderCandidate>,
            pub(crate) boundaries: Vec<symbols::SymbolHandle>,
        }

        pub(crate) fn discover(
            checked: &CheckedTrees,
            plans: UnitPlans<'_>,
            entry: symbols::SymbolHandle,
            unit_roots: &[symbols::SymbolHandle],
            program_roots: crate::TerminalProgramRoots<'_>,
            scalar_entry: bool,
        ) -> Result<CheckedCallCatalog, LoweringError> {
            let mut retained_roots = unit_roots.to_vec();
            let mut family_scalar_roots = Vec::new();
            let mut retained_boundary_roots = Vec::new();
            for boundary in program_roots.boundaries {
                if let Some((_, signature)) = checked.trait_machine_signature(*boundary) {
                    retained_boundary_roots.extend(
                        checked
                            .executable_requirement_machine_instances(signature)
                            .map(|instance| instance.symbol),
                    );
                } else {
                    retained_boundary_roots.push(*boundary);
                }
            }
            for boundary in &retained_boundary_roots {
                super::unique_unit_boundary(plans, *boundary).map_err(unit_machine_plan_error(
                    checked,
                    checked.symbols.name(*boundary),
                    *boundary,
                ))?;
            }
            let native_roots = value_facts(checked)
                .structural_values
                .nodes
                .iter()
                .filter_map(|(_, node)| {
                    let owner = checked.typed.machine_by_symbol(node.machine)?;
                    if checked.typed.callable_body_has_open_parameters(owner) {
                        return None;
                    }
                    let checked_trees::CheckedStructuralValueKind::NativeCodeLayout {
                        selected_state,
                    } = node.kind
                    else {
                        return None;
                    };
                    validation::find_machine_by_entry_state(&checked.typed, selected_state)
                        .map(|(machine, _)| machine.symbol)
                });
            for callable in program_roots
                .machines
                .iter()
                .copied()
                .chain(native_roots)
                .chain(
                    super::target_families::runtime_families(checked)?
                        .into_iter()
                        .flat_map(|family| {
                            std::iter::once(family.canonical).chain(
                                family
                                    .members
                                    .iter()
                                    .filter(|member| member.machine != family.canonical)
                                    .map(|member| member.machine),
                            )
                        }),
                )
            {
                let machine =
                    checked
                        .typed
                        .machine_by_symbol(callable)
                        .ok_or(LoweringError::Unsupported(
                            "program-root callable is absent from checked input",
                        ))?;
                for usage in checked
                    .facts
                    .nominal_machine_uses
                    .uses
                    .iter()
                    .filter(|usage| {
                        usage.selected_machine == machine.symbol && usage.callback_entry.is_some()
                    })
                {
                    for requirement in &usage.callback_requirements {
                        if !retained_boundary_roots.contains(requirement) {
                            retained_boundary_roots.push(*requirement);
                        }
                    }
                }
                for conformance in checked.typed.machine_trait_conformances(machine) {
                    if plans.boundary_machines().iter().any(|boundary| {
                        boundary.machine == conformance.requirement_symbol
                            && (boundary.kind
                                != semantic_vocabulary::BoundaryRequirementKind::Trait
                                || boundary.owning_service.is_some())
                    }) && !retained_boundary_roots.contains(&conformance.requirement_symbol)
                    {
                        retained_boundary_roots.push(conformance.requirement_symbol);
                    }
                }
                if !machine.supplies_checked_body() {
                    super::unique_unit_boundary(plans, callable).map_err(
                        unit_machine_plan_error(checked, machine.name.as_str(), callable),
                    )?;
                    if !retained_boundary_roots.contains(&callable) {
                        retained_boundary_roots.push(callable);
                    }
                    continue;
                }
                if UnitBody::contains(plans, callable)
                    && UnitBody::find(plans, callable)?
                        .scalar_result_type()
                        .is_none()
                {
                    if !retained_roots.contains(&callable) {
                        retained_roots.push(callable);
                    }
                } else {
                    CheckedScalarCallee::find_for_unit_call(checked, callable)?;
                    if !family_scalar_roots.contains(&callable) {
                        family_scalar_roots.push(callable);
                    }
                }
            }
            let mut dispatched_scalar_roots = Vec::new();
            let mut known_candidates = Vec::new();
            loop {
                let closure = match retained_roots.split_first() {
                    Some((root, additional)) => {
                        checked_unit_call_closure_including(checked, plans, *root, additional)?
                    }
                    None => Vec::new(),
                };
                let source_machines = plans.source_machines(checked, &closure)?;
                for source in &source_machines {
                    for boundary in boundary_targets(checked, *source)? {
                        if !retained_boundary_roots.contains(&boundary) {
                            retained_boundary_roots.push(boundary);
                        }
                    }
                }
                let candidates = checked_unit_provider_candidates(
                    checked,
                    plans,
                    &closure,
                    &mut retained_boundary_roots,
                    &mut known_candidates,
                )?;
                let mut new_roots = Vec::new();
                for candidate in &candidates {
                    if !closure.contains(&candidate.candidate)
                        && !new_roots.contains(&candidate.candidate)
                    {
                        new_roots.push(candidate.candidate);
                    }
                }
                let mut ordinary_scalar_roots = Vec::new();
                let mut structural_scalar_roots = Vec::new();
                for root in &family_scalar_roots {
                    if !ordinary_scalar_roots.contains(root) {
                        ordinary_scalar_roots.push(*root);
                    }
                    if !CheckedScalarCallee::find_for_unit_call(checked, *root)?
                        .structural_parameters()
                        .is_empty()
                    {
                        structural_scalar_roots.push(*root);
                    }
                }
                if scalar_entry {
                    ordinary_scalar_roots.push(entry);
                    structural_scalar_roots.push(entry);
                }
                for root in &dispatched_scalar_roots {
                    if !ordinary_scalar_roots.contains(root) {
                        ordinary_scalar_roots.push(*root);
                    }
                    if !CheckedScalarCallee::find_for_unit_call(checked, *root)?
                        .structural_parameters()
                        .is_empty()
                        && !structural_scalar_roots.contains(root)
                    {
                        structural_scalar_roots.push(*root);
                    }
                }
                for machine_symbol in &source_machines {
                    let computed_structural_roots =
                        structural_call_targets(checked, *machine_symbol)?;
                    for target in call_targets(checked, *machine_symbol)? {
                        if computed_structural_roots.contains(&target) {
                            CheckedScalarCallee::find_for_unit_call(checked, target)?;
                            if !structural_scalar_roots.contains(&target) {
                                structural_scalar_roots.push(target);
                            }
                        } else {
                            CheckedScalarCallee::find(checked, target)?;
                        }
                        if !ordinary_scalar_roots.contains(&target) {
                            ordinary_scalar_roots.push(target);
                        }
                    }
                    for target in static_realization_targets(checked, *machine_symbol) {
                        match CheckedScalarCallee::find_for_unit_call(checked, target) {
                            Ok(callee) => {
                                if !callee.structural_parameters().is_empty()
                                    && !structural_scalar_roots.contains(&target)
                                {
                                    structural_scalar_roots.push(target);
                                }
                                if !ordinary_scalar_roots.contains(&target) {
                                    ordinary_scalar_roots.push(target);
                                }
                            }
                            Err(_) if UnitBody::contains(plans, target) => {
                                if !closure.contains(&target) && !new_roots.contains(&target) {
                                    new_roots.push(target);
                                }
                            }
                            Err(error) => return Err(error),
                        }
                    }
                }
                for machine_symbol in &closure {
                    for operation in UnitBody::find(plans, *machine_symbol)?.operations() {
                        if let Some(call) = operation
                            .call()
                            .filter(|call| call.is_module() && call.scalar_result().is_some())
                        {
                            let target_machine = call.target.machine;
                            if !ordinary_scalar_roots.contains(&target_machine) {
                                ordinary_scalar_roots.push(target_machine);
                            }
                            if (!call.structural_arguments.is_empty()
                                || !call.claim_transfers().is_empty())
                                && !structural_scalar_roots.contains(&target_machine)
                            {
                                structural_scalar_roots.push(target_machine);
                            }
                        }
                    }
                }
                let scalar_roots = ordinary_scalar_roots;
                let scalar_closure = checked_scalar_call_closure_with_structural_roots(
                    checked,
                    &scalar_roots,
                    &structural_scalar_roots,
                )?;

                for source in source_machines.iter().chain(&scalar_closure) {
                    for cleanup in
                        crate::unit::unit_cleanup::computation_cleanups(checked, *source)?
                    {
                        let target = cleanup.cleanup_machine;
                        if !closure.contains(&target) && !new_roots.contains(&target) {
                            new_roots.push(target);
                        }
                    }
                    for operation in value_call_operations(checked, *source, None)? {
                        if let Some(call) = operation
                            .call()
                            .filter(|call| call.is_module() && call.scalar_result().is_none())
                        {
                            let target = call.target.machine;
                            if !closure.contains(&target) && !new_roots.contains(&target) {
                                new_roots.push(target);
                            }
                        }
                    }
                }
                for source in &scalar_closure {
                    CheckedScalarCallee::find_for_unit_call(checked, *source)?;
                    if !closure.contains(source) && !new_roots.contains(source) {
                        new_roots.push(*source);
                    }
                }
                let mut dispatched = false;
                for source in source_machines.iter().chain(&scalar_closure) {
                    for selection in checked
                        .facts
                        .dynamic_conformances
                        .selections
                        .iter()
                        .filter(|selection| selection.machine == *source)
                    {
                        for row in &selection.rows {
                            let realization = row.realization_machine;
                            if UnitBody::contains(plans, realization) {
                                if !closure.contains(&realization)
                                    && !new_roots.contains(&realization)
                                {
                                    new_roots.push(realization);
                                }
                            } else if !scalar_closure.contains(&realization)
                                && !dispatched_scalar_roots.contains(&realization)
                            {
                                dispatched_scalar_roots.push(realization);
                                dispatched = true;
                            }
                        }
                    }
                }
                if !new_roots.is_empty() || dispatched {
                    retained_roots.extend(new_roots);
                    continue;
                }
                if scalar_closure
                    .iter()
                    .any(|source| !closure.contains(source))
                {
                    return unsupported("scalar helper is absent from the Unit machine closure");
                }
                return Ok(CheckedCallCatalog {
                    operations: closure,
                    source_machines,
                    providers: candidates,
                    boundaries: retained_boundary_roots,
                });
            }
        }
    }
    pub(crate) mod call_closure {

        use super::super::CheckedTerminalSignatureEligibility;
        use super::bodies::UnitPlans;
        use super::{
            CheckedBoundaryMachinePlan, CheckedTrees, CheckedUnitEffectOperationPlan,
            LoweringError, UnitBody, unsupported,
        };
        use crate::shared::checked_facts::flow_facts;
        use checked_trees::{CheckedUnitCallResult, CheckedUnitCallRoute};
        use typed_trees::type_identity::CallableIdentityId;

        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(crate) enum BoundaryIdentity {
            Requirement(CallableIdentityId),
            ForeignLeaf(symbols::SymbolHandle),
        }

        impl BoundaryIdentity {
            pub(crate) fn order_key(self) -> (bool, CallableIdentityId, u32, u32) {
                match self {
                    Self::Requirement(callable) => (false, callable, 0, 0),
                    Self::ForeignLeaf(declaration) => (
                        true,
                        CallableIdentityId::default(),
                        declaration.arena_index(),
                        declaration.generation(),
                    ),
                }
            }

            pub(crate) fn render(self, checked: &CheckedTrees) -> Result<String, LoweringError> {
                match self {
                    Self::Requirement(callable) => {
                        Ok(checked.typed.callable_identity_text(callable))
                    }
                    Self::ForeignLeaf(declaration) => {
                        Ok(unique_terminal_selection(checked, declaration)?
                            .name
                            .clone())
                    }
                }
            }
        }

        pub(crate) fn checked_unit_call_closure_including(
            checked: &CheckedTrees,
            plans: UnitPlans<'_>,
            entry: symbols::SymbolHandle,
            additional_roots: &[symbols::SymbolHandle],
        ) -> Result<Vec<symbols::SymbolHandle>, LoweringError> {
            let mut closure = vec![entry];
            for root in additional_roots {
                if closure.contains(root) {
                    return unsupported("attached Unit closure contains a duplicate explicit root");
                }
                closure.push(*root);
            }
            let mut next = 0_usize;
            while let Some(machine_symbol) = closure.get(next).copied() {
                next += 1;
                let name = checked_terminal_machine_name(checked, machine_symbol)?;
                let machine = UnitBody::find(plans, machine_symbol).map_err(
                    crate::shared::lowering_error::unit_machine_plan_error(
                        checked,
                        name,
                        machine_symbol,
                    ),
                )?;
                for target in machine
                    .operations()
                    .filter_map(|operation| unit_closure_target(plans, operation))
                {
                    if !closure.contains(&target) {
                        closure.push(target);
                    }
                }
                for request in checked
                    .facts
                    .quotient_operations
                    .iter()
                    .filter(|row| row.public_machine == machine_symbol)
                {
                    let (representative, _) = validation::find_machine_by_entry_state(
                        &checked.typed,
                        request.representative_state,
                    )
                    .ok_or(LoweringError::Unsupported(
                        "quotient operation has no representative body",
                    ))?;
                    if !closure.contains(&representative.symbol) {
                        closure.push(representative.symbol);
                    }
                }
                for cleanup in crate::unit::unit_cleanup::continuation_cleanups(
                    &checked.facts.flow.terminal_unit_effects.cleanup_control,
                    machine.operations(),
                ) {
                    if !closure.contains(&cleanup.cleanup_machine) {
                        closure.push(cleanup.cleanup_machine);
                    }
                }
                {
                    let rows = plans
                        .for_machine(machine_symbol)
                        .map(|machine| machine.normal_exit_cleanups)
                        .into_iter()
                        .chain(
                            plans
                                .composed_for_machine(machine_symbol)
                                .into_iter()
                                .flat_map(|machine| {
                                    machine
                                        .states
                                        .iter()
                                        .map(|state| state.normal_exit_cleanups)
                                }),
                        );
                    for cleanup in rows
                        .flat_map(|rows| {
                            flow_facts(checked)
                                .terminal_unit_effects
                                .cleanup_control
                                .normal_actions(rows)
                        })
                        .filter_map(|action| match action {
                            checked_trees::CheckedNormalCleanupAction::InvokeNominal(cleanup) => {
                                Some(cleanup)
                            }
                            checked_trees::CheckedNormalCleanupAction::Discard(_)
                            | checked_trees::CheckedNormalCleanupAction::RetireHome { .. } => None,
                        })
                    {
                        if !closure.contains(&cleanup.cleanup_machine) {
                            closure.push(cleanup.cleanup_machine);
                        }
                    }
                }
            }
            Ok(closure)
        }

        pub(crate) use crate::scalar_lowering::call_closure::embedded::checked_scalar_call_closure_with_structural_roots;

        pub(crate) fn unique_unit_boundary(
            plans: UnitPlans<'_>,
            symbol: symbols::SymbolHandle,
        ) -> Result<&CheckedBoundaryMachinePlan, LoweringError> {
            let mut matches = plans
                .boundary_machines()
                .iter()
                .filter(|plan| plan.machine == symbol);
            let plan = matches.next().ok_or(LoweringError::Unsupported(
                "attached Unit closure is missing a checked boundary machine plan",
            ))?;
            if matches.next().is_some() {
                return unsupported(
                    "attached Unit closure contains duplicate boundary machine plans",
                );
            }
            if !plan.pending_scalar_requires.is_empty() || !plan.pending_scalar_recipes.is_empty() {
                return unsupported(
                    "attached Unit boundary retains pending scalar contract clauses",
                );
            }
            Ok(plan)
        }

        pub(crate) fn checked_terminal_machine_name(
            checked: &CheckedTrees,
            symbol: symbols::SymbolHandle,
        ) -> Result<&str, LoweringError> {
            let selection = unique_terminal_selection(checked, symbol)?;
            if !matches!(
                selection.signature,
                CheckedTerminalSignatureEligibility::FreeUnitEffect
                    | CheckedTerminalSignatureEligibility::Eligible
            ) {
                return unsupported("Unit-effect member has an invalid checked terminal selection");
            }
            Ok(&selection.name)
        }

        fn unique_terminal_selection(
            checked: &CheckedTrees,
            symbol: symbols::SymbolHandle,
        ) -> Result<&checked_trees::CheckedTerminalMachineSelection, LoweringError> {
            let mut matches = flow_facts(checked)
                .terminal_machines
                .machines
                .iter()
                .filter(|selection| selection.machine == symbol);
            let selection =
                matches
                    .next()
                    .ok_or_else(|| LoweringError::InvalidUnitMachinePlan {
                        machine: checked.symbols.name(symbol).to_owned(),
                        reason: "is called by this closure but the selected target selects no \
                         terminal machine for it",
                        gap: flow_facts(checked)
                            .terminal_unit_effects
                            .gap_diagnostic(symbol, &checked.symbols),
                    })?;
            if matches.next().is_some() || selection.name.is_empty() {
                return unsupported("Unit-effect member has an invalid checked terminal selection");
            }
            Ok(selection)
        }

        pub(crate) fn checked_unit_boundary_identity(
            checked: &CheckedTrees,
            symbol: symbols::SymbolHandle,
        ) -> Result<BoundaryIdentity, LoweringError> {
            let requirements = checked
                .typed
                .traits()
                .iter()
                .flat_map(|definition| {
                    checked
                        .typed
                        .executable_trait_machine_instances(definition)
                        .filter(move |signature| signature.symbol == symbol)
                        .map(move |signature| (definition, signature))
                })
                .collect::<Vec<_>>();
            if let [(definition, signature)] = requirements.as_slice() {
                let identity = checked
                    .typed
                    .trait_requirement_callable_identity(definition, signature);
                if identity.is_valid() {
                    return Ok(BoundaryIdentity::Requirement(identity));
                }
            }
            if let Some(accessor) =
                typed_trees::typed_trees::ScalarAccessorDeclaration::authenticate(
                    &checked.typed,
                    symbol,
                )
                .map_err(LoweringError::InvalidSealedAccessorDeclaration)?
            {
                let machine =
                    checked
                        .typed
                        .machine(accessor.machine())
                        .ok_or(LoweringError::Unsupported(
                            "sealed accessor lost its exact machine",
                        ))?;
                let identity = checked.typed.machine_callable_identity(machine);
                if !identity.is_valid() {
                    return unsupported("sealed accessor has no normalized callable identity");
                }
                return Ok(BoundaryIdentity::Requirement(identity));
            }
            let mut requirements = checked.typed.machines().iter().filter(|machine| {
                machine.symbol == symbol
                    && (matches!(
                        machine.supply_mode,
                        language_semantics::MachineSupplyMode::Requirement
                            | language_semantics::MachineSupplyMode::TopLevelRequirement
                    ) || (machine.supply_mode
                        == language_semantics::MachineSupplyMode::Boundary
                        && machine.service_reach_is_installation_bound))
            });
            if let Some(requirement) = requirements.next() {
                if requirements.next().is_some() {
                    return unsupported("top-level boundary requirement identity is ambiguous");
                }
                let identity = checked.typed.machine_callable_identity(requirement);
                if identity.is_valid() {
                    return Ok(BoundaryIdentity::Requirement(identity));
                }
                return unsupported(
                    "top-level boundary has no normalized machine overload identity",
                );
            }
            if let Some(operator) = checked
                .typed
                .operators()
                .iter()
                .find(|operator| operator.is_boundary && operator.symbol == symbol)
            {
                let identity = checked
                    .typed
                    .boundary_operator_requirement_callable_identity(operator);
                if !identity.is_valid() {
                    return unsupported("boundary operator has no normalized requirement identity");
                }
                return Ok(BoundaryIdentity::Requirement(identity));
            }
            unique_terminal_selection(checked, symbol)
                .map(|selection| BoundaryIdentity::ForeignLeaf(selection.machine))
        }

        fn unit_closure_target(
            plans: UnitPlans<'_>,
            operation: &CheckedUnitEffectOperationPlan,
        ) -> Option<symbols::SymbolHandle> {
            let call = operation.call()?;
            match (call.route(), call.result()) {
                (
                    CheckedUnitCallRoute::Module { .. },
                    CheckedUnitCallResult::Unit | CheckedUnitCallResult::NoNormal,
                ) => Some(call.target.machine),
                (CheckedUnitCallRoute::Module { .. }, CheckedUnitCallResult::Structural { .. })
                    if UnitBody::contains(plans, call.target.machine) =>
                {
                    Some(call.target.machine)
                }
                _ => None,
            }
        }
    }
    pub(crate) mod call_parameters {

        use super::bodies::{UnitBody, UnitPlans};
        use super::{CheckedTrees, LoweringError, unique_unit_boundary, unsupported};
        use crate::scalar_lowering::call_closure::callee::CheckedScalarCallee;
        use crate::shared::checked_facts::flow_facts;
        use checked_trees::{
            CheckedStructuralScalarParameterPlan, CheckedUnitCallPlan, CheckedUnitCallResult,
            CheckedUnitStructuralParameterPlan,
        };

        struct ParameterRoster<'checked> {
            scalar: &'checked [CheckedStructuralScalarParameterPlan],
            structural: &'checked [CheckedUnitStructuralParameterPlan],
        }

        fn parameter_roster<'checked>(
            checked: &'checked CheckedTrees,
            call: &CheckedUnitCallPlan,
        ) -> Result<ParameterRoster<'checked>, LoweringError> {
            let plans = UnitPlans::published(&checked.facts.flow);
            let (state, scalar, structural) = if call.dynamic_selector().is_some() {
                let target = flow_facts(checked)
                    .terminal_unit_effects
                    .dynamic_slots
                    .iter()
                    .find(|target| target.slot == call.target.machine)
                    .ok_or(LoweringError::Unsupported(
                        "call parameter roster names no checked dynamic slot",
                    ))?;
                (
                    target.slot,
                    target.scalar_parameters.as_slice(),
                    target.structural_parameters.as_slice(),
                )
            } else if call.is_boundary()
                || matches!(
                    call.route(),
                    checked_trees::CheckedUnitCallRoute::ActivationStorage { .. }
                )
            {
                let target = unique_unit_boundary(plans, call.target.machine)?;
                (
                    target.state,
                    target.scalar_parameters.as_slice(),
                    target.structural_parameters.as_slice(),
                )
            } else if matches!(call.result(), CheckedUnitCallResult::Scalar(_)) {
                let target = CheckedScalarCallee::find_for_unit_call(checked, call.target.machine)?
                    .body()
                    .entry()?;
                (
                    target.state,
                    target.scalar_parameters,
                    target.structural_parameters,
                )
            } else {
                let name = checked.symbols.display_path(call.target.machine, "::");
                let target = UnitBody::find(plans, call.target.machine)
                    .map_err(crate::shared::lowering_error::unit_machine_plan_error(
                        checked,
                        &name,
                        call.target.machine,
                    ))?
                    .entry()?;
                (
                    target.state,
                    target.scalar_parameters,
                    target.structural_parameters,
                )
            };
            if state != call.target.state {
                return unsupported("call parameter roster belongs to another entry state");
            }
            Ok(ParameterRoster { scalar, structural })
        }

        pub(crate) fn structural_parameters<'checked>(
            checked: &'checked CheckedTrees,
            call: &CheckedUnitCallPlan,
        ) -> Result<&'checked [CheckedUnitStructuralParameterPlan], LoweringError> {
            Ok(parameter_roster(checked, call)?.structural)
        }

        pub(crate) fn scalar_positions(
            checked: &CheckedTrees,
            call: &CheckedUnitCallPlan,
            signature: &crate::shared::declarations::TargetSignature<'_>,
        ) -> Result<Vec<usize>, LoweringError> {
            let parameters = parameter_roster(checked, call)?.scalar;
            if parameters.len() != call.scalar_operands().len() {
                return unsupported("call scalar arguments disagree with the retained formals");
            }
            let mut positions = Vec::with_capacity(parameters.len());
            for retained in parameters {
                let position = signature
                    .authored_parameter_position(retained.source_position)
                    .ok_or(LoweringError::Unsupported(
                        "retained scalar formal has no authored position",
                    ))? as usize;
                let authored = &signature.parameters[position];
                if authored.relevance.is_erased()
                    || checked.primitive_type_reference(authored.type_reference)
                        != Some(retained.primitive_type)
                    || positions
                        .last()
                        .is_some_and(|previous| *previous >= position)
                {
                    return unsupported(
                        "retained scalar formal disagrees with its authored partition",
                    );
                }
                positions.push(position);
            }
            Ok(positions)
        }

        pub(crate) fn structural_positions(
            checked: &CheckedTrees,
            call: &CheckedUnitCallPlan,
            signature: &crate::shared::declarations::TargetSignature<'_>,
        ) -> Result<Vec<u32>, LoweringError> {
            let parameters = structural_parameters(checked, call)?;
            if parameters.len() != call.structural_arguments.len() {
                return unsupported("call structural arguments disagree with the retained formals");
            }
            let mut positions = Vec::with_capacity(parameters.len());
            for retained in parameters {
                let position = signature
                    .authored_parameter_position(retained.position)
                    .ok_or(LoweringError::Unsupported(
                        "retained call formal has no authored position",
                    ))?;
                let authored = &signature.parameters[position as usize];
                if authored.relevance.is_erased()
                    || checked
                        .primitive_type_reference(authored.type_reference)
                        .is_some()
                    || positions
                        .last()
                        .is_some_and(|previous| *previous >= position)
                {
                    return unsupported(
                        "retained call formal disagrees with its authored partition",
                    );
                }
                positions.push(position);
            }
            Ok(positions)
        }
    }
    pub(crate) mod call_results {
        use crate::emission::claim_bindings::{self, ClaimBindings, PreparedResultClaim};
        use crate::shared::lowering_error::{LoweringError, unsupported};
        use crate::shared::terminal_identities::{
            StructuralDomainBinding, lookup_domain_id_for_carrier,
        };
        use checked_trees::{CheckedTrees, CheckedUnitCallCoordinate, CheckedUnitCallPlan};
        use semantic_vocabulary::{PlaceId, StructuralTypeId};
        use terminal_psi::{StructuralOperationResult, StructuralTypeDeclaration};

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct Prepared {
            value: StructuralOperationResult,
            claims: Vec<PreparedResultClaim>,
            domains: Vec<language_semantics::SemanticDomainId>,
            projected: Vec<checked_trees::CheckedStructuralPathQualification>,
            issuer: symbols::SymbolHandle,
        }

        pub(crate) fn module_result_qualification_domains(
            checked: &CheckedTrees,
            call: &CheckedUnitCallPlan,
        ) -> Vec<language_semantics::SemanticDomainId> {
            let mut domains = call
                .structural_custody()
                .map(|custody| custody.result_qualifications.clone())
                .unwrap_or_default();
            if let checked_trees::CheckedUnitCallRoute::Module { .. } = call.route() {
                domains.append(&mut super::parameters::authored_result_whole_domains(
                    checked,
                    call.target.machine,
                ));
            }
            domains.sort_by_key(|domain| domain.0);
            domains.dedup();
            domains
        }

        pub(crate) fn prepare(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            call: &CheckedUnitCallPlan,
            place: PlaceId,
            structural_type: StructuralTypeId,
            types: &[StructuralTypeDeclaration],
        ) -> Result<Prepared, LoweringError> {
            let (Some(binding), Some(custody)) =
                (call.structural_result(), call.structural_custody())
            else {
                return unsupported("structural call result has no checked receiving custody");
            };
            let plans = &checked.facts.flow.terminal_unit_effects;
            let public_result = match call.route() {
                checked_trees::CheckedUnitCallRoute::Module { .. }
                | checked_trees::CheckedUnitCallRoute::ActivationStorage { .. }
                | checked_trees::CheckedUnitCallRoute::DescriptorTable { .. } => None,
                checked_trees::CheckedUnitCallRoute::Boundary => Some(
                    &super::call_closure::unique_unit_boundary(
                        super::bodies::UnitPlans::published(&checked.facts.flow),
                        call.target.machine,
                    )?
                    .result,
                ),
                checked_trees::CheckedUnitCallRoute::Dynamic { .. } => Some(
                    &plans
                        .dynamic_slots
                        .iter()
                        .find(|slot| slot.slot == call.target.machine)
                        .ok_or(LoweringError::Unsupported(
                            "structural result names an unpublished dynamic slot",
                        ))?
                        .result,
                ),
            };
            let (domains, projected, issuer) = if let Some(public_result) = public_result {
                let checked_trees::CheckedBoundaryMachineResultPlan::Structural {
                    type_identity,
                    multiplicity,
                    qualifications,
                    projected_qualifications,
                } = public_result
                else {
                    return unsupported("structural call target declares a non-structural result");
                };
                if *type_identity != binding.type_identity || *multiplicity != binding.multiplicity
                {
                    return unsupported("structural call result disagrees with its declaration");
                }
                (
                    qualifications.clone(),
                    projected_qualifications.clone(),
                    call.target.machine,
                )
            } else {
                let (_, state) =
                    crate::shared::declarations::state_declaration(checked, call.target.state)?;
                (
                    module_result_qualification_domains(checked, call),
                    super::parameters::result_projected_qualifications(
                        checked,
                        call.target.machine,
                        state.return_type,
                    )?,
                    symbols::SymbolHandle::default(),
                )
            };
            Ok(Prepared {
                value: StructuralOperationResult {
                    qualification_establishments: Vec::new(),
                    place,
                    structural_type,
                    multiplicity: crate::shared::terminal_identities::structural_multiplicity(
                        binding.multiplicity,
                    ),
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    additional_input_qualifications: Vec::new(),
                    claims: Vec::new(),
                },
                claims: claim_bindings::prepare_call_result_claims(
                    identities,
                    &custody.result_claims,
                    structural_type,
                    types,
                )?,
                domains,
                projected,
                issuer,
            })
        }

        impl Prepared {
            pub(crate) fn emit(
                &self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                checked: &CheckedTrees,
                state: symbols::SymbolHandle,
                coordinate: CheckedUnitCallCoordinate,
                domain_ids: &[StructuralDomainBinding],
                types: &[StructuralTypeDeclaration],
                claims: &mut ClaimBindings<'_>,
            ) -> Result<StructuralOperationResult, LoweringError> {
                let mut value = self.value.clone();
                value.qualifications = self
                    .domains
                    .iter()
                    .map(|domain| {
                        lookup_domain_id_for_carrier(domain_ids, *domain, value.structural_type)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                value.qualifications.sort();
                value.projected_qualifications = super::parameters::lower_projected_qualifications(
                    identities,
                    &self.projected,
                    domain_ids,
                    value.structural_type,
                    types,
                )?;
                value.claims = claim_bindings::emit_call_result_claims(
                    &self.claims,
                    state,
                    coordinate,
                    claims,
                )?;
                value.claims.sort();
                if self.issuer.is_valid() {
                    value.qualification_establishments =
                        super::catalog::call_result_qualification_establishments(
                            identities,
                            checked,
                            state,
                            coordinate,
                            self.issuer,
                            &self.domains,
                            domain_ids,
                        )?;
                }
                Ok(value)
            }
        }
    }
    pub(crate) mod calling_signatures {

        use super::{LoweringError, UnitPlans, lookup_type_id};
        use semantic_vocabulary::StructuralTypeId;
        use terminal_psi::TerminalIdentityTableBuilder;
        use terminal_psi::{CallingAbiDeclaration, CallingAbiStorage, CallingSignatureTelescope};
        use typed_trees::type_identity::TypeIdentityId;

        pub(crate) fn lower(
            identities: &mut TerminalIdentityTableBuilder,
            plans: UnitPlans<'_>,
            types: &[(TypeIdentityId, StructuralTypeId)],
        ) -> Result<Vec<CallingAbiDeclaration>, LoweringError> {
            let mut rows = Vec::with_capacity(plans.calling_signatures().len());
            for source in plans.calling_signatures() {
                let storage = match &source.storage {
                    Ok(storage) => Ok(CallingAbiStorage {
                        parameters: storage.parameters.iter().map(|parameter| Ok(terminal_psi::CallingAbiParameter {
                            structural_type: lookup_type_id(types, &parameter.type_identity)?,
                            callback_backing: parameter.callback_backing.map(|identity| lookup_type_id(types, &identity)).transpose()?,
                            access: crate::expression_preparation::bindings::structural_paths::terminal_access(parameter.access),
                        })).collect::<Result<_, LoweringError>>()?,
                        result: storage.result.map(|identity| lookup_type_id(types, &identity)).transpose()?,
                        telescope: CallingSignatureTelescope {
                            parameters: storage.telescope.parameters.iter().map(|formal| terminal_psi::CallingRuntimeFormal {
                                identity: formal.identity, native_ordinal: formal.native_ordinal,
                            }).collect(),
                            binders: storage.telescope.binders.iter().map(|binder| terminal_psi::CallingCallbackBinder {
                                binder: binder.binder, requirement: binder.requirement,
                                requirement_identity: identities.intern(&binder.requirement_identity),
                                static_machine_ordinal: binder.static_machine_ordinal,
                                invokes_synchronously: binder.invokes_synchronously,
                            }).collect(),
                            direct_callbacks: storage.telescope.direct_callbacks.iter().map(|callback| terminal_psi::CallingDirectCallback {
                                identity: callback.identity, native_ordinal: callback.native_ordinal,
                                binder: callback.binder, requirement: callback.requirement,
                            }).collect(),
                        },
                    }),
                    Err(reason) => Err(reason.clone()),
                };
                rows.push(CallingAbiDeclaration {
                    id: source.id,
                    requirement_identity: identities.intern(&source.requirement_identity),
                    declaring_application: identities.intern(&source.declaring_application),
                    operands: source
                        .operands
                        .iter()
                        .map(|operand| terminal_psi::CallingStaticOperand {
                            ordinal: operand.ordinal,
                            kind: operand.kind,
                            identity: identities.intern(&operand.identity),
                        })
                        .collect(),
                    default_eligible: source.default_eligible,
                    physical_owner: source.physical_owner.clone(),
                    formals: source.formals.clone(),
                    storage,
                });
            }
            rows.sort_by_key(|row| row.id);
            if rows.windows(2).any(|pair| pair[0].id == pair[1].id) {
                return Err(LoweringError::Unsupported(
                    "declaration ABI application identities collide",
                ));
            }
            Ok(rows)
        }
    }
    pub(crate) mod catalog {
        use super::super::{
            BTreeSet, CheckedUnitStructuralTypeShape, DomainSemanticId,
            InstallationReachDependency, ProgramLocalRootIntroductionSchema, ServiceDeclaration,
            ServiceId, ServiceReachId, ServiceReachInterface, ServiceReachPlan, ServiceReachRowId,
            StructuralAccess, StructuralCaseDeclaration, StructuralCaseId,
            StructuralDomainDeclaration, StructuralFieldDeclaration,
            StructuralParameterDeclaration, StructuralTypeDeclaration, lower_mixed_cases,
            lower_mixed_fields, program_local_root_introduction_compatibility_report_identity,
            service_id, structural_domain_id, structural_field_id, structural_type_id,
            terminal_byte_sequence_carrier,
        };
        use super::bodies::UnitPlans;
        use super::{
            CheckedBoundaryMachinePlan, CheckedTrees, CheckedUnitEffectMachinePlan,
            CheckedUnitEffectOperationPlan, CheckedUnitProviderCandidate, LoweringError,
            SemanticDomainId, ServiceReachSummary, StructuralTypeId, StructuralTypeShape, UnitBody,
            allocate_dense, content_conservation, dense_identity, lookup_domain_id,
            lookup_service_id, lookup_type_id, terminal_scalar_type, unique_unit_boundary,
            unsupported,
        };
        use crate::emission::call_qualifications::declaration_represents;
        use crate::emission::call_qualifications::for_call;
        use crate::emission::structural_byte_sequence_store::literal_view_type;
        use crate::expression_preparation::bindings::structural_paths::terminal_access;
        use crate::expression_preparation::computation_graph::reachable_structural_values;
        use crate::expression_preparation::computation_graph::value_call_operations;
        use crate::expression_preparation::structural_value_requirements::collect_operation_requirements;
        use crate::expression_preparation::structural_value_requirements::collect_value_requirements;
        use crate::machine_lowering::declarations::semantic_identity;
        use crate::scalar_lowering::computations::operand_types::type_requirements;
        use crate::shared::checked_facts::{flow_facts, value_facts};
        use crate::shared::declarations::state_declaration;
        use crate::shared::structural_types::lower_field_type;
        use crate::shared::terminal_identities::stored_structural_identity;
        use crate::shared::terminal_identities::{
            BYTE_VIEW_ELEMENT_TYPE_IDENTITY, StructuralDomainBinding, lookup_domain_id_for_carrier,
            service_declaration_identity, structural_domain_identity,
        };
        use crate::unit::attached_unit::call_closure::BoundaryIdentity;
        use checked_trees::{CheckedBoundaryMachineResultPlan, CheckedUnitStructuralTypePlan};
        use terminal_psi::StructuralReferentIdentity;

        pub(crate) mod operation_types {
            use super::*;
            use crate::shared::terminal_identities::byte_view_element_identity;

            pub(crate) fn collect_operation_type_roots(
                checked: &CheckedTrees,
                plans: UnitPlans<'_>,
                operation: &CheckedUnitEffectOperationPlan,
                roots: &mut Vec<typed_trees::type_identity::TypeIdentityId>,
            ) -> Result<(), LoweringError> {
                for argument in operation.structural_operands() {
                    roots.push(argument.type_identity);
                    roots.extend(argument.borrowed_root_type_identity());
                    roots.extend(argument.subslice_root_type_identity());
                    if let Some(storage) = argument.subslice_storage_type_identity()
                        && storage.is_valid()
                    {
                        roots.push(storage);
                    }
                }
                if let Some((result, _)) = operation.discardable_structural_result() {
                    roots.push(result.type_identity);
                }
                if let Some((value, _)) = operation.structural_value_evaluation() {
                    collect_value_roots(checked, value, roots)?;
                }
                match operation {
                    CheckedUnitEffectOperationPlan::AtomicAccess(access) => {
                        super::projection_types::collect(
                            plans,
                            access.receiver_root_type_identity,
                            &access.receiver.path,
                            roots,
                        )?;
                    }
                    CheckedUnitEffectOperationPlan::CaptureAssignmentDestination {
                        destination,
                        ..
                    } => {
                        roots.push(destination.type_identity);
                    }
                    CheckedUnitEffectOperationPlan::EstablishReference {
                        result,
                        source,
                        source_contract,
                        ..
                    } => {
                        super::collect_reference_contract_types(checked, source_contract, roots);
                        roots.push(result.type_identity);
                        roots.push(source.type_identity);
                    }
                    CheckedUnitEffectOperationPlan::EstablishLocal(
                        checked_trees::CheckedUnitLocalBindingPlan::ScalarArray { result, .. },
                    )
                    | CheckedUnitEffectOperationPlan::EstablishLocal(
                        checked_trees::CheckedUnitLocalBindingPlan::ByteSequence { result, .. },
                    ) => {
                        roots.push(result.type_identity);
                    }
                    CheckedUnitEffectOperationPlan::EstablishViewSubslice { result, source } => {
                        roots.push(result.type_identity);
                        roots.extend(source.subslice_root_type_identity());
                        if let Some(storage) = source.subslice_storage_type_identity()
                            && storage.is_valid()
                        {
                            roots.push(storage);
                        }
                    }
                    CheckedUnitEffectOperationPlan::EstablishLocal(
                        checked_trees::CheckedUnitLocalBindingPlan::StructuralValue {
                            result,
                            value,
                            ..
                        },
                    ) => {
                        roots.push(result.type_identity);
                        if establishes_byte_element_view(
                            checked,
                            plans,
                            result.type_identity,
                            *value,
                        )? && let Some(identity) =
                            byte_view_element_identity(plans.structural_types())
                        {
                            roots.push(identity);
                        }
                    }
                    CheckedUnitEffectOperationPlan::EstablishLocal(
                        checked_trees::CheckedUnitLocalBindingPlan::Primitive {
                            type_identity, ..
                        },
                    ) => {
                        roots.push(*type_identity);
                    }
                    CheckedUnitEffectOperationPlan::MoveStructuralField {
                        result, source, ..
                    } => {
                        roots.push(result.type_identity);
                        roots.push(source.type_identity);
                    }
                    CheckedUnitEffectOperationPlan::Store(checked_trees::CheckedStorePlan {
                        destination:
                            checked_trees::CheckedStoreDestination {
                                leaf:
                                    checked_trees::CheckedStoreLeaf::StructuralStore {
                                        type_identity,
                                        ..
                                    }
                                    | checked_trees::CheckedStoreLeaf::OwnedRebind { type_identity },
                                ..
                            },
                        value: checked_trees::CheckedStoreValue::Place(value),
                        ..
                    }) => {
                        roots.push(*type_identity);
                        roots.push(value.type_identity);
                    }
                    CheckedUnitEffectOperationPlan::Store(checked_trees::CheckedStorePlan {
                        value: checked_trees::CheckedStoreValue::Bytes(value),
                        ..
                    }) => {
                        for (_, node) in value.nodes.iter() {
                            if let checked_trees::CheckedByteSequenceValue::Descriptor { source, .. } = node
                                && !matches!(source.source, checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { .. }) {
                                roots.push(source.type_identity);
                            }
                        }
                    }
                    _ => {}
                }
                Ok(())
            }
        }
        pub(crate) mod place_types {

            use super::*;
            use crate::expression_preparation::computation_graph::reachable_nodes;
            use crate::expression_preparation::computation_graph::reachable_structural_values;
            use crate::shared::checked_facts::value_facts;
            use crate::shared::declarations::state_declaration;
            use crate::shared::terminal_identities::stored_structural_identity;
            use checked_trees::{
                CheckedUnitStructuralArgumentPlan,
                CheckedUnitStructuralArgumentSourcePlan as Source,
                CheckedUnitStructuralPathSegment as Segment,
            };
            use typed_trees::type_identity::TypeIdentityId;

            pub(crate) fn collect<'plans>(
                checked: &'plans CheckedTrees,
                plans: UnitPlans<'_>,
                body: UnitBody<'plans>,
                roots: &mut Vec<TypeIdentityId>,
            ) -> Result<(), LoweringError> {
                match body {
                    UnitBody::Ordinary(machine) => collect_scope(
                        checked,
                        plans,
                        machine.machine,
                        machine.state,
                        &machine.structural_parameters,
                        &machine.place_loans,
                        &machine.operations,
                        None,
                        roots,
                    ),
                    UnitBody::Composed(machine) => {
                        for state in &machine.states {
                            collect_scope(
                                checked,
                                plans,
                                state_declaration(checked, state.state)?.0.symbol,
                                state.state,
                                &state.structural_parameters,
                                &state.place_loans,
                                &state.operations,
                                Some(state),
                                roots,
                            )?;
                        }
                        Ok(())
                    }
                }
            }

            fn collect_scope<'plans>(
                checked: &'plans CheckedTrees,
                plans: UnitPlans<'_>,
                machine: symbols::SymbolHandle,
                state: symbols::SymbolHandle,
                parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
                place_loans: &[checked_trees::CheckedPlaceLoanPlan],
                operations: &'plans [CheckedUnitEffectOperationPlan],
                control: Option<&'plans checked_trees::CheckedComposedUnitControlStatePlan>,
                roots: &mut Vec<TypeIdentityId>,
            ) -> Result<(), LoweringError> {
                let mut collector = StorageDemand {
                    checked,
                    plans,
                    machine,
                    state,
                    parameters,
                    place_loans,
                    roots,
                };
                let mut namespace = Vec::new();
                collector.operations(operations.iter(), &mut namespace)?;
                if let Some(control) = control {
                    use checked_trees::{
                        CheckedComposedUnitControlTerminatorPlan as Terminator,
                        CheckedStructuralControlTransferSourcePlan as Transfer,
                    };
                    if let Terminator::Guarded { return_values, .. } = &control.terminator {
                        for operation in return_values {
                            collector.operations(
                                std::iter::once(&operation.operation),
                                &mut namespace.clone(),
                            )?;
                        }
                    }
                    if let Terminator::CompleteExpression { completion } = &control.terminator {
                        for operations in completion.expression.invocation_scopes() {
                            collector.operations(operations.iter(), &mut namespace.clone())?;
                        }
                    }
                    for arm in control.terminator.return_arms() {
                        for scope in arm.operation_scopes() {
                            collector.operations(scope.into_iter(), &mut namespace.clone())?;
                        }
                    }
                    for successor in control.successors() {
                        for transfer in &successor.transfers {
                            let mut selected = namespace.clone();
                            let mut payload;
                            let subject = match &transfer.source {
                                Transfer::EstablishedValue {
                                    operands,
                                    establishment,
                                    presentation,
                                } => {
                                    collector.operations(
                                        operands
                                            .iter()
                                            .chain(std::iter::once(establishment.as_ref())),
                                        &mut selected,
                                    )?;
                                    Some(presentation)
                                }
                                Transfer::CasePayload {
                                    subject,
                                    case_identity,
                                    field_identity,
                                    path,
                                } => {
                                    payload = subject.clone();
                                    payload.path.extend([
                                        Segment::Case(case_identity.clone()),
                                        Segment::Field(field_identity.clone()),
                                    ]);
                                    payload.path.extend_from_slice(path);
                                    Some(&payload)
                                }
                                Transfer::ProjectedValue { subject }
                                | Transfer::ProjectedLoan { subject, .. }
                                | Transfer::ViewSubslice(subject) => Some(subject),
                                _ => None,
                            };
                            if let Some(subject) = subject {
                                collector.argument(
                                    Some(successor.statement_ordinal),
                                    subject,
                                    &selected,
                                )?;
                            }
                        }
                    }
                }
                let mut arguments = Vec::new();
                let values = &checked.facts.values.structural_values;
                let value_roots = reachable_structural_values(checked, machine)?;
                let computations = &checked.facts.values.scalar_computations;
                for (_, root) in computations
                    .roots
                    .iter()
                    .filter(|(_, root)| root.machine == machine && root.state == state)
                {
                    for handle in reachable_nodes(checked, &[root.root])? {
                        use checked_trees::{
                            CheckedScalarComputationKind as Computation,
                            CheckedScalarComputationStructuralArgument as Argument,
                        };
                        match &computations.nodes.get(handle).kind {
                            Computation::Call {
                                structural_arguments,
                                ..
                            } => {
                                for argument in computations
                                    .structural_arguments
                                    .span(*structural_arguments)
                                    .ok_or(LoweringError::Unsupported(
                                        "operation type demand lost its computed argument roster",
                                    ))?
                                {
                                    if let Argument::Place(subject) = argument {
                                        arguments.push((Some(root.statement_ordinal), subject));
                                    }
                                }
                            }
                            Computation::StructuralField {
                                subject,
                                source_type,
                                ..
                            } => {
                                if subject.path.contains(&Segment::Referent) {
                                    let identity =
                                        stored_structural_identity(checked, *source_type);
                                    collector.roots.push(identity);
                                    collect_path(
                                        collector.checked,
                                        collector.plans,
                                        identity,
                                        &subject.path,
                                        collector.roots,
                                    )
                                    .map_err(|error| {
                                        collector.at_statement(Some(root.statement_ordinal), error)
                                    })?;
                                }
                            }
                            Computation::ByteSequenceLiteralComparison { subject, .. }
                            | Computation::ByteSequencePredicate { subject, .. }
                            | Computation::CaseMembership {
                                subject: Argument::Place(subject),
                                ..
                            } => {
                                arguments.push((Some(root.statement_ordinal), subject));
                            }
                            _ => {}
                        }
                    }
                }
                for root in value_roots {
                    let value = values.nodes.get(root);
                    if value.state == state && value.machine == machine {
                        let mut prefix = namespace
                            .iter()
                            .copied()
                            .filter(|operation| {
                                operation
                                    .source_statement_index()
                                    .is_some_and(|statement| statement < value.statement_index)
                            })
                            .collect::<Vec<_>>();
                        collector.value(root, &prefix)?;
                        if let Some((_, calls)) = values.call_schedule(root) {
                            collector.calls(calls, &mut prefix)?;
                        }
                    }
                }
                for (statement, argument) in arguments {
                    collector.argument(statement, argument, &namespace)?;
                }
                Ok(())
            }

            struct StorageDemand<'checked, 'plans, 'output> {
                checked: &'checked CheckedTrees,
                plans: UnitPlans<'plans>,
                machine: symbols::SymbolHandle,
                state: symbols::SymbolHandle,
                parameters: &'checked [checked_trees::CheckedUnitStructuralParameterPlan],
                place_loans: &'checked [checked_trees::CheckedPlaceLoanPlan],
                roots: &'output mut Vec<TypeIdentityId>,
            }

            impl<'checked> StorageDemand<'checked, '_, '_> {
                fn at_statement(
                    &self,
                    statement: Option<u32>,
                    error: LoweringError,
                ) -> LoweringError {
                    match (statement, error) {
                        (Some(statement), LoweringError::Unsupported(reason)) => {
                            LoweringError::at_statement(
                                self.checked,
                                self.machine,
                                self.state,
                                statement,
                                reason,
                            )
                        }
                        (Some(statement), LoweringError::MissingUnitStructuralType(reason)) => {
                            LoweringError::MissingUnitStructuralType(format!(
                                "{reason}; machine `{}`, state `{}`, statement {statement}",
                                self.checked.symbols.display_path(self.machine, "::"),
                                self.checked.symbols.display_path(self.state, "::"),
                            ))
                        }
                        (_, error) => error,
                    }
                }

                fn operations(
                    &mut self,
                    operations: impl Iterator<Item = &'checked CheckedUnitEffectOperationPlan>,
                    namespace: &mut Vec<&'checked CheckedUnitEffectOperationPlan>,
                ) -> Result<(), LoweringError> {
                    for operation in operations {
                        if let Some((value, calls)) = operation.structural_value_evaluation() {
                            self.value(value, namespace)?;
                            self.calls(calls, &mut namespace.clone())?;
                        }
                        let subject = match operation {
                            CheckedUnitEffectOperationPlan::CaptureAssignmentDestination {
                                destination,
                                ..
                            } => Some(destination),
                            CheckedUnitEffectOperationPlan::MoveStructuralField {
                                source, ..
                            } => Some(source),
                            CheckedUnitEffectOperationPlan::EstablishViewSubslice {
                                source,
                                ..
                            } => Some(source),
                            _ => operation.established_reference_source(
                                &self.checked.facts.values.structural_values,
                            ),
                        };
                        for argument in subject.into_iter().chain(operation.structural_operands()) {
                            self.argument(operation.source_statement_index(), argument, namespace)?;
                        }
                        if let CheckedUnitEffectOperationPlan::Store(store) = operation {
                            match &store.value {
                                checked_trees::CheckedStoreValue::Place(source) => {
                                    self.argument(Some(store.statement_index), source, namespace)?;
                                }
                                checked_trees::CheckedStoreValue::Bytes(value) => {
                                    for (_, node) in value.nodes.iter() {
                                        if let checked_trees::CheckedByteSequenceValue::Descriptor {
                                    source,
                                    ..
                                } = node
                                {
                                    self.argument(Some(store.statement_index), source, namespace)?;
                                }
                                    }
                                }
                                _ => {}
                            }
                        }
                        namespace.push(operation);
                    }
                    Ok(())
                }

                fn calls(
                    &mut self,
                    calls: &'checked [checked_trees::CheckedStructuralValueCall],
                    namespace: &mut Vec<&'checked CheckedUnitEffectOperationPlan>,
                ) -> Result<(), LoweringError> {
                    for call in calls {
                        self.operations(call.operands().iter().chain(call.operation()), namespace)?;
                    }
                    Ok(())
                }

                fn value(
                    &mut self,
                    root: checked_trees::CheckedStructuralValueHandle,
                    namespace: &[&CheckedUnitEffectOperationPlan],
                ) -> Result<(), LoweringError> {
                    let values = &self.checked.facts.values.structural_values;
                    for handle in
                        values
                            .value_dependencies(root)
                            .ok_or(LoweringError::Unsupported(
                                "operation type demand lost its structural value dependencies",
                            ))?
                    {
                        let value = values.nodes.get(handle);
                        use checked_trees::CheckedStructuralValueKind as Value;
                        if let Value::Projection { source, path, .. } = &value.kind
                            && path.contains(&Segment::Referent)
                        {
                            let source = values.nodes.get(*source);
                            if let Value::CopiedStructuralPlace { source: copied, .. } =
                                &source.kind
                            {
                                let mut selected = copied.clone();
                                selected.path.extend_from_slice(path);
                                self.argument(Some(value.statement_index), &selected, namespace)?;
                            } else {
                                let identity =
                                    stored_structural_identity(self.checked, source.type_reference);
                                self.roots.push(identity);
                                collect_path(self.checked, self.plans, identity, path, self.roots)
                                    .map_err(|error| {
                                        self.at_statement(Some(value.statement_index), error)
                                    })?;
                            }
                        }
                        let argument = match &value.kind {
                            Value::Reference { source, .. }
                            | Value::Place(source)
                            | Value::CopiedStructuralPlace { source, .. }
                            | Value::BorrowedSliceView { source, .. }
                            | Value::MutableSliceView { source, .. } => Some(source),
                            _ => None,
                        };
                        if let Some(argument) = argument {
                            self.argument(Some(value.statement_index), argument, namespace)?;
                        }
                    }
                    Ok(())
                }

                fn argument(
                    &mut self,
                    statement: Option<u32>,
                    argument: &CheckedUnitStructuralArgumentPlan,
                    namespace: &[&CheckedUnitEffectOperationPlan],
                ) -> Result<(), LoweringError> {
                    if !argument.path.contains(&Segment::Referent) {
                        return Ok(());
                    }
                    let mut presented = argument.clone();
                    if let Source::StructuralLocal { symbol } = argument.source
                        && let Some(alias) =
                            self.place_loans.iter().find(|loan| loan.symbol == symbol)
                    {
                        crate::expression_preparation::bindings::structural_paths::retain_place_loan_presentation(
                            self.checked,
                            alias,
                            &mut presented,
                        );
                    }
                    let argument = &presented;
                    let root = source_type(
                        self.checked,
                        self.state,
                        statement,
                        self.parameters,
                        namespace,
                        argument,
                    )
                    .map_err(|error| self.at_statement(statement, error))?;
                    let mut path = argument.path.as_slice();
                    if let Source::StructuralLocalBorrow {
                        root_reference_hops,
                        ..
                    } = argument.source
                    {
                        path = path.get(root_reference_hops as usize..).unwrap_or_default();
                    }
                    self.roots.push(root);
                    collect_path(self.checked, self.plans, root, path, self.roots)
                        .map_err(|error| self.at_statement(statement, error))
                }
            }

            fn source_type(
                checked: &CheckedTrees,
                state: symbols::SymbolHandle,
                statement: Option<u32>,
                parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
                operations: &[&CheckedUnitEffectOperationPlan],
                argument: &CheckedUnitStructuralArgumentPlan,
            ) -> Result<TypeIdentityId, LoweringError> {
                let identity = match argument.source {
                    Source::Parameter { parameter_index } => parameters
                        .get(parameter_index as usize)
                        .map(|parameter| parameter.type_identity),
                    Source::StructuralLocalBorrow {
                        root_type_identity, ..
                    } => Some(root_type_identity),
                    Source::StructuralLocal { symbol } => checked
                        .state_by_symbol(state)
                        .and_then(|state| {
                            let formals = checked.state_parameters(state);
                            parameters
                                .iter()
                                .find(|parameter| {
                                    formals
                                        .get(parameter.position as usize)
                                        .is_some_and(|formal| formal.symbol == symbol)
                                })
                                .map(|parameter| parameter.type_identity)
                        })
                        .or_else(|| {
                            operations.iter().find_map(|operation| {
                                operation
                                    .statement_structural_result_binding()
                                    .filter(|result| result.local.symbol == symbol)
                                    .map(|result| result.type_identity)
                            })
                        }),
                    Source::StructuralResult { binding_ordinal } => {
                        operations.iter().find_map(|operation| {
                            operation
                                .structural_result_binding()
                                .filter(|result| result.binding_ordinal == binding_ordinal)
                                .map(|result| result.type_identity)
                        })
                    }
                    Source::StructuralExpression {
                        expression,
                        value_dependency: Some(value),
                        ..
                    } => value_facts(checked)
                        .structural_values
                        .root_for_value_home(value, expression)
                        .filter(|root| {
                            root.state == state && statement == Some(root.statement_ordinal)
                        })
                        .map(|root| stored_structural_identity(checked, root.type_reference)),
                    Source::StructuralExpression {
                        value_dependency: None,
                        logical_root_type_identity: Some(identity),
                        ..
                    } => Some(identity),
                    Source::StructuralExpression {
                        expression,
                        value_dependency: None,
                        logical_root_type_identity: None,
                        ..
                    } => statement
                        .and_then(|statement| {
                            value_facts(checked)
                                .structural_values
                                .completed_checked_expression_dependency(
                                    &checked.facts.executable_bindings,
                                    state,
                                    Some(statement),
                                    expression,
                                )
                        })
                        .map(|root| stored_structural_identity(checked, root.type_reference)),
                    Source::ByteSequenceSubslice { root, .. }
                    | Source::ElementViewSubslice { root, .. } => match root {
                        checked_trees::CheckedStorageRoot::Parameter { index } => parameters
                            .get(index as usize)
                            .map(|parameter| parameter.type_identity),
                        checked_trees::CheckedStorageRoot::OwnedLocal { symbol }
                        | checked_trees::CheckedStorageRoot::ViewLocal { symbol } => {
                            operations.iter().find_map(|operation| {
                                operation
                                    .statement_structural_result_binding()
                                    .filter(|result| result.local.symbol == symbol)
                                    .map(|result| result.type_identity)
                            })
                        }
                    },
                    _ => None,
                };
                identity.ok_or(LoweringError::Unsupported(
                    "operation storage path has no retained source type",
                ))
            }

            fn collect_path(
                checked: &CheckedTrees,
                plans: UnitPlans<'_>,
                mut identity: TypeIdentityId,
                path: &[Segment],
                roots: &mut Vec<TypeIdentityId>,
            ) -> Result<(), LoweringError> {
                let mut payload = None;
                for (position, segment) in path.iter().enumerate() {
                    let declaration =
                selected_declaration(plans, identity).map_err(|error| match error {
                    LoweringError::MissingUnitStructuralType(_) => {
                        LoweringError::MissingUnitStructuralType(format!(
                            "{identity:?} ({}) at storage path segment {position}: {path:?}",
                            checked
                                .type_identity_text(identity)
                                .unwrap_or_else(|| "unresolved identity".to_owned())
                        ))
                    }
                    error => error,
                })?;
                    match segment {
                        Segment::RepresentationView { type_identity, .. } => {
                            identity = *type_identity;
                            roots.push(identity);
                        }
                        Segment::FixedRange { .. } if position + 1 == path.len() => return Ok(()),
                        Segment::FixedRange { .. } | Segment::AllElements => {
                            return unsupported(
                                "operation storage path has a non-addressable segment",
                            );
                        }
                        Segment::Referent => {
                            let CheckedUnitStructuralTypeShape::Reference {
                                referent_identity, ..
                            } = declaration.shape
                            else {
                                return unsupported(
                                    "operation storage path crosses a non-reference",
                                );
                            };
                            identity = referent_identity;
                            roots.push(identity);
                        }
                        Segment::Case(member) => {
                            let (CheckedUnitStructuralTypeShape::Sum { cases }
                            | CheckedUnitStructuralTypeShape::Mixed { cases, .. }) =
                                &declaration.shape
                            else {
                                return unsupported(
                                    "operation storage path selects a case of a non-sum",
                                );
                            };
                            let mut matching = cases.iter().filter(|case| case.identity == *member);
                            payload = Some(
                                matching
                                    .next()
                                    .ok_or(LoweringError::Unsupported(
                                        "operation storage path case is absent",
                                    ))?
                                    .fields
                                    .as_slice(),
                            );
                            if matching.next().is_some() {
                                return unsupported("operation storage path case is ambiguous");
                            }
                        }
                        Segment::Field(member) => {
                            let fields = match payload.take() {
                                Some(fields) => fields,
                                None => match &declaration.shape {
                                    CheckedUnitStructuralTypeShape::Record { fields, .. }
                                    | CheckedUnitStructuralTypeShape::Mixed { fields, .. } => {
                                        fields.as_slice()
                                    }
                                    _ => {
                                        return unsupported(
                                            "operation storage path field has no record",
                                        );
                                    }
                                },
                            };
                            let mut matching =
                                fields.iter().filter(|field| field.identity == *member);
                            let field = matching.next().ok_or(LoweringError::Unsupported(
                                "operation storage path field is absent",
                            ))?;
                            if matching.next().is_some() || field.relevance.is_erased() {
                                return unsupported(
                                    "operation storage path field is ambiguous or erased",
                                );
                            }
                            if position + 1 == path.len() {
                                return Ok(());
                            }
                            identity = field.field_type.structural_type_identity().ok_or(
                                LoweringError::Unsupported(
                                    "operation storage path crosses a scalar field",
                                ),
                            )?;
                        }
                        Segment::FixedIndex(_)
                        | Segment::RuntimeIndex(_)
                        | Segment::ComputedIndex(_) => {
                            identity = match declaration.shape {
                                CheckedUnitStructuralTypeShape::FixedArray {
                                    element_type_identity,
                                    ..
                                }
                                | CheckedUnitStructuralTypeShape::BorrowedSliceView {
                                    element_type_identity,
                                    ..
                                } => element_type_identity,
                                _ => {
                                    return unsupported(
                                        "operation storage path index has no element type",
                                    );
                                }
                            };
                        }
                    }
                }
                Ok(())
            }
        }
        pub(crate) mod projection_types {
            use super::*;

            pub(crate) fn collect_parameter(
                plans: UnitPlans<'_>,
                parameter: &checked_trees::CheckedUnitStructuralParameterPlan,
                roots: &mut Vec<typed_trees::type_identity::TypeIdentityId>,
            ) -> Result<(), LoweringError> {
                roots.push(parameter.type_identity);
                collect_qualifications(
                    plans,
                    parameter.type_identity,
                    &parameter.projected_qualifications,
                    roots,
                )
            }

            pub(crate) fn collect_boundary_backing(
                plans: UnitPlans<'_>,
                mut identity: typed_trees::type_identity::TypeIdentityId,
                roots: &mut Vec<typed_trees::type_identity::TypeIdentityId>,
            ) -> Result<(), LoweringError> {
                let mut reference_shells = Vec::new();
                loop {
                    if reference_shells.contains(&identity) || reference_shells.len() >= 128 {
                        return unsupported(
                            "boundary formal has a recursive or excessive reference shell",
                        );
                    }
                    let declaration = selected_declaration(plans, identity)?;
                    let CheckedUnitStructuralTypeShape::Reference {
                        referent_identity, ..
                    } = &declaration.shape
                    else {
                        return Ok(());
                    };
                    reference_shells.push(identity);
                    identity = *referent_identity;
                    roots.push(identity);
                }
            }

            pub(crate) fn collect_qualifications(
                plans: UnitPlans<'_>,
                root: typed_trees::type_identity::TypeIdentityId,
                qualifications: &[checked_trees::CheckedStructuralPathQualification],
                roots: &mut Vec<typed_trees::type_identity::TypeIdentityId>,
            ) -> Result<(), LoweringError> {
                for qualification in qualifications {
                    collect(plans, root, &qualification.path, roots)?;
                }
                Ok(())
            }

            pub(crate) fn collect(
                plans: UnitPlans<'_>,
                mut identity: typed_trees::type_identity::TypeIdentityId,
                path: &[checked_trees::CheckedUnitStructuralPathSegment],
                roots: &mut Vec<typed_trees::type_identity::TypeIdentityId>,
            ) -> Result<(), LoweringError> {
                use checked_trees::{
                    CheckedUnitStructuralPathSegment as Step,
                    CheckedUnitStructuralTypeShape as Shape,
                };
                let mut selected_case = None;
                roots.push(identity);
                for (position, step) in path.iter().enumerate() {
                    let declaration = selected_declaration(plans, identity)?;
                    match step {
                        Step::Case(selected) => {
                            let cases = match &declaration.shape {
                                Shape::Sum { cases } | Shape::Mixed { cases, .. } => cases,
                                _ => {
                                    return unsupported(
                                        "projection case lost its declared namespace",
                                    );
                                }
                            };
                            selected_case = Some(
                                cases
                                    .iter()
                                    .find(|case| case.identity == *selected)
                                    .ok_or(LoweringError::Unsupported(
                                        "projection case declaration missing",
                                    ))?
                                    .fields
                                    .as_slice(),
                            );
                            continue;
                        }
                        Step::Field(selected) => {
                            let fields = match selected_case.take() {
                                Some(fields) => fields,
                                None => match &declaration.shape {
                                    Shape::Record { fields, .. } | Shape::Mixed { fields, .. } => {
                                        fields
                                    }
                                    _ => {
                                        return unsupported(
                                            "projection field lost its declared container",
                                        );
                                    }
                                },
                            };
                            let field = fields
                                .iter()
                                .find(|field| field.identity == *selected)
                                .ok_or(LoweringError::Unsupported(
                                    "projection field declaration missing",
                                ))?;
                            match field.field_type.structural_type_identity() {
                                Some(field_type) => identity = field_type,
                                None if position + 1 == path.len() => return Ok(()),
                                None => {
                                    return unsupported("projection crosses a nonstructural field");
                                }
                            }
                        }
                        Step::Referent => {
                            let Shape::Reference {
                                referent_identity, ..
                            } = &declaration.shape
                            else {
                                return unsupported(
                                    "projection pointer crossing lost its descriptor",
                                );
                            };
                            identity = *referent_identity;
                        }
                        Step::FixedIndex(_) | Step::AllElements => {
                            identity = match &declaration.shape {
                                Shape::FixedArray {
                                    element_type_identity,
                                    ..
                                }
                                | Shape::BorrowedSliceView {
                                    element_type_identity,
                                    ..
                                } => *element_type_identity,
                                _ => {
                                    return unsupported(
                                        "projection index lost its collection type",
                                    );
                                }
                            };
                        }
                        Step::RepresentationView { type_identity, .. } => identity = *type_identity,
                        _ => {
                            return unsupported(
                                "projection type demand has no static storage path",
                            );
                        }
                    }
                    roots.push(identity);
                }
                Ok(())
            }
        }
        pub(crate) mod scalar_fields {

            use super::{CheckedTrees, CheckedUnitStructuralTypeShape, LoweringError, UnitPlans};
            use crate::expression_preparation::qualifications::FieldType;
            use semantic_vocabulary::StructuralTypeId;
            use terminal_psi::{
                StructuralTypeDeclaration, StructuralTypeShape, TerminalIdentityTableBuilder,
            };
            use typed_trees::type_identity::TypeIdentityId;

            pub(crate) fn scalar_field_types(
                identities: &mut TerminalIdentityTableBuilder,
                checked: &CheckedTrees,
                plans: UnitPlans<'_>,
                type_ids: &[(TypeIdentityId, StructuralTypeId)],
                declarations: &[StructuralTypeDeclaration],
            ) -> Result<Vec<FieldType>, LoweringError> {
                let mut result = Vec::new();
                for (source, target) in type_ids {
                    let Some(declaration) = declarations.iter().find(|row| row.id == *target)
                    else {
                        continue;
                    };
                    let plan = super::selected_declaration(plans, *source)?;
                    let (fields, cases) = match &plan.shape {
                        CheckedUnitStructuralTypeShape::Record { fields, .. } => {
                            (fields.as_slice(), &[][..])
                        }
                        CheckedUnitStructuralTypeShape::Sum { cases } => {
                            (&[][..], cases.as_slice())
                        }
                        CheckedUnitStructuralTypeShape::Mixed { fields, cases } => {
                            (fields.as_slice(), cases.as_slice())
                        }
                        _ => continue,
                    };
                    let (emitted, emitted_cases) = match &declaration.shape {
                        StructuralTypeShape::Record { fields, .. } => (fields.as_slice(), &[][..]),
                        StructuralTypeShape::Sum { cases } => (&[][..], cases.as_slice()),
                        StructuralTypeShape::Mixed { fields, cases } => {
                            (fields.as_slice(), cases.as_slice())
                        }
                        _ => {
                            return Err(LoweringError::Unsupported(
                                "scalar field owner changed shape",
                            ));
                        }
                    };
                    for field in fields
                        .iter()
                        .chain(cases.iter().flat_map(|case| &case.fields))
                    {
                        if field.relevance.is_erased()
                            || checked
                                .primitive_type_reference(field.type_reference)
                                .is_none()
                        {
                            continue;
                        }
                        let identity = identities.intern(&field.identity.spelling());
                        let emitted = emitted
                            .iter()
                            .chain(emitted_cases.iter().flat_map(|case| &case.fields))
                            .find(|row| row.identity == identity)
                            .ok_or(LoweringError::Unsupported(
                                "scalar field lost its exact declaration",
                            ))?;
                        if emitted.field_type.scalar_type().is_some() {
                            result.push((emitted.id, field.type_reference));
                        }
                    }
                }
                Ok(result)
            }
        }
        pub(crate) use operation_types::collect_operation_type_roots;
        pub(crate) use scalar_fields::scalar_field_types;

        pub(crate) fn collect_reference_contract_types(
            checked: &CheckedTrees,
            contract: &checked_trees::CheckedReferenceSourceContract,
            roots: &mut Vec<typed_trees::type_identity::TypeIdentityId>,
        ) {
            if let checked_trees::CheckedReferenceSourceContract::Standing { contract, .. } =
                contract
            {
                roots.extend(
                    contract
                        .domains
                        .iter()
                        .map(|(_, reference)| reference)
                        .chain(&contract.projection_types)
                        .map(|reference| stored_structural_identity(checked, *reference)),
                );
            }
        }

        pub(crate) fn collect_value_roots(
            checked: &CheckedTrees,
            value: checked_trees::CheckedStructuralValueHandle,
            roots: &mut Vec<typed_trees::type_identity::TypeIdentityId>,
        ) -> Result<(), LoweringError> {
            let node = checked.facts.values.structural_values.nodes.get(value);
            roots.push(stored_structural_identity(checked, node.type_reference));
            match &node.kind {
                checked_trees::CheckedStructuralValueKind::Reference {
                    source,
                    source_value,
                    source_contract,
                    ..
                } => {
                    roots.push(source.type_identity);
                    collect_reference_contract_types(checked, source_contract, roots);
                    if source_value.is_valid() {
                        roots.push(validation::stored_structural_type_identity(
                            &checked.typed,
                            value_facts(checked)
                                .structural_values
                                .nodes
                                .get(*source_value)
                                .type_reference,
                            &[],
                            &[],
                        ));
                        collect_value_roots(checked, *source_value, roots)?;
                    }
                }
                checked_trees::CheckedStructuralValueKind::ByteLiteralView { .. } => {
                    roots.push(validation::structural_type_identity(
                        &checked.typed,
                        node.type_reference,
                        &[],
                        &[],
                    ));
                }
                checked_trees::CheckedStructuralValueKind::ViewSubslice { source, .. } => {
                    let child = checked.facts.values.structural_values.nodes.get(*source);
                    roots.push(stored_structural_identity(checked, child.type_reference));
                    collect_value_roots(checked, *source, roots)?;
                }
                checked_trees::CheckedStructuralValueKind::Place(source)
                | checked_trees::CheckedStructuralValueKind::BorrowedSliceView { source, .. }
                | checked_trees::CheckedStructuralValueKind::MutableSliceView { source, .. } => {
                    roots.push(source.type_identity);
                    roots.extend(source.subslice_root_type_identity());
                }
                checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace {
                    source, ..
                } => {
                    roots.push(source.type_identity);
                }
                checked_trees::CheckedStructuralValueKind::Projection {
                    source,
                    type_identity,
                    ..
                } => {
                    roots.push(*type_identity);
                    let source_node = checked.facts.values.structural_values.nodes.get(*source);
                    roots.push(stored_structural_identity(
                        checked,
                        source_node.type_reference,
                    ));
                    collect_value_roots(checked, *source, roots)?;
                }
                checked_trees::CheckedStructuralValueKind::Record { fields, .. }
                | checked_trees::CheckedStructuralValueKind::StructuralCase { fields, .. } => {
                    for field in value_facts(checked)
                        .structural_values
                        .record_fields
                        .span_or_empty(*fields)
                    {
                        let (_, declaration) = checked.any_field_owner(field.field).ok_or(
                            LoweringError::Unsupported(
                                "structural initializer field declaration missing",
                            ),
                        )?;
                        if declaration.relevance.is_erased() {
                            continue;
                        }
                        if let checked_trees::CheckedStructuralRecordFieldValue::Structural(
                            nested,
                        ) = field.value
                        {
                            if matches!(
                                value_facts(checked)
                                    .structural_values
                                    .nodes
                                    .get(nested)
                                    .kind,
                                checked_trees::CheckedStructuralValueKind::BorrowedSliceView { .. }
                                    | checked_trees::CheckedStructuralValueKind::MutableSliceView { .. }
                            ) {
                                roots.push(stored_structural_identity(
                                    checked,
                                    field.type_reference,
                                ));
                            }
                            collect_value_roots(checked, nested, roots)?;
                        }
                    }
                }
                checked_trees::CheckedStructuralValueKind::FixedArray { elements } => {
                    for element in elements {
                        collect_value_roots(checked, *element, roots)?;
                    }
                }
                checked_trees::CheckedStructuralValueKind::Dispatch { subject, arms } => {
                    let captured = subject.structural_value();
                    if captured.is_valid() {
                        let source = checked.facts.values.structural_values.nodes.get(captured);
                        roots.push(stored_structural_identity(checked, source.type_reference));
                        collect_value_roots(checked, captured, roots)?;
                    }
                    for arm in value_facts(checked)
                        .structural_values
                        .dispatch_arms
                        .span_or_empty(*arms)
                    {
                        collect_value_roots(checked, arm.value, roots)?;
                    }
                }
                _ => {}
            }
            Ok(())
        }

        fn establishes_byte_element_view(
            checked: &CheckedTrees,
            plans: UnitPlans<'_>,
            result_identity: typed_trees::type_identity::TypeIdentityId,
            value: checked_trees::CheckedStructuralValueHandle,
        ) -> Result<bool, LoweringError> {
            fn establishes_element_view(
                checked: &CheckedTrees,
                value: checked_trees::CheckedStructuralValueHandle,
            ) -> bool {
                let values = &checked.facts.values.structural_values;
                match &values.nodes.get(value).kind {
                    checked_trees::CheckedStructuralValueKind::BorrowedSliceView { .. }
                    | checked_trees::CheckedStructuralValueKind::MutableSliceView { .. } => true,
                    checked_trees::CheckedStructuralValueKind::Dispatch { arms, .. } => values
                        .dispatch_arms
                        .span_or_empty(*arms)
                        .iter()
                        .any(|arm| establishes_element_view(checked, arm.value)),
                    _ => false,
                }
            }
            if !establishes_element_view(checked, value) {
                return Ok(false);
            }
            Ok(matches!(
                selected_declaration(plans, result_identity)?.shape,
                CheckedUnitStructuralTypeShape::ByteSequence(
                    checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. }
                )
            ))
        }

        pub(crate) fn lower_program_local_root_introductions(
            checked: &CheckedTrees,
            plan: &CheckedBoundaryMachinePlan,
            requirement_identity: BoundaryIdentity,
            parameters: &[StructuralParameterDeclaration],
            domain_ids: &[StructuralDomainBinding],
        ) -> Result<Vec<ProgramLocalRootIntroductionSchema>, LoweringError> {
            let mut output = Vec::new();
            for requirement in &plan.domain_requirements {
                let Some(parameter) = plan
                    .structural_parameters
                    .get(requirement.argument_index as usize)
                else {
                    return unsupported("program-local root route parameter is out of range");
                };
                let Some(projection) = checked
                    .facts
                    .qualifications
                    .content
                    .for_semantic_domain(requirement.domain)
                else {
                    continue;
                };
                let Some(domain) = checked.domain_definition(projection.domain) else {
                    continue;
                };
                let authorizes_requirement = domain.establishment_routes.iter().any(|route| {
                    matches!(
                        route,
                        language_semantics::DomainEstablishmentRoute::BoundaryRequirement {
                            requirement,
                            ..
                        } if *requirement == plan.machine
                    )
                });
                if !authorizes_requirement {
                    continue;
                }
                if projection.report_fingerprint == 0 {
                    return unsupported("program-local root projection has a null identity");
                }
                let carrier = parameters
                    .get(requirement.argument_index as usize)
                    .ok_or(LoweringError::Unsupported(
                        "program-local root terminal parameter is out of range",
                    ))?
                    .structural_type;
                let qualification =
                    lookup_domain_id_for_carrier(domain_ids, requirement.domain, carrier)?;
                let qualification_identity = flow_facts(checked)
                    .terminal_unit_effects
                    .structural_domains
                    .iter()
                    .find(|candidate| {
                        candidate.domain == requirement.domain
                            && candidate.carrier_type_identity == parameter.type_identity
                    })
                    .map(|candidate| {
                        structural_domain_identity(
                            &candidate.name,
                            &checked
                                .type_identity_text(candidate.carrier_type_identity)
                                .unwrap_or_default(),
                        )
                    })
                    .ok_or(LoweringError::Unsupported(
                        "program-local root qualification has no normalized identity",
                    ))?;
                let owner_projection = content_conservation::lower_structural_content_projection(
                    checked,
                    requirement.domain,
                    &checked
                        .type_identity_text(parameter.type_identity)
                        .unwrap_or_default(),
                )?
                .ok_or(LoweringError::Unsupported(
                    "program-local root route has no owner content projection",
                ))?;
                let mut schema = ProgramLocalRootIntroductionSchema {
                    argument_index: requirement.argument_index,
                    source_parameter_position: parameter.position,
                    qualification,
                    carrier,
                    projection: owner_projection.identity,
                    algebra: owner_projection.algebra,
                    capacity: owner_projection.expression,
                    compatibility_report_identity: 0,
                };
                schema.compatibility_report_identity =
                    program_local_root_introduction_compatibility_report_identity(
                        &requirement_identity.render(checked)?,
                        &qualification_identity,
                        &checked
                            .type_identity_text(parameter.type_identity)
                            .unwrap_or_default(),
                        &schema,
                    );
                output.push(schema);
            }
            output.sort_by_key(|schema| (schema.argument_index, schema.qualification));
            if output.windows(2).any(|pair| {
                (pair[0].argument_index, pair[0].qualification)
                    == (pair[1].argument_index, pair[1].qualification)
            }) {
                return unsupported("program-local root introduction schema is duplicated");
            }
            Ok(output)
        }

        pub(crate) fn lower_unit_structural_types<'checked>(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            scalar_applications: &super::scalar_applications::SourceScalarApplications,
            plans: UnitPlans<'checked>,
            closure: &[symbols::SymbolHandle],
            boundaries: &[(&CheckedBoundaryMachinePlan, BoundaryIdentity)],
            additional_roots: &[typed_trees::type_identity::TypeIdentityId],
        ) -> Result<
            (
                Vec<StructuralTypeDeclaration>,
                Vec<(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)>,
                Vec<StructuralReferentIdentity>,
                Vec<super::native_layouts::NativeRecordProjection<'checked>>,
            ),
            LoweringError,
        > {
            let mut roots = additional_roots.to_vec();
            for signature in plans.calling_signatures() {
                if let Ok(storage) = &signature.storage {
                    roots.extend(storage.type_roots());
                }
            }
            let mut literal_view = false;
            let sources = plans.source_machines(checked, closure)?;
            for symbol in &sources {
                let operands = type_requirements(checked, *symbol)?;
                roots.extend(operands.identities);
                literal_view |= operands.literal_view;
            }
            for symbol in closure {
                let body = UnitBody::find(plans, *symbol)?;
                place_types::collect(checked, plans, body, &mut roots)?;
                if let checked_trees::CheckedControlResultPlan::Structural(result) =
                    body.result()?
                {
                    roots.push(result.type_identity);
                }
                for loan in body.projected_loans() {
                    if let checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan {
                        carrier_type_identity,
                        source_contract,
                        ..
                    } = loan
                    {
                        if carrier_type_identity.is_valid() {
                            roots.push(*carrier_type_identity);
                        }
                        collect_reference_contract_types(checked, source_contract, &mut roots);
                    }
                }
                for parameter in body.structural_parameters() {
                    projection_types::collect_parameter(plans, parameter, &mut roots)?;
                }
                if body.scalar_result_type().is_none()
                    && let checked_trees::CheckedControlResultPlan::Structural(result) =
                        body.result()?
                {
                    let (_, state) = state_declaration(checked, body.entry()?.state)?;
                    let qualifications = super::parameters::result_projected_qualifications(
                        checked,
                        *symbol,
                        state.return_type,
                    )?;
                    projection_types::collect_qualifications(
                        plans,
                        result.type_identity,
                        &qualifications,
                        &mut roots,
                    )?;
                }
                roots.extend(
                    body.parameter_storage()
                        .map(|storage| storage.type_identity),
                );
                for local in match body {
                    UnitBody::Ordinary(machine) => machine.trivial_affine_locals.as_slice(),
                    UnitBody::Composed(_) => &[],
                } {
                    roots.push(local.type_identity);
                    if let Some(construction) = &local.construction {
                        roots.push(construction.root_type_identity);
                    }
                }
                let mut value_operations = Vec::new();
                for source in plans.source_machines(checked, &[*symbol])? {
                    value_operations.extend(value_call_operations(checked, source, None)?);
                }
                for operation in body.operations().chain(value_operations) {
                    collect_operation_type_roots(checked, plans, operation, &mut roots)?;
                    match operation {
                        CheckedUnitEffectOperationPlan::Call(call)
                            if let (
                                checked_trees::CheckedUnitCallRoute::Module { .. },
                                checked_trees::CheckedUnitCallResult::Structural { .. },
                            ) = (call.route(), call.result())
                                && UnitBody::contains(plans, call.target.machine) =>
                        {
                            let target = UnitBody::find(plans, call.target.machine)?;
                            let checked_trees::CheckedControlResultPlan::Structural(signature) =
                                target.result()?
                            else {
                                return unsupported(
                                    "structural graph call has no structural result catalog",
                                );
                            };
                            target.entry()?;
                            roots.push(signature.type_identity);
                            roots.extend(
                                target
                                    .structural_parameters()
                                    .map(|parameter| parameter.type_identity),
                            );
                        }
                        CheckedUnitEffectOperationPlan::Call(call)
                            if let Some(result) = call.structural_result()
                                && call.is_boundary() =>
                        {
                            roots.push(result.type_identity);
                        }
                        _ => {}
                    }
                }
            }
            for slot in &checked.facts.flow.terminal_unit_effects.dynamic_slots {
                roots.extend(slot.result.structural_identity());
                for parameter in &slot.structural_parameters {
                    projection_types::collect_parameter(plans, parameter, &mut roots)?;
                }
                if let CheckedBoundaryMachineResultPlan::Structural {
                    type_identity,
                    projected_qualifications,
                    ..
                } = &slot.result
                {
                    projection_types::collect_qualifications(
                        plans,
                        *type_identity,
                        projected_qualifications,
                        &mut roots,
                    )?;
                }
            }
            for (boundary, _) in boundaries {
                for (_, operation) in checked.typed.access_lifecycle_plans.iter() {
                    if matches!(operation, typed_trees::typed_trees::AccessLifecyclePlan::Evaluated { callable, .. }
                        if *callable == boundary.machine || *callable == boundary.state)
                    {
                        for definition in operation
                            .storage_definitions(&checked.typed)
                            .map_err(LoweringError::Unsupported)?
                        {
                            let identity = match definition.generic_instance {
                                Some(application) => {
                                    checked.typed.normalized_type_identity(application)
                                }
                                None => checked
                                    .typed
                                    .normalized_nominal_type_identity(definition.symbol)
                                    .ok_or(LoweringError::Unsupported(
                                        "placement storage lost its nominal type identity",
                                    ))?,
                            };
                            roots.push(identity);
                        }
                    }
                }
                for parameter in &boundary.structural_parameters {
                    projection_types::collect_parameter(plans, parameter, &mut roots)?;
                    projection_types::collect_boundary_backing(
                        plans,
                        parameter.type_identity,
                        &mut roots,
                    )?;
                }
                roots.extend(boundary.result.structural_identity());
                if let CheckedBoundaryMachineResultPlan::Structural {
                    type_identity,
                    projected_qualifications,
                    ..
                } = &boundary.result
                {
                    projection_types::collect_qualifications(
                        plans,
                        *type_identity,
                        projected_qualifications,
                        &mut roots,
                    )?;
                }
            }
            let (mut declarations, mut identity_ids, referents, native_records) =
                lower_unit_structural_type_roots(
                    identities,
                    checked,
                    scalar_applications,
                    plans,
                    &roots,
                )?;
            if literal_view {
                literal_view_type(identities, &mut declarations)?;
            }
            let scalar = terminal_scalar_type(checked_trees::types::PrimitiveType::U64)?;
            if !declarations.iter().any(|declaration| {
                declaration.shape == StructuralTypeShape::PrimitiveScalar(scalar)
            }) {
                let mut needs_coordinate = false;
                for machine in closure {
                    let states = match UnitBody::find(plans, *machine)? {
                        UnitBody::Ordinary(body) => vec![body.state],
                        UnitBody::Composed(body) => {
                            body.states.iter().map(|state| state.state).collect()
                        }
                    };
                    for state in states {
                        let loans =
                            crate::scalar_lowering::computations::fields::prepare_place_observations(
                                identities,
                                checked,
                                state_declaration(checked, state)?.0.symbol,
                                state,
                                &declarations,
                            );
                        needs_coordinate |=
                            loans.iter().any(|loan| {
                                loan.loan.path.iter().any(|segment| {
                                    matches!(
                                    segment,
                                    checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(_)
                                )
                                })
                            });
                    }
                }
                if needs_coordinate {
                    let identity = "named(name(u64))";
                    let identity_id = checked.type_identity_by_text(identity);
                    if identity_ids
                        .iter()
                        .any(|(existing, _)| *existing == identity_id)
                    {
                        return unsupported(
                            "record coordinate carrier conflicts with its catalog identity",
                        );
                    }
                    let id = structural_type_id(dense_identity(identity_ids.len())?);
                    identity_ids.push((identity_id, id));
                    declarations.push(StructuralTypeDeclaration {
                        requires: Vec::new(),
                        quotient_carrier: None,
                        owns_linear_claim: false,
                        no_code_affine_cleanup: false,
                        copyable: false,
                        descriptor_cleanup: false,
                        scalar_restriction: None,
                        id,
                        identity: identities.intern(identity),
                        shape: StructuralTypeShape::PrimitiveScalar(scalar),
                    });
                }
            }
            Ok((declarations, identity_ids, referents, native_records))
        }

        fn select_from<'a>(
            roster: impl Iterator<Item = &'a CheckedUnitStructuralTypePlan>,
            identity: typed_trees::type_identity::TypeIdentityId,
            selected: &mut Option<&'a CheckedUnitStructuralTypePlan>,
        ) -> Result<(), LoweringError> {
            let mut matches = roster.filter(|plan| plan.identity == identity);
            let Some(plan) = matches.next() else {
                return Ok(());
            };
            if matches.next().is_some() {
                return unsupported(
                    "Unit closure contains a duplicate or empty structural type identity",
                );
            }
            if selected.is_some_and(|previous| previous != plan) {
                return unsupported(
                    "Unit closure structural type declarations conflict across owners",
                );
            }
            *selected = Some(plan);
            Ok(())
        }

        pub(crate) fn selected_declaration<'a>(
            plans: UnitPlans<'a>,
            identity: typed_trees::type_identity::TypeIdentityId,
        ) -> Result<&'a CheckedUnitStructuralTypePlan, LoweringError> {
            if !identity.is_valid() {
                return unsupported(
                    "Unit closure contains a duplicate or empty structural type identity",
                );
            }
            let mut selected = None;
            select_from(plans.structural_types(), identity, &mut selected)?;
            selected
                .ok_or_else(|| LoweringError::MissingUnitStructuralType(format!("{identity:?}")))
        }

        pub(crate) fn lower_unit_structural_type_roots<'checked>(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            scalar_applications: &super::scalar_applications::SourceScalarApplications,
            plans: UnitPlans<'checked>,
            roots: &[typed_trees::type_identity::TypeIdentityId],
        ) -> Result<
            (
                Vec<StructuralTypeDeclaration>,
                Vec<(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)>,
                Vec<StructuralReferentIdentity>,
                Vec<super::native_layouts::NativeRecordProjection<'checked>>,
            ),
            LoweringError,
        > {
            fn collect<'a>(
                checked: &CheckedTrees,
                plans: UnitPlans<'a>,
                identity: typed_trees::type_identity::TypeIdentityId,
                active: &mut Vec<typed_trees::type_identity::TypeIdentityId>,
                selected: &mut Vec<&'a CheckedUnitStructuralTypePlan>,
            ) -> Result<(), LoweringError> {
                if active.contains(&identity) {
                    return unsupported(
                        "recursive structural type is outside the Unit terminal slice",
                    );
                }
                if selected
                    .iter()
                    .any(|candidate| candidate.identity == identity)
                {
                    return Ok(());
                }
                let plan = selected_declaration(plans, identity).map_err(|error| match error {
                    LoweringError::MissingUnitStructuralType(_) => {
                        LoweringError::MissingUnitStructuralType(format!(
                            "{identity:?} ({})",
                            checked
                                .type_identity_text(identity)
                                .unwrap_or_else(|| "unresolved identity".to_owned())
                        ))
                    }
                    error => error,
                })?;
                active.push(identity);
                if plan.descriptor_presentation.is_valid() {
                    collect(
                        checked,
                        plans,
                        plan.descriptor_presentation,
                        active,
                        selected,
                    )?;
                }
                if let Some(representation) = plans
                    .opaque_representations()
                    .iter()
                    .find(|row| row.opaque == identity)
                {
                    collect(checked, plans, representation.carrier, active, selected)?;
                }
                match &plan.shape {
                    CheckedUnitStructuralTypeShape::Reference { .. } => {}
                    CheckedUnitStructuralTypeShape::ByteSequence(_) => collect(
                        checked,
                        plans,
                        checked.type_identity_by_text(BYTE_VIEW_ELEMENT_TYPE_IDENTITY),
                        active,
                        selected,
                    )?,
                    CheckedUnitStructuralTypeShape::PrimitiveScalar(_)
                    | CheckedUnitStructuralTypeShape::AtomicCell(_) => {}
                    CheckedUnitStructuralTypeShape::BorrowedSliceView {
                        element_type_identity,
                        ..
                    } => {
                        collect(checked, plans, *element_type_identity, active, selected)?;
                    }
                    CheckedUnitStructuralTypeShape::Record { fields, storage } => {
                        if let checked_trees::CheckedUnitRecordStorage::TargetLayout {
                            schema_type_identity,
                            ..
                        }
                        | checked_trees::CheckedUnitRecordStorage::AccessView {
                            schema_type_identity,
                            ..
                        } = storage
                        {
                            collect(checked, plans, *schema_type_identity, active, selected)?;
                        }
                        for type_identity in fields.iter().flat_map(
                            checked_trees::CheckedUnitStructuralFieldPlan::retained_type_identities,
                        ) {
                            collect(checked, plans, type_identity, active, selected)?;
                        }
                    }
                    CheckedUnitStructuralTypeShape::FixedArray {
                        element_type_identity,
                        ..
                    } => {
                        collect(checked, plans, *element_type_identity, active, selected)?;
                    }
                    CheckedUnitStructuralTypeShape::Sum { cases } => {
                        for type_identity in cases.iter().flat_map(|case| &case.fields).flat_map(
                            checked_trees::CheckedUnitStructuralFieldPlan::retained_type_identities,
                        ) {
                            collect(checked, plans, type_identity, active, selected)?;
                        }
                    }
                    CheckedUnitStructuralTypeShape::Mixed { fields, cases } => {
                        for type_identity in fields
                            .iter()
                            .chain(cases.iter().flat_map(|case| &case.fields))
                            .flat_map(
                                checked_trees::CheckedUnitStructuralFieldPlan::retained_type_identities,
                            )
                        {
                            collect(checked, plans, type_identity, active, selected)?;
                        }
                    }
                }
                active.pop();
                selected.push(plan);
                Ok(())
            }

            let mut selected = Vec::new();
            let mut active = Vec::new();
            for identity in roots {
                collect(checked, plans, *identity, &mut active, &mut selected)?;
            }
            let identity_text = |identity: typed_trees::type_identity::TypeIdentityId| {
                checked.type_identity_text(identity).unwrap_or_default()
            };
            selected.sort_by_key(|left| identity_text(left.identity));
            let mut referent_ids = selected
                .iter()
                .filter_map(|plan| match &plan.shape {
                    CheckedUnitStructuralTypeShape::Reference {
                        referent_identity, ..
                    } if !selected
                        .iter()
                        .any(|definition| definition.identity == *referent_identity) =>
                    {
                        Some(*referent_identity)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            referent_ids.sort_by_key(|left| identity_text(*left));
            referent_ids.dedup();
            let type_ids = referent_ids
                .iter()
                .copied()
                .chain(selected.iter().map(|plan| plan.identity))
                .enumerate()
                .map(|(index, identity)| Ok((identity, structural_type_id(dense_identity(index)?))))
                .collect::<Result<Vec<_>, LoweringError>>()?;
            let referents = referent_ids
                .into_iter()
                .map(|identity| {
                    let mut representations = selected.iter().filter_map(|plan| match &plan.shape {
                        CheckedUnitStructuralTypeShape::Reference {
                            referent_identity,
                            foreign_descriptor,
                            abstract_referent,
                            ..
                        } if *referent_identity == identity => {
                            Some((*foreign_descriptor, *abstract_referent))
                        }
                        _ => None,
                    });
                    let representation = representations.next().ok_or_else(|| {
                        LoweringError::MissingUnitStructuralType(
                            "referent has no reference transport declaration".into(),
                        )
                    })?;
                    if representations.any(|other| other != representation) {
                        return unsupported(
                            "one referent identity has conflicting foreign reference representations",
                        );
                    }
                    Ok(StructuralReferentIdentity {
                        id: lookup_type_id(&type_ids, &identity)?,
                        identity: identities.intern(&identity_text(identity)),
                        foreign_descriptor: representation.0,
                        abstract_opaque: representation.1,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            let mut next_field = 1_u64;
            let mut next_case = 1_u64;
            let mut next_layout_application = 1_u32;
            let mut declarations = Vec::with_capacity(selected.len());
            let mut native_records = Vec::new();
            for plan in selected {
                let identity = plan.identity;
                let shape = match &plan.shape {
                    CheckedUnitStructuralTypeShape::Reference {
                        referent_identity,
                        access,
                        ..
                    } => {
                        let access = match *access {
                            checked_trees::CheckedStructuralAccess::MutableBorrow => {
                                StructuralAccess::MutableBorrow
                            }
                            checked_trees::CheckedStructuralAccess::SharedBorrow => {
                                StructuralAccess::SharedBorrow
                            }
                            checked_trees::CheckedStructuralAccess::WriteOnlyBorrow => {
                                StructuralAccess::WriteOnlyBorrow
                            }
                            checked_trees::CheckedStructuralAccess::Owned => {
                                return unsupported("reference type must retain borrowed access");
                            }
                        };
                        StructuralTypeShape::Reference {
                            referent: lookup_type_id(&type_ids, referent_identity)?,
                            access,
                        }
                    }
                    CheckedUnitStructuralTypeShape::PrimitiveScalar(primitive) => {
                        StructuralTypeShape::PrimitiveScalar(terminal_scalar_type(*primitive)?)
                    }
                    CheckedUnitStructuralTypeShape::AtomicCell(cell) => {
                        StructuralTypeShape::AtomicCell(*cell)
                    }
                    CheckedUnitStructuralTypeShape::ByteSequence(carrier) => {
                        StructuralTypeShape::ByteSequence(terminal_byte_sequence_carrier(*carrier))
                    }
                    CheckedUnitStructuralTypeShape::BorrowedSliceView {
                        element_type_identity,
                        access,
                    } => StructuralTypeShape::ElementView {
                        element: lookup_type_id(&type_ids, element_type_identity)?,
                        access: access.map(terminal_access),
                    },
                    CheckedUnitStructuralTypeShape::Record { fields, storage } => {
                        let mut native_fields = Vec::with_capacity(fields.len());
                        let mut field_identities = BTreeSet::new();
                        let fields = fields.iter().map(|field| {
                        if !field.identity.is_valid() || !field_identities.insert(field.identity.spelling()) {
                            return Err(LoweringError::Unsupported(
                                "Unit structural type contains an empty or duplicate field identity",
                            ));
                        }
                        let (relevance, field_type) = (
                            field.relevance,
                            lower_field_type(identities, checked, &field.field_type, &type_ids)?,
                        );
                        let declaration = StructuralFieldDeclaration {
                            inline_carrier: crate::shared::structural_types::lower_inline_carrier(field, &type_ids)?,
                            storage: field.storage.clone(),
                            id: structural_field_id(allocate_dense(&mut next_field)?),
                            identity: identities.intern(&field.identity.spelling()),
                            relevance,
                            field_type,
                        };
                        native_fields.push(super::native_layouts::NativeFieldProjection {
                            field: declaration.id,
                            native_identity: field.native_identity.as_deref(),
                            declaration: field.source_declaration,
                            relevance: field.relevance,
                        });
                        Ok(declaration)
                        }).collect::<Result<Vec<_>, LoweringError>>()?;
                        native_records.push(super::native_layouts::NativeRecordProjection {
                            source_declaration: plan.source_declaration,
                            source_application: &plan.source_application,
                            id: lookup_type_id(&type_ids, &identity)?,
                            source_identity: identity,
                            native_identity: plan.native_identity.as_deref(),
                            policy_identity: plan.native_policy_identity.as_deref(),
                            semantic_identity: plan.native_semantic_identity.as_deref(),
                            fields: native_fields,
                        });
                        StructuralTypeShape::Record {
                            fields,
                            storage: match storage {
                                checked_trees::CheckedUnitRecordStorage::AccessView {
                                    storage,
                                    ..
                                } => terminal_psi::StructuralRecordStorage::AccessView(*storage),
                                checked_trees::CheckedUnitRecordStorage::Accessor(storage) => {
                                    terminal_psi::StructuralRecordStorage::Accessor(*storage)
                                }
                                checked_trees::CheckedUnitRecordStorage::Packed => {
                                    terminal_psi::StructuralRecordStorage::Packed
                                }
                                checked_trees::CheckedUnitRecordStorage::Authored(storage) => {
                                    terminal_psi::StructuralRecordStorage::Authored(*storage)
                                }
                                checked_trees::CheckedUnitRecordStorage::TargetLayout {
                                    ..
                                } => {
                                    let id =
                                        terminal_psi::LayoutApplicationId(next_layout_application);
                                    next_layout_application = next_layout_application
                                        .checked_add(1)
                                        .ok_or(LoweringError::Unsupported(
                                            "target layout application identity overflow",
                                        ))?;
                                    terminal_psi::StructuralRecordStorage::TargetLayout(id)
                                }
                            },
                        }
                    }
                    CheckedUnitStructuralTypeShape::FixedArray {
                        element_type_identity,
                        length,
                    } => StructuralTypeShape::FixedArray {
                        element: lookup_type_id(&type_ids, element_type_identity)?,
                        length: scalar_applications.array_extent(checked, *length)?,
                    },
                    CheckedUnitStructuralTypeShape::Sum { cases } => {
                        let mut case_identities = BTreeSet::new();
                        let cases = cases
                            .iter()
                            .map(|case| {
                                if !case.identity.is_valid()
                                    || !case_identities.insert(case.identity.spelling())
                                {
                                    return Err(LoweringError::Unsupported(
                                        "Unit structural type contains an empty or duplicate case identity",
                                    ));
                                }
                                Ok(StructuralCaseDeclaration {
                                    id: StructuralCaseId::new(allocate_dense(&mut next_case)?)
                                        .expect("allocated structural case identity is nonzero"),
                                    identity: identities.intern(&case.identity.spelling()),
                                    fields: {
                                        let mut field_identities = BTreeSet::new();
                                        case.fields
                                            .iter()
                                            .map(|field| {
                                                if !field.identity.is_valid()
                                                    || !field_identities.insert(field.identity.spelling())
                                                {
                                                    return Err(LoweringError::Unsupported(
                                                        "Unit structural sum case contains an empty or duplicate payload field identity",
                                                    ));
                                                }
                                                let field_type = lower_field_type(identities, checked, &field.field_type, &type_ids)?;
                                                Ok(StructuralFieldDeclaration {
                                                    inline_carrier: crate::shared::structural_types::lower_inline_carrier(field, &type_ids)?,
                                                    storage: field.storage.clone(),
                                                    id: structural_field_id(allocate_dense(
                                                        &mut next_field,
                                                    )?),
                                                    identity: identities.intern(&field.identity.spelling()),
                                                    relevance: field.relevance,
                                                    field_type,
                                                })
                                            })
                                            .collect::<Result<Vec<_>, LoweringError>>()?
                                    },
                                })
                            })
                            .collect::<Result<Vec<_>, LoweringError>>()?;
                        StructuralTypeShape::Sum { cases }
                    }
                    CheckedUnitStructuralTypeShape::Mixed { fields, cases } => {
                        StructuralTypeShape::Mixed {
                            fields: lower_mixed_fields(
                                identities,
                                checked,
                                fields,
                                &type_ids,
                                &mut next_field,
                            )?,
                            cases: lower_mixed_cases(
                                identities,
                                checked,
                                cases,
                                &type_ids,
                                &mut next_field,
                                &mut next_case,
                            )?,
                        }
                    }
                };
                declarations.push(StructuralTypeDeclaration {
                    requires: Vec::new(),
                    quotient_carrier: None,
                    owns_linear_claim: plan.owns_linear_claim,
                    copyable: plan.copyable,
                    no_code_affine_cleanup: plan.no_code_affine_cleanup,
                    descriptor_cleanup: plan.descriptor_cleanup,
                    scalar_restriction: plan
                        .scalar_restriction
                        .as_ref()
                        .map(|restriction| {
                            lower_field_type(identities, checked, restriction, &type_ids)
                        })
                        .transpose()?,
                    id: lookup_type_id(&type_ids, &identity)?,
                    identity: identities.intern(&identity_text(identity)),
                    shape,
                });
            }
            Ok((declarations, type_ids, referents, native_records))
        }

        pub(crate) fn lower_unit_structural_domains(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            plans: UnitPlans<'_>,
            closure: &[symbols::SymbolHandle],
            boundaries: &[(&CheckedBoundaryMachinePlan, BoundaryIdentity)],
            type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
            structural_types: &[StructuralTypeDeclaration],
        ) -> Result<
            (
                Vec<StructuralDomainDeclaration>,
                Vec<StructuralDomainBinding>,
            ),
            LoweringError,
        > {
            let mut selected = Vec::new();
            let sources = plans.source_machines(checked, closure)?;
            for (_, region) in checked.facts.flow.ownership.loan_completions.iter() {
                if !sources.contains(&region.machine) {
                    continue;
                }
                for fact in crate::emission::loan_completions::qualification_facts(checked, region)
                {
                    if declaration_represents(identities, checked, fact)?.is_complete() {
                        continue;
                    }
                    if let Some((_, _, _, domain)) = fact.payload.domain_membership()
                        && !selected.contains(&domain)
                    {
                        selected.push(domain);
                    }
                }
            }
            for symbol in closure {
                let body = UnitBody::find(plans, *symbol)?;
                for loan in body.projected_loans() {
                    if let checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan {
                        source_contract,
                        ..
                    } = loan
                    {
                        crate::expression_preparation::structural_value_requirements::collect_reference_contract(
                            source_contract, &mut selected,
                        );
                    }
                }

                for (state, operation) in body.operations_with_states() {
                    let Some(call) = operation.call() else {
                        continue;
                    };
                    let (machine, _) = state_declaration(checked, state)?;
                    let coordinate =
                        crate::emission::operation_emission::buffer::SourceCallCoordinate {
                            state,
                            statement_index: call.coordinate.statement_index as usize,
                            call_ordinal: call.coordinate.call_ordinal as usize,
                        };
                    let homes = body.structural_home_roots(checked, state)?;
                    for fact in for_call(checked, machine.symbol, coordinate, |root| {
                        homes.contains(&root)
                    }) {
                        if declaration_represents(identities, checked, fact)?.is_complete() {
                            continue;
                        }
                        if let Some((_, _, _, domain)) = fact.payload.domain_membership()
                            && !selected.contains(&domain)
                        {
                            selected.push(domain);
                        }
                    }
                }
                if !body.entry()?.returns_unit && body.scalar_result_type().is_none() {
                    let (_, state) = state_declaration(checked, body.entry()?.state)?;
                    let mut result_domains = validation::view_result_carrier_qualifications(
                        &checked.typed,
                        state.return_type,
                    )
                    .map_err(LoweringError::Unsupported)?;
                    result_domains.extend(
                        validation::structural_result_projected_qualifications(
                            &checked.typed,
                            state.return_type,
                        )
                        .map_err(LoweringError::Unsupported)?
                        .into_iter()
                        .map(|row| row.domain),
                    );
                    for domain in result_domains {
                        if !selected.contains(&domain) {
                            selected.push(domain);
                        }
                    }
                }
                for (state, operation) in body.operations_with_states() {
                    let (owner, _) = state_declaration(checked, state)?;
                    collect_operation_requirements(
                        checked,
                        owner.symbol,
                        operation,
                        &mut selected,
                    )?;
                }
                for domain in body
                    .structural_parameters()
                    .flat_map(|parameter| {
                        parameter.qualifications.iter().chain(
                            parameter
                                .projected_qualifications
                                .iter()
                                .map(|row| &row.domain),
                        )
                    })
                    .chain(body.qualifications())
                    .chain(body.operations().flat_map(|operation| match operation {
                        CheckedUnitEffectOperationPlan::EstablishLocal(
                            checked_trees::CheckedUnitLocalBindingPlan::ByteSequence {
                                qualifications,
                                ..
                            },
                        )
                        | CheckedUnitEffectOperationPlan::EstablishLocal(
                            checked_trees::CheckedUnitLocalBindingPlan::Structural {
                                qualifications,
                                ..
                            },
                        )
                        | CheckedUnitEffectOperationPlan::EstablishLocal(
                            checked_trees::CheckedUnitLocalBindingPlan::ScalarArray {
                                qualifications,
                                ..
                            },
                        ) => qualifications.as_slice(),
                        _ => &[],
                    }))
                {
                    if !selected.contains(domain) {
                        selected.push(*domain);
                    }
                }
                let received = body
                    .operations()
                    .filter_map(|operation| operation.call())
                    .flat_map(|call| {
                        super::call_results::module_result_qualification_domains(checked, call)
                    })
                    .chain(super::parameters::authored_result_qualification_domains(
                        checked, *symbol,
                    ));
                for domain in received {
                    if !selected.contains(&domain) {
                        selected.push(domain);
                    }
                }
            }
            for source in sources {
                for value in reachable_structural_values(checked, source)? {
                    collect_value_requirements(checked, source, value, &mut selected)?;
                }
            }
            for (boundary, _) in boundaries {
                let (result_domains, result_projected): (
                    &[SemanticDomainId],
                    &[checked_trees::CheckedStructuralPathQualification],
                ) = match &boundary.result {
                    CheckedBoundaryMachineResultPlan::Structural {
                        qualifications,
                        projected_qualifications,
                        ..
                    } => (qualifications, projected_qualifications),
                    CheckedBoundaryMachineResultPlan::Unit
                    | CheckedBoundaryMachineResultPlan::Scalar(_) => (&[], &[]),
                };
                for domain in boundary
                    .structural_parameters
                    .iter()
                    .flat_map(|parameter| {
                        parameter.qualifications.iter().chain(
                            parameter
                                .projected_qualifications
                                .iter()
                                .map(|row| &row.domain),
                        )
                    })
                    .chain(
                        boundary
                            .domain_requirements
                            .iter()
                            .map(|requirement| &requirement.domain),
                    )
                    .chain(result_domains)
                    .chain(result_projected.iter().map(|row| &row.domain))
                {
                    if !selected.contains(domain) {
                        selected.push(*domain);
                    }
                }
            }
            for slot in &checked.facts.flow.terminal_unit_effects.dynamic_slots {
                let mut domains = slot
                    .structural_parameters
                    .iter()
                    .flat_map(|parameter| {
                        parameter.qualifications.iter().chain(
                            parameter
                                .projected_qualifications
                                .iter()
                                .map(|row| &row.domain),
                        )
                    })
                    .copied()
                    .collect::<Vec<_>>();
                if let CheckedBoundaryMachineResultPlan::Structural {
                    qualifications,
                    projected_qualifications,
                    ..
                } = &slot.result
                {
                    domains.extend(qualifications);
                    domains.extend(projected_qualifications.iter().map(|row| row.domain));
                }
                for domain in domains {
                    if !selected.contains(&domain) {
                        selected.push(domain);
                    }
                }
            }
            let mut cursor = 0;
            while let Some(domain) = selected.get(cursor).copied() {
                cursor += 1;
                for plan in plans.structural_domains().iter().filter(|plan| {
                    plan.domain == domain
                        && type_ids
                            .iter()
                            .any(|(identity, _)| *identity == plan.carrier_type_identity)
                }) {
                    if let Some(required) = &plan.required_memberships {
                        for parent in required {
                            if !selected.contains(parent) {
                                selected.push(*parent);
                            }
                        }
                    }
                }
            }
            let mut selected_plans: Vec<&checked_trees::CheckedUnitStructuralDomainPlan> =
                Vec::new();
            for domain in selected {
                let mut found = false;
                let mut carriers = Vec::new();
                for plan in plans.structural_domains().iter().filter(|plan| {
                    plan.domain == domain
                        && type_ids
                            .iter()
                            .any(|(identity, _)| *identity == plan.carrier_type_identity)
                }) {
                    found = true;
                    if carriers.contains(&plan.carrier_type_identity) {
                        return unsupported(
                            "Unit closure has duplicate structural domain ownership",
                        );
                    }
                    carriers.push(plan.carrier_type_identity);
                    if !domain.is_valid()
                        || plan.name.is_empty()
                        || !plan.carrier_type_identity.is_valid()
                    {
                        return Err(LoweringError::Unsupported(
                            "Unit structural domain is duplicate, null, or incomplete",
                        ));
                    }
                    selected_plans.push(plan);
                }
                if !found {
                    return Err(LoweringError::MissingUnitDomainPlan {
                        domain,
                        name: checked
                            .typed
                            .semantic_domains
                            .name(domain)
                            .unwrap_or_default()
                            .to_owned(),
                    });
                }
            }
            selected_plans.sort_by(|left, right| {
                (
                    &left.name,
                    checked
                        .type_identity_text(left.carrier_type_identity)
                        .unwrap_or_default(),
                )
                    .cmp(&(
                        &right.name,
                        checked
                            .type_identity_text(right.carrier_type_identity)
                            .unwrap_or_default(),
                    ))
            });
            if selected_plans.windows(2).any(|pair| {
                pair[0].name == pair[1].name
                    && pair[0].carrier_type_identity == pair[1].carrier_type_identity
            }) {
                return unsupported("Unit structural domains have duplicate canonical identities");
            }
            let mut domain_ids = selected_plans
                .iter()
                .enumerate()
                .map(|(index, plan)| {
                    Ok(StructuralDomainBinding {
                        byte_predicate: None,
                        semantic_domain: plan.domain,
                        carrier: lookup_type_id(type_ids, &plan.carrier_type_identity)?,
                        domain: structural_domain_id(dense_identity(index)?),
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            let mut declarations = selected_plans
                .iter()
                .copied()
                .map(|plan| {
                    let content_projection =
                        content_conservation::lower_structural_content_projection(
                            checked,
                            plan.domain,
                            &checked
                                .type_identity_text(plan.carrier_type_identity)
                                .unwrap_or_default(),
                        )?;
                    let establishment_routes = lower_establishment_routes(
                        identities,
                        checked,
                        &plan.establishment_routes,
                    )?;
                    let carrier = lookup_type_id(type_ids, &plan.carrier_type_identity)?;
                    let predicate = plan
                        .predicate
                        .as_ref()
                        .map(|predicate| {
                            let subject = semantic_vocabulary::PlaceId::new(1).ok_or(
                                LoweringError::Unsupported(
                                    "domain predicate subject identity is null",
                                ),
                            )?;
                            let formal = terminal_psi::StructuralParameterDeclaration {
                                place: subject,
                                position: 0,
                                structural_type: carrier,
                                multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                                access: terminal_psi::StructuralAccess::SharedBorrow,
                                qualifications: Vec::new(),
                                projected_qualifications: Vec::new(),
                                additional_input_qualifications: Vec::new(),
                            };
                            let scalar = terminal_semantics::primitive_place_type(
                                structural_types.iter(),
                                carrier,
                                &[],
                            )
                            .map(|scalar_type| {
                                Ok(terminal_psi::ValueDeclaration {
                                    id: semantic_vocabulary::ValueId::new(1).ok_or(
                                        LoweringError::Unsupported(
                                            "domain scalar subject identity is null",
                                        ),
                                    )?,
                                    scalar_type,
                                    qualifications:
                                        semantic_vocabulary::ScalarQualificationSetId::ZERO,
                                })
                            })
                            .transpose()?;
                            let mut propositions =
                                crate::proofs::crash_routes::lower_structural_crash_predicates(
                                    identities,
                                    std::slice::from_ref(predicate),
                                    scalar.as_slice(),
                                    std::slice::from_ref(&formal),
                                    structural_types,
                                    &[],
                                )?;
                            let lowered = propositions.pop().ok_or(LoweringError::Unsupported(
                                "domain predicate lowering produced no proposition",
                            ))?;
                            let mut proposition = match predicate {
                                checked_trees::CheckedBooleanExpression::Constant(true) => {
                                    semantic_vocabulary::Proposition::Truth
                                }
                                checked_trees::CheckedBooleanExpression::Constant(false) => {
                                    semantic_vocabulary::Proposition::Falsehood
                                }
                                _ => lowered.proposition().clone(),
                            };
                            if !propositions.is_empty() {
                                return unsupported(
                                    "domain predicate lowering produced extra propositions",
                                );
                            }
                            if let Some(scalar) = scalar {
                                let observed = match scalar.scalar_type {
                                    semantic_vocabulary::ScalarType::Boolean => {
                                        semantic_vocabulary::ScalarTerm::boolean_field_path(
                                            subject,
                                            Vec::new(),
                                        )
                                    }
                                    semantic_vocabulary::ScalarType::Integer(integer) => {
                                        semantic_vocabulary::ScalarTerm::integer_field_path(
                                            subject,
                                            Vec::new(),
                                            integer,
                                        )
                                    }
                                    semantic_vocabulary::ScalarType::IeeeFloat(format) => {
                                        semantic_vocabulary::ScalarTerm::ieee_float_field_path(
                                            subject,
                                            Vec::new(),
                                            format,
                                        )
                                    }
                                };
                                proposition =
                                    terminal_semantics::substitution::substitute_proposition_values(
                                        &proposition,
                                        &std::collections::BTreeMap::from([(scalar.id, observed)]),
                                    );
                            }
                            let predicate = terminal_psi::StructuralDomainPredicate {
                                subject,
                                proposition,
                            };
                            if !terminal_semantics::domain_predicate_is_closed(
                                structural_types.iter(),
                                formal.structural_type,
                                &predicate,
                            ) {
                                return unsupported(
                                    "lowered domain predicate is not closed over its subject",
                                );
                            }
                            Ok(predicate)
                        })
                        .transpose()
                        .map_err(|cause| LoweringError::InvalidUnitDomainPredicate {
                            domain: plan.domain,
                            name: plan.name.clone(),
                            carrier,
                            cause: Box::new(cause),
                        })?;
                    Ok(StructuralDomainDeclaration {
                        required_memberships: plan
                            .required_memberships
                            .as_ref()
                            .map(|required| {
                                required
                                    .iter()
                                    .map(|domain| {
                                        DomainSemanticId::new(u64::from(domain.0))
                                            .ok_or(LoweringError::InvalidContentDomainIdentity)
                                    })
                                    .collect::<Result<Vec<_>, _>>()
                            })
                            .transpose()?,
                        complete_predicate_theory: plan.complete_predicate_theory,
                        predicate,
                        byte_predicate: plan.byte_predicate,
                        establishment_routes,
                        id: lookup_domain_id_for_carrier(&domain_ids, plan.domain, carrier)?,
                        semantic_domain: DomainSemanticId::new(u64::from(plan.domain.0))
                            .ok_or(LoweringError::InvalidContentDomainIdentity)?,
                        identity: identities.intern(&structural_domain_identity(
                            &plan.name,
                            &checked
                                .type_identity_text(plan.carrier_type_identity)
                                .unwrap_or_default(),
                        )),
                        carrier,
                        content_projection,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            for plan in selected_plans {
                let source_carrier = lookup_type_id(type_ids, &plan.carrier_type_identity)?;
                let source_domain =
                    lookup_domain_id_for_carrier(&domain_ids, plan.domain, source_carrier)?;
                let source = declarations
                    .iter()
                    .find(|declaration| declaration.id == source_domain)
                    .ok_or(LoweringError::Unsupported(
                        "domain presentation has no source theory",
                    ))?
                    .clone();
                for (identity, carrier) in type_ids {
                    if domain_ids.iter().any(|binding| {
                        binding.semantic_domain == plan.domain && binding.carrier == *carrier
                    }) || terminal_semantics::borrowed_referent_presentation(
                        structural_types.iter(),
                        source_carrier,
                        *carrier,
                    ) != Some(terminal_semantics::BorrowedReferentPresentation::Unchanged)
                    {
                        continue;
                    }
                    let id = structural_domain_id(dense_identity(declarations.len())?);
                    let mut declaration = source.clone();
                    declaration.id = id;
                    declaration.carrier = *carrier;
                    declaration.identity = identities.intern(&structural_domain_identity(
                        &plan.name,
                        &checked.type_identity_text(*identity).ok_or(
                            LoweringError::Unsupported(
                                "domain presentation has no carrier identity",
                            ),
                        )?,
                    ));
                    domain_ids.push(StructuralDomainBinding {
                        semantic_domain: plan.domain,
                        carrier: *carrier,
                        domain: id,
                        byte_predicate: None,
                    });
                    declarations.push(declaration);
                }
            }
            for (binding, declaration) in domain_ids.iter_mut().zip(&declarations) {
                binding.byte_predicate = if declaration.content_projection.is_none()
                    && declaration.establishment_routes.is_empty()
                {
                    declaration.byte_predicate
                } else {
                    None
                };
            }
            Ok((declarations, domain_ids))
        }

        pub(crate) fn lower_establishment_routes(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            routes: &[language_semantics::DomainEstablishmentRoute],
        ) -> Result<Vec<terminal_psi::StructuralEstablishmentRoute>, LoweringError> {
            let mut lowered = crate::expression_preparation::qualifications::establishment_routes(
                identities, checked, routes,
            )?;
            lowered.sort();
            if lowered.windows(2).any(|pair| pair[0] == pair[1]) {
                return unsupported("domain establishment catalog retains a duplicate route");
            }
            Ok(lowered)
        }

        pub(crate) fn call_result_qualification_establishments(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            caller_state: symbols::SymbolHandle,
            coordinate: checked_trees::CheckedUnitCallCoordinate,
            boundary_callee: symbols::SymbolHandle,
            result_domains: &[SemanticDomainId],
            domain_ids: &[StructuralDomainBinding],
        ) -> Result<Vec<terminal_psi::ResultQualificationEstablishment>, LoweringError> {
            let caller_machine = checked
                .machines()
                .iter()
                .find(|machine| {
                    checked
                        .typed
                        .machine_states(machine)
                        .iter()
                        .any(|entry| entry.symbol == caller_state)
                })
                .map(|machine| machine.symbol)
                .unwrap_or_default();
            let point = facts::ProgramPoint::CallEnsures {
                machine_symbol: caller_machine,
                state_symbol: caller_state,
                statement_index: usize::try_from(coordinate.statement_index).map_err(|_| {
                    LoweringError::Unsupported("call statement index exceeds usize")
                })?,
                call_ordinal: usize::try_from(coordinate.call_ordinal)
                    .map_err(|_| LoweringError::Unsupported("call ordinal exceeds usize"))?,
            };
            let mut establishments = Vec::new();
            for domain in result_domains {
                let Some(plan) = flow_facts(checked)
                    .terminal_unit_effects
                    .structural_domains
                    .iter()
                    .find(|plan| plan.domain == *domain)
                else {
                    continue;
                };
                if plan.establishment_routes.is_empty() {
                    continue;
                }
                let established_here = checked.facts.semantic.facts.iter().any(|(_, fact)| {
                    fact.origin == facts::FactOrigin::CallEnsures
                        && fact.point == point
                        && fact.evidence.origin
                            == language_semantics::QualificationEvidenceOrigin::AdmittedReceipt
                        && matches!(
                            fact.payload,
                            facts::FactPayload::ContractDomainMembership { semantic_domain, .. }
                                if semantic_domain == *domain
                        )
                });
                if !established_here {
                    continue;
                }
                let catalog =
                    lower_establishment_routes(identities, checked, &plan.establishment_routes)?;
                let Some((owner, signature)) = checked.trait_machine_signature(boundary_callee)
                else {
                    continue;
                };
                let requirement = identities
                    .intern(&checked.trait_requirement_callable_identity_text(owner, signature));
                let declaration =
                    semantic_identity(identities, checked, boundary_callee).map_err(|_| {
                        LoweringError::Unsupported(
                            "boundary issuer lost its exact declaration owner",
                        )
                    })?;
                if let Some(index) = catalog.iter().position(|route| {
                    matches!(route, terminal_psi::StructuralEstablishmentRoute::BoundaryRequirement {
                        requirement_identity: candidate,
                        declaration: candidate_owner,
                    } if *candidate == requirement && candidate_owner.identity() == declaration)
                }) {
                    establishments.push(terminal_psi::ResultQualificationEstablishment {
                        domain: lookup_domain_id(domain_ids, *domain)?,
                        route: u32::try_from(index).map_err(|_| {
                            LoweringError::Unsupported("establishment route index exceeds u32")
                        })?,
                    });
                }
            }
            establishments.sort_by_key(|binding| (binding.domain, binding.route));
            Ok(establishments)
        }

        pub(crate) fn lower_unit_services(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            plans: UnitPlans<'_>,
            closure: &[symbols::SymbolHandle],
            boundaries: &[(&CheckedBoundaryMachinePlan, BoundaryIdentity)],
            provider_candidates: &[CheckedUnitProviderCandidate],
        ) -> Result<(Vec<ServiceDeclaration>, Vec<(ServiceReachId, ServiceId)>), LoweringError>
        {
            lower_unit_services_including(
                identities,
                checked,
                plans,
                closure,
                boundaries,
                provider_candidates,
                &[],
            )
        }

        pub(crate) fn lower_unit_services_including(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            plans: UnitPlans<'_>,
            closure: &[symbols::SymbolHandle],
            boundaries: &[(&CheckedBoundaryMachinePlan, BoundaryIdentity)],
            provider_candidates: &[CheckedUnitProviderCandidate],
            additional_roots: &[ServiceReachId],
        ) -> Result<(Vec<ServiceDeclaration>, Vec<(ServiceReachId, ServiceId)>), LoweringError>
        {
            let facts = &checked.facts.service_reaches;
            let mut selected = additional_roots.to_vec();
            for symbol in closure {
                let body = UnitBody::find(plans, *symbol)?;
                if let Some(provider) = provider_candidates
                    .iter()
                    .find(|candidate| candidate.candidate == *symbol)
                {
                    collect_provider_candidate_services(
                        &facts.rows,
                        plans,
                        provider,
                        body,
                        &mut selected,
                    )?;
                } else {
                    collect_installation_machine_contract_services(
                        checked,
                        *symbol,
                        body.entry()?.contract_service_reach,
                        body.service_reach(),
                        &mut selected,
                    )?;
                }
                for operation in body.operations() {
                    match operation {
                        CheckedUnitEffectOperationPlan::Call(call) => {
                            collect_service_summary(
                                &facts.rows,
                                call.target.service_reach,
                                &mut selected,
                            )?;
                        }
                        CheckedUnitEffectOperationPlan::RegisterAccess {
                            service_reach, ..
                        }
                        | CheckedUnitEffectOperationPlan::GlobalDescriptorTable {
                            service_reach,
                            ..
                        } => {
                            collect_service_summary(&facts.rows, *service_reach, &mut selected)?;
                        }
                        CheckedUnitEffectOperationPlan::EstablishLocal(
                            checked_trees::CheckedUnitLocalBindingPlan::Primitive { .. },
                        )
                        | CheckedUnitEffectOperationPlan::EstablishLocal(
                            checked_trees::CheckedUnitLocalBindingPlan::ScalarArray { .. },
                        )
                        | checked_trees::reference_hold_pattern!()
                        | checked_trees::byte_sequence_establishment_pattern!()
                        | CheckedUnitEffectOperationPlan::EstablishLocal(
                            checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { .. },
                        )
                        | CheckedUnitEffectOperationPlan::EstablishLocal(
                            checked_trees::CheckedUnitLocalBindingPlan::TrivialAffine { .. },
                        )
                        | CheckedUnitEffectOperationPlan::EstablishLocal(
                            checked_trees::CheckedUnitLocalBindingPlan::Structural { .. },
                        )
                        | CheckedUnitEffectOperationPlan::EstablishLocal(
                            checked_trees::CheckedUnitLocalBindingPlan::Scalar { .. },
                        )
                        | CheckedUnitEffectOperationPlan::Store(_)
                        | CheckedUnitEffectOperationPlan::AtomicAccess(_)
                        | checked_trees::structural_field_write_pattern!()
                        | CheckedUnitEffectOperationPlan::ContinuationCleanup { .. }
                        | CheckedUnitEffectOperationPlan::AtomicFence { .. }
                        | CheckedUnitEffectOperationPlan::EvaluateStructuralValue { .. }
                        | CheckedUnitEffectOperationPlan::Complete { .. } => {}
                    }
                }
            }
            for (boundary, _) in boundaries {
                if let Some(owner) = boundary.owning_service
                    && !selected.contains(&owner)
                {
                    selected.push(owner);
                }
                collect_published_contract_services(
                    &facts.rows,
                    boundary.contract_service_reach,
                    boundary.service_reach,
                    &mut selected,
                )?;
            }
            lower_selected_unit_services(identities, checked, selected)
        }

        pub(crate) fn lower_selected_unit_services(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            mut selected: Vec<ServiceReachId>,
        ) -> Result<(Vec<ServiceDeclaration>, Vec<(ServiceReachId, ServiceId)>), LoweringError>
        {
            let facts = &checked.facts.service_reaches;
            let mut next = 0_usize;
            while let Some(service) = selected.get(next).copied() {
                next += 1;
                let definition =
                    facts
                        .services
                        .definition(service)
                        .ok_or(LoweringError::Unsupported(
                            "Unit closure references an unknown checked service",
                        ))?;
                for parent in &definition.parents {
                    if !selected.contains(parent) {
                        selected.push(*parent);
                    }
                }
            }
            let mut selected_definitions =
                selected
                    .iter()
                    .map(|service| {
                        let definition = facts.services.definition(*service).ok_or(
                            LoweringError::Unsupported(
                                "Unit closure references an unknown checked service",
                            ),
                        )?;
                        let identity =
                            service_declaration_identity(identities, checked, definition.symbol)?;
                        Ok((*service, definition, identity))
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
            selected_definitions
                .sort_by(|left, right| identities.text(left.2).cmp(&identities.text(right.2)));
            if selected_definitions
                .iter()
                .any(|(_, _, identity)| identities.text(*identity).is_none_or(str::is_empty))
                || selected_definitions
                    .windows(2)
                    .any(|pair| pair[0].2 == pair[1].2)
            {
                return unsupported("Unit services have empty or duplicate canonical identities");
            }
            let service_ids = selected_definitions
                .iter()
                .enumerate()
                .map(|(service_index, (source, _, _))| {
                    Ok((*source, service_id(dense_identity(service_index)?)))
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            let declarations = selected_definitions
                .into_iter()
                .map(|(source, definition, identity)| {
                    let mut parents = definition
                        .parents
                        .iter()
                        .map(|parent| lookup_service_id(&service_ids, *parent))
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                    parents.sort();
                    parents.dedup();
                    Ok(ServiceDeclaration {
                        id: lookup_service_id(&service_ids, source)?,
                        identity,
                        parents,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            Ok((declarations, service_ids))
        }

        pub(crate) fn lower_root_service_reach(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            entry: symbols::SymbolHandle,
            service_ids: &[(ServiceReachId, ServiceId)],
        ) -> Result<terminal_psi::TerminalRootServiceReach, LoweringError> {
            let Some(reach) = checked.facts.service_reaches.for_machine(entry) else {
                return Ok(terminal_psi::TerminalRootServiceReach::default());
            };
            let mut concrete = checked
                .facts
                .service_reaches
                .rows
                .services(reach.concrete_effective)
                .iter()
                .map(|service| lookup_service_id(service_ids, *service))
                .collect::<Result<Vec<_>, LoweringError>>()?;
            concrete.sort();
            concrete.dedup();

            let mut installation_dependencies = reach
                .unresolved_installation_reaches
                .iter()
                .map(|dependency| {
                    let trait_matches = checked
                        .typed
                        .traits()
                        .iter()
                        .flat_map(|owner| {
                            checked
                                .typed
                                .executable_trait_machine_instances(owner)
                                .filter(move |requirement| requirement.symbol == dependency.requirement)
                                .map(move |requirement| (owner, requirement))
                        })
                        .collect::<Vec<_>>();
                    let machine_matches = checked
                        .typed
                        .machines()
                        .iter()
                        .filter(|machine| machine.symbol == dependency.requirement)
                        .collect::<Vec<_>>();
                    let requirement_identity = match (trait_matches.as_slice(), machine_matches.as_slice())
                    {
                        ([(owner, requirement)], []) => checked
                            .typed
                            .trait_requirement_callable_identity_text(owner, requirement),
                        ([], [machine])
                            if machine.service_reach_is_installation_bound
                                && matches!(
                                    machine.supply_mode,
                                    language_semantics::MachineSupplyMode::TopLevelRequirement
                                        | language_semantics::MachineSupplyMode::Boundary
                                ) =>
                        {
                            checked
                                .typed
                                .machine_callable_identity_text(machine)
                                .ok_or({
                                    LoweringError::Unsupported(
                                        "top-level installation reach requirement has no normalized machine overload identity",
                                    )
                                })?
                        }
                        _ => {
                            return unsupported(
                                "terminal installation reach does not resolve to one exact typed requirement",
                            );
                        }
                    };
                    let mut upper_bound = checked
                        .facts
                        .service_reaches
                        .rows
                        .services(dependency.upper_bound)
                        .iter()
                        .map(|service| lookup_service_id(service_ids, *service))
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                    upper_bound.sort();
                    upper_bound.dedup();
                    Ok(InstallationReachDependency {
                        requirement_identity: identities.intern(&requirement_identity),
                        upper_bound,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            installation_dependencies.sort_by_key(|left| left.requirement_identity);
            if installation_dependencies
                .windows(2)
                .any(|pair| pair[0].requirement_identity == pair[1].requirement_identity)
            {
                return unsupported("terminal installation reach contains duplicate requirements");
            }
            Ok(terminal_psi::TerminalRootServiceReach {
                concrete,
                installation_dependencies,
            })
        }

        pub(crate) fn collect_contract_services(
            rows: &language_semantics::ServiceReachRowTable,
            contract: ServiceReachPlan,
            summary: ServiceReachSummary,
            selected: &mut Vec<ServiceReachId>,
        ) -> Result<(), LoweringError> {
            collect_service_summary(rows, summary, selected)?;
            if contract.checked_inferred != summary.transitive {
                return unsupported(
                    "Unit contract reach does not match the exact checked transitive reach",
                );
            }
            let ServiceReachInterface::PublishedCeiling(published) = contract.interface else {
                return Ok(());
            };
            require_valid_service_row(published)?;
            let ceiling = rows.services(published);
            if rows
                .services(summary.transitive)
                .iter()
                .any(|service| !ceiling.contains(service))
            {
                return unsupported("checked Unit service reach exceeds its published ceiling");
            }
            for service in ceiling {
                if !selected.contains(service) {
                    selected.push(*service);
                }
            }
            Ok(())
        }

        pub(crate) fn collect_published_contract_services(
            rows: &language_semantics::ServiceReachRowTable,
            contract: ServiceReachPlan,
            summary: ServiceReachSummary,
            selected: &mut Vec<ServiceReachId>,
        ) -> Result<(), LoweringError> {
            if matches!(contract.interface, ServiceReachInterface::InternalInferred) {
                return unsupported("public Unit contract has no published service ceiling");
            }
            collect_contract_services(rows, contract, summary, selected)
        }

        pub(crate) fn collect_installation_machine_contract_services(
            checked: &CheckedTrees,
            machine: symbols::SymbolHandle,
            contract: ServiceReachPlan,
            summary: ServiceReachSummary,
            selected: &mut Vec<ServiceReachId>,
        ) -> Result<(), LoweringError> {
            if let Some(reach) = checked.facts.service_reaches.for_machine(machine)
                && matches!(contract.interface, ServiceReachInterface::InternalInferred)
                && !reach.unresolved_installation_reaches.is_empty()
            {
                return collect_service_summary(
                    &checked.facts.service_reaches.rows,
                    summary,
                    selected,
                );
            }
            collect_contract_services(
                &checked.facts.service_reaches.rows,
                contract,
                summary,
                selected,
            )
        }

        fn collect_provider_candidate_services(
            rows: &language_semantics::ServiceReachRowTable,
            plans: UnitPlans<'_>,
            provider: &CheckedUnitProviderCandidate,
            candidate: UnitBody<'_>,
            selected: &mut Vec<ServiceReachId>,
        ) -> Result<(), LoweringError> {
            let reach = candidate.service_reach();
            collect_service_summary(rows, reach, selected)?;
            if candidate.entry()?.contract_service_reach.checked_inferred != reach.transitive {
                return unsupported(
                    "checked provider adapter contract reach does not match its transitive reach",
                );
            }
            let boundary = unique_unit_boundary(plans, provider.boundary)?;
            let ceiling = match boundary.contract_service_reach.interface {
                ServiceReachInterface::PublishedCeiling(row) => row,
                ServiceReachInterface::InternalInferred => {
                    return unsupported(
                        "checked provider boundary has no published service ceiling",
                    );
                }
            };
            if rows
                .services(reach.transitive)
                .iter()
                .any(|service| !rows.services(ceiling).contains(service))
            {
                return unsupported(
                    "checked provider adapter reach exceeds its boundary requirement",
                );
            }
            Ok(())
        }

        pub(crate) fn lower_provider_candidate_service_ceiling(
            checked: &CheckedTrees,
            plans: UnitPlans<'_>,
            provider: &CheckedUnitProviderCandidate,
            candidate: &CheckedUnitEffectMachinePlan,
            service_ids: &[(ServiceReachId, ServiceId)],
        ) -> Result<Vec<ServiceId>, LoweringError> {
            let rows = &checked.facts.service_reaches.rows;
            let mut selected = Vec::new();
            collect_provider_candidate_services(
                rows,
                plans,
                provider,
                UnitBody::Ordinary(candidate),
                &mut selected,
            )?;
            let source = rows.services(candidate.service_reach.transitive);
            let lowered = crate::shared::terminal_identities::lower_service_reach_sorted(
                source,
                service_ids,
            )?;
            if lowered.len() != source.len() {
                return unsupported("checked provider adapter reach contains duplicates");
            }
            Ok(lowered)
        }

        pub(crate) fn collect_service_summary(
            rows: &language_semantics::ServiceReachRowTable,
            summary: ServiceReachSummary,
            selected: &mut Vec<ServiceReachId>,
        ) -> Result<(), LoweringError> {
            require_valid_service_row(summary.direct)?;
            require_valid_service_row(summary.transitive)?;
            let transitive = rows.services(summary.transitive);
            if rows
                .services(summary.direct)
                .iter()
                .any(|service| !transitive.contains(service))
            {
                return unsupported(
                    "Unit direct service reach is not contained in transitive reach",
                );
            }
            for service in rows.services(summary.direct).iter().chain(transitive) {
                if !selected.contains(service) {
                    selected.push(*service);
                }
            }
            Ok(())
        }

        pub(crate) fn require_valid_service_row(
            row: ServiceReachRowId,
        ) -> Result<(), LoweringError> {
            if row.is_valid() {
                Ok(())
            } else {
                unsupported("Unit closure contains a null checked service row")
            }
        }
    }
    pub(crate) mod claims {

        use super::super::{
            CarryPolicy, CheckedUnitEntryClaimPlan, CheckedUnitStructuralParameterPlan, ClaimId,
            EntryClaim, PermissionClaimIdentity, StructuralParameterDeclaration, claim_id,
        };
        use super::{
            CheckedTrees, LoweringError, PlaceId, StructuralPlaceKind, TerminalMachineResult,
            allocate_dense, lookup_claim_id, unsupported,
        };
        use crate::expression_preparation::bindings::structural_paths::lower_static_argument_path;
        #[derive(Clone)]
        pub(crate) struct LoweredUnitClaims {
            pub(crate) entry_claims: Vec<EntryClaim>,
            pub(crate) source_claims: Vec<(PermissionClaimIdentity, ClaimId)>,
            pub(crate) unreceived_results:
                Vec<crate::emission::claim_bindings::UnreceivedResultClaim>,
            pub(crate) completed_values: Vec<crate::emission::claim_bindings::CompletedValueClaim>,
        }

        impl LoweredUnitClaims {
            pub(crate) fn published(&self) -> crate::emission::claim_bindings::PublishedClaims<'_> {
                crate::emission::claim_bindings::PublishedClaims {
                    bindings: &self.source_claims,
                    unreceived_results: &self.unreceived_results,
                    completed_values: &self.completed_values,
                }
            }
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn lower_return_claims(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            machine: symbols::SymbolHandle,
            state: symbols::SymbolHandle,
            result: &checked_trees::CheckedUnitStructuralReturnPlan,
            source: &terminal_psi::StructuralArgument,
            operations: &crate::emission::operation_emission::buffer::OperationBuffer,
            current_claims: &crate::emission::current_claims::CurrentClaimNames,
            bindings: &[(PermissionClaimIdentity, ClaimId)],
            structural_type: semantic_vocabulary::StructuralTypeId,
            structural_types: &[terminal_psi::StructuralTypeDeclaration],
        ) -> Result<Vec<terminal_psi::StructuralResultClaimBinding>, LoweringError> {
            let (statement_index, expression, evaluation) = match result.completion {
                checked_trees::CheckedStructuralReturnCompletion::Value {
                    statement_index,
                    expression,
                    evaluation,
                } => (statement_index, expression, evaluation),
                checked_trees::CheckedStructuralReturnCompletion::Call { coordinate } => {
                    let mut matching = operations.source_calls.iter().filter(|occurrence| {
                        occurrence.matches_statement(
                            state,
                            coordinate.statement_index as usize,
                            coordinate.call_ordinal as usize,
                        )
                    });
                    let occurrence = matching.next().ok_or(LoweringError::Unsupported(
                        "terminal return call occurrence is absent",
                    ))?;
                    if matching.next().is_some() {
                        return unsupported("terminal return call occurrence is repeated");
                    }
                    let mut matching = operations.iter().filter(|operation| {
                        occurrence.terminal_site.operation() == Some(operation.id)
                    });
                    let produced = matching
                        .next()
                        .and_then(|operation| operation.result.as_structural())
                        .ok_or(LoweringError::Unsupported(
                            "terminal return call lost its concrete result",
                        ))?;
                    if matching.next().is_some() || produced.structural_type != structural_type {
                        return unsupported(
                            "terminal return call result is ambiguous or has another carrier",
                        );
                    }
                    return Ok(produced.claims.clone());
                }
            };
            let materialized =
                if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal,
                } = result.source
                {
                    let mut matching = operations
                        .structural_values
                        .iter()
                        .filter(|(ordinal, _)| *ordinal == binding_ordinal);
                    let binding = matching.next().map(|(_, binding)| binding).ok_or(
                        LoweringError::Unsupported("return lost its materialized source custody"),
                    )?;
                    if matching.next().is_some() {
                        return unsupported("return has ambiguous materialized custody");
                    }
                    if binding.structural_type() != structural_type
                        || binding.multiplicity()
                            != crate::shared::terminal_identities::structural_multiplicity(
                                result.multiplicity,
                            )
                    {
                        return unsupported("return materialized custody has another carrier");
                    }
                    Some(binding)
                } else {
                    None
                };
            let completed_claims = current_claims.source(source)?;
            if validation::storage_place_from_expression_in_state(
                &checked.typed,
                state,
                statement_index as usize,
                expression,
            )
            .is_none_or(|place| matches!(place.root, facts::PlaceRoot::Expression(_)))
            {
                let binding = materialized.ok_or(LoweringError::Unsupported(
                    "computed return has no materialized result binding",
                ))?;
                if !binding.path().is_empty() {
                    return unsupported("computed return binding is projected");
                }
                let mut claims = completed_claims.ok_or(LoweringError::Unsupported(
                    "computed return lost its current source custody",
                ))?;
                claims.sort_by_key(|binding| binding.claim);
                return Ok(claims);
            }
            lower_owned_transfer_claims(
                identities,
                checked,
                machine,
                state,
                statement_index,
                expression,
                evaluation,
                bindings,
                completed_claims.as_deref(),
                structural_type,
                structural_types,
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn lower_owned_transfer_claims(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            machine: symbols::SymbolHandle,
            state: symbols::SymbolHandle,
            statement_index: u32,
            expression: typed_trees::expression::ExpressionHandle,
            evaluation: checked_trees::FlowPermissionEvaluation,
            bindings: &[(PermissionClaimIdentity, ClaimId)],
            completed_claims: Option<&[terminal_psi::StructuralResultClaimBinding]>,
            structural_type: semantic_vocabulary::StructuralTypeId,
            structural_types: &[terminal_psi::StructuralTypeDeclaration],
        ) -> Result<Vec<terminal_psi::StructuralResultClaimBinding>, LoweringError> {
            crate::emission::claim_bindings::lower_owned_operand_claims(
                identities,
                checked,
                machine,
                state,
                statement_index,
                expression,
                Some(evaluation),
                Some(language_semantics::PermissionEventSource::Statement {
                    statement_index: statement_index as usize,
                }),
                bindings,
                completed_claims,
                structural_type,
                structural_types,
            )
        }

        pub(crate) fn lower_unit_entry_claims(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            machine: symbols::SymbolHandle,
            state: symbols::SymbolHandle,
            claims: &[CheckedUnitEntryClaimPlan],
            parameters: &[StructuralParameterDeclaration],
            structural_types: &[terminal_psi::StructuralTypeDeclaration],
        ) -> Result<LoweredUnitClaims, LoweringError> {
            let mut entry_claims = Vec::with_capacity(claims.len());
            let mut source_claims = Vec::with_capacity(claims.len());
            let mut next_claim = 1_u64;
            for claim in claims {
                if claim.carry != CarryPolicy::STRICT {
                    return unsupported("Unit entry claim has a non-default carry policy");
                }
                let parameter = parameters
                    .get(usize::try_from(claim.parameter_index).map_err(|_| {
                        LoweringError::Unsupported("Unit entry claim parameter index exceeds usize")
                    })?)
                    .ok_or(LoweringError::Unsupported(
                        "Unit entry claim has an invalid parameter index",
                    ))?;
                let PermissionClaimIdentity::Established {
                    machine_symbol,
                    state_symbol,
                    source: language_semantics::PermissionEventSource::StateEntry,
                    ..
                } = claim.claim_identity
                else {
                    return unsupported(
                        "Unit entry claim is not an exact checked state-entry claim",
                    );
                };
                if machine_symbol != machine || state_symbol != state {
                    return unsupported("Unit entry claim belongs to another checked state");
                }
                if source_claims
                    .iter()
                    .any(|(identity, _)| *identity == claim.claim_identity)
                {
                    return unsupported("Unit entry claim identity is duplicated");
                }
                let id = claim_id(allocate_dense(&mut next_claim)?);
                entry_claims.push(EntryClaim {
                    claim: id,
                    input: parameter.place,
                    path: lower_static_argument_path(
                        identities,
                        &claim.path,
                        parameter.structural_type,
                        structural_types,
                    )?,
                });
                source_claims.push((claim.claim_identity, id));
            }
            Ok(LoweredUnitClaims {
                entry_claims,
                source_claims,
                unreceived_results: Vec::new(),
                completed_values: Vec::new(),
            })
        }

        pub(crate) fn lower_result_identity(
            checked: &CheckedTrees,
            machine: symbols::SymbolHandle,
            state: symbols::SymbolHandle,
            checked_parameters: &[CheckedUnitStructuralParameterPlan],
            parameters: &[StructuralParameterDeclaration],
            result: &TerminalMachineResult,
            source_claims: &[(PermissionClaimIdentity, ClaimId)],
        ) -> Result<Vec<terminal_psi::ContentIdentityReshuffle>, LoweringError> {
            let facts = checked
                .facts
                .qualifications
                .content
                .identity_reshuffles
                .iter()
                .filter(|fact| fact.machine_symbol == machine && fact.state_symbol == state)
                .cloned()
                .collect::<Vec<_>>();
            let mut lowered =
                crate::proofs::content_conservation::lower_content_identity_reshuffles(&facts)?;
            let remap = |place: PlaceId| -> Result<PlaceId, LoweringError> {
                let declaration = lowered
                    .structural_places
                    .iter()
                    .find(|declaration| declaration.id == place)
                    .ok_or(LoweringError::Unsupported(
                        "content result has an absent structural root",
                    ))?;
                match declaration.kind {
                    StructuralPlaceKind::Parameter { position } => checked_parameters
                        .iter()
                        .zip(parameters)
                        .find(|(source, _)| source.position == position)
                        .map(|(_, parameter)| parameter.place)
                        .ok_or(LoweringError::Unsupported(
                            "content result lost its exact input parameter",
                        )),
                    StructuralPlaceKind::Result => {
                        result.structural().map(|result| result.place).ok_or(
                            LoweringError::Unsupported("content identity has no structural result"),
                        )
                    }
                    _ => unsupported("content identity requires whole invocation roots"),
                }
            };
            for reshuffle in &mut lowered.reshuffles {
                let source = lowered
                    .source_claims
                    .iter()
                    .find(|(_, claim)| *claim == reshuffle.claim)
                    .ok_or(LoweringError::Unsupported(
                        "content result lost its semantic claim",
                    ))?
                    .0;
                reshuffle.claim = lookup_claim_id(source_claims, source)?;
                reshuffle.input.root = remap(reshuffle.input.root)?;
                reshuffle.output.root = remap(reshuffle.output.root)?;
            }
            Ok(lowered.reshuffles)
        }
    }
    pub(crate) mod completion_cleanup {

        use super::{CheckedTrees, LoweringError};
        use crate::emission::operation_emission::buffer::OperationBuffer;
        use crate::unit::attached_unit::argument_evaluation::{
            Evaluation, ScalarReturnCleanup, StructuralValueStorage,
        };
        use crate::unit::unit_cleanup::CleanupRegion;
        use crate::unit::unit_cleanup::bind_consumed_claims;
        pub(crate) mod owner_order {
            use super::{CleanupRegion, Evaluation, LoweringError, OperationBuffer};
            use crate::shared::checked_facts::flow_facts;
            use checked_trees::CheckedUnitEffectOperationPlan;
            use semantic_vocabulary::PlaceId;

            struct CleanupOwner {
                place: PlaceId,
                declaration: Option<(u8, u32)>,
                order: (u8, u32, usize),
            }

            pub(crate) fn declaring_order(
                checked: &checked_trees::CheckedTrees,
                state: symbols::SymbolHandle,
                source: super::NormalExitSource<'_>,
                storage: &checked_trees::CheckedUnitStructuralArgumentSourcePlan,
            ) -> Result<Option<(u8, u32)>, LoweringError> {
                use checked_trees::CheckedUnitStructuralArgumentSourcePlan as Source;
                let named =
                    |symbol| -> Result<_, LoweringError> {
                        if let Some(local) = checked.facts.state_locals.named(state, symbol) {
                            return Ok(Some((1, local.statement_index)));
                        }
                        let state = checked.typed.state_by_symbol(state).ok_or(
                            LoweringError::Unsupported(
                                "cleanup declaring state is absent from the checked program",
                            ),
                        )?;
                        let position = checked
                            .typed
                            .state_parameters(state)
                            .iter()
                            .position(|parameter| parameter.symbol == symbol)
                            .ok_or(LoweringError::Unsupported(
                                "cleanup owner has no checked declaration",
                            ))?;
                        Ok(Some((
                            0,
                            u32::try_from(position).map_err(|_| {
                                LoweringError::Unsupported(
                                    "cleanup parameter declaration exceeds its checked coordinate",
                                )
                            })?,
                        )))
                    };
                match *storage {
                    Source::Parameter { parameter_index } => {
                        let parameter = source.parameters.get(parameter_index as usize).ok_or(
                            LoweringError::Unsupported("cleanup parameter declaration is absent"),
                        )?;
                        Ok(Some((0, parameter.position)))
                    }
                    Source::StructuralLocal { symbol } => named(symbol),
                    Source::CapturedStorage {
                        selection,
                        source_ordinal,
                    } => {
                        let receipt = checked.facts.flow.ownership.owned_selections.get(selection);
                        if receipt.state != state {
                            return Err(LoweringError::Unsupported(
                                "captured cleanup belongs to another state",
                            ));
                        }
                        let candidate = flow_facts(checked)
                            .ownership
                            .selection_sources
                            .span_or_empty(receipt.sources)
                            .get(source_ordinal as usize)
                            .ok_or(LoweringError::Unsupported(
                                "captured cleanup source declaration is absent",
                            ))?;
                        named(candidate.symbol)
                    }
                    Source::StructuralResult { binding_ordinal } => {
                        let result = source
                            .operations
                            .iter()
                            .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
                            .filter_map(CheckedUnitEffectOperationPlan::structural_result_binding)
                            .find(|result| result.binding_ordinal == binding_ordinal)
                            .ok_or(LoweringError::Unsupported(
                                "cleanup result declaration is absent",
                            ))?;
                        if result.local.is_named() {
                            named(result.local.symbol)
                        } else {
                            Ok(None)
                        }
                    }
                    _ => Ok(None),
                }
            }

            pub(crate) fn selection(
                state: symbols::SymbolHandle,
                planned: &[CheckedUnitEffectOperationPlan],
                evaluation: &Evaluation,
                operations: &OperationBuffer,
                actions: &mut Vec<CleanupRegion>,
            ) -> Result<(), LoweringError> {
                let results = planned
                    .iter()
                    .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
                    .filter_map(CheckedUnitEffectOperationPlan::structural_result_binding)
                    .collect::<Vec<_>>();
                let mut declarations = Vec::new();
                for result in &results {
                    if result.local.is_named()
                        && !declarations
                            .iter()
                            .any(|(symbol, _)| *symbol == result.local.symbol)
                    {
                        declarations.push((result.local.symbol, result.statement_index));
                    }
                }
                for operation in planned {
                    if let CheckedUnitEffectOperationPlan::EstablishLocal(
                        checked_trees::CheckedUnitLocalBindingPlan::Structural {
                            symbol,
                            statement_index,
                            ..
                        },
                    ) = operation
                        && !declarations.iter().any(|(declared, _)| declared == symbol)
                    {
                        declarations.push((*symbol, *statement_index));
                    }
                }
                let mut roots = Vec::<CleanupOwner>::new();
                for action in actions.iter() {
                    let place = action.action.place();
                    let explicit = action.declaration_order;
                    if roots
                        .iter()
                        .any(|root| root.place == place && root.declaration == explicit)
                    {
                        continue;
                    }
                    let parameter = evaluation
                        .structural_parameters
                        .iter()
                        .find(|(_, declaration)| declaration.place == place);
                    let current = evaluation
                        .structural_locals
                        .iter()
                        .find(|local| {
                            local.argument.place == place && local.argument.path.is_empty()
                        })
                        .and_then(|local| local.root.as_symbol())
                        .and_then(|symbol| {
                            declarations
                                .iter()
                                .find(|(declared, _)| *declared == symbol)
                        })
                        .map(|(_, statement)| (1, *statement, usize::MAX));
                    let produced = operations
                        .structural_values
                        .iter()
                        .find(|(_, binding)| binding.place() == place && binding.path().is_empty())
                        .and_then(|(ordinal, _)| {
                            results
                                .iter()
                                .find(|result| result.binding_ordinal == *ordinal)
                                .copied()
                        });
                    let declared_result = produced
                        .filter(|result| result.local.is_named())
                        .map(|result| (1, result.statement_index, usize::MAX));
                    let physical = operations
                        .structural_establishments
                        .iter()
                        .position(|root| *root == place);
                    let parameter = if let Some((position, current)) = parameter {
                        let materialized_ingress = explicit.is_none()
                            && current.access == terminal_psi::StructuralAccess::Owned
                            && evaluation.previous_parameter_homes.iter().any(|previous| {
                                previous.position == current.position
                                    && previous.place != current.place
                                    && previous.access
                                        == terminal_psi::StructuralAccess::SharedBorrow
                            });
                        if materialized_ingress {
                            let receiving = evaluation
                                .structural_value_owners
                                .iter()
                                .find(|owner| owner.value.place == place)
                                .map(|owner| owner.statement)
                                .or_else(|| produced.map(|result| result.statement_index))
                                .or(operations
                                    .structural_materialization_statement(place, state)?)
                                .ok_or(LoweringError::Unsupported(
                                    "rebound borrowed formal has no completed receiving occurrence",
                                ))?;
                            Some((
                                1,
                                receiving,
                                physical.ok_or(LoweringError::Unsupported(
                                    "rebound borrowed formal has no materialized descriptor",
                                ))?,
                            ))
                        } else {
                            Some((0, *position, 0))
                        }
                    } else {
                        None
                    };
                    let declared_order = explicit
                        .map(|(kind, coordinate)| {
                            (kind, coordinate, if kind == 0 { 0 } else { usize::MAX })
                        })
                        .or(parameter)
                        .or(current)
                        .or(declared_result);
                    let order = if let Some(order) = declared_order {
                        order
                    } else {
                        operations
                            .structural_materialization_statement(place, state)?
                            .zip(physical)
                            .map(|(statement, ordinal)| (1, statement, ordinal))
                            .ok_or(LoweringError::Unsupported(
                                "normal cleanup owner has no checked declaration or completed producer coordinate",
                            ))?
                    };
                    roots.push(CleanupOwner {
                        place,
                        declaration: explicit,
                        order,
                    });
                }
                roots.sort_by_key(|root| std::cmp::Reverse(root.order));
                let previous = std::mem::take(actions);
                for root in roots {
                    for region in previous.iter().filter(|region| {
                        region.action.place() == root.place
                            && region.declaration_order == root.declaration
                    }) {
                        let mut region = region.clone();
                        if matches!(
                            region.action.physical(),
                            Some(terminal_psi::TerminalAffineCleanupAction::RetireHome(_))
                        ) {
                            if region.selected_region.as_ref().is_some_and(|selected| {
                                selected.place != root.place
                                    || !selected.path.is_empty()
                                    || selected.access != terminal_psi::StructuralAccess::Owned
                            }) {
                                return Err(LoweringError::Unsupported(
                                    "home retirement selects a different owned occurrence",
                                ));
                            }
                            region.selected_region = Some(terminal_psi::StructuralArgument {
                                place: root.place,
                                path: Vec::new(),
                                access: terminal_psi::StructuralAccess::Owned,
                            });
                        }
                        actions.push(region);
                    }
                }
                Ok(())
            }
        }
        pub(crate) mod remaining_partition {
            use super::{CleanupRegion, Evaluation, LoweringError, OperationBuffer};

            pub(crate) fn retain(
                evaluation: &Evaluation,
                regions: Vec<CleanupRegion>,
                types: &[terminal_psi::StructuralTypeDeclaration],
                operations: &OperationBuffer,
            ) -> Result<Vec<CleanupRegion>, LoweringError> {
                use terminal_psi::TerminalAffineCleanupAction as Action;
                let mut remaining = Vec::new();
                let mut partial_owners = Vec::new();
                for region in regions {
                    if region.action.physical().is_none() {
                        remaining.push(region);
                        continue;
                    }
                    let place = region.action.place();
                    let retired = evaluation
                        .selection_cleanups
                        .iter()
                        .flat_map(|cleanup| &cleanup.retired_reference_paths)
                        .filter(|(source, _)| *source == place)
                        .map(|(_, path)| path)
                        .collect::<Vec<_>>();
                    if retired.is_empty() {
                        remaining.push(region);
                        continue;
                    }
                    let partition = evaluation.normal_discard_regions(place, types, operations)?;
                    match region.action.physical() {
                        Some(Action::DiscardRoot(_)) => {
                            if let Some(leaf) = &region.partition_leaf {
                                partial_owners.push(leaf.owner);
                            }
                            for mut part in partition {
                                part.declaration_order = region.declaration_order;
                                part.selected_region = selected_residual(&region, &part)?;
                                for condition in &region.conditions {
                                    if !part.conditions.contains(condition) {
                                        part.conditions.push(condition.clone());
                                    }
                                }
                                remaining.push(part);
                            }
                        }
                        Some(Action::DiscardResidual(discard))
                            if retired.iter().any(|path| {
                                path.starts_with(&discard.path) || discard.path.starts_with(path)
                            }) =>
                        {
                            if let Some(leaf) = &region.partition_leaf {
                                partial_owners.push(leaf.owner);
                            }
                            for part in &partition {
                                let Some(Action::DiscardResidual(residual)) =
                                    part.action.physical()
                                else {
                                    continue;
                                };
                                if !residual.path.starts_with(&discard.path) {
                                    continue;
                                }
                                let mut selected = part.clone();
                                selected.declaration_order = region.declaration_order;
                                selected.selected_region = selected_residual(&region, &selected)?;
                                for condition in &region.conditions {
                                    if !selected.conditions.contains(condition) {
                                        selected.conditions.push(condition.clone());
                                    }
                                }
                                remaining.push(selected);
                            }
                        }
                        Some(Action::InvokeNominal(cleanup))
                            if retired.iter().any(|path| {
                                path.starts_with(&cleanup.path) || cleanup.path.starts_with(path)
                            }) =>
                        {
                            return Err(LoweringError::Unsupported(
                                "selected complement nominal receiver was dismantled before its checked hook",
                            ));
                        }
                        _ => remaining.push(region),
                    }
                }
                // This consumer selected a smaller residual schedule. Keep its
                // old strict history checks; the full checked tree no longer
                // describes that complete sequence of actions.
                partial_owners.sort_by_key(|owner| (owner.arena_index(), owner.generation()));
                partial_owners.dedup();
                remaining.retain_mut(|region| {
                    if region.partition_leaf.as_ref().is_some_and(|leaf| {
                        partial_owners
                            .binary_search_by_key(
                                &(leaf.owner.arena_index(), leaf.owner.generation()),
                                |owner| (owner.arena_index(), owner.generation()),
                            )
                            .is_ok()
                    }) {
                        region.partition_leaf = None;
                        return region.action.physical().is_some();
                    }
                    true
                });
                Ok(remaining)
            }

            fn selected_residual(
                original: &CleanupRegion,
                residual: &CleanupRegion,
            ) -> Result<Option<terminal_psi::StructuralArgument>, LoweringError> {
                let Some(receiver) = original.selected_region.as_ref() else {
                    return Ok(None);
                };
                if receiver.access != terminal_psi::StructuralAccess::Owned
                    || receiver.place != residual.action.place()
                    || !residual.action.path().starts_with(&receiver.path)
                {
                    return Err(LoweringError::Unsupported(
                        "residual cleanup selection does not enclose its exact receiver",
                    ));
                }
                Ok(Some(terminal_psi::StructuralArgument {
                    place: residual.action.place(),
                    path: residual.action.path().to_vec(),
                    access: terminal_psi::StructuralAccess::Owned,
                }))
            }
        }
        use checked_trees::{
            CheckedScalarBranchDestination, FlowClaimJoinExit, FlowClaimJoinExitKind,
        };
        pub(crate) use owner_order::selection;
        use terminal_psi::StructuralParameterDeclaration;

        #[derive(Clone, Copy)]
        pub(crate) struct NormalExitSource<'a> {
            pub(crate) completion: checked_trees::CheckedNormalCompletion,
            pub(crate) cleanups: &'a [checked_trees::CheckedNormalExitCleanupPlan],
            pub(crate) trivial_affine_discards: &'a [semantic_vocabulary::PlaceId],
            pub(crate) operations: &'a [checked_trees::CheckedUnitEffectOperationPlan],
            pub(crate) parameters: &'a [checked_trees::CheckedUnitStructuralParameterPlan],
        }

        pub(crate) struct CompletionCleanupCatalog<'a, 'calls, 'context> {
            pub(crate) control: &'a checked_trees::CheckedCleanupControlPlans,
            pub(crate) types: &'a [terminal_psi::StructuralTypeDeclaration],
            pub(crate) type_ids: &'a [(
                typed_trees::type_identity::TypeIdentityId,
                semantic_vocabulary::StructuralTypeId,
            )],
            pub(crate) calls:
                &'calls mut crate::emission::operation_emission::calls::CallEmissionContext<
                    'context,
                >,
        }

        pub(crate) fn destination(
            source: NormalExitSource<'_>,
            destination: &CheckedScalarBranchDestination,
        ) -> Result<FlowClaimJoinExit, LoweringError> {
            let CheckedScalarBranchDestination::Return {
                statement_ordinal,
                is_continuation,
            } = destination
            else {
                return Err(LoweringError::Unsupported(
                    "cleanup destination is not a normal value return",
                ));
            };
            coordinate(source, *statement_ordinal as usize, *is_continuation)
        }

        /// Keep each authored scalar return's cleanup before its private result join.
        /// Preparing action identities does not discharge their prerequisites; complete
        /// graph proof production observes the selected path and outgoing call effects.
        /// Working state for the exact source-published normal result tree.
        pub(crate) enum NormalResultCleanup {
            NoNormal {
                computation: checked_trees::CheckedScalarComputationHandle,
            },
            Value {
                computation: checked_trees::CheckedScalarComputationHandle,
                actions: Vec<CleanupRegion>,
            },
            Dispatch {
                computation: checked_trees::CheckedScalarComputationHandle,
                arms: Vec<(checked_trees::CheckedScalarDispatchArm, NormalResultCleanup)>,
            },
        }

        pub(crate) fn scalar_returns(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            claims: &[(
                language_semantics::PermissionClaimIdentity,
                semantic_vocabulary::ClaimId,
            )],
            checked: &CheckedTrees,
            source: NormalExitSource<'_>,
            control: &checked_trees::CheckedUnitScalarControlPlan,
            catalog: &mut CompletionCleanupCatalog<'_, '_, '_>,
            parameters: &[StructuralParameterDeclaration],
            evaluation: &Evaluation,
            operations: &OperationBuffer,
        ) -> Result<Vec<ScalarReturnCleanup>, LoweringError> {
            if source.cleanups.is_empty() {
                return Err(LoweringError::Unsupported(
                    "scalar return has no source cleanup coordinates",
                ));
            }
            // Source admission retains all authored exits. Only the checked control
            // plan selects runtime returns; preparing dead cleanup rows would allocate
            // orphan completion blocks and their obligations. Every selected return
            // still requires its unique, exact source-owned cleanup coordinate.
            control
                .selected_copies
                .iter()
                .map(|row| {
                    let destination = CheckedScalarBranchDestination::Return {
                        statement_ordinal: row.statement_ordinal,
                        is_continuation: row.is_continuation,
                    };
                    let exit = self::destination(source, &destination)?;
                    let plan = plan_for_exit(source, exit)?;
                    let (actions, result) = if plan.result_completion.is_valid() {
                        (
                            Vec::new(),
                            Some(normal_result_cleanup(
                                identities,
                                claims,
                                checked,
                                source,
                                exit,
                                plan,
                                plan.result_completion,
                                catalog,
                                parameters,
                                evaluation,
                                operations,
                            )?),
                        )
                    } else {
                        let mut actions = for_planned_actions(
                            identities,
                            claims,
                            checked,
                            source,
                            exit,
                            plan,
                            catalog.control.actions.span_or_empty(plan.actions),
                            plan.partitions,
                            catalog,
                            parameters,
                            evaluation,
                            operations,
                        )?;
                        selection(
                            exit.state_symbol,
                            source.operations,
                            evaluation,
                            operations,
                            &mut actions,
                        )?;
                        (actions, None)
                    };
                    Ok(ScalarReturnCleanup {
                        exit,
                        destination,
                        actions,
                        result,
                        copied_payloads: row.copied_payloads.clone(),
                    })
                })
                .collect()
        }

        pub(crate) fn coordinate(
            source: NormalExitSource<'_>,
            statement_index: usize,
            continuation: bool,
        ) -> Result<FlowClaimJoinExit, LoweringError> {
            optional_coordinate(source, statement_index, continuation)?.ok_or(
                LoweringError::Unsupported("normal exit requested from a no-normal invocation"),
            )
        }

        pub(crate) fn optional_coordinate(
            source: NormalExitSource<'_>,
            statement_index: usize,
            continuation: bool,
        ) -> Result<Option<FlowClaimJoinExit>, LoweringError> {
            let mut rows = source.cleanups.iter().filter(|row| {
                row.exit.statement_index == statement_index
                    && if continuation {
                        row.exit.kind == FlowClaimJoinExitKind::TransitionContinuation
                    } else {
                        matches!(
                            row.exit.kind,
                            FlowClaimJoinExitKind::TransitionTarget
                                | FlowClaimJoinExitKind::FinalExpression
                        )
                    }
            });
            let Some(row) = rows.next() else {
                return match source.completion {
                    checked_trees::CheckedNormalCompletion::NoNormal => Ok(None),
                    checked_trees::CheckedNormalCompletion::Possible => Err(
                        LoweringError::Unsupported("normal exit has no cleanup plan"),
                    ),
                };
            };
            if rows.next().is_some() {
                return Err(LoweringError::Unsupported(
                    "normal exit has more than one cleanup plan",
                ));
            }
            Ok(Some(row.exit))
        }

        #[allow(clippy::too_many_arguments)]
        fn plan_for_exit<'a>(
            source: NormalExitSource<'a>,
            exit: FlowClaimJoinExit,
        ) -> Result<&'a checked_trees::CheckedNormalExitCleanupPlan, LoweringError> {
            let mut selected = source.cleanups.iter().filter(|plan| plan.exit == exit);
            let selected_exit = selected.next().ok_or(LoweringError::Unsupported(
                "normal return is missing its exact cleanup plan",
            ))?;
            if selected.next().is_some() {
                return Err(LoweringError::Unsupported(
                    "normal cleanup repeats its selected exit",
                ));
            }
            Ok(selected_exit)
        }

        pub(crate) fn for_exit_regions(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            claims: &[(
                language_semantics::PermissionClaimIdentity,
                semantic_vocabulary::ClaimId,
            )],
            checked: &CheckedTrees,
            source: NormalExitSource<'_>,
            exit: FlowClaimJoinExit,
            catalog: &mut CompletionCleanupCatalog<'_, '_, '_>,
            parameters: &[StructuralParameterDeclaration],
            evaluation: &Evaluation,
            operations: &OperationBuffer,
        ) -> Result<Vec<CleanupRegion>, LoweringError> {
            let plan = plan_for_exit(source, exit)?;
            for_planned_actions(
                identities,
                claims,
                checked,
                source,
                exit,
                plan,
                catalog.control.actions.span_or_empty(plan.actions),
                plan.partitions,
                catalog,
                parameters,
                evaluation,
                operations,
            )
        }

        #[allow(clippy::too_many_arguments)]
        fn for_planned_actions(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            claims: &[(
                language_semantics::PermissionClaimIdentity,
                semantic_vocabulary::ClaimId,
            )],
            checked: &CheckedTrees,
            source: NormalExitSource<'_>,
            exit: FlowClaimJoinExit,
            selected_exit: &checked_trees::CheckedNormalExitCleanupPlan,
            selected_actions: &[checked_trees::CheckedNormalCleanupAction],
            partitions: arena::HandleSpan<checked_trees::CheckedCleanupPartitionRef>,
            catalog: &mut CompletionCleanupCatalog<'_, '_, '_>,
            parameters: &[StructuralParameterDeclaration],
            evaluation: &Evaluation,
            operations: &OperationBuffer,
        ) -> Result<Vec<CleanupRegion>, LoweringError> {
            let cleanup_control = catalog.control;
            let planned = crate::unit::unit_cleanup::outstanding_actions(
                selected_actions,
                source.operations,
                source.parameters,
                cleanup_control,
            );
            let mut actions = Vec::with_capacity(planned.len());
            for action in planned {
                if !matches!(
                    action,
                    checked_trees::CheckedNormalCleanupAction::InvokeNominal(_)
                ) && evaluation.cleanup_source_is_retired(
                    action.source(),
                    parameters,
                    |binding| result_storage(operations, binding),
                )? {
                    continue;
                }
                actions.push(action);
            }
            let declaration_orders = actions
                .iter()
                .map(|action| {
                    owner_order::declaring_order(
                        checked,
                        exit.state_symbol,
                        source,
                        action.source(),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let selected = crate::unit::unit_cleanup::select_cleanup_partition_sequence(
                cleanup_control,
                cleanup_control.partition_refs.span_or_empty(partitions),
                &actions,
            )?;
            let mut rows = lower_regions(
                identities,
                claims,
                checked,
                selected.actions.iter().map(std::borrow::Cow::as_ref),
                &selected.partitions,
                cleanup_control
                    .normal_exit_regions
                    .span_or_empty(selected_exit.selected_regions),
                catalog,
                parameters,
                evaluation,
                operations,
            )?;
            let mut orders = declaration_orders.into_iter();
            for row in &mut rows {
                row.declaration_order = if let Some(leaf) = row
                    .partition_leaf
                    .as_ref()
                    .filter(|leaf| leaf.leaf_count == 0)
                {
                    owner_order::declaring_order(
                        checked,
                        exit.state_symbol,
                        source,
                        &cleanup_control.partitions.get(leaf.owner).source,
                    )?
                } else {
                    orders
                        .next()
                        .ok_or(LoweringError::InvalidCleanupPartition {
                            reason: "cleanup action order lost its source association",
                        })?
                };
            }
            rows = remaining_partition::retain(evaluation, rows, catalog.types, operations)?;
            let mut regions = crate::unit::unit_cleanup::lower_reference_completion_regions(
                identities,
                checked,
                source.operations,
                rows,
                parameters,
                evaluation,
                operations,
                |identities, owner| {
                    lower_regions(
                        identities,
                        claims,
                        checked,
                        cleanup_control.actions.span_or_empty(owner.actions),
                        cleanup_control
                            .partition_refs
                            .span_or_empty(owner.partitions),
                        std::slice::from_ref(&owner.owner),
                        catalog,
                        parameters,
                        evaluation,
                        operations,
                    )
                },
                |binding| result_storage(operations, binding),
            )?;
            regions.extend(source.trivial_affine_discards.iter().map(|place| {
                CleanupRegion::unconditional(
                    terminal_psi::TerminalAffineCleanupAction::DiscardRoot(*place),
                )
            }));
            Ok(regions)
        }

        #[allow(clippy::too_many_arguments)]
        fn normal_result_cleanup(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            claims: &[(
                language_semantics::PermissionClaimIdentity,
                semantic_vocabulary::ClaimId,
            )],
            checked: &CheckedTrees,
            source: NormalExitSource<'_>,
            exit: FlowClaimJoinExit,
            plan: &checked_trees::CheckedNormalExitCleanupPlan,
            result: arena::Handle<checked_trees::CheckedNormalResultCompletion>,
            catalog: &mut CompletionCleanupCatalog<'_, '_, '_>,
            parameters: &[StructuralParameterDeclaration],
            evaluation: &Evaluation,
            operations: &OperationBuffer,
        ) -> Result<NormalResultCleanup, LoweringError> {
            let control = catalog.control;
            Ok(match control.normal_result_completions.get(result) {
                checked_trees::CheckedNormalResultCompletion::NoNormal { computation } => {
                    NormalResultCleanup::NoNormal {
                        computation: *computation,
                    }
                }
                checked_trees::CheckedNormalResultCompletion::Value {
                    computation,
                    actions,
                    partitions,
                } => {
                    let mut actions = for_planned_actions(
                        identities,
                        claims,
                        checked,
                        source,
                        exit,
                        plan,
                        control.actions.span_or_empty(*actions),
                        *partitions,
                        catalog,
                        parameters,
                        evaluation,
                        operations,
                    )?;
                    selection(
                        exit.state_symbol,
                        source.operations,
                        evaluation,
                        operations,
                        &mut actions,
                    )?;
                    NormalResultCleanup::Value {
                        computation: *computation,
                        actions,
                    }
                }
                checked_trees::CheckedNormalResultCompletion::Dispatch { computation, arms } => {
                    let mut results = Vec::new();
                    for arm in control.normal_result_arms.span_or_empty(*arms) {
                        let child = normal_result_cleanup(
                            identities, claims, checked, source, exit, plan, arm.result, catalog,
                            parameters, evaluation, operations,
                        )?;
                        results.push((
                            checked
                                .facts
                                .values
                                .scalar_computations
                                .dispatch_arms
                                .get(arm.arm)
                                .clone(),
                            child,
                        ));
                    }
                    NormalResultCleanup::Dispatch {
                        computation: *computation,
                        arms: results,
                    }
                }
            })
        }

        fn result_storage(
            operations: &OperationBuffer,
            binding: u32,
        ) -> Result<StructuralValueStorage, LoweringError> {
            let (_, result) = operations
                .structural_values
                .iter()
                .find(|(ordinal, _)| *ordinal == binding)
                .ok_or(LoweringError::Unsupported(
                    "cleanup result is not established",
                ))?;
            Ok(result.into())
        }

        #[allow(clippy::too_many_arguments)]
        fn lower_regions<'actions>(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            claims: &[(
                language_semantics::PermissionClaimIdentity,
                semantic_vocabulary::ClaimId,
            )],
            checked: &CheckedTrees,
            actions: impl IntoIterator<Item = &'actions checked_trees::CheckedNormalCleanupAction>,
            partition_refs: &[checked_trees::CheckedCleanupPartitionRef],
            selected_regions: &[checked_trees::CheckedUnitStructuralArgumentPlan],
            catalog: &mut CompletionCleanupCatalog<'_, '_, '_>,
            parameters: &[StructuralParameterDeclaration],
            evaluation: &Evaluation,
            operations: &OperationBuffer,
        ) -> Result<Vec<CleanupRegion>, LoweringError> {
            crate::unit::unit_cleanup::lower_checked_cleanup_regions(
                identities,
                checked,
                actions,
                partition_refs,
                selected_regions,
                catalog.control,
                catalog.types,
                catalog.type_ids,
                |identities, source, path| {
                    let mut storage =
                        evaluation.cleanup_value_storage(source, parameters, |binding| {
                            result_storage(operations, binding)
                        })?;
                    storage.path.extend(crate::expression_preparation::bindings::structural_paths::lower_typed_argument_path(
                        identities, path, storage.selected_type, catalog.types, None, &mut std::iter::empty(), None,
                    )?);
                    Ok((storage.place, storage.path))
                },
                |place| evaluation.carrier_multiplicity(place, operations),
                |cleanup, place, structural_type, path| {
                    let mut action = crate::unit::unit_cleanup::lower_nominal_cleanup_action(
                        cleanup,
                        place,
                        structural_type,
                        path,
                        catalog.calls.machine_ids,
                        catalog.calls.cleanup_receivers,
                        catalog.calls.requirement_counts,
                        &mut catalog.calls.next_obligation_identity,
                    )?;
                    bind_consumed_claims(&mut action, &cleanup.consumed_claims, claims)?;
                    Ok(action)
                },
            )
        }
    }
    pub(crate) mod entry_requirements {
        use crate::shared::lowering_error::LoweringError;
        use crate::unit::runtime_requirements::lower_structural_runtime_requirement;
        use checked_trees::CheckedTrees;
        use semantic_vocabulary::Proposition;
        use symbols::SymbolHandle;
        use terminal_psi::{
            StructuralParameterDeclaration, StructuralTypeDeclaration, ValueDeclaration,
        };

        pub(crate) struct EntryRequirements {
            pub(crate) requires: Vec<Proposition>,
            pub(crate) runtime_requirements: Vec<Proposition>,
            pub(crate) source_checked_requires: Vec<Proposition>,
        }

        pub(crate) fn lower(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            scalar_applications: &super::scalar_applications::SourceScalarApplications,
            source: SymbolHandle,
            scalar_parameters: &[ValueDeclaration],
            erased_scalar_parameters: &[ValueDeclaration],
            predicate_parameters: &[StructuralParameterDeclaration],
            structural_types: &[StructuralTypeDeclaration],
        ) -> Result<EntryRequirements, LoweringError> {
            let contract = checked.facts.contract_plans.for_machine(source).ok_or(
                LoweringError::Unsupported("callable entry has no canonical checked contract"),
            )?;
            let mut requires = Vec::new();
            let mut runtime_requirements = Vec::new();
            if contract.crash.structural_runtime_requirements().is_some()
                || contract.crash.uses_structural_proof_gated_arithmetic()
            {
                let checked_requirements = contract.crash.structural_runtime_requirements().ok_or(
                    LoweringError::Unsupported(
                        "proof-gated structural arithmetic lacks a complete checked requirement package",
                    ),
                )?;
                crate::unit::runtime_requirements::validate_runtime_memberships(checked, source)?;
                runtime_requirements = checked_requirements
                    .iter()
                    .map(|requirement| {
                        lower_structural_runtime_requirement(
                            identities,
                            requirement,
                            scalar_parameters,
                            erased_scalar_parameters,
                            predicate_parameters,
                            structural_types,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(crate::shared::lowering_error::unit_machine_plan_error(
                        checked,
                        checked.symbols.name(source),
                        source,
                    ))?;
            }
            let contextual = &checked.facts.contextual_cleanup.entry_requirements;
            for (_, requirement) in contextual.rows.iter() {
                if requirement.machine != source {
                    continue;
                }
                let expression = requirement.predicate();
                runtime_requirements.push(lower_structural_runtime_requirement(
                    identities,
                    &expression,
                    scalar_parameters,
                    erased_scalar_parameters,
                    predicate_parameters,
                    structural_types,
                )?);
            }
            crate::scalar_lowering::contracts::canonicalize_requires(&mut runtime_requirements)?;
            let authored_requires = contract.closed_scalar_values.authored_requires();
            if authored_requires.iter().all(Option::is_some) {
                let view_roster: Vec<(u32, StructuralParameterDeclaration)> = predicate_parameters
                    .iter()
                    .map(|parameter| (parameter.position, parameter.clone()))
                    .collect();
                let views = crate::scalar_lowering::contracts::ContractViewNamespace {
                    scalar_applications,
                    constant_values: &checked.typed.constant_recipe_closure.values,
                    result: crate::scalar_lowering::contracts::ContractStructuralResult::Absent,
                    parameters: &view_roster,
                    structural_types,
                };
                requires = authored_requires
                    .iter()
                    .map(|clause| {
                        crate::scalar_lowering::contracts::clauses(
                            identities,
                            std::slice::from_ref(clause),
                            scalar_parameters,
                            erased_scalar_parameters,
                            &views,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .flatten()
                    .filter(|clause| *clause != Proposition::Truth)
                    .collect();
            }
            let mut source_checked_requires = requires.clone();
            normalize_requires(&mut source_checked_requires)?;
            requires.extend(runtime_requirements.iter().cloned());
            normalize_requires(&mut requires)?;
            Ok(EntryRequirements {
                requires,
                runtime_requirements,
                source_checked_requires,
            })
        }

        fn normalize_requires(requires: &mut Vec<Proposition>) -> Result<(), LoweringError> {
            let mut pending = std::mem::take(requires);
            pending.reverse();
            while let Some(requirement) = pending.pop() {
                match requirement {
                    Proposition::Conjunction(conjuncts) => {
                        pending.extend(conjuncts.into_iter().rev());
                    }
                    other => requires.push(other),
                }
            }
            crate::scalar_lowering::contracts::canonicalize_requires(requires)
        }
    }
    pub(crate) mod folded_constants {
        use super::{
            CheckedTrees, CheckedUnitProviderCandidate, LoweringError, MachineId, lookup_machine_id,
        };
        use terminal_psi::{
            FoldedConstantCandidate, FoldedConstantClosure, FoldedConstantDependency,
            ProviderCandidateConformance, TerminalIdentityTableBuilder,
        };

        pub(crate) fn lower(
            checked: &CheckedTrees,
            identities: &mut TerminalIdentityTableBuilder,
            exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
            sources: &[CheckedUnitProviderCandidate],
            candidates: &[ProviderCandidateConformance],
            machines: &[(symbols::SymbolHandle, MachineId)],
        ) -> Result<FoldedConstantClosure, LoweringError> {
            let typed = &checked.typed;
            let closure = &typed.constant_recipe_closure;
            if closure.receipts.is_empty() {
                return Ok(Default::default());
            }
            if closure.target_scope.is_empty() {
                return Err(LoweringError::Unsupported(
                    "folded constants lost their primary target closure",
                ));
            }
            let mut dependencies = Vec::new();
            for receipt in &closure.receipts {
                let (requirement, candidate) = if receipt.provider_machine.is_valid() {
                    let mut matching = sources.iter().filter(|source| {
                        source.boundary == receipt.requirement
                            && source.provider == receipt.provider_declaration
                            && source.candidate == receipt.provider_machine
                    });
                    let source = matching.next().ok_or(LoweringError::Unsupported(
                        "folded constant lost its exact selected provider candidate",
                    ))?;
                    if matching.next().is_some() {
                        return Err(LoweringError::Unsupported(
                            "folded constant provider candidate is ambiguous",
                        ));
                    }
                    let machine = lookup_machine_id(machines, receipt.provider_machine)?;
                    let declaration = crate::machine_lowering::declarations::nominal(
                        identities,
                        checked,
                        exact_toolchain_sources,
                        receipt.provider_declaration,
                    )
                    .map_err(LoweringError::Unsupported)?;
                    let identity = typed.callable_identity_text(source.requirement_identity);
                    let mut matching = candidates.iter().filter(|candidate| {
                        candidate.candidate == machine
                            && candidate.provider_declaration == declaration
                            && identities.text(candidate.requirement_identity)
                                == Some(identity.as_str())
                    });
                    let row = matching.next().ok_or(LoweringError::Unsupported(
                        "folded constant has no retained canonical provider association",
                    ))?;
                    if matching.next().is_some() {
                        return Err(LoweringError::Unsupported(
                            "folded constant canonical association is ambiguous",
                        ));
                    }
                    (
                        row.source_requirement_identity,
                        Some(FoldedConstantCandidate {
                            boundary: row.boundary,
                            machine: row.candidate,
                            declaration: row.provider_declaration.clone(),
                        }),
                    )
                } else {
                    if receipt.provider_declaration.is_valid() {
                        return Err(LoweringError::Unsupported(
                            "intrinsic fold carries a checked provider declaration without its body",
                        ));
                    }
                    let identity = if let Some((owner, signature)) =
                        typed.trait_machine_signature(receipt.requirement)
                    {
                        typed.trait_requirement_callable_identity(owner, signature)
                    } else if let Some(operator) =
                        typed.operator_definition_by_symbol(receipt.requirement)
                    {
                        typed.boundary_operator_requirement_callable_identity(operator)
                    } else if let Some(machine) = typed.machine_by_symbol(receipt.requirement) {
                        typed.machine_callable_identity(machine)
                    } else {
                        return Err(LoweringError::Unsupported(
                            "intrinsic fold lost its exact requirement declaration",
                        ));
                    };
                    (
                        identities.intern(&typed.callable_identity_text(identity)),
                        None,
                    )
                };
                dependencies.push(FoldedConstantDependency {
                    target: receipt.target_scope.clone(),
                    requirement,
                    plan: receipt.plan_identity,
                    candidate,
                });
            }
            dependencies.sort();
            dependencies.dedup();
            Ok(FoldedConstantClosure {
                target: Some(closure.target_scope.clone()),
                dependencies,
            })
        }
    }
    pub(crate) mod native_layouts {

        use super::{CheckedTrees, LoweringError, unsupported};
        use semantic_vocabulary::{StructuralFieldId, StructuralTypeId};
        use terminal_psi::{
            NativeLayoutCatalog, NativeLayoutFieldIdentity, NativeLayoutRealization,
            TargetLayoutInput, TerminalIdentityTableBuilder,
        };
        use typed_trees::type_identity::TypeIdentityId;

        pub(crate) struct NativeRecordProjection<'checked> {
            pub(crate) source_declaration: symbols::SymbolHandle,
            pub(crate) source_application: &'checked typed_trees::SourceTypeApplication,
            pub(crate) id: StructuralTypeId,
            pub(crate) source_identity: TypeIdentityId,
            pub(crate) native_identity: Option<&'checked str>,
            pub(crate) policy_identity: Option<&'checked str>,
            pub(crate) semantic_identity: Option<&'checked str>,
            pub(crate) fields: Vec<NativeFieldProjection<'checked>>,
        }

        pub(crate) struct NativeFieldProjection<'checked> {
            pub(crate) field: StructuralFieldId,
            pub(crate) native_identity: Option<&'checked str>,
            pub(crate) declaration: symbols::SymbolHandle,
            pub(crate) relevance: terminal_psi::BindingRelevance,
        }

        pub(crate) fn lower(
            checked: &CheckedTrees,
            records: &[NativeRecordProjection<'_>],
            identities: &mut TerminalIdentityTableBuilder,
        ) -> Result<
            (
                NativeLayoutCatalog,
                Vec<typed_trees::SourceRecordCorrespondence>,
            ),
            LoweringError,
        > {
            let mut catalog = NativeLayoutCatalog::default();
            let mut source_records = Vec::with_capacity(records.len());
            for record in records {
                let source_identity = record.source_identity;
                let fields = record
                    .fields
                    .iter()
                    .map(|field| NativeLayoutFieldIdentity {
                        field: field.field,
                        declaration_identity: field
                            .native_identity
                            .map(|identity| identities.intern(identity))
                            .unwrap_or_default(),
                    })
                    .collect::<Vec<_>>();
                let mut candidates =
                    checked
                        .typed
                        .plan_laid_layouts
                        .iter()
                        .enumerate()
                        .filter(|(_, plan)| {
                            checked.typed.normalized_type_identity(plan.application)
                                == source_identity
                        });
                let planned_coordinate = candidates.next();
                let planned = planned_coordinate.map(|(_, plan)| plan);
                if candidates.next().is_some() {
                    return unsupported("native record has multiple checked layout applications");
                }
                let policy = match planned {
                    Some(_) => identities.intern(record.policy_identity.ok_or(
                        LoweringError::Unsupported(
                            "planned native record lost its checked policy identity",
                        ),
                    )?),
                    None => Default::default(),
                };
                let mut realizations = Vec::new();
                let mut source_realizations = Vec::new();
                if let Some(plan) = planned {
                    match &plan.realizations {
                        typed_trees::PlanLaidRealizations::Invariant(geometry) => {
                            source_realizations.push(
                                typed_trees::SourceLayoutRealizationCoordinate {
                                    target: String::new(),
                                    member: Default::default(),
                                },
                            );
                            realizations.push(NativeLayoutRealization {
                                target: None,
                                report: geometry.native_layout_report(),
                            })
                        }
                        typed_trees::PlanLaidRealizations::TargetInputs(span) => {
                            let alternatives =
                                checked.typed.plan_laid_target_layouts.span(*span).ok_or(
                                    LoweringError::Unsupported(
                                        "native layout lost its target member storage",
                                    ),
                                )?;
                            for (member, alternative) in span.handles().zip(alternatives) {
                                source_realizations.push(
                                    typed_trees::SourceLayoutRealizationCoordinate {
                                        target: alternative.target.as_str().to_owned(),
                                        member,
                                    },
                                );
                                realizations.push(NativeLayoutRealization {
                                    target: Some(TargetLayoutInput {
                                        target: alternative.target.as_str().to_owned(),
                                        pointer_size: alternative.target_facts.pointer_size,
                                        pointer_alignment: alternative
                                            .target_facts
                                            .pointer_alignment,
                                        case_tag_size: alternative.target_facts.case_tag_size,
                                        case_tag_alignment: alternative
                                            .target_facts
                                            .case_tag_alignment,
                                        little_endian: alternative.target_facts.little_endian,
                                    }),
                                    report: alternative.geometry.native_layout_report(),
                                });
                            }
                            realizations.sort_by(|left, right| {
                                left.target
                                    .as_ref()
                                    .map(|target| &target.target)
                                    .cmp(&right.target.as_ref().map(|target| &target.target))
                            });
                        }
                    }
                }
                let source_fields = record
                    .fields
                    .iter()
                    .map(|field| {
                        let schema = match planned {
                            Some(plan) => {
                                match plan
                                    .field_symbols
                                    .iter()
                                    .position(|candidate| *candidate == field.declaration)
                                {
                                    Some(ordinal) => *plan.schema_field_symbols.get(ordinal).ok_or(
                                        LoweringError::Unsupported(
                                            "native review field lost its source schema correspondence",
                                        ),
                                    )?,
                                    None if field.relevance.is_erased() => symbols::SymbolHandle::invalid(),
                                    None => {
                                        return Err(LoweringError::Unsupported(
                                            "native review field lost its source layout owner",
                                        ));
                                    }
                                }
                            }
                            None => field.declaration,
                        };
                        Ok(typed_trees::SourceFieldCorrespondence {
                            field: field.field,
                            declaration: field.declaration,
                            schema,
                        })
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                source_records.push(typed_trees::SourceRecordCorrespondence {
                    record: record.id,
                    declaration: record.source_declaration,
                    application: record.source_application.clone(),
                    storage_identity: checked
                        .typed
                        .type_identity_text(record.source_identity)
                        .ok_or(LoweringError::Unsupported(
                            "native record lost its exact storage identity",
                        ))?,
                    schema: planned.map_or(record.source_declaration, |plan| plan.schema_symbol),
                    policy: planned
                        .map_or_else(symbols::SymbolHandle::invalid, |plan| plan.policy_symbol),
                    layout: planned_coordinate
                        .map(|(ordinal, plan)| {
                            Ok(typed_trees::SourceLayoutPlanCoordinate {
                                ordinal: u32::try_from(ordinal).map_err(|_| {
                                    LoweringError::Unsupported(
                                        "native source layout roster exceeds u32",
                                    )
                                })?,
                                application: plan.application,
                            })
                        })
                        .transpose()?,
                    fields: source_fields,
                    realizations: source_realizations,
                });
                catalog.insert(
                    record.id,
                    record
                        .native_identity
                        .map(|identity| identities.intern(identity))
                        .unwrap_or_default(),
                    policy,
                    record
                        .semantic_identity
                        .map(|identity| identities.intern(identity))
                        .unwrap_or_default(),
                    fields,
                    realizations,
                );
            }
            Ok((catalog, source_records))
        }
    }
    pub(crate) mod providers {

        use super::bodies::UnitPlans;
        use super::operation_frame::BoundaryParameters;
        use super::{
            BoundaryMachineDeclaration, CheckedTrees, CheckedUnitProviderCandidate, LoweringError,
            MachineId, ProviderCandidateConformance, ProviderParameterRefinement,
            ProviderRefinement, ProviderSignature, ProviderSignatureParameter, TerminalMachine,
            UnitBody, lookup_machine_id, unique_unit_boundary, unsupported,
        };
        use crate::expression_preparation::computation_graph::boundary_targets;
        use checked_trees::machine::Machine;

        pub(crate) mod selection_coordinates {

            use super::{CheckedTrees, CheckedUnitProviderCandidate, LoweringError};
            use terminal_psi::{TerminalIdentityIndex, TerminalIdentityTableBuilder};

            pub(crate) struct ReceivingApplication {
                pub(crate) declaration_application: TerminalIdentityIndex,
                pub(crate) application: TerminalIdentityIndex,
                pub(crate) regions: Vec<TerminalIdentityIndex>,
            }

            pub(crate) fn intern_receiving_application(
                identities: &mut TerminalIdentityTableBuilder,
                checked: &CheckedTrees,
                candidate: &CheckedUnitProviderCandidate,
            ) -> Result<ReceivingApplication, LoweringError> {
                intern_prepared_receiving_application(
                    identities,
                    &checked.facts.provider_instance_publication,
                    candidate.receiving,
                )
            }

            pub(crate) fn intern_prepared_receiving_application(
                identities: &mut TerminalIdentityTableBuilder,
                publication: &checked_trees::CheckedProviderInstancePublication,
                receiving: arena::Handle<checked_trees::CheckedProviderReceivingApplication>,
            ) -> Result<ReceivingApplication, LoweringError> {
                if !receiving.is_valid() {
                    return Ok(ReceivingApplication {
                        declaration_application: TerminalIdentityIndex::default(),
                        application: TerminalIdentityIndex::default(),
                        regions: Vec::new(),
                    });
                }
                let receiving = publication.receivers.get(receiving);
                let receiving = receiving
                    .description
                    .as_ref()
                    .map_err(|reason| LoweringError::InvalidBoundaryApplication(reason.clone()))?;
                intern_receiving_description(identities, publication, receiving)
            }

            fn intern_receiving_description(
                identities: &mut TerminalIdentityTableBuilder,
                publication: &checked_trees::CheckedProviderInstancePublication,
                receiving: &checked_trees::CheckedProviderReceivingDescription,
            ) -> Result<ReceivingApplication, LoweringError> {
                let regions = publication.regions.span(receiving.regions).ok_or_else(|| {
                    LoweringError::InvalidBoundaryApplication(
                        "provider lost its checked receiving region publication".into(),
                    )
                })?;
                Ok(ReceivingApplication {
                    declaration_application: identities.intern(&receiving.declaration_application),
                    application: identities.intern(&receiving.application),
                    regions: regions
                        .iter()
                        .map(|region| identities.intern(region))
                        .collect(),
                })
            }

            pub(crate) fn intern(
                identities: &mut TerminalIdentityTableBuilder,
                checked: &CheckedTrees,
                candidate: &CheckedUnitProviderCandidate,
            ) -> Result<
                (
                    TerminalIdentityIndex,
                    TerminalIdentityIndex,
                    Option<semantic_vocabulary::PackageKeyIdentity>,
                ),
                LoweringError,
            > {
                let typed = &checked.typed;
                let requirement_template = immediate_template(checked, candidate.boundary)?;
                let requirement = if let Some(application) =
                    typed.closed_requirement_application(candidate.boundary)
                {
                    let (owner, signature) = typed
                        .trait_machine_signature(application.template)
                        .ok_or(LoweringError::Unsupported(
                            "closed provider requirement lost its original signature",
                        ))?;
                    typed.trait_requirement_callable_identity_text(owner, signature)
                } else if requirement_template.is_valid() {
                    let machine =
                        typed
                            .machine(requirement_template)
                            .ok_or(LoweringError::Unsupported(
                                "closed provider requirement lost its specialization template",
                            ))?;
                    typed.machine_callable_identity_text(machine).ok_or(
                        LoweringError::Unsupported(
                            "provider requirement template has no callable identity",
                        ),
                    )?
                } else {
                    typed.callable_identity_text(candidate.requirement_identity)
                };
                let realization_template = immediate_template(checked, candidate.candidate)?;
                let realization_symbol = if realization_template.is_valid() {
                    realization_template
                } else {
                    candidate.candidate
                };
                let machine =
                    typed
                        .machine(realization_symbol)
                        .ok_or(LoweringError::Unsupported(
                            "closed provider realization lost its specialization template",
                        ))?;
                let realization = typed.machine_callable_identity_text(machine).ok_or(
                    LoweringError::Unsupported(
                        "provider realization template has no callable identity",
                    ),
                )?;
                if requirement.is_empty() || realization.is_empty() {
                    return Err(LoweringError::Unsupported(
                        "provider selection coordinate has an empty original callable identity",
                    ));
                }
                Ok((
                    identities.intern(&requirement),
                    identities.intern(&realization),
                    typed.symbols.symbol_package_identity(realization_symbol),
                ))
            }

            fn immediate_template(
                checked: &CheckedTrees,
                instance: symbols::SymbolHandle,
            ) -> Result<symbols::SymbolHandle, LoweringError> {
                let mut receipts = checked
                    .typed
                    .machine_specializations
                    .iter()
                    .filter(|receipt| receipt.instance == instance);
                let template = receipts
                    .next()
                    .map_or(symbols::SymbolHandle::default(), |receipt| receipt.template);
                if receipts.next().is_some() {
                    return Err(LoweringError::Unsupported(
                        "provider callable has more than one specialization receipt",
                    ));
                }
                Ok(template)
            }
        }

        pub(crate) fn instance_requirements(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
            boundary_parameters: &[BoundaryParameters],
        ) -> Result<Vec<terminal_psi::ProviderInstanceRequirements>, LoweringError> {
            let mut templates = Vec::new();
            let publication = &checked.facts.provider_instance_publication;
            for (_, obligation) in publication.requirements.iter() {
                if boundary_parameters.iter().any(|boundary| {
                    boundary.source == obligation.template || boundary.source == obligation.boundary
                }) && !templates.contains(&obligation.template)
                {
                    templates.push(obligation.template);
                }
            }
            let mut requirements: Vec<terminal_psi::ProviderInstanceRequirements> = Vec::new();
            for (_, obligation) in publication.requirements.iter() {
                if !templates.contains(&obligation.template) {
                    continue;
                }
                let declaration = obligation
                    .declaration
                    .as_ref()
                    .map_err(|reason| LoweringError::InvalidBoundaryApplication(reason.clone()))?;
                let template = intern_callable(identities, checked, declaration.template_identity);
                let package = declaration.package;
                let selecting_declaration = if declaration.selecting_declaration.is_valid() {
                    Some(
                        crate::machine_lowering::declarations::nominal(
                            identities,
                            checked,
                            exact_toolchain_sources,
                            declaration.selecting_declaration,
                        )
                        .map_err(|reason| {
                            LoweringError::InvalidDeclarationIdentity(reason.to_owned())
                        })?,
                    )
                } else {
                    None
                };
                let source_candidate_identity =
                    intern_callable(identities, checked, declaration.source_candidate_identity);
                let receiving = selection_coordinates::intern_prepared_receiving_application(
                    identities,
                    publication,
                    declaration.receiving,
                )?;
                let required = terminal_psi::RequiredProviderInstance {
                    requirement_identity: intern_callable(
                        identities,
                        checked,
                        declaration.requirement_identity,
                    ),
                    candidate_identity: intern_callable(
                        identities,
                        checked,
                        declaration.candidate_identity,
                    ),
                    declaration_application: receiving.declaration_application,
                    receiving_application: receiving.application,
                    receiving_region_arguments: receiving.regions,
                };
                match requirements.iter_mut().find(|row| {
                    row.template == template
                        && row.package == package
                        && row.selecting_declaration == selecting_declaration
                        && row.source_candidate_identity == source_candidate_identity
                }) {
                    Some(row) => row.required_instances.push(required),
                    None => requirements.push(terminal_psi::ProviderInstanceRequirements {
                        template,
                        package,
                        selecting_declaration,
                        source_candidate_identity,
                        required_instances: vec![required],
                    }),
                }
            }
            for row in &mut requirements {
                row.required_instances.sort();
                row.required_instances.dedup();
            }
            requirements.sort_by(|left, right| {
                (
                    left.template,
                    left.package,
                    &left.selecting_declaration,
                    left.source_candidate_identity,
                )
                    .cmp(&(
                        right.template,
                        right.package,
                        &right.selecting_declaration,
                        right.source_candidate_identity,
                    ))
            });
            Ok(requirements)
        }

        fn callable_candidate<'program>(
            checked: &CheckedTrees,
            plans: UnitPlans<'program>,
            machine: &Machine,
        ) -> Result<UnitBody<'program>, LoweringError> {
            UnitBody::find(plans, machine.symbol).map_err(
                crate::shared::lowering_error::unit_machine_plan_error(
                    checked,
                    machine.name.as_str(),
                    machine.symbol,
                ),
            )
        }

        #[derive(Clone, Copy)]
        enum CatalogRequirement<'a> {
            Trait {
                definition: &'a checked_trees::trait_definition::TraitDefinition,
                signature: &'a checked_trees::signature::StateSignature,
            },
            TopLevel(&'a checked_trees::machine::Machine),
            Operator(&'a typed_trees::operator::OperatorDefinition),
        }

        pub(crate) fn checked_unit_provider_candidates(
            checked: &CheckedTrees,
            plans: UnitPlans<'_>,
            closure: &[symbols::SymbolHandle],
            retained_requirements: &mut Vec<symbols::SymbolHandle>,
            known: &mut Vec<(symbols::SymbolHandle, Vec<CheckedUnitProviderCandidate>)>,
        ) -> Result<Vec<CheckedUnitProviderCandidate>, LoweringError> {
            let bodies = closure
                .iter()
                .map(|symbol| UnitBody::find(plans, *symbol))
                .collect::<Result<Vec<_>, LoweringError>>()?;
            let mut boundary_symbols = bodies
                .into_iter()
                .flat_map(UnitBody::operations)
                .filter_map(|operation| {
                    operation
                        .call()
                        .filter(|call| call.is_boundary())
                        .map(|call| call.target.machine)
                })
                .chain(retained_requirements.iter().copied())
                .collect::<Vec<_>>();
            for machine in closure {
                boundary_symbols.extend(boundary_targets(checked, *machine)?);
            }
            let demanded_traits = checked
                .typed
                .traits()
                .iter()
                .filter(|definition| {
                    definition.is_boundary
                        && checked
                            .typed
                            .executable_trait_machine_instances(definition)
                            .any(|signature| boundary_symbols.contains(&signature.symbol))
                })
                .collect::<Vec<_>>();
            for definition in demanded_traits {
                typed_trees::trait_definition::AppliedTraitScope::abstract_scope(definition).visit(
                    &checked.typed,
                    &mut |scope| {
                        boundary_symbols.extend(
                            checked
                                .typed
                                .executable_trait_machine_instances(scope.declaration)
                                .map(|signature| signature.symbol),
                        );
                    },
                );
            }
            boundary_symbols.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
            boundary_symbols.dedup();
            *retained_requirements = boundary_symbols.clone();
            let mut output = Vec::new();
            for boundary_symbol in boundary_symbols {
                let index = match known
                    .iter()
                    .position(|(boundary, _)| *boundary == boundary_symbol)
                {
                    Some(index) => index,
                    None => {
                        let candidates = boundary_candidates(checked, plans, boundary_symbol)?;
                        known.push((boundary_symbol, candidates));
                        known.len() - 1
                    }
                };
                output.extend(known[index].1.iter().cloned());
            }
            output.sort_by(|left, right| {
                (
                    left.boundary.arena_index(),
                    left.boundary.generation(),
                    &left.provider_identity,
                    left.provider.arena_index(),
                    left.provider.generation(),
                    left.candidate.arena_index(),
                    left.candidate.generation(),
                )
                    .cmp(&(
                        right.boundary.arena_index(),
                        right.boundary.generation(),
                        &right.provider_identity,
                        right.provider.arena_index(),
                        right.provider.generation(),
                        right.candidate.arena_index(),
                        right.candidate.generation(),
                    ))
            });
            if output.windows(2).any(|pair| {
                pair[0].boundary == pair[1].boundary
                    && pair[0].provider_identity == pair[1].provider_identity
                    && pair[0].provider == pair[1].provider
                    && pair[0].candidate == pair[1].candidate
            }) {
                return unsupported("Unit provider catalog contains a duplicate exact candidate");
            }
            Ok(output)
        }

        fn boundary_candidates(
            checked: &CheckedTrees,
            plans: UnitPlans<'_>,
            boundary_symbol: symbols::SymbolHandle,
        ) -> Result<Vec<CheckedUnitProviderCandidate>, LoweringError> {
            let mut output = Vec::new();
            let boundary = unique_unit_boundary(plans, boundary_symbol).map_err(
                crate::shared::lowering_error::unit_machine_plan_error(
                    checked,
                    checked.symbols.name(boundary_symbol),
                    boundary_symbol,
                ),
            )?;
            let exact_requirements = checked
                .typed
                .traits()
                .iter()
                .flat_map(|definition| {
                    checked
                        .typed
                        .executable_trait_machine_instances(definition)
                        .filter(move |signature| signature.symbol == boundary_symbol)
                        .map(move |signature| (definition, signature))
                })
                .collect::<Vec<_>>();
            let requirement = match exact_requirements.as_slice() {
                [(definition, signature)] => CatalogRequirement::Trait {
                    definition,
                    signature,
                },
                [] => {
                    match checked
                        .typed
                        .machines_with_symbol(boundary_symbol)
                        .find(|machine| {
                            machine.supply_mode
                                == language_semantics::MachineSupplyMode::TopLevelRequirement
                        }) {
                        Some(requirement) => CatalogRequirement::TopLevel(requirement),
                        None => match checked.typed.operators().iter().find(|operator| {
                            operator.is_boundary && operator.symbol == boundary_symbol
                        }) {
                            Some(operator) => CatalogRequirement::Operator(operator),
                            None => return Ok(Vec::new()),
                        },
                    }
                }
                _ => {
                    return unsupported(
                        "Unit boundary provider catalog requires one exact trait/signature symbol coordinate",
                    );
                }
            };
            let requirement_identity = match requirement {
                CatalogRequirement::Trait {
                    definition,
                    signature,
                } => checked
                    .typed
                    .trait_requirement_callable_identity(definition, signature),
                CatalogRequirement::TopLevel(requirement) => {
                    checked.typed.machine_callable_identity(requirement)
                }
                CatalogRequirement::Operator(operator) => checked
                    .typed
                    .boundary_operator_requirement_callable_identity(operator),
            };
            if !requirement_identity.is_valid() {
                return unsupported("Unit boundary requirement has an empty overload identity");
            }
            for (_, published) in checked
                .facts
                .provider_instance_publication
                .candidates
                .iter()
                .filter(|(_, published)| published.boundary == boundary_symbol)
            {
                let declaration = published
                    .declaration
                    .as_ref()
                    .map_err(|reason| LoweringError::InvalidBoundaryApplication(reason.clone()))?;
                let machine = checked.typed.machine(published.candidate).ok_or(
                    LoweringError::Unsupported("checked provider candidate lost its exact machine"),
                )?;
                match &boundary.result {
                    checked_trees::CheckedBoundaryMachineResultPlan::Unit => {
                        let candidate = callable_candidate(checked, plans, machine)?;
                        if candidate.result()? != checked_trees::CheckedControlResultPlan::Unit {
                            return unsupported(
                                "provider result disagrees with its Unit boundary requirement",
                            );
                        }
                    }
                    checked_trees::CheckedBoundaryMachineResultPlan::Structural {
                        type_identity,
                        multiplicity,
                        qualifications,
                        projected_qualifications,
                    } => {
                        let candidate = callable_candidate(checked, plans, machine)?;
                        let checked_trees::CheckedControlResultPlan::Structural(result) =
                            candidate.result()?
                        else {
                            return unsupported(
                                "provider result disagrees with its structural boundary requirement",
                            );
                        };
                        if result.type_identity != *type_identity
                            || result.multiplicity != *multiplicity
                            || projected_qualifications
                                .iter()
                                .any(|row| !result.projected_qualifications.contains(row))
                            || result
                                .qualifications
                                .iter()
                                .any(|domain| !qualifications.contains(domain))
                        {
                            return unsupported(
                                "provider structural result disagrees with its boundary requirement",
                            );
                        }
                    }
                    checked_trees::CheckedBoundaryMachineResultPlan::Scalar(expected) => {
                        let candidate = callable_candidate(checked, plans, machine)?;
                        if candidate.scalar_result_type() != Some(*expected) {
                            return unsupported(
                                "provider result disagrees with its scalar boundary requirement",
                            );
                        }
                    }
                }
                {
                    let provider = declaration.provider;
                    if !provider.is_valid() {
                        return unsupported("provider candidate has no exact declaration owner");
                    }
                    output.push(CheckedUnitProviderCandidate {
                        provider,
                        boundary: boundary_symbol,
                        candidate: machine.symbol,
                        requirement_identity,
                        provider_identity: checked.typed.symbols.display_path(provider, "::"),
                        receiving: declaration.receiving,
                        candidate_identity: checked.typed.machine_callable_identity(machine),
                    });
                }
            }
            Ok(output)
        }

        pub(crate) fn conformances(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &checked_trees::CheckedTrees,
            exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
            candidates: &[CheckedUnitProviderCandidate],
            boundary_parameters: &[BoundaryParameters],
            boundary_machines: &[BoundaryMachineDeclaration],
            machine_ids: &[(symbols::SymbolHandle, MachineId)],
            machines: &[TerminalMachine],
        ) -> Result<Vec<ProviderCandidateConformance>, LoweringError> {
            let mut conformances = candidates
                .iter()
                .map(|candidate| {
                    let BoundaryParameters {
                        id: boundary,
                        structural: parameters,
                        scalar: scalar_parameters,
                        ..
                    } = boundary_parameters
                        .iter()
                        .find(|boundary| boundary.source == candidate.boundary)
                        .ok_or(LoweringError::Unsupported(
                            "provider candidate references an unlowered Unit boundary requirement",
                        ))?;
                    let terminal_candidate = lookup_machine_id(machine_ids, candidate.candidate)?;
                    let realized = machines
                        .iter()
                        .find(|machine| machine.id == terminal_candidate)
                        .expect("provider candidate root was lowered as an ordinary terminal machine");
                    if scalar_parameters.len() != realized.parameters.len()
                        || scalar_parameters
                            .iter()
                            .zip(&realized.parameters)
                            .any(|(boundary, candidate)| boundary.scalar_type != candidate.scalar_type)
                    {
                        return unsupported(
                            "provider candidate scalar signature disagrees with its boundary requirement",
                        );
                    }
                    let provider_identity = identities.intern(&candidate.provider_identity);
                    let (
                        source_requirement_identity,
                        source_candidate_identity,
                        source_candidate_package_identity,
                    ) = selection_coordinates::intern(identities, checked, candidate)?;
                    let receiving = selection_coordinates::intern_receiving_application(
                        identities, checked, candidate,
                    )?;
                    Ok(ProviderCandidateConformance {
                        boundary: *boundary,
                        source_requirement_identity,
                        declaration_application: receiving.declaration_application,
                        receiving_application: receiving.application,
                        receiving_region_arguments: receiving.regions,
                        source_candidate_identity,
                        source_candidate_package_identity,
                        requirement_identity: intern_callable(
                            identities,
                            checked,
                            candidate.requirement_identity,
                        ),
                        provider_identity,
                        provider_declaration: crate::machine_lowering::declarations::nominal(
                            identities,
                            checked,
                            exact_toolchain_sources,
                            candidate.provider,
                        )
                        .map_err(LoweringError::Unsupported)?,
                        candidate_identity: intern_callable(
                            identities,
                            checked,
                            candidate.candidate_identity,
                        ),
                        candidate: terminal_candidate,
                        signature: ProviderSignature {
                            parameters: parameters
                                .iter()
                                .map(|parameter| ProviderSignatureParameter {
                                    position: parameter.position,
                                    structural_type: parameter.structural_type,
                                    multiplicity: parameter.multiplicity,
                                    access: parameter.access,
                                    qualifications: parameter.qualifications.clone(),
                                    projected_qualifications: parameter.projected_qualifications.clone(),
                                })
                                .collect(),
                        },
                        refinement: ProviderRefinement {
                            positional_parameters: (0..parameters.len())
                                .map(|index| {
                                    let index = u32::try_from(index).map_err(|_| {
                                        LoweringError::Unsupported("provider signature arity exceeds u32")
                                    })?;
                                    Ok(ProviderParameterRefinement {
                                        boundary_index: index,
                                        candidate_index: index,
                                    })
                                })
                                .collect::<Result<Vec<_>, LoweringError>>()?,
                            required_domains: boundary_machines
                                .iter()
                                .find(|declaration| declaration.id == *boundary)
                                .expect("lowered provider boundary declaration exists")
                                .requires
                                .clone(),
                            realized_service_ceiling: realized.published_service_ceiling.clone(),
                        },
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            conformances.sort_by(|left, right| {
                (
                    left.boundary,
                    identities.text(left.provider_identity).unwrap_or_default(),
                    identities
                        .text(left.provider_declaration.identity())
                        .unwrap_or_default(),
                    identities.text(left.candidate_identity).unwrap_or_default(),
                    left.candidate,
                )
                    .cmp(&(
                        right.boundary,
                        identities.text(right.provider_identity).unwrap_or_default(),
                        identities
                            .text(right.provider_declaration.identity())
                            .unwrap_or_default(),
                        identities
                            .text(right.candidate_identity)
                            .unwrap_or_default(),
                        right.candidate,
                    ))
            });
            Ok(conformances)
        }

        fn intern_callable(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            identity: typed_trees::type_identity::CallableIdentityId,
        ) -> terminal_psi::TerminalIdentityIndex {
            identities.intern(&checked.typed.callable_identity_text(identity))
        }
    }
    pub(crate) mod scalar_applications {

        use crate::emission::scalar_types::terminal_scalar_type;
        use crate::shared::checked_facts::flow_facts;
        use crate::shared::lowering_error::LoweringError;
        use checked_trees::{CheckedScalarComputationKind, CheckedTrees};
        use semantic_vocabulary::{ScalarApplicationId, ScalarType, StructuralTypeId};
        use terminal_psi::{LayoutMeasurement, ScalarApplication, ScalarApplications};
        use typed_trees::constant_recipes::{
            ConstantEvaluationEntry, ConstantRecipeHandle, RetainedConstantPosition,
        };
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::type_identity::TypeIdentityId;

        pub(crate) mod conditions {

            use super::SourceScalarApplications;
            use crate::shared::lowering_error::LoweringError;
            use crate::unit::attached_unit::{BoundaryParameters, target_families};
            use checked_trees::{CheckedTrees, NominalMachineUseSite};
            use lowered_psi::{LoweredSourceCallOccurrence, LoweredSourceCallSite};
            use numerics::bignum::BigInt;
            use numerics::integer_bounds::IntegerBoundStep;
            use numerics::scalar_predicates::{PredicateNode, PredicateOperator};
            use semantic_vocabulary::{MachineId, ScalarApplicationId, ScalarConditionRole};
            use symbols::SymbolHandle;
            use terminal_psi::{
                ScalarConditionSubject, ScalarConditions, ScalarRangeObservation,
                ScalarRangeRequirement, TargetMachineCallable, TerminalMachine,
            };
            use typed_trees::expression::ExpressionHandle;

            pub(crate) struct Condition {
                owner: SymbolHandle,
                expression: ExpressionHandle,
                subject: Subject,
                requirement: Requirement,
            }

            enum Requirement {
                Completion(ScalarApplicationId),
                Range {
                    role: ScalarConditionRole,
                    requirement: ScalarRangeRequirement,
                    steps: Vec<IntegerBoundStep<Observation>>,
                },
                Predicate {
                    role: ScalarConditionRole,
                    description: String,
                    nodes: Vec<PredicateNode<usize, Observation>>,
                    root: usize,
                },
            }

            pub(crate) fn completion(
                checked: &CheckedTrees,
                applications: &mut SourceScalarApplications,
                owner: SymbolHandle,
                application: ScalarApplicationId,
            ) -> Result<Condition, LoweringError> {
                let expression = ExpressionHandle::invalid();
                let subject = subject(checked, applications, owner).map_err(|reason| {
                    LoweringError::UnpublishedScalarCondition {
                        owner,
                        expression,
                        reason,
                    }
                })?;
                Ok(Condition {
                    owner,
                    expression,
                    subject,
                    requirement: Requirement::Completion(application),
                })
            }

            enum Subject {
                Module,
                Declaration { owner: SymbolHandle },
                Application(ScalarApplicationId),
            }

            #[derive(Clone)]
            enum Observation {
                Literal(BigInt),
                Application(ScalarApplicationId),
                EntryCall {
                    application: ScalarApplicationId,
                    entry: SymbolHandle,
                    owner: SymbolHandle,
                    expression: ExpressionHandle,
                },
            }

            impl Condition {
                fn error(&self, reason: String) -> LoweringError {
                    LoweringError::UnpublishedScalarCondition {
                        owner: self.owner,
                        expression: self.expression,
                        reason,
                    }
                }
            }

            pub(crate) fn prepare(
                checked: &CheckedTrees,
                applications: &mut SourceScalarApplications,
            ) -> Result<Vec<Condition>, LoweringError> {
                let mut ranges = Vec::new();
                for obligation in &checked.facts.integer_range_obligations {
                    if !obligation.is_pending() {
                        continue;
                    }
                    let owner = obligation.owner();
                    let expression = obligation.expression();
                    let failure = |reason| LoweringError::UnpublishedScalarCondition {
                        owner,
                        expression,
                        reason,
                    };
                    let subject = subject(checked, applications, owner).map_err(&failure)?;
                    let steps = obligation
                        .map_steps(&mut |expression, owner| {
                            observation(checked, applications, expression, owner)
                        })
                        .map_err(failure)?;
                    let requirement = match obligation.fixed_limits() {
                        Some((minimum, maximum)) => ScalarRangeRequirement::Fixed {
                            minimum: minimum.clone(),
                            maximum: maximum.clone(),
                        },
                        None => ScalarRangeRequirement::AddressModel,
                    };
                    ranges.push(Condition {
                        owner,
                        expression,
                        subject,
                        requirement: Requirement::Range {
                            role: obligation.role(),
                            requirement,
                            steps,
                        },
                    });
                }
                for obligation in &checked.facts.constant_predicate_obligations {
                    if !obligation.is_pending() {
                        continue;
                    }
                    let owner = obligation.caller();
                    let expression = ExpressionHandle::invalid();
                    let failure = |reason| LoweringError::UnpublishedScalarCondition {
                        owner,
                        expression,
                        reason,
                    };
                    let subject = subject(checked, applications, owner).map_err(&failure)?;
                    let mut nodes = Vec::new();
                    let root = obligation
                        .map_nodes(
                            &mut |expression, owner| {
                                observation(checked, applications, expression, owner)
                            },
                            &mut |node| {
                                let ordinal = nodes.len();
                                nodes.push(node);
                                Ok(ordinal)
                            },
                        )
                        .map_err(failure)?;
                    ranges.push(Condition {
                        owner,
                        expression,
                        subject,
                        requirement: Requirement::Predicate {
                            role: obligation.role(),
                            description: obligation.description().to_owned(),
                            nodes,
                            root,
                        },
                    });
                }
                for obligation in &checked.facts.constant_recipe_obligations {
                    if !obligation.is_pending() {
                        continue;
                    }
                    let owner = obligation.owner();
                    let expression = ExpressionHandle::invalid();
                    let failure = |reason| LoweringError::UnpublishedScalarCondition {
                        owner,
                        expression,
                        reason,
                    };
                    let subject = if owner.is_valid() {
                        subject(checked, applications, owner).map_err(&failure)?
                    } else {
                        Subject::Module
                    };
                    let left = recipe_observation(
                        checked,
                        applications,
                        obligation.recipe(),
                        obligation.recipe_owner(),
                    )
                    .map_err(&failure)?;
                    use checked_trees::ConstantRecipeRequirement;
                    let (operator, right) = match obligation.requirement() {
                        ConstantRecipeRequirement::ExceedsBound(bound) => (
                            PredicateOperator::Greater,
                            Observation::Literal(bound.clone()),
                        ),
                        ConstantRecipeRequirement::AtLeast(bound) => (
                            PredicateOperator::GreaterOrEqual,
                            Observation::Literal(bound.clone()),
                        ),
                        ConstantRecipeRequirement::AtMost(bound) => (
                            PredicateOperator::LessOrEqual,
                            Observation::Literal(bound.clone()),
                        ),
                        ConstantRecipeRequirement::EqualRecipe(other) => (
                            PredicateOperator::Equal,
                            recipe_observation(
                                checked,
                                applications,
                                *other,
                                obligation.recipe_owner(),
                            )
                            .map_err(failure)?,
                        ),
                    };
                    ranges.push(Condition {
                        owner,
                        expression,
                        subject,
                        requirement: Requirement::Predicate {
                            role: obligation.role(),
                            description: obligation.description().to_owned(),
                            nodes: vec![
                                PredicateNode::Leaf(left),
                                PredicateNode::Leaf(right),
                                PredicateNode::Binary {
                                    operator,
                                    left: 0,
                                    right: 1,
                                },
                            ],
                            root: 2,
                        },
                    });
                }
                Ok(ranges)
            }

            fn recipe_observation(
                checked: &CheckedTrees,
                applications: &mut SourceScalarApplications,
                recipe: typed_trees::constant_recipes::ConstantRecipeHandle,
                owner: SymbolHandle,
            ) -> Result<Observation, String> {
                if let Some(value) = checked.typed.closed_constant_recipe_value(recipe) {
                    return Ok(Observation::Literal(value.clone()));
                }
                applications
                    .retain_recipe_in(checked, recipe, owner)
                    .map(Observation::Application)
                    .map_err(|error| error.to_string())
            }

            fn subject(
                checked: &CheckedTrees,
                applications: &mut SourceScalarApplications,
                owner: SymbolHandle,
            ) -> Result<Subject, String> {
                let program = &checked.typed;
                if let Some(entry) = program
                    .machine_by_own_or_state_symbol(owner)
                    .and_then(|machine| program.constant_evaluation_entry(machine.symbol))
                {
                    return applications
                        .retain_entry(checked, entry)
                        .map(Subject::Application)
                        .map_err(|error| error.to_string());
                }
                if checked
                    .facts
                    .proof_applications
                    .machines
                    .iter()
                    .any(|(_, machine)| {
                        machine.machine == owner
                            || checked
                                .facts
                                .proof_applications
                                .symbols
                                .span_or_empty(machine.states)
                                .contains(&owner)
                    })
                {
                    return Ok(Subject::Declaration { owner });
                }
                if program.const_declaration_by_symbol(owner).is_some()
                    || program.data_definition_by_symbol(owner).is_some()
                {
                    return Ok(Subject::Declaration { owner });
                }
                let plan = &checked.facts.proof_applications;
                if plan
                    .symbols
                    .span_or_empty(plan.declarations)
                    .contains(&owner)
                {
                    return Ok(Subject::Declaration { owner });
                }
                Err("scalar condition has no retained declaration or evaluation owner".to_owned())
            }

            fn observation(
                checked: &CheckedTrees,
                applications: &mut SourceScalarApplications,
                expression: ExpressionHandle,
                owner: SymbolHandle,
            ) -> Result<Observation, String> {
                let program = &checked.typed;
                if let Some(value) = program.closed_integer_expression_value_in(expression, owner) {
                    return Ok(Observation::Literal(value));
                }
                if checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .layout_observations
                    .iter()
                    .any(|observation| {
                        observation.expression == expression
                            && (!owner.is_valid() || observation.owner == owner)
                    })
                {
                    return applications
                        .retain_expression_in(checked, expression, owner)
                        .map(Observation::Application)
                        .map_err(|error| error.to_string());
                }
                if let Some(entry) = program
                    .machine_by_own_or_state_symbol(owner)
                    .and_then(|machine| program.constant_evaluation_entry(machine.symbol))
                {
                    let application = applications
                        .retain_entry(checked, entry)
                        .map_err(|error| error.to_string())?;
                    return Ok(Observation::EntryCall {
                        application,
                        entry: entry.entry,
                        owner,
                        expression,
                    });
                }
                applications
                    .retain_expression_in(checked, expression, owner)
                    .map(Observation::Application)
                    .map_err(|error| error.to_string())
            }

            pub(crate) struct EmittedScalars<'a> {
                pub(crate) checked: &'a CheckedTrees,
                pub(crate) identities: &'a mut terminal_psi::TerminalIdentityTableBuilder,
                pub(crate) sources: &'a [(source::SourceId, [u8; 32])],
                pub(crate) calls: &'a [LoweredSourceCallOccurrence],
                pub(crate) machines: &'a [TerminalMachine],
            }

            pub(crate) fn publish(
                ranges: &[Condition],
                machines: &[(SymbolHandle, MachineId)],
                boundaries: &[BoundaryParameters],
                declarations: &[(SymbolHandle, terminal_psi::ProofDeclarationId)],
                emitted: EmittedScalars<'_>,
            ) -> Result<ScalarConditions, LoweringError> {
                let mut result = ScalarConditions::default();
                for range in ranges {
                    let subject = match range.subject {
                        Subject::Module => ScalarConditionSubject::Module,
                        Subject::Declaration { owner: source, .. } => ScalarConditionSubject::Declaration(
                            declarations
                                .iter()
                                .find(|(owner, _)| *owner == source)
                                .map(|(_, identity)| *identity)
                                .ok_or_else(|| {
                                    range.error(
                                        "scalar condition lost its retained declaration coordinate".into(),
                                    )
                                })?,
                        ),
                        Subject::Application(application) => ScalarConditionSubject::Application(application),
                    };
                    match &range.requirement {
                        Requirement::Completion(application) => {
                            result.retain_completion(subject, *application)
                        }
                        Requirement::Range {
                            role,
                            requirement,
                            steps,
                        } => {
                            let steps = steps
                                .iter()
                                .map(|step| {
                                    step.map_observation(&mut |observation| {
                                        publish_observation(
                                            observation,
                                            machines,
                                            boundaries,
                                            &emitted,
                                        )
                                    })
                                })
                                .collect::<Result<Vec<_>, _>>()
                                .map_err(|reason| range.error(reason))?;
                            result
                                .retain_range(subject, *role, requirement.clone(), steps)
                                .map_err(|reason| range.error(reason.to_owned()))?;
                        }
                        Requirement::Predicate {
                            role,
                            description,
                            nodes,
                            root,
                        } => {
                            let mut published = arena::Arena::new();
                            let mut coordinates = Vec::new();
                            for node in nodes {
                                let mapped = node
                                    .try_map(
                                        &mut |reference| {
                                            coordinates.get(reference).copied().ok_or_else(|| {
                                                "scalar predicate lost a preceding node".to_owned()
                                            })
                                        },
                                        &mut |observation| {
                                            publish_observation(
                                                observation,
                                                machines,
                                                boundaries,
                                                &emitted,
                                            )
                                        },
                                    )
                                    .map_err(|reason| range.error(reason))?;
                                coordinates.push(
                                    published.insert(terminal_psi::ScalarPredicateNode(mapped)),
                                );
                            }
                            let root = coordinates.get(*root).copied().ok_or_else(|| {
                                range.error("scalar predicate lost its root".to_owned())
                            })?;
                            result
                                .retain_predicate(
                                    subject,
                                    *role,
                                    description.clone(),
                                    published,
                                    root,
                                )
                                .map_err(|reason| range.error(reason.to_owned()))?;
                        }
                    }
                }
                Ok(result)
            }

            fn publish_observation(
                observation: &Observation,
                machines: &[(SymbolHandle, MachineId)],
                boundaries: &[BoundaryParameters],
                emitted: &EmittedScalars<'_>,
            ) -> Result<ScalarRangeObservation, String> {
                let (application, entry, owner, expression) = match *observation {
                    Observation::Literal(ref value) => {
                        return Ok(ScalarRangeObservation::Literal(value.clone()));
                    }
                    Observation::Application(application) => {
                        return Ok(ScalarRangeObservation::Application(application));
                    }
                    Observation::EntryCall {
                        application,
                        entry,
                        owner,
                        expression,
                    } => (application, entry, owner, expression),
                };
                let callable = target_families::callable(entry, machines, boundaries)
                    .map_err(|error| error.to_string())?;
                let TargetMachineCallable::Machine(machine) = callable else {
                    return Err(
                        "scalar observation's complete evaluation entry has no ordinary body"
                            .to_owned(),
                    );
                };
                let body = emitted
                    .machines
                    .iter()
                    .find(|body| body.id == machine)
                    .ok_or_else(|| "scalar observation lost its emitted entry body".to_owned())?;
                let mut occurrences = emitted.calls.iter().filter(|call| {
                    let Some(site) = call.origin.statement() else {
                        return false;
                    };
                    call.source_site == Some(NominalMachineUseSite::Expression(expression))
                        && (site.state == owner
                            || (owner == entry
                                && emitted
                                    .checked
                                    .typed
                                    .machine_by_own_or_state_symbol(site.state)
                                    .is_some_and(|machine| machine.symbol == entry)))
                });
                let occurrence = occurrences.next().ok_or_else(|| {
                    "scalar observation lost its exact emitted call occurrence".to_owned()
                })?;
                if occurrences.next().is_some() {
                    return Err(
                        "scalar observation maps to multiple emitted call occurrences".to_owned(),
                    );
                }
                let LoweredSourceCallSite::Operation(operation) = occurrence.terminal_site else {
                    return Err(
                        "scalar observation names a call without normal completion".to_owned()
                    );
                };
                let value = body
                    .operations()
                    .find(|emitted| emitted.id == operation)
                    .and_then(|operation| operation.result.as_scalar())
                    .ok_or_else(|| {
                        "scalar observation's call has no scalar result in its entry body"
                            .to_owned()
                    })?;
                if value.scalar_type.as_integer().is_none() {
                    return Err(
                        "scalar observation's exact call result is not an integer".to_owned()
                    );
                }
                Ok(ScalarRangeObservation::EntryResult {
                    application,
                    machine,
                    result: value.id,
                    scalar_type: value.scalar_type,
                })
            }

            #[cfg(test)]
            mod tests {
                use super::*;
                use checked_trees::{
                    CheckFacts, ConstantRecipeObligation, ConstantRecipeRequirement,
                };
                use numerics::scalar_predicates::ScalarValue;
                use typed_trees::constant_recipes::RetainedConstantRecipe;

                #[test]
                fn scalar_condition_publication_keeps_the_exact_state_owner() {
                    let machine = SymbolHandle::from_parts(1, 1);
                    let state = SymbolHandle::from_parts(2, 1);
                    let mut facts = CheckFacts::default();
                    let states = facts.proof_applications.symbols.insert_many([state]);
                    facts.proof_applications.machines.insert(
                        semantic_vocabulary::proof_applications::ProofApplicationMachine {
                            machine,
                            states,
                            ..Default::default()
                        },
                    );
                    let checked =
                        CheckedTrees::with_roots(typed_trees::TypedTrees::default(), facts);
                    let mut applications = SourceScalarApplications::default();
                    assert!(
                        matches!(subject(&checked, &mut applications, state).unwrap(),
                        Subject::Declaration { owner, .. } if owner == state)
                    );
                }

                #[test]
                fn scalar_condition_publication_retains_recipe_relations_and_explicit_module_scope()
                {
                    let mut typed = typed_trees::TypedTrees::default();
                    let first = typed
                        .constant_recipes
                        .insert(RetainedConstantRecipe::default());
                    let second = typed
                        .constant_recipes
                        .insert(RetainedConstantRecipe::default());
                    let first_application = ScalarApplicationId::new(1).unwrap();
                    let second_application = ScalarApplicationId::new(2).unwrap();
                    let mut facts = CheckFacts::default();
                    let roles = [
                        ScalarConditionRole::Result,
                        ScalarConditionRole::Prerequisite,
                        ScalarConditionRole::Prerequisite,
                        ScalarConditionRole::Result,
                    ];
                    for (requirement, role) in [
                        ConstantRecipeRequirement::EqualRecipe(second),
                        ConstantRecipeRequirement::ExceedsBound(BigInt::from_u64(23)),
                        ConstantRecipeRequirement::AtLeast(BigInt::from_u64(24)),
                        ConstantRecipeRequirement::AtMost(BigInt::from_u64(24)),
                    ]
                    .into_iter()
                    .zip(roles)
                    {
                        facts
                            .constant_recipe_obligations
                            .push(ConstantRecipeObligation::retain(
                                SymbolHandle::invalid(),
                                role,
                                first,
                                requirement,
                                source::SourceSpan::default(),
                                "test recipe condition".into(),
                            ));
                    }
                    let checked = CheckedTrees::with_roots(typed, facts);
                    let mut applications = SourceScalarApplications::default();
                    applications.recipes.extend([
                        (first, SymbolHandle::invalid(), first_application),
                        (second, SymbolHandle::invalid(), second_application),
                    ]);
                    let prepared = prepare(&checked, &mut applications).unwrap();
                    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
                    let conditions = publish(
                        &prepared,
                        &[],
                        &[],
                        &[],
                        EmittedScalars {
                            checked: &checked,
                            identities: &mut identities,
                            sources: &[],
                            calls: &[],
                            machines: &[],
                        },
                    )
                    .unwrap();
                    assert_eq!(conditions.predicates().len(), 4);
                    for (condition, role) in conditions.predicates().iter().zip(roles) {
                        assert_eq!(condition.subject, ScalarConditionSubject::Module);
                        assert_eq!(condition.role, role);
                    }
                    for (value, expected) in [
                        (23, [true, false, false, true]),
                        (24, [true, true, true, true]),
                        (25, [true, true, true, false]),
                    ] {
                        for (condition, expected) in conditions.predicates().iter().zip(expected) {
                            let result = condition.evaluate(&mut |observation| match observation {
                                ScalarRangeObservation::Application(_) => {
                                    Some(ScalarValue::Integer(BigInt::from_u64(value)))
                                }
                                _ => panic!("literals retain their own value"),
                            });
                            assert_eq!(result, Some(ScalarValue::Boolean(expected)));
                        }
                    }
                    for (second_value, expected) in [
                        (Some(24), Some(true)),
                        (Some(16), Some(false)),
                        (None, None),
                    ] {
                        let mut observed = Vec::new();
                        let value = conditions.predicates()[0].evaluate(&mut |observation| {
                            let ScalarRangeObservation::Application(application) = observation
                            else {
                                panic!("expected application");
                            };
                            observed.push(application);
                            let value = if application == first_application {
                                Some(24)
                            } else {
                                second_value
                            }?;
                            Some(ScalarValue::Integer(BigInt::from_u64(value)))
                        });
                        assert_eq!(observed, vec![first_application, second_application]);
                        assert_eq!(value, expected.map(ScalarValue::Boolean));
                    }
                    assert!(
                        subject(&checked, &mut applications, SymbolHandle::from_parts(99, 1))
                            .is_err()
                    );
                    assert!(subject(&checked, &mut applications, SymbolHandle::invalid()).is_err());
                }
            }
        }
        pub(crate) mod proof_scope {
            use crate::machine_lowering::declarations;
            use crate::shared::lowering_error::LoweringError;
            use checked_trees::CheckedTrees;
            use semantic_vocabulary::proof_applications::ProofCallGraph;
            use symbols::SymbolHandle;
            use terminal_psi::{
                ProofApplicationScope, ProofDeclarationId, ProofScopeDeclaration,
                TerminalIdentityTableBuilder,
            };

            pub(crate) struct PublishedScope {
                pub scope: ProofApplicationScope,
                pub coordinates: Vec<(SymbolHandle, ProofDeclarationId)>,
            }

            pub(crate) fn publish(
                checked: &CheckedTrees,
                identities: &mut TerminalIdentityTableBuilder,
                sources: &[(source::SourceId, [u8; 32])],
                machines: &[(SymbolHandle, semantic_vocabulary::MachineId)],
                boundaries: &[super::super::operation_frame::BoundaryParameters],
            ) -> Result<PublishedScope, LoweringError> {
                let source = &checked.facts.proof_applications;
                let mut coordinates = Vec::new();
                let mut assigned = std::collections::BTreeMap::new();
                let mut descriptions = arena::Arena::default();
                let mut identity =
                    |symbol: SymbolHandle| -> Result<ProofDeclarationId, LoweringError> {
                        if !symbol.is_valid() {
                            return Ok(ProofDeclarationId::default());
                        }
                        let key = (symbol.arena_index(), symbol.generation());
                        if let Some(identity) = assigned.get(&key) {
                            return Ok(*identity);
                        }
                        let ordinal = u32::try_from(coordinates.len())
                            .ok()
                            .and_then(|position| position.checked_add(1))
                            .ok_or(LoweringError::Unsupported(
                                "proof declaration namespace is exhausted",
                            ))?;
                        let identity = ProofDeclarationId(ordinal);
                        let description = describe(checked, identities, sources, symbol);
                        descriptions
                            .insert(description.unwrap_or_else(ProofScopeDeclaration::Unavailable));
                        coordinates.push((symbol, identity));
                        assigned.insert(key, identity);
                        Ok(identity)
                    };
                let plan = source.try_map(&mut identity)?;
                let mut calls = ProofCallGraph::capture(
                    &checked.facts.operational,
                    source.machines.iter().map(|(_, row)| row.machine),
                    &mut identity,
                )?;
                retain_selected_algebra_calls(checked, &mut calls, &mut identity)?;
                calls.index_machines();
                let mut callables = arena::Arena::new();
                for &(source, body) in machines {
                    callables.insert(terminal_psi::ProofCallableAssociation::Callable {
                        declaration: identity(source)?,
                        callable: terminal_psi::TargetMachineCallable::Machine(body),
                    });
                }
                for boundary in boundaries {
                    callables.insert(terminal_psi::ProofCallableAssociation::Callable {
                        declaration: identity(boundary.source)?,
                        callable: terminal_psi::TargetMachineCallable::Boundary(boundary.id),
                    });
                }
                let mut headers = checked
                    .facts
                    .declaration_descriptions
                    .headers
                    .iter()
                    .map(|(_, header)| Ok((identity(header.machine)?, header)))
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                let (recursive_components, recursive_members, recursive_calls) =
                    retain_recursive_components(checked, &mut identity)?;
                let mut pending_evaluation_conditions = arena::Arena::new();
                for (_, owner) in checked
                    .facts
                    .declaration_descriptions
                    .evaluation_dependencies
                    .pending_conditions
                    .iter()
                {
                    pending_evaluation_conditions.insert(identity(*owner)?);
                }
                drop(identity);
                headers.sort_by_key(|(declaration, _)| *declaration);
                let mut callable_headers = arena::Arena::new();
                for (declaration, header) in headers {
                    let callable = if header.callable.is_valid() {
                        Ok(identities.intern(&checked.callable_identity_text(header.callable)))
                    } else {
                        Err("checked machine has no callable signature identity".to_owned())
                    };
                    let synchronous_invocations = header
                        .synchronous_invocations
                        .as_ref()
                        .map_err(Clone::clone)
                        .and_then(|span| {
                            let targets = checked
                                .facts
                                .declaration_descriptions
                                .invocation_targets
                                .span(*span)
                                .ok_or_else(|| {
                                    "checked declaration lost its retained invocation targets"
                                        .to_owned()
                                })?;
                            Ok(targets
                                .iter()
                                .map(|target| terminal_psi::DeclaredSynchronousBoundaryTarget {
                                    declaration: identities.intern(&target.declaration),
                                    package: target.package,
                                    application: target
                                        .application
                                        .as_ref()
                                        .map(|application| identities.intern(application)),
                                })
                                .collect())
                        });
                    callable_headers.insert(terminal_psi::ProofCallableHeader {
                        declaration,
                        callable,
                        package: header.package,
                        source_target: header.source_target.clone(),
                        product_eligible: header.product_eligible,
                        supplies_checked_body: header.supplies_checked_body,
                        type_parameter_count: header.type_parameter_count,
                        compiler_evaluation: header.compiler_evaluation.clone(),
                        synchronous_invocations,
                    });
                }
                Ok(PublishedScope {
                    scope: ProofApplicationScope {
                        declarations: descriptions,
                        callable_headers,
                        callables,
                        plan,
                        calls,
                        recursive_components,
                        recursive_members,
                        recursive_calls,
                        pending_evaluation_conditions,
                    },
                    coordinates,
                })
            }

            fn retain_selected_algebra_calls(
                checked: &CheckedTrees,
                graph: &mut ProofCallGraph<ProofDeclarationId>,
                identity: &mut impl FnMut(SymbolHandle) -> Result<ProofDeclarationId, LoweringError>,
            ) -> Result<(), LoweringError> {
                let operators = &checked.facts.operators;
                let mut selected = operators
                    .resolved_expression_selections()
                    .filter_map(|(expression, origin, _, _)| {
                        let owner = origin.machine_symbol()?;
                        let candidate =
                            operators.selected_trait_candidate_in_machine(expression, owner)?;
                        (candidate.realization_machine_symbol.is_valid()
                            && candidate.realization_state_symbol.is_valid())
                        .then_some((owner, candidate))
                    })
                    .collect::<Vec<_>>();
                selected.sort_by_key(|(owner, _)| (owner.arena_index(), owner.generation()));
                let mut position = 0;
                while position < selected.len() {
                    let owner = selected[position].0;
                    let end = position
                        + selected[position..]
                            .partition_point(|(candidate, _)| *candidate == owner);
                    let declaration = identity(owner)?;
                    if let Some((row, span)) = graph.machines().iter().find_map(|(handle, row)| {
                        (row.machine == declaration).then_some((handle, row.calls))
                    }) {
                        let mut calls = graph.calls.span_or_empty(span).to_vec();
                        for (_, candidate) in &selected[position..end] {
                            calls.push(
                                semantic_vocabulary::proof_applications::ProofApplicationCall {
                                    binder: ProofDeclarationId::default(),
                                    callee: identity(candidate.realization_machine_symbol)?,
                                    state: identity(candidate.realization_state_symbol)?,
                                    operator: identity(candidate.trait_requirement_symbol)?,
                                    arguments: arena::HandleSpan::empty(),
                                },
                            );
                        }
                        let calls = graph.calls.insert_many(calls);
                        graph.update_machine(row, |machine| machine.calls = calls);
                    }
                    position = end;
                }
                Ok(())
            }

            fn retain_recursive_components(
                checked: &CheckedTrees,
                identity: &mut impl FnMut(SymbolHandle) -> Result<ProofDeclarationId, LoweringError>,
            ) -> Result<
                (
                    arena::Arena<terminal_psi::ProofCheckedRecursiveComponent>,
                    arena::Arena<ProofDeclarationId>,
                    arena::Arena<terminal_psi::ProofCheckedRecursiveCall>,
                ),
                LoweringError,
            > {
                let source = &checked
                    .facts
                    .declaration_descriptions
                    .evaluation_dependencies;
                let mut components = arena::Arena::new();
                let mut members = arena::Arena::new();
                let mut calls = arena::Arena::new();
                for (_, component) in source.recursive_components.iter() {
                    let mapped_members = source
                        .recursive_members
                        .span_or_empty(component.members)
                        .iter()
                        .map(|body| identity(body.machine))
                        .collect::<Result<Vec<_>, _>>()?;
                    let mapped_calls = source
                        .recursive_calls
                        .span_or_empty(component.calls)
                        .iter()
                        .map(|call| {
                            Ok(terminal_psi::ProofCheckedRecursiveCall {
                                caller: identity(call.caller)?,
                                call_ordinal: call.call_ordinal,
                                callee: identity(call.callee)?,
                            })
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                    components.insert(terminal_psi::ProofCheckedRecursiveComponent {
                        members: members.insert_many(mapped_members),
                        calls: calls.insert_many(mapped_calls),
                    });
                }
                Ok((components, members, calls))
            }

            fn describe(
                checked: &CheckedTrees,
                identities: &mut TerminalIdentityTableBuilder,
                sources: &[(source::SourceId, [u8; 32])],
                symbol: SymbolHandle,
            ) -> Result<ProofScopeDeclaration, String> {
                if let Some(declaration) =
                    declarations::boundary_identity(identities, checked, sources, symbol)?
                {
                    return Ok(ProofScopeDeclaration::Callable(declaration));
                }
                if let Some(operator) = checked.typed.operator_definition_by_symbol(symbol)
                    && operator.is_boundary
                {
                    return declarations::operator_requirement(
                        identities,
                        checked,
                        sources,
                        symbol,
                        checked
                            .typed
                            .boundary_operator_requirement_callable_identity(operator),
                    )
                    .map(ProofScopeDeclaration::Operator)
                    .map_err(str::to_owned);
                }
                declarations::semantic_identity(identities, checked, symbol)
                    .map(ProofScopeDeclaration::Nominal)
            }
        }

        #[derive(Default)]
        pub(crate) struct SourceScalarApplications {
            rows: Vec<Application>,
            expressions: Vec<(ExpressionHandle, symbols::SymbolHandle, ScalarApplicationId)>,
            recipes: Vec<(
                ConstantRecipeHandle,
                symbols::SymbolHandle,
                ScalarApplicationId,
            )>,
            conditions: Vec<conditions::Condition>,
        }

        struct Application {
            definition: Definition,
            scalar_type: ScalarType,
        }

        #[derive(PartialEq, Eq)]
        enum Definition {
            Layout {
                subject: TypeIdentityId,
                measurement: LayoutMeasurement,
            },
            Evaluation {
                entry: symbols::SymbolHandle,
            },
        }

        impl SourceScalarApplications {
            pub(crate) const EMPTY: Self = Self {
                rows: Vec::new(),
                expressions: Vec::new(),
                recipes: Vec::new(),
                conditions: Vec::new(),
            };

            pub(crate) fn prepare(checked: &CheckedTrees) -> Result<Self, LoweringError> {
                let mut applications = Self::default();
                let references =
                    typed_trees::references::collect_program_references(&checked.typed);
                let owners =
                    typed_trees::references::collect_program_operand_owners(&checked.typed);
                for (recipe, declaration) in checked.typed.constant_recipes.iter() {
                    if checked.typed.closed_constant_recipe_value(recipe).is_some() {
                        continue;
                    }
                    let retained = match declaration.position {
                        RetainedConstantPosition::ArrayLength(reference)
                        | RetainedConstantPosition::ConstApplication {
                            site: reference, ..
                        } => references.type_references.contains(&reference),
                        RetainedConstantPosition::ContractExpression(expression) => {
                            references.expressions.contains(&expression)
                        }
                        RetainedConstantPosition::None => false,
                    };
                    if !retained {
                        continue;
                    }
                    if !declaration.contexts.is_empty() {
                        for context in checked
                            .typed
                            .constant_expression_contexts
                            .span_or_empty(declaration.contexts)
                        {
                            if checked
                                .typed
                                .machine_by_symbol(context.evaluation_entry)
                                .is_some()
                            {
                                let application = applications.retain_recipe_in(
                                    checked,
                                    recipe,
                                    context.owner,
                                )?;
                                let condition = conditions::completion(
                                    checked,
                                    &mut applications,
                                    context.owner,
                                    application,
                                )?;
                                applications.conditions.push(condition);
                            }
                        }
                    } else {
                        let declaring_owners = match declaration.position {
                            RetainedConstantPosition::ArrayLength(reference)
                            | RetainedConstantPosition::ConstApplication {
                                site: reference, ..
                            } => owners.type_owners(reference).collect::<Vec<_>>(),

                            _ => Vec::new(),
                        };
                        if declaring_owners.is_empty() {
                            return Err(LoweringError::Unsupported(
                                "live constant formation has no exact declaring owner",
                            ));
                        }
                        for owner in declaring_owners {
                            let application =
                                applications.retain_recipe_in(checked, recipe, owner)?;
                            let condition = conditions::completion(
                                checked,
                                &mut applications,
                                owner,
                                application,
                            )?;
                            applications.conditions.push(condition);
                        }
                    }
                }
                for declaration in checked.typed.const_declarations() {
                    if declaration.evaluated_value.is_some()
                        || !declaration.evaluation_entry.is_valid()
                    {
                        continue;
                    }
                    if !checked
                        .typed
                        .tables
                        .machines
                        .is_valid(declaration.evaluation_entry)
                    {
                        return Err(LoweringError::Unsupported(
                            "constant declaration has a stale evaluation entry",
                        ));
                    }
                    let entry = checked
                        .typed
                        .tables
                        .machines
                        .get(declaration.evaluation_entry)
                        .symbol;
                    let entry = checked.typed.constant_evaluation_entry(entry).ok_or(
                        LoweringError::Unsupported(
                            "constant declaration lost its evaluation entry",
                        ),
                    )?;
                    let application = applications.retain_entry(checked, entry)?;
                    let condition = conditions::completion(
                        checked,
                        &mut applications,
                        declaration.symbol,
                        application,
                    )?;
                    applications.conditions.push(condition);
                }
                for observation in &checked.facts.flow.terminal_unit_effects.layout_observations {
                    if !references.expressions.contains(&observation.expression)
                        || checked
                            .landed_integer_constant(observation.expression)
                            .is_some()
                    {
                        continue;
                    }
                    let application = applications.retain_expression_in(
                        checked,
                        observation.expression,
                        observation.owner,
                    )?;
                    let condition = conditions::completion(
                        checked,
                        &mut applications,
                        observation.owner,
                        application,
                    )?;
                    applications.conditions.push(condition);
                }
                for (_, computation) in checked.facts.values.scalar_computations.nodes.iter() {
                    if let CheckedScalarComputationKind::RetainedConstant { expression } =
                        computation.kind
                        && references.expressions.contains(&expression)
                        && checked.landed_integer_constant(expression).is_none()
                        && !applications
                            .expressions
                            .iter()
                            .any(|(source, _, _)| *source == expression)
                    {
                        applications.retain_expression_in(
                            checked,
                            expression,
                            symbols::SymbolHandle::invalid(),
                        )?;
                    }
                }
                let conditions = conditions::prepare(checked, &mut applications)?;
                applications.conditions.extend(conditions);
                Ok(applications)
            }

            fn retain_recipe_in(
                &mut self,
                checked: &CheckedTrees,
                recipe: ConstantRecipeHandle,
                owner: symbols::SymbolHandle,
            ) -> Result<ScalarApplicationId, LoweringError> {
                if let Some(application) = self
                    .recipes
                    .iter()
                    .find(|(source, scope, _)| *source == recipe && *scope == owner)
                    .map(|(_, _, application)| *application)
                {
                    return Ok(application);
                }
                if !checked.typed.constant_recipes.is_valid(recipe) {
                    return Err(LoweringError::Unsupported(
                        "scalar condition lost its exact recipe",
                    ));
                }
                let declaration = checked.typed.constant_recipes.get(recipe);
                let application = match declaration.position {
                    RetainedConstantPosition::ArrayLength(reference) => {
                        self.retain_array_length(checked, reference)?
                    }
                    RetainedConstantPosition::ContractExpression(expression)
                    | RetainedConstantPosition::ConstApplication {
                        root: expression, ..
                    } => self.retain_expression_in(checked, expression, owner)?,
                    RetainedConstantPosition::None => {
                        return Err(LoweringError::Unsupported(
                            "pending constant catalog contains an absent source position",
                        ));
                    }
                };
                self.recipes.push((recipe, owner, application));
                Ok(application)
            }

            fn retain_array_length(
                &mut self,
                checked: &CheckedTrees,
                reference: typed_trees::types::TypeReferenceHandle,
            ) -> Result<ScalarApplicationId, LoweringError> {
                let typed_trees::types::TypeReferenceNode::FixedArray {
                    length: typed_trees::types::FixedArrayLength::ConstCall { machine, .. },
                    ..
                } = checked.typed.type_reference_table.type_reference(reference)
                else {
                    return Err(LoweringError::Unsupported(
                        "pending array extent has no complete retained invocation",
                    ));
                };
                let entry =
                    checked
                        .typed
                        .machine_by_symbol(*machine)
                        .ok_or(LoweringError::Unsupported(
                            "pending array extent lost its exact invocation declaration",
                        ))?;
                let state = checked.typed.machine_states(entry).first().ok_or(
                    LoweringError::Unsupported(
                        "pending array extent invocation has no entry signature",
                    ),
                )?;
                if !checked.typed.state_parameters(state).is_empty() {
                    return Err(LoweringError::Unsupported(
                        "pending array extent invocation requires runtime arguments",
                    ));
                }
                let primitive = checked
                    .typed
                    .primitive_type_reference(state.return_type)
                    .ok_or(LoweringError::Unsupported(
                        "pending array extent invocation has no scalar result",
                    ))?;
                let scalar_type = terminal_scalar_type(primitive)?;
                if scalar_type.as_integer().is_none() {
                    return Err(LoweringError::Unsupported(
                        "pending array extent result is not an integer",
                    ));
                }
                self.insert(Definition::Evaluation { entry: *machine }, scalar_type)
            }

            fn retain_expression_in(
                &mut self,
                checked: &CheckedTrees,
                expression: ExpressionHandle,
                owner: symbols::SymbolHandle,
            ) -> Result<ScalarApplicationId, LoweringError> {
                if let Some(application) = self.expression_in(expression, owner) {
                    return Ok(application);
                }
                if let Some(entry) = checked
                    .typed
                    .constant_evaluation_for_expression(expression, owner)
                    .map_err(LoweringError::Unsupported)?
                {
                    let application = self.retain_entry(checked, entry)?;
                    self.expressions.push((expression, owner, application));
                    return Ok(application);
                }
                let (definition, scalar_type) = if flow_facts(checked)
                    .terminal_unit_effects
                    .layout_observations
                    .iter()
                    .any(|observation| {
                        observation.expression == expression
                            && (!owner.is_valid() || observation.owner == owner)
                    }) {
                    let mut observations = flow_facts(checked)
                        .terminal_unit_effects
                        .layout_observations
                        .iter()
                        .filter(|observation| {
                            observation.expression == expression
                                && (!owner.is_valid() || observation.owner == owner)
                        });
                    let observation = observations.next().ok_or(LoweringError::Unsupported(
                        "pending layout query has no checked structural subject",
                    ))?;
                    if observations.any(|other| {
                        other.subject != observation.subject
                            || other.measurement != observation.measurement
                    }) {
                        return Err(LoweringError::Unsupported(
                            "pending layout query has ambiguous lexical structural subjects",
                        ));
                    }
                    (
                        Definition::Layout {
                            subject: observation.subject,
                            measurement: match observation.measurement {
                                language_core::data_layout_queries::DataLayoutQueryKind::Size => {
                                    LayoutMeasurement::Size
                                }
                                language_core::data_layout_queries::DataLayoutQueryKind::Alignment => {
                                    LayoutMeasurement::Alignment
                                }
                            },
                        },
                        terminal_scalar_type(typed_trees::types::PrimitiveType::U64)?,
                    )
                } else {
                    let contexts = checked
                        .typed
                        .constant_recipes
                        .iter()
                        .filter(|(_, recipe)| recipe.position.expression() == expression)
                        .flat_map(|(_, recipe)| {
                            checked
                                .typed
                                .constant_expression_contexts
                                .span_or_empty(recipe.contexts)
                        })
                        .map(|context| {
                            format!(
                                "{} (entry {:?})",
                                checked.typed.symbols.name(context.owner),
                                context.evaluation_entry
                            )
                        })
                        .collect::<Vec<_>>();
                    return Err(LoweringError::UnpublishedScalarCondition {
                        owner,
                        expression,
                        reason: format!(
                            "pending constant expression has no retained complete evaluation entry in `{}`; retained contexts: {}",
                            checked.typed.symbols.name(owner),
                            contexts.join(", "),
                        ),
                    });
                };
                let application = self.insert(definition, scalar_type)?;
                self.expressions.push((expression, owner, application));
                Ok(application)
            }

            fn retain_entry(
                &mut self,
                checked: &CheckedTrees,
                entry: ConstantEvaluationEntry,
            ) -> Result<ScalarApplicationId, LoweringError> {
                let failures = checked.body_crash_summary(entry.entry);
                let refusal = match failures {
                    Some(summary) if summary.buckets().is_empty() => None,
                    Some(summary) => {
                        let causes = summary
                            .buckets()
                            .iter()
                            .map(|bucket| format!("{:?}", bucket.cause()))
                            .collect::<Vec<_>>()
                            .join(", ");
                        let calls = checked
                            .facts
                            .contract_plans
                            .for_machine(entry.entry)
                            .into_iter()
                            .flat_map(|plan| plan.crash.checked_calls())
                            .filter(|call| !call.surviving_buckets().is_empty())
                            .map(|call| checked.symbols.name(call.target_machine()))
                            .collect::<Vec<_>>()
                            .join(", ");
                        Some(format!(
                            "a checked crash route remains possible ({causes}); calls: [{calls}]"
                        ))
                    }
                    None => Some("the invocation has no complete checked crash summary".to_owned()),
                };
                semantic_vocabulary::compiler_evaluation::require_compiler_evaluation_failures(
                    checked.symbols.name(entry.entry),
                    refusal.as_deref().map_or(Ok(()), Err),
                )
                .map_err(LoweringError::CompilerEvaluationAdmission)?;
                let primitive = checked
                    .typed
                    .primitive_type_reference(entry.result_type)
                    .ok_or(LoweringError::Unsupported(
                        "constant entry has no exact scalar result carrier",
                    ))?;
                self.insert(
                    Definition::Evaluation { entry: entry.entry },
                    terminal_scalar_type(primitive)?,
                )
            }

            fn insert(
                &mut self,
                definition: Definition,
                scalar_type: ScalarType,
            ) -> Result<ScalarApplicationId, LoweringError> {
                if matches!(definition, Definition::Evaluation { entry } if !entry.is_valid()) {
                    return Err(LoweringError::Unsupported(
                        "pending scalar application has no exact evaluation entry",
                    ));
                }
                if let Some(ordinal) = self
                    .rows
                    .iter()
                    .position(|row| row.definition == definition && row.scalar_type == scalar_type)
                {
                    return Self::identity(ordinal);
                }
                let application = Self::identity(self.rows.len())?;
                self.rows.push(Application {
                    definition,
                    scalar_type,
                });
                Ok(application)
            }

            fn identity(ordinal: usize) -> Result<ScalarApplicationId, LoweringError> {
                u64::try_from(ordinal)
                    .ok()
                    .and_then(|length| length.checked_add(1))
                    .and_then(ScalarApplicationId::new)
                    .ok_or(LoweringError::Unsupported(
                        "scalar application namespace is exhausted",
                    ))
            }

            pub(crate) fn expression(
                &self,
                expression: ExpressionHandle,
            ) -> Option<ScalarApplicationId> {
                let mut matching = self
                    .expressions
                    .iter()
                    .filter(|(source, _, _)| *source == expression)
                    .map(|(_, _, application)| *application);
                let first = matching.next()?;
                matching.all(|other| other == first).then_some(first)
            }

            pub(crate) fn expression_in(
                &self,
                expression: ExpressionHandle,
                owner: symbols::SymbolHandle,
            ) -> Option<ScalarApplicationId> {
                self.expressions
                    .iter()
                    .find(|(source, scope, _)| *source == expression && *scope == owner)
                    .map(|(_, _, application)| *application)
                    .or_else(|| {
                        self.expressions
                            .iter()
                            .find(|(source, scope, _)| *source == expression && !scope.is_valid())
                            .map(|(_, _, application)| *application)
                    })
                    .or_else(|| {
                        (!owner.is_valid())
                            .then(|| self.expression(expression))
                            .flatten()
                    })
            }

            pub(crate) fn recipe(
                &self,
                recipe: ConstantRecipeHandle,
            ) -> Option<ScalarApplicationId> {
                let mut matching = self
                    .recipes
                    .iter()
                    .filter(|(source, _, _)| *source == recipe)
                    .map(|(_, _, application)| *application);
                let first = matching.next()?;
                matching.all(|other| other == first).then_some(first)
            }

            pub(crate) fn recipe_in(
                &self,
                recipe: ConstantRecipeHandle,
                owner: symbols::SymbolHandle,
            ) -> Option<ScalarApplicationId> {
                self.recipes
                    .iter()
                    .find(|(source, scope, _)| *source == recipe && *scope == owner)
                    .map(|(_, _, application)| *application)
                    .or_else(|| {
                        self.recipes
                            .iter()
                            .find(|(source, scope, _)| *source == recipe && !scope.is_valid())
                            .map(|(_, _, application)| *application)
                    })
                    .or_else(|| (!owner.is_valid()).then(|| self.recipe(recipe)).flatten())
            }

            pub(crate) fn scalar_type(
                &self,
                application: ScalarApplicationId,
            ) -> Option<ScalarType> {
                let ordinal = usize::try_from(application.get().checked_sub(1)?).ok()?;
                self.rows.get(ordinal).map(|row| row.scalar_type)
            }

            pub(crate) fn array_extent(
                &self,
                checked: &CheckedTrees,
                extent: typed_trees::constant_recipes::DeclaredArrayExtent,
            ) -> Result<terminal_psi::ArrayExtent, LoweringError> {
                if let Some(length) = extent.closed(&checked.typed) {
                    return Ok(terminal_psi::ArrayExtent::Literal(length));
                }
                let typed_trees::constant_recipes::DeclaredArrayExtent::Recipe(recipe) = extent
                else {
                    return Err(LoweringError::Unsupported(
                        "literal array extent failed to resolve",
                    ));
                };
                let application = self.recipe(recipe).ok_or(LoweringError::Unsupported(
                    "fixed array extent has no retained scalar application in this product",
                ))?;
                Ok(terminal_psi::ArrayExtent::Application(application))
            }

            pub(crate) fn evaluation_roots(
                &self,
            ) -> impl Iterator<Item = symbols::SymbolHandle> + '_ {
                self.rows.iter().filter_map(|row| match row.definition {
                    Definition::Evaluation { entry } => Some(entry),
                    _ => None,
                })
            }

            pub(crate) fn type_roots(&self) -> impl Iterator<Item = TypeIdentityId> + '_ {
                self.rows.iter().filter_map(|row| match row.definition {
                    Definition::Layout { subject, .. } => Some(subject),
                    _ => None,
                })
            }

            pub(crate) fn publish(
                &self,
                types: &[(TypeIdentityId, StructuralTypeId)],
                machines: &[(symbols::SymbolHandle, semantic_vocabulary::MachineId)],
                boundaries: &[super::operation_frame::BoundaryParameters],
                emitted: conditions::EmittedScalars<'_>,
            ) -> Result<
                (
                    ScalarApplications,
                    Vec<(symbols::SymbolHandle, terminal_psi::ProofDeclarationId)>,
                ),
                LoweringError,
            > {
                let mut result = ScalarApplications::default();
                let scope = proof_scope::publish(
                    emitted.checked,
                    emitted.identities,
                    emitted.sources,
                    machines,
                    boundaries,
                )?;
                result.proof_scope = scope.scope;
                for row in &self.rows {
                    let declaration = match row.definition {
                        Definition::Layout {
                            subject,
                            measurement,
                        } => ScalarApplication::LayoutObservation {
                            subject: super::lookup_type_id(types, &subject)?,
                            measurement,
                            semantics_version: LayoutMeasurement::SEMANTICS_VERSION,
                            scalar_type: row.scalar_type,
                        },
                        Definition::Evaluation { entry } => ScalarApplication::Evaluate {
                            entry: super::target_families::callable(entry, machines, boundaries)?,
                            scalar_type: row.scalar_type,
                        },
                    };
                    result
                        .insert(declaration)
                        .map_err(LoweringError::Unsupported)?;
                }
                result.conditions = conditions::publish(
                    &self.conditions,
                    machines,
                    boundaries,
                    &scope.coordinates,
                    emitted,
                )?;
                Ok((result, scope.coordinates))
            }
        }

        pub(crate) use conditions::EmittedScalars;
        #[cfg(test)]
        mod tests {
            use super::*;
            use typed_trees::constant_recipes::{
                ConstantExpressionContext, RetainedConstantRecipe, create_evaluation_entry,
            };
            use typed_trees::expression::ExpressionNode;
            use typed_trees::expression::{BinaryOperator, TableBinaryExpression};
            use typed_trees::name::Identifier;
            use typed_trees::types::TypeReferenceNode;

            fn shared_root() -> (
                typed_trees::TypedTrees,
                ConstantRecipeHandle,
                [ConstantExpressionContext; 2],
            ) {
                let mut program = typed_trees::TypedTrees::default();
                let mut symbols = symbols::SymbolTableBuilder::new();
                let source = symbols.insert_root(
                    symbols::SymbolKind::Machine,
                    symbols::SymbolNameRef::Static("source"),
                );
                program.symbols = symbols.finish();
                let carrier = program
                    .type_reference_table
                    .insert(TypeReferenceNode::Named {
                        symbol: symbols::SymbolHandle::invalid(),
                        name: Identifier::generated("u64"),
                    });
                let leaf = program.expression_table.insert(ExpressionNode::Integer(
                    numerics::literals::IntegerLiteral::from_parts(
                        false,
                        numerics::literals::IntegerRadix::Decimal,
                        "8",
                    )
                    .expect("integer"),
                ));
                let root = program.expression_table.insert(ExpressionNode::Binary(
                    TableBinaryExpression {
                        policy: numerics::arithmetic::ArithmeticDomain::Exact,
                        left: leaf,
                        operator: BinaryOperator::Add,
                        right: leaf,
                    },
                ));
                let contexts = ["first", "second"].map(|name| {
                    let owner =
                        create_evaluation_entry(&mut program, source, name.into(), root, carrier);
                    let owner_symbol = owner.symbol;
                    program.push_machine(owner);
                    let copied = program.expression_table.insert_copy(root);
                    let entry = create_evaluation_entry(
                        &mut program,
                        owner_symbol,
                        format!("{name}-constant"),
                        copied,
                        carrier,
                    );
                    let context = ConstantExpressionContext {
                        owner: owner_symbol,
                        result_type: carrier,
                        evaluation_entry: entry.symbol,
                        evaluation_expression: copied,
                    };
                    program.push_machine(entry);
                    context
                });
                let contexts_span = program.constant_expression_contexts.insert_many(contexts);
                let recipe = program.constant_recipes.insert(RetainedConstantRecipe {
                    position: RetainedConstantPosition::ContractExpression(root),
                    contexts: contexts_span,
                    ..Default::default()
                });
                (program, recipe, contexts)
            }

            fn checked_with_scope(program: typed_trees::TypedTrees) -> CheckedTrees {
                let mut facts = checked_trees::CheckFacts::default();
                for machine in program.machines() {
                    facts
                        .contract_plans
                        .machines_mut()
                        .push(checked_trees::MachineContractPlan {
                            machine: machine.symbol,
                            closed_scalar_values: Default::default(),
                            restored_declaration_predicates: Vec::new(),
                            crash: checked_trees::CrashPlan::default().with_checked_body_summary(
                                Some(checked_trees::CheckedBodyCrashSummary::new(
                                    machine.symbol,
                                    machine.states,
                                    Vec::new(),
                                )),
                            ),
                            report_fingerprint: 0,
                            commitment: checked_trees::MachineContractCommitment::from_digest(
                                [0; 32],
                            ),
                        });
                    facts.proof_applications.machines.insert(
                        checked_trees::ProofApplicationMachine {
                            machine: machine.symbol,
                            product_selected: true,
                            ..Default::default()
                        },
                    );
                }
                CheckedTrees::with_roots(program, facts)
            }

            #[test]
            fn constant_entries_keep_shared_expression_scopes_distinct() {
                let (program, recipe, contexts) = shared_root();
                let checked = checked_with_scope(program);
                let applications =
                    SourceScalarApplications::prepare(&checked).expect("scoped applications");
                let first = applications
                    .recipe_in(recipe, contexts[0].owner)
                    .expect("first scope");
                let second = applications
                    .recipe_in(recipe, contexts[1].owner)
                    .expect("second scope");
                assert_ne!(first, second);
                assert!(
                    applications.recipe(recipe).is_none(),
                    "unscoped selection cannot choose a context"
                );
                assert_eq!(
                    applications.evaluation_roots().collect::<Vec<_>>(),
                    contexts.map(|context| context.evaluation_entry)
                );
            }

            #[test]
            fn constant_entries_do_not_resurrect_a_pruned_context() {
                let (mut program, recipe, contexts) = shared_root();
                let retained = program
                    .machines()
                    .iter()
                    .filter(|machine| {
                        machine.symbol != contexts[1].owner
                            && machine.symbol != contexts[1].evaluation_entry
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                program.roots.machines = arena::HandleSpan::empty();
                for machine in retained {
                    program.push_machine(machine);
                }
                let checked = checked_with_scope(program);
                let applications =
                    SourceScalarApplications::prepare(&checked).expect("remaining application");
                assert!(applications.recipe_in(recipe, contexts[0].owner).is_some());
                assert!(applications.recipe_in(recipe, contexts[1].owner).is_none());
                assert_eq!(
                    applications.evaluation_roots().collect::<Vec<_>>(),
                    [contexts[0].evaluation_entry]
                );
            }
        }
    }
    pub(crate) mod signatures {

        use super::admission::AdmittedBody;
        use super::claims::{LoweredUnitClaims, lower_unit_entry_claims};
        use super::ordinary_calls;
        use super::parameters::{
            lower_unit_parameters, lower_unit_scalar_parameter_declarations,
            lower_unit_scalar_parameter_types,
        };
        use crate::expression_preparation::qualifications::PreparedScalarQualifications;
        use crate::shared::lowering_error::LoweringError;
        use crate::shared::lowering_error::unsupported;
        use crate::shared::terminal_identities::StructuralDomainBinding;
        use crate::shared::terminal_identities::allocate_dense;
        use crate::shared::terminal_identities::value_id;
        use checked_trees::CheckedTrees;

        use semantic_vocabulary::Proposition;

        use semantic_vocabulary::StructuralTypeId;
        use symbols::SymbolHandle;
        use terminal_psi::StructuralParameterDeclaration;
        use terminal_psi::StructuralTypeDeclaration;
        use terminal_psi::ValueDeclaration;

        #[derive(Clone)]
        pub(crate) struct MachineSignature {
            pub source: SymbolHandle,
            pub parameters: Vec<StructuralParameterDeclaration>,
            pub scalar_parameters: Vec<ValueDeclaration>,
            pub parameter_order: Vec<terminal_psi::BoundaryParameterKind>,
            pub scalar_result: Option<semantic_vocabulary::QualifiedScalarType>,
            pub erased_scalar_parameters: Vec<ValueDeclaration>,
            pub erased_proof_parameters: Vec<checked_trees::CheckedErasedProofParameterPlan>,
            pub predicate_parameters: Vec<StructuralParameterDeclaration>,
            pub(crate) claims: LoweredUnitClaims,
            pub cleanup_receiver: Option<semantic_vocabulary::PlaceId>,
            pub requires: Vec<Proposition>,
            pub runtime_requirements: Vec<Proposition>,
            pub source_checked_requires: Vec<Proposition>,
        }

        impl MachineSignature {
            pub(crate) fn call_target(&self) -> ordinary_calls::Target<'_> {
                ordinary_calls::Target {
                    parameters: &self.parameters,
                    scalar_parameters: &self.scalar_parameters,
                    erased_scalar_parameters: &self.erased_scalar_parameters,
                    predicate_parameters: &self.predicate_parameters,
                    requires: &self.requires,
                    runtime_requirements: &self.runtime_requirements,
                    source_checked_requires: &self.source_checked_requires,
                }
            }
        }

        pub(crate) fn find(
            signatures: &[MachineSignature],
            source: SymbolHandle,
        ) -> Result<&MachineSignature, LoweringError> {
            signatures
                .iter()
                .find(|signature| signature.source == source)
                .ok_or(LoweringError::Unsupported(
                    "call target has no allocated machine signature",
                ))
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn lower(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            scalar_applications: &super::scalar_applications::SourceScalarApplications,
            bodies: &[AdmittedBody<'_>],
            qualifications: &PreparedScalarQualifications,
            type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
            domain_ids: &[StructuralDomainBinding],
            structural_types: &[StructuralTypeDeclaration],
            next_place: &mut u64,
            next_value: &mut u64,
        ) -> Result<Vec<MachineSignature>, LoweringError> {
            let mut signatures = Vec::with_capacity(bodies.len());

            for admitted in bodies {
                let body = admitted.source();
                let plan = body.entry()?;
                if body.qualifications().iter().any(|domain| {
                    !plan
                        .structural_parameters
                        .iter()
                        .any(|parameter| parameter.qualifications.contains(domain))
                }) {
                    return unsupported(
                        "Unit body qualification is not represented by an exact structural parameter precondition",
                    );
                }
                let parameters = lower_unit_parameters(
                    identities,
                    plan.structural_parameters,
                    type_ids,
                    structural_types,
                    domain_ids,
                    next_place,
                )?;
                let scalar_parameters = lower_unit_scalar_parameter_declarations(
                    checked,
                    plan.state,
                    plan.scalar_parameters,
                    qualifications,
                    next_value,
                )?;
                let erased_scalar_parameters =
                    lower_unit_scalar_parameter_types(plan.erased_scalar_parameters, true)?
                        .into_iter()
                        .map(|scalar_type| {
                            Ok(ValueDeclaration {
                                qualifications: Default::default(),
                                id: value_id(allocate_dense(next_value)?),
                                scalar_type,
                            })
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                let scalar_result = body
                    .scalar_result_type()
                    .map(|primitive| {
                        super::parameters::unit_scalar_result_type(
                            checked,
                            plan.state,
                            crate::emission::scalar_types::terminal_scalar_type(primitive)?,
                            qualifications,
                        )
                    })
                    .transpose()?;
                let erased_proof_parameters = plan.erased_proof_parameters.to_vec();
                let claims = lower_unit_entry_claims(
                    identities,
                    plan.machine,
                    plan.state,
                    plan.entry_claims,
                    &parameters,
                    structural_types,
                )?;
                signatures.push(MachineSignature {
                    source: plan.machine,
                    parameters,
                    scalar_parameters,
                    parameter_order: super::parameters::lower_runtime_parameter_order(
                        plan.scalar_parameters,
                        plan.structural_parameters,
                    )?,
                    scalar_result,
                    erased_scalar_parameters,
                    erased_proof_parameters,
                    predicate_parameters: Vec::new(),
                    claims,
                    cleanup_receiver: None,
                    requires: Vec::new(),
                    runtime_requirements: Vec::new(),
                    source_checked_requires: Vec::new(),
                });
            }

            for (signature, admitted) in signatures.iter_mut().zip(bodies) {
                let plan = admitted.source().entry()?;
                if plan.structural_parameters.len() != signature.parameters.len() {
                    return unsupported(
                        "Unit predicate signature does not match its structural roster",
                    );
                }
                signature.predicate_parameters = plan
                    .structural_parameters
                    .iter()
                    .zip(&signature.parameters)
                    .map(|(source, terminal)| StructuralParameterDeclaration {
                        position: source.position,
                        ..terminal.clone()
                    })
                    .collect();
            }

            let mut selected_subjects =
                crate::unit::unit_cleanup::contexts::SelectedCleanupSubjects::new(checked);
            for signature in &mut signatures {
                let cleanup_context = crate::unit::unit_cleanup::contexts::prepare_signature(
                    identities,
                    checked,
                    &mut selected_subjects,
                    signature.source,
                    &mut signature.predicate_parameters,
                    structural_types,
                    next_place,
                )?;
                signature.cleanup_receiver = cleanup_context
                    .as_ref()
                    .and_then(|context| context.receiver);
                let requirements = super::entry_requirements::lower(
                    identities,
                    checked,
                    scalar_applications,
                    signature.source,
                    &signature.scalar_parameters,
                    &signature.erased_scalar_parameters,
                    &signature.predicate_parameters,
                    structural_types,
                )?;
                signature.requires = requirements.requires;
                signature.runtime_requirements = requirements.runtime_requirements;
                signature.source_checked_requires = requirements.source_checked_requires;
                if let Some(context) = cleanup_context {
                    context.validate_contract(&signature.requires)?;
                }
            }
            Ok(signatures)
        }
    }
    pub(crate) mod target_families {

        use super::{
            BoundaryParameters, CheckedTrees, LoweringError, UnitPlans, unique_unit_boundary,
            unsupported,
        };
        use semantic_vocabulary::MachineId;
        use symbols::SymbolHandle;
        use terminal_psi::{
            TargetMachineCallable, TargetMachineFamily, TargetMachineFamilyMember,
            TargetMachineSupply,
        };

        pub(crate) fn runtime_families(
            checked: &CheckedTrees,
        ) -> Result<Vec<&typed_trees::machine::TargetMachineFamily>, LoweringError> {
            let mut families = Vec::new();
            for family in &checked.typed.target_machine_families {
                if family.purpose != typed_trees::machine::MachinePurpose::Runtime {
                    continue;
                }
                let head = checked.typed.machine_by_symbol(family.canonical).ok_or(
                    LoweringError::Unsupported("target-family head lost its typed declaration"),
                )?;
                let open = head.has_specialization_parameters();
                for member in &family.members {
                    let member = checked.typed.machine_by_symbol(member.machine).ok_or(
                        LoweringError::Unsupported(
                            "target-family member lost its typed declaration",
                        ),
                    )?;
                    if member.has_specialization_parameters() != open {
                        return unsupported(
                            "target-family cohort has mixed open and closed declarations",
                        );
                    }
                }
                if !open {
                    families.push(family);
                }
            }
            Ok(families)
        }

        pub(crate) fn callable(
            source: SymbolHandle,
            machines: &[(SymbolHandle, MachineId)],
            boundaries: &[BoundaryParameters],
        ) -> Result<TargetMachineCallable, LoweringError> {
            let mut owners = machines
                .iter()
                .filter(|(symbol, _)| *symbol == source)
                .map(|(_, identity)| TargetMachineCallable::Machine(*identity))
                .chain(
                    boundaries
                        .iter()
                        .filter(|boundary| boundary.source == source)
                        .map(|boundary| TargetMachineCallable::Boundary(boundary.id)),
                );
            let owner = owners.next().ok_or(LoweringError::Unsupported(
                "target-family member has no emitted callable",
            ))?;
            if owners.next().is_some() {
                return unsupported("target-family member has multiple emitted callables");
            }
            Ok(owner)
        }

        pub(crate) fn lower(
            checked: &CheckedTrees,
            machines: &[(SymbolHandle, MachineId)],
            boundaries: &[BoundaryParameters],
        ) -> Result<Vec<TargetMachineFamily>, LoweringError> {
            let mut families = Vec::with_capacity(checked.typed.target_machine_families.len());
            for family in runtime_families(checked)? {
                let canonical = callable(family.canonical, machines, boundaries)?;
                let head_contract =
                    match canonical {
                        TargetMachineCallable::Boundary(_) => {
                            unique_unit_boundary(
                                UnitPlans::published(&checked.facts.flow),
                                family.canonical,
                            )
                            .map_err(crate::shared::lowering_error::unit_machine_plan_error(
                                checked,
                                checked.symbols.name(family.canonical),
                                family.canonical,
                            ))?
                            .contract_commitment
                        }
                        TargetMachineCallable::Machine(_) => checked
                            .facts
                            .contract_plans
                            .for_machine(family.canonical)
                            .ok_or(LoweringError::Unsupported(
                                "target-family executable head has no checked contract commitment",
                            ))?
                            .commitment,
                    };
                if head_contract.is_zero() {
                    return unsupported(
                        "target-family head has an empty checked contract commitment",
                    );
                }
                let mut members = Vec::with_capacity(family.members.len());
                for member in &family.members {
                    let machine = checked.typed.machine_by_symbol(member.machine).ok_or(
                        LoweringError::Unsupported(
                            "target-family member lost its typed declaration",
                        ),
                    )?;
                    let supply = match machine.supply_mode {
                        language_semantics::MachineSupplyMode::CheckedBody => {
                            TargetMachineSupply::CheckedBody
                        }
                        language_semantics::MachineSupplyMode::Requirement => {
                            TargetMachineSupply::Requirement
                        }
                        language_semantics::MachineSupplyMode::TopLevelRequirement => {
                            TargetMachineSupply::TopLevelRequirement
                        }
                        language_semantics::MachineSupplyMode::Boundary => {
                            TargetMachineSupply::Boundary
                        }
                        language_semantics::MachineSupplyMode::AdmissionClaim => {
                            TargetMachineSupply::AdmissionClaim
                        }
                        language_semantics::MachineSupplyMode::ExternalRealization { .. } => {
                            TargetMachineSupply::ExternalRealization
                        }
                    };
                    members.push(TargetMachineFamilyMember {
                        target: member.target.as_str().to_owned(),
                        callable: callable(member.machine, machines, boundaries)?,
                        supply,
                    });
                }
                families.push(TargetMachineFamily {
                    canonical,
                    head_contract: head_contract.as_bytes(),
                    members,
                });
            }
            families.sort_by_key(|family| family.canonical);
            Ok(families)
        }
    }
    pub(crate) mod target_layouts {

        use super::{CheckedTrees, LoweringError, UnitPlans, unsupported};
        use checked_trees::{CheckedUnitRecordStorage, CheckedUnitStructuralTypeShape};
        use semantic_vocabulary::{AuthoredRecordStorage, StructuralTypeId};
        use symbols::SymbolHandle;
        use terminal_psi::{
            StructuralFieldDeclaration, StructuralRecordStorage, StructuralTypeDeclaration,
            StructuralTypeShape, TargetLayoutApplication, TargetLayoutFieldLayoutSource,
            TargetLayoutFieldStorage, TargetLayoutRealization, TargetLayoutSchemaArgument,
            TargetLayoutSchemaField, TargetLayoutSchemaFieldKind,
        };
        use typed_trees::type_identity::TypeIdentityId;

        pub(crate) fn lower(
            checked: &CheckedTrees,
            plans: UnitPlans<'_>,
            types: &[StructuralTypeDeclaration],
            type_ids: &[(TypeIdentityId, StructuralTypeId)],
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        ) -> Result<Vec<TargetLayoutApplication>, LoweringError> {
            let mut applications = Vec::new();
            for record in types {
                let StructuralTypeShape::Record {
                    fields,
                    storage: StructuralRecordStorage::TargetLayout(id),
                } = &record.shape
                else {
                    continue;
                };
                let type_identity = type_ids
                    .iter()
                    .find_map(|(identity, id)| (*id == record.id).then_some(*identity))
                    .ok_or(LoweringError::Unsupported(
                        "target layout record lost its exact type identity mapping",
                    ))?;
                let checked_record = super::catalog::selected_declaration(plans, type_identity)?;
                let CheckedUnitStructuralTypeShape::Record {
                    storage:
                        CheckedUnitRecordStorage::TargetLayout {
                            application,
                            application_identity: _,
                            schema_type_identity,
                        },
                    ..
                } = &checked_record.shape
                else {
                    return unsupported(
                        "target layout record lost its exact checked application marker",
                    );
                };
                let plan = checked
                    .typed
                    .plan_laid_layout_for_type(*application)
                    .ok_or(LoweringError::Unsupported(
                        "target layout application lost its typed policy result",
                    ))?;
                let typed_trees::PlanLaidRealizations::TargetInputs(span) = plan.realizations
                else {
                    return unsupported("target layout marker names an invariant application");
                };
                let alternatives = checked.typed.plan_laid_target_layouts.span(span).ok_or(
                    LoweringError::Unsupported(
                        "target layout application lost its alternative span",
                    ),
                )?;
                let schema_id = crate::shared::terminal_identities::lookup_type_id(
                    type_ids,
                    schema_type_identity,
                )?;
                let schema = types
                    .iter()
                    .find(|candidate| candidate.id == schema_id)
                    .ok_or(LoweringError::Unsupported(
                        "target layout application lost its retained schema",
                    ))?;
                let StructuralTypeShape::Record {
                    fields: schema_fields,
                    ..
                } = &schema.shape
                else {
                    return unsupported("target layout Schema ABI requires a record schema");
                };
                let record_fields = live_fields(fields);
                let schema_fields = live_fields(schema_fields);
                validate_field_inventory(
                    checked,
                    plan.data_symbol,
                    &plan.field_symbols,
                    &record_fields,
                )?;
                validate_field_inventory(
                    checked,
                    plan.schema_symbol,
                    &plan.schema_field_symbols,
                    &schema_fields,
                )?;
                let policy_identity = identities.intern(
                    &checked
                        .typed
                        .package_qualified_type_identity_text(plan.application),
                );
                let policy_machine = checked
                    .typed
                    .machine_by_symbol(plan.policy_plan_machine_symbol)
                    .ok_or(LoweringError::Unsupported(
                        "fixed representation lost its exact checked plan",
                    ))?;
                let plan_identity = identities.intern(
                    &validation::contract_identities::normalized_machine_identity(
                        &checked.typed,
                        policy_machine,
                    )
                    .ok_or(LoweringError::Unsupported(
                        "fixed representation policy has no checked callable identity",
                    ))?,
                );
                if alternatives.is_empty() && !plan.target_inputs.is_empty() {
                    return unsupported("fixed representation omitted its target inputs");
                }
                let mut realizations = Vec::with_capacity(alternatives.len());
                for alternative in alternatives {
                    let argument =
                        alternative
                            .schema_argument
                            .as_ref()
                            .ok_or(LoweringError::Unsupported(
                                "target layout application lost its exact Schema argument",
                            ))?;
                    if argument.abi_version != 4 {
                        return unsupported(
                            "target layout application uses an unsupported Schema ABI version",
                        );
                    }
                    let argument_fields = checked
                        .typed
                        .plan_laid_schema_fields
                        .span(argument.fields)
                        .ok_or(LoweringError::Unsupported(
                            "target layout application lost its Schema argument span",
                        ))?;
                    if argument_fields.len() != record_fields.len()
                        || argument_fields.len() != schema_fields.len()
                    {
                        return unsupported(
                            "unsupported target layout Schema argument field inventory",
                        );
                    }
                    let mut captured_fields = Vec::with_capacity(argument_fields.len());
                    for field in argument_fields {
                        let position = plan
                            .schema_field_symbols
                            .iter()
                            .position(|symbol| *symbol == field.schema_field)
                            .ok_or(LoweringError::Unsupported(
                                "target layout Schema argument changed exact field correspondence",
                            ))?;
                        captured_fields.push(TargetLayoutSchemaField {
                            record_field: record_fields[position].id,
                            schema_field: schema_fields[position].id,
                            key: field.key,
                            identity: field.identity,
                            byte_size: field.byte_size,
                            byte_alignment: field.byte_alignment,
                            layout_source: match field.layout_source {
                                typed_trees::PlanLaidFieldLayoutSource::Default => {
                                    TargetLayoutFieldLayoutSource::Default
                                }
                                typed_trees::PlanLaidFieldLayoutSource::Scalar => {
                                    TargetLayoutFieldLayoutSource::Scalar
                                }
                                typed_trees::PlanLaidFieldLayoutSource::Explicit => {
                                    TargetLayoutFieldLayoutSource::Explicit
                                }
                            },
                            kind: match field.kind {
                                typed_trees::PlanLaidSchemaFieldKind::Scalar => {
                                    TargetLayoutSchemaFieldKind::Scalar
                                }
                                typed_trees::PlanLaidSchemaFieldKind::Nested => {
                                    TargetLayoutSchemaFieldKind::Nested
                                }
                                typed_trees::PlanLaidSchemaFieldKind::Repeated => {
                                    TargetLayoutSchemaFieldKind::Repeated
                                }
                            },
                        });
                    }
                    let geometry = &alternative.geometry;
                    let byte_size =
                        geometry
                            .validated_layout
                            .size
                            .ok_or(LoweringError::Unsupported(
                                "target layout alternative has no fixed extent",
                            ))?;
                    if usize::try_from(byte_size).ok() != Some(geometry.size)
                        || usize::try_from(geometry.validated_layout.align).ok()
                            != Some(geometry.align)
                        || geometry.offsets.len() != record_fields.len()
                    {
                        return unsupported("unsupported target layout alternative geometry");
                    }
                    let fields = record_fields
                        .iter()
                        .enumerate()
                        .map(|(position, field)| {
                            Ok(TargetLayoutFieldStorage {
                                field: field.id,
                                storage: geometry
                                    .authored_field_storage(
                                        position,
                                        field.field_type.scalar_type().is_some(),
                                    )
                                    .ok_or(LoweringError::Unsupported(
                                        "target layout field has no exact supported storage",
                                    ))?,
                            })
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                    realizations.push(TargetLayoutRealization {
                        schema_argument: TargetLayoutSchemaArgument {
                            abi_version: argument.abi_version,
                            fields: captured_fields,
                            retired_field_identities: checked
                                .typed
                                .plan_laid_retired_field_identities
                                .span(argument.retired_field_identities)
                                .ok_or(LoweringError::Unsupported(
                                    "target layout application lost its retired Schema identities",
                                ))?
                                .iter()
                                .map(|row| row.identity)
                                .collect(),
                        },
                        target: alternative.target.as_str().to_owned(),
                        pointer_size: alternative.target_facts.pointer_size,
                        pointer_alignment: alternative.target_facts.pointer_alignment,
                        case_tag_size: alternative.target_facts.case_tag_size,
                        case_tag_alignment: alternative.target_facts.case_tag_alignment,
                        little_endian: alternative.target_facts.little_endian,
                        storage: AuthoredRecordStorage {
                            byte_size,
                            byte_alignment: geometry.validated_layout.align,
                        },
                        fields,
                    });
                }
                realizations.sort_by(|a, b| a.target.cmp(&b.target));
                let mut target_inputs = plan
                    .target_inputs
                    .iter()
                    .map(|target| terminal_psi::TargetLayoutInput {
                        target: target.profile.clone(),
                        pointer_size: target.pointer_size,
                        pointer_alignment: target.pointer_alignment,
                        case_tag_size: target.case_tag_size,
                        case_tag_alignment: target.case_tag_alignment,
                        little_endian: target.little_endian,
                    })
                    .collect::<Vec<_>>();

                target_inputs.sort_by(|a, b| a.target.cmp(&b.target));
                applications.push(TargetLayoutApplication {
                    id: *id,
                    record: record.id,
                    schema: schema.id,
                    policy_identity,
                    plan_identity,
                    target_inputs,
                    realizations,
                });
            }
            applications.sort_by_key(|application| application.id);
            Ok(applications)
        }

        fn live_fields(fields: &[StructuralFieldDeclaration]) -> Vec<&StructuralFieldDeclaration> {
            fields
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .collect()
        }

        fn validate_field_inventory(
            checked: &CheckedTrees,
            owner: SymbolHandle,
            symbols: &[SymbolHandle],
            fields: &[&StructuralFieldDeclaration],
        ) -> Result<(), LoweringError> {
            let data = checked.typed.data_definition_by_symbol(owner).ok_or(
                LoweringError::Unsupported("target layout lost an exact data declaration"),
            )?;
            if symbols.len() != fields.len()
                || symbols.iter().copied().ne(checked
                    .typed
                    .live_data_fields(checked.typed.data_members(data))
                    .map(|field| field.symbol))
            {
                return unsupported(
                    "unsupported target layout source/emitted live field inventory",
                );
            }
            Ok(())
        }
    }

    pub(crate) mod bodies {

        use super::{
            CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan, LoweringError,
            SemanticDomainId, ServiceReachSummary, unsupported,
        };
        use checked_trees::{
            CheckedComposedUnitControlMachinePlan, CheckedUnitStructuralParameterPlan,
        };

        #[derive(Clone, Copy)]
        pub(crate) struct UnitPlans<'a> {
            checked: &'a checked_trees::CheckedUnitEffectPlans,
            partial_affine: &'a [checked_trees::CheckedPartialAffineUnitCleanupMachinePlan],
        }

        impl<'a> UnitPlans<'a> {
            pub(crate) fn published(flow: &'a checked_trees::FlowFacts) -> Self {
                Self {
                    checked: &flow.terminal_unit_effects,
                    partial_affine: &flow.terminal_partial_affine_unit_cleanups.machines,
                }
            }

            pub(crate) fn machines(self) -> impl Iterator<Item = &'a CheckedUnitEffectMachinePlan> {
                self.checked
                    .machines
                    .iter()
                    .chain(self.partial_affine.iter().map(|plan| &plan.machine))
            }

            pub(crate) fn for_machine(
                self,
                machine: symbols::SymbolHandle,
            ) -> Option<&'a CheckedUnitEffectMachinePlan> {
                self.machines().find(|plan| plan.machine == machine)
            }

            pub(crate) fn composed_machines(self) -> &'a [CheckedComposedUnitControlMachinePlan] {
                &self.checked.composed_machines
            }

            pub(crate) fn composed_for_machine(
                self,
                machine: symbols::SymbolHandle,
            ) -> Option<&'a CheckedComposedUnitControlMachinePlan> {
                self.checked.composed_for_machine(machine)
            }

            pub(crate) fn boundary_machines(
                self,
            ) -> &'a [checked_trees::CheckedBoundaryMachinePlan] {
                &self.checked.boundary_machines
            }

            pub(crate) fn calling_signatures(
                self,
            ) -> &'a [checked_trees::CheckedCallingSignaturePlan] {
                &self.checked.calling_signatures
            }

            pub(crate) fn opaque_representations(
                self,
            ) -> &'a [checked_trees::CheckedOpaqueRepresentation] {
                &self.checked.opaque_representations
            }

            pub(crate) fn structural_domains(
                self,
            ) -> &'a [checked_trees::CheckedUnitStructuralDomainPlan] {
                &self.checked.structural_domains
            }

            pub(crate) fn structural_types(
                self,
            ) -> impl Iterator<Item = &'a checked_trees::CheckedUnitStructuralTypePlan>
            {
                self.checked.structural_types.iter()
            }

            pub(crate) fn source_machines(
                self,
                checked: &checked_trees::CheckedTrees,
                bodies: &[symbols::SymbolHandle],
            ) -> Result<Vec<symbols::SymbolHandle>, LoweringError> {
                let mut sources = Vec::new();
                for symbol in bodies {
                    match UnitBody::find(self, *symbol)? {
                        UnitBody::Ordinary(plan) => {
                            if !sources.contains(&plan.machine) {
                                sources.push(plan.machine);
                            }
                        }
                        UnitBody::Composed(plan) => {
                            for state in &plan.states {
                                let (owner, _) = crate::shared::declarations::state_declaration(
                                    checked,
                                    state.state,
                                )?;
                                if !sources.contains(&owner.symbol) {
                                    sources.push(owner.symbol);
                                }
                            }
                        }
                    }
                }
                Ok(sources)
            }
        }

        #[derive(Clone, Copy)]
        pub(crate) enum UnitBody<'a> {
            Ordinary(&'a CheckedUnitEffectMachinePlan),
            Composed(&'a CheckedComposedUnitControlMachinePlan),
        }

        pub(crate) type UnitEntry<'a> = checked_trees::CheckedUnitCallableEntry<'a>;

        impl<'a> UnitBody<'a> {
            pub(crate) fn contains(plans: UnitPlans<'_>, symbol: symbols::SymbolHandle) -> bool {
                plans.for_machine(symbol).is_some() || plans.composed_for_machine(symbol).is_some()
            }

            pub(crate) fn scalar_result_type(self) -> Option<checked_trees::types::PrimitiveType> {
                let result = match self {
                    Self::Ordinary(plan) => &plan.result,
                    Self::Composed(plan) => &plan.result,
                };
                match result {
                    checked_trees::CheckedControlResultPlan::Scalar { primitive_type } => {
                        Some(*primitive_type)
                    }
                    _ => None,
                }
            }

            pub(crate) fn result(
                self,
            ) -> Result<checked_trees::CheckedControlResultPlan, LoweringError> {
                Ok(match self {
                    Self::Ordinary(plan) => plan.result.clone(),
                    Self::Composed(plan) => plan.result.clone(),
                })
            }
            pub(crate) fn find(
                plans: UnitPlans<'a>,
                symbol: symbols::SymbolHandle,
            ) -> Result<Self, LoweringError> {
                let mut matches = plans
                    .machines()
                    .filter(|plan| plan.machine == symbol)
                    .map(Self::Ordinary)
                    .chain(
                        plans
                            .composed_machines()
                            .iter()
                            .filter(|plan| plan.machine == symbol)
                            .map(Self::Composed),
                    );
                let body = matches.next().ok_or(LoweringError::Unsupported(
                    "attached Unit closure is missing a checked transitive machine plan",
                ))?;
                if matches.next().is_some() {
                    return unsupported(
                        "attached Unit closure contains duplicate checked machine plans",
                    );
                }
                Ok(body)
            }

            pub(crate) fn entry(self) -> Result<UnitEntry<'a>, LoweringError> {
                Ok(match self {
                    Self::Ordinary(plan) => plan.callable_entry(),
                    Self::Composed(plan) => plan.callable_entry().ok_or(
                        LoweringError::Unsupported("composed Unit body has no entry state"),
                    )?,
                })
            }

            pub(crate) fn operations(
                self,
            ) -> impl Iterator<Item = &'a CheckedUnitEffectOperationPlan> {
                self.operations_with_states()
                    .map(|(_, operation)| operation)
            }

            pub(crate) fn projected_loans(
                self,
            ) -> impl Iterator<Item = &'a checked_trees::CheckedStructuralControlTransferSourcePlan>
            {
                let composed = match self {
                    Self::Composed(machine) => Some(machine),
                    Self::Ordinary(_) => None,
                };
                composed
                    .into_iter()
                    .flat_map(|machine| &machine.states)
                    .flat_map(|state| state.successors())
                    .flat_map(|edge| &edge.transfers)
                    .map(|transfer| &transfer.source)
                    .filter(|source| {
                        matches!(
                            source,
                            checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan { .. }
                        )
                    })
            }

            pub(crate) fn operations_with_states(
                self,
            ) -> impl Iterator<Item = (symbols::SymbolHandle, &'a CheckedUnitEffectOperationPlan)>
            {
                let (ordinary, composed) = match self {
                    Self::Ordinary(plan) => (Some(plan), &[][..]),
                    Self::Composed(plan) => (None, plan.states.as_slice()),
                };
                ordinary
                    .into_iter()
                    .flat_map(|plan| {
                        plan.operations
                            .iter()
                            .map(move |operation| (plan.state, operation))
                    })
                    .chain(composed.iter().flat_map(|state| {
                        state
                            .operation_dependencies()
                            .map(move |operation| (state.state, operation))
                    }))
                    .flat_map(|(state, operation)| {
                        operation
                            .with_value_calls()
                            .map(move |operation| (state, operation))
                    })
            }

            pub(crate) fn structural_parameters(
                self,
            ) -> impl Iterator<Item = &'a CheckedUnitStructuralParameterPlan> {
                let (ordinary, composed) = match self {
                    Self::Ordinary(plan) => (plan.structural_parameters.as_slice(), &[][..]),
                    Self::Composed(plan) => (&[][..], plan.states.as_slice()),
                };
                ordinary.iter().chain(
                    composed
                        .iter()
                        .flat_map(|state| &state.structural_parameters),
                )
            }

            pub(crate) fn structural_home_roots(
                self,
                checked: &checked_trees::CheckedTrees,
                state: symbols::SymbolHandle,
            ) -> Result<Vec<facts::PlaceRoot>, LoweringError> {
                let (_, declaration) =
                    crate::shared::declarations::state_declaration(checked, state)?;
                let (parameters, scalar_parameters, storage) = match self {
                    Self::Ordinary(plan) if plan.state == state => (
                        plan.structural_parameters.as_slice(),
                        plan.scalar_parameters.as_slice(),
                        plan.parameter_storage.as_slice(),
                    ),
                    Self::Ordinary(_) => (&[][..], &[][..], &[][..]),
                    Self::Composed(plan) => plan
                        .states
                        .iter()
                        .find(|plan| plan.state == state)
                        .map_or((&[][..], &[][..], &[][..]), |plan| {
                            (
                                plan.structural_parameters.as_slice(),
                                plan.scalar_parameters.as_slice(),
                                plan.parameter_storage.as_slice(),
                            )
                        }),
                };
                let mut roots = Vec::new();
                for parameter in parameters {
                    let formal = checked
                        .state_parameters(declaration)
                        .get(parameter.position as usize)
                        .ok_or(LoweringError::Unsupported(
                            "call membership home lost its structural formal",
                        ))?;
                    roots.push(facts::PlaceRoot::Symbol(formal.symbol));
                }
                for storage in storage {
                    roots.push(facts::PlaceRoot::Symbol(storage.symbol));
                }
                for (_, operation) in self
                    .operations_with_states()
                    .filter(|(owner, _)| *owner == state)
                {
                    if let Some(result) = operation.structural_result_binding()
                        && result.local.is_named()
                    {
                        let root = facts::PlaceRoot::Symbol(result.local.symbol);
                        if !roots.contains(&root) {
                            roots.push(root);
                        }
                    }
                    if let checked_trees::CheckedUnitEffectOperationPlan::EstablishLocal(
                        checked_trees::CheckedUnitLocalBindingPlan::Structural { symbol, .. }
                        | checked_trees::CheckedUnitLocalBindingPlan::Primitive { symbol, .. },
                    ) = operation
                    {
                        let root = facts::PlaceRoot::Symbol(*symbol);
                        if !roots.contains(&root) {
                            roots.push(root);
                        }
                    }
                    if let Some(call) = operation.call() {
                        for argument in &call.structural_arguments {
                            use checked_trees::CheckedUnitStructuralArgumentSourcePlan as Source;
                            let root = match argument.source {
                                Source::PrimitiveLocal { symbol }
                                | Source::StructuralLocal { symbol }
                                | Source::StructuralLocalBorrow { symbol, .. } => {
                                    Some(facts::PlaceRoot::Symbol(symbol))
                                }
                                Source::StructuralExpression { expression, .. } => {
                                    Some(facts::PlaceRoot::Expression(expression))
                                }
                                Source::ScalarParameter { parameter_index } => scalar_parameters
                                    .get(parameter_index as usize)
                                    .and_then(|parameter| {
                                        checked
                                            .state_parameters(declaration)
                                            .get(parameter.source_position as usize)
                                    })
                                    .map(|parameter| facts::PlaceRoot::Symbol(parameter.symbol)),
                                _ => None,
                            };
                            if let Some(root) = root
                                && !roots.contains(&root)
                            {
                                roots.push(root);
                            }
                        }
                    }
                    if let Some(call) = operation.call()
                        && call.structural_result().is_some()
                        && let Some(checked_trees::NominalMachineUseSite::Expression(expression)) =
                            call.source_site()
                    {
                        let root = facts::PlaceRoot::Expression(expression);
                        if !roots.contains(&root) {
                            roots.push(root);
                        }
                    }
                }
                Ok(roots)
            }

            pub(crate) fn parameter_storage(
                self,
            ) -> impl Iterator<Item = &'a checked_trees::CheckedUnitParameterStoragePlan>
            {
                let (ordinary, composed) = match self {
                    Self::Ordinary(plan) => (plan.parameter_storage.as_slice(), &[][..]),
                    Self::Composed(plan) => (&[][..], plan.states.as_slice()),
                };
                ordinary
                    .iter()
                    .chain(composed.iter().flat_map(|state| &state.parameter_storage))
            }

            pub(crate) fn qualifications(self) -> &'a [SemanticDomainId] {
                match self {
                    Self::Ordinary(plan) => &plan.body_qualifications,
                    Self::Composed(plan) => &plan.body_qualifications,
                }
            }

            pub(crate) fn service_reach(self) -> ServiceReachSummary {
                match self {
                    Self::Ordinary(plan) => plan.service_reach,
                    Self::Composed(plan) => plan.service_reach,
                }
            }

            pub(crate) fn ordinary(
                self,
            ) -> Result<&'a CheckedUnitEffectMachinePlan, LoweringError> {
                match self {
                    Self::Ordinary(plan) => Ok(plan),
                    Self::Composed(_) => {
                        unsupported("this Unit body consumer requires ordinary body metadata")
                    }
                }
            }
        }
    }
    pub(crate) use operation_frame::completed_arguments::{
        emit_snapshot as emit_argument_snapshot, selected_memberships as argument_memberships,
    };
    pub(crate) use operation_frame::{BoundaryParameters, OperationEnvironment};
    pub(crate) mod normal_guarantees {

        use crate::proofs::contract_predicates::connective;
        use crate::scalar_lowering::contracts::{
            self, ContractStructuralResult, ContractViewNamespace,
        };
        use crate::shared::checked_facts::value_facts;
        use crate::shared::lowering_error::LoweringError;
        use crate::unit::ContractClause;
        use checked_trees::CheckedTrees;
        use symbols::SymbolHandle;
        use terminal_psi::{TerminalMachineResult, ValueDeclaration};

        pub(crate) fn lower(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            state: SymbolHandle,
            parameters: &[ValueDeclaration],
            result: &TerminalMachineResult,
            erased: &[ValueDeclaration],
            views: &ContractViewNamespace<'_>,
            next_obligation: &mut u64,
        ) -> Result<Vec<ContractClause>, LoweringError> {
            let (source, source_state) =
                crate::shared::declarations::state_declaration(checked, state)?;
            let contract = checked
                .facts
                .contract_plans
                .for_machine(source.symbol)
                .ok_or(LoweringError::Unsupported(
                    "normal return has no checked contract",
                ))?;
            let result_refinements = if result.scalar().is_some()
                && matches!(
                    checked
                        .type_reference_table
                        .type_reference(source_state.return_type),
                    checked_trees::types::TypeReferenceNode::Constrained { .. }
                ) {
                value_facts(checked)
                    .scalar_type_refinement_predicates(source_state.return_type)
                    .ok_or(LoweringError::Unsupported(
                        "scalar result has no complete checked value refinement",
                    ))?
            } else {
                &[]
            };
            let guarantees =
                contracts::projected_guarantees(contract.closed_scalar_values.ensures())
                    .cloned()
                    .collect::<Vec<_>>();
            let restored_guarantees = contract
                .restored_declaration_predicates
                .iter()
                .map(|predicate| {
                    Some(checked_trees::ClosedScalarContractValue::Predicate(
                        predicate.clone(),
                    ))
                })
                .collect::<Vec<_>>();
            let mut namespace = parameters.to_vec();
            namespace.extend(result.scalar());
            let views = ContractViewNamespace {
                result: match result {
                    TerminalMachineResult::Structural(declaration) => {
                        let entry = checked.typed.machine_states(source).first().ok_or(
                            LoweringError::Unsupported("normal guarantee owner has no entry state"),
                        )?;
                        let position = u32::try_from(checked.typed.state_parameters(entry).len())
                            .map_err(|_| {
                            LoweringError::Unsupported(
                                "normal result coordinate exceeds the contract namespace",
                            )
                        })?;
                        ContractStructuralResult::Declared {
                            position,
                            declaration,
                        }
                    }
                    _ => ContractStructuralResult::Absent,
                },
                ..*views
            };
            let mut propositions =
                contracts::clauses(identities, &guarantees, &namespace, erased, &views)?;
            for predicate in result_refinements {
                let result_predicate = contracts::proposition(
                    identities,
                    predicate,
                    result.scalar().as_slice(),
                    &[],
                    &ContractViewNamespace {
                        scalar_applications: views.scalar_applications,
                        constant_values: views.constant_values,
                        ..ContractViewNamespace::EMPTY
                    },
                )?;
                propositions = Some(match propositions {
                    Some(previous) => connective(previous, result_predicate, true)?,
                    None => result_predicate,
                });
            }
            if let Some(restored) =
                contracts::clauses(identities, &restored_guarantees, &namespace, erased, &views)?
            {
                propositions = Some(match propositions {
                    Some(previous) => connective(previous, restored, true)?,
                    None => restored,
                });
            }
            propositions
                .into_iter()
                .map(|proposition| {
                    Ok(ContractClause {
                        obligation: crate::shared::terminal_identities::obligation_id(
                            crate::shared::terminal_identities::allocate_dense(next_obligation)?,
                        ),
                        proposition,
                    })
                })
                .collect()
        }
    }
    pub(crate) mod ordinary_machine {

        use super::bodies::{UnitBody, UnitPlans};
        use super::catalog::lower_provider_candidate_service_ceiling;
        use super::composed_control::callable::EmissionCounters;
        use super::operation_frame::{
            CallInputs, Callees, CallerCustody, OperationFrame, StructuralResults,
            StructuralTypeRoster,
        };
        use super::parameters::lower_declared_service_reach;
        use super::parameters::lower_installation_machine_service_ceiling;
        use super::signatures::MachineSignature;
        use super::{
            CheckedUnitProviderCandidate, argument_evaluation, argument_schedule, byte_subslices,
            parameters, primitive_locals,
        };
        use crate::emission::operation_emission::buffer::OperationBuffer;
        use crate::emission::operation_emission::calls::CallEmissionContext;
        use crate::emission::operation_emission::place_declarations::retain_block_parameters;
        use crate::emission::structural_byte_sequence_store::retain_literal_declarations;
        use crate::expression_preparation::qualifications::SourceScalarReturn;
        use crate::scalar_lowering::computations::produced_values::prepare;
        use crate::shared::checked_facts::flow_facts;
        use crate::shared::terminal_identities::{
            StructuralDomainBinding, lookup_domain_id_for_carrier,
        };
        use crate::unit::{
            Block, CheckedTrees, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan,
            LoweringError, MachineContract, Operation, PlaceId, StructuralPlaceDeclaration,
            StructuralPlaceKind, StructuralTypeId, TerminalMachine, TerminalMachineResult,
            Terminator, ValueDeclaration, allocate_dense, content_conservation, contract_id,
            dense_identity, edge_id, lookup_type_id, lower_checked_crash_route_buckets,
            lower_structural_crash_route_buckets, place_id, terminal_scalar_type, unsupported,
            value_id,
        };
        use crate::unit::{
            MachineId, ServiceId, ServiceReachId, StructuralParameterDeclaration,
            StructuralTypeDeclaration,
        };
        use checked_trees::CheckedUnitStructuralArgumentSourcePlan;
        use lowered_psi::{LoweredSelectedComparisonOccurrence, LoweredSourceCallOccurrence};

        #[derive(Clone, Copy)]
        pub(crate) struct ClosureCatalog<'a> {
            pub(crate) scalar_applications:
                &'a std::sync::Arc<super::scalar_applications::SourceScalarApplications>,
            pub(crate) type_ids:
                &'a [(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
            pub(crate) domain_ids: &'a [StructuralDomainBinding],
            pub(crate) domain_declarations: &'a [terminal_psi::StructuralDomainDeclaration],
            pub(crate) service_ids: &'a [(ServiceReachId, ServiceId)],
            pub(crate) boundary_parameters: &'a [super::operation_frame::BoundaryParameters],
            pub(crate) machine_ids: &'a [(symbols::SymbolHandle, MachineId)],
            pub(crate) signatures: &'a [MachineSignature],
            pub(crate) scalar_requirement_counts: &'a [(symbols::SymbolHandle, usize)],
            pub(crate) plans: UnitPlans<'a>,
            pub(crate) closure: &'a [symbols::SymbolHandle],
            pub(crate) provider_candidate_plans: &'a [CheckedUnitProviderCandidate],
            pub(crate) prepared_scalar_qualifications:
                &'a crate::expression_preparation::qualifications::PreparedScalarQualifications,
            pub(crate) dynamic_dispatch:
                &'a std::sync::Arc<crate::unit::dynamic_dispatch::DynamicDispatchIndex>,
        }

        pub(crate) mod locals {

            use super::MachineEmission;
            use crate::unit::{CheckedUnitEffectOperationPlan, LoweringError, OperationKind};

            impl MachineEmission<'_> {
                pub(crate) fn establish_trivial_affine_local(
                    &mut self,
                    operation: &CheckedUnitEffectOperationPlan,
                ) -> Result<Option<OperationKind>, LoweringError> {
                    let CheckedUnitEffectOperationPlan::EstablishLocal(
                        checked_trees::CheckedUnitLocalBindingPlan::TrivialAffine {
                            declaration_ordinal,
                            statement_index,
                            ..
                        },
                    ) = operation
                    else {
                        unreachable!("dispatched establish_trivial_affine_local")
                    };
                    let local = self
                        .local_places
                        .get(usize::try_from(*declaration_ordinal).map_err(|_| {
                            LoweringError::Unsupported("Unit local ordinal exceeds usize")
                        })?)
                        .ok_or(LoweringError::Unsupported(
                            "Unit local ordinal is not dense",
                        ))?;
                    self.operations.register_structural_materialization(
                        local.id,
                        self.plan.state,
                        *statement_index,
                    )?;
                    Ok(Some(OperationKind::EstablishTrivialAffineLocal {
                        destination: local.id,
                    }))
                }
            }
        }
        pub(crate) mod projected_moves {

            use std::collections::{BTreeMap, BTreeSet};

            use crate::shared::call_operands::structural_arguments;
            use crate::unit::{
                Operation, PlaceId, StructuralAccess, StructuralMultiplicity,
                StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeDeclaration,
            };
            use terminal_psi::StructuralPathSegment;

            pub(crate) fn moved_subtrees(
                operations: &[Operation],
            ) -> BTreeMap<PlaceId, BTreeSet<Vec<StructuralPathSegment>>> {
                let mut moved: BTreeMap<PlaceId, BTreeSet<Vec<StructuralPathSegment>>> =
                    BTreeMap::new();
                for operation in operations {
                    let arguments = structural_arguments(&operation.kind);
                    for argument in arguments {
                        if argument.access == StructuralAccess::Owned && !argument.path.is_empty() {
                            moved
                                .entry(argument.place)
                                .or_default()
                                .insert(argument.path.clone());
                        }
                    }
                }
                moved
            }

            pub(crate) fn retires(
                types: &[StructuralTypeDeclaration],
                operations: &[Operation],
                place: &StructuralPlaceDeclaration,
                moved: &BTreeSet<Vec<StructuralPathSegment>>,
            ) -> bool {
                if moved.is_empty() {
                    return false;
                }
                let StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                } = place.kind
                else {
                    return false;
                };
                let Some(operation) = operations.iter().find(|candidate| candidate.id == producer)
                else {
                    return false;
                };
                let Some(result) = operation.result.structural() else {
                    return false;
                };
                if result.place != place.id
                    || result.structural_type != structural_type
                    || result.multiplicity != StructuralMultiplicity::Affine
                    || !result.qualifications.is_empty()
                    || !result.projected_qualifications.is_empty()
                    || !result.claims.is_empty()
                {
                    return false;
                }
                terminal_semantics::partial_affine_residuals(types, structural_type, moved, 0)
                    .is_some_and(|residuals| residuals.is_empty())
            }
        }

        pub(crate) struct MachineEmission<'a> {
            checked: &'a CheckedTrees,
            plan: &'a CheckedUnitEffectMachinePlan,
            plans: UnitPlans<'a>,
            signature: &'a MachineSignature,
            parameters: &'a Vec<StructuralParameterDeclaration>,
            scalar_parameter_count: usize,
            structural_types: &'a mut Vec<StructuralTypeDeclaration>,
            type_ids: &'a [(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
            domain_ids: &'a [StructuralDomainBinding],
            domain_declarations: &'a [terminal_psi::StructuralDomainDeclaration],
            service_ids: &'a [(ServiceReachId, ServiceId)],
            lowered_boundary_parameters: &'a [super::operation_frame::BoundaryParameters],
            machine_signatures: &'a [MachineSignature],
            closure: &'a [symbols::SymbolHandle],
            next_place: u64,
            next_value_identity: u64,
            next_block: u64,
            next_edge: u64,
            next_call_obligation: u64,
            operations: OperationBuffer,
            evaluation: argument_evaluation::Evaluation,
            borrowed_windows: crate::emission::borrowed_window::BorrowedWindowLedger,
            scalar_result_values: Vec<ValueDeclaration>,
            primitive_local_places: Vec<primitive_locals::PrimitiveLocal>,
            structural_result_places: Vec<(Option<StructuralPlaceDeclaration>, bool)>,
            local_places: Vec<StructuralPlaceDeclaration>,
            private_places: Vec<StructuralPlaceDeclaration>,
            next_literal_argument: usize,
            staged_arguments: Vec<super::argument_evaluation::ScalarArgumentCaptures>,
            staged_subslices: Vec<Vec<(usize, PlaceId)>>,
            staged_selectors: Vec<Vec<usize>>,
            staged_structural_arguments:
                Vec<Vec<Option<super::operation_frame::CompletedStructuralArgument>>>,
            retained_scalar_prefix: Option<usize>,
            scalar_calls: CallEmissionContext<'a>,
        }

        pub(crate) struct StepInputs {
            pub(crate) operation_index: usize,
            pub(crate) staged: bool,
            pub(crate) finishes_argument_group: bool,
            pub(crate) source_value_count: usize,
            pub(crate) evaluated_scalar_arguments: Option<Vec<ValueDeclaration>>,
            pub(crate) evaluated_selectors: Option<Vec<ValueDeclaration>>,
            pub(crate) completed_structural_arguments:
                Option<Vec<Option<super::operation_frame::CompletedStructuralArgument>>>,
        }

        pub(crate) struct EmittedMachine {
            pub(crate) source_returns: Vec<SourceScalarReturn>,
            pub(crate) scalar_block_invariants: Vec<terminal_psi::ScalarBlockInvariant>,
            pub(crate) qualification_coercions: Vec<terminal_psi::ScalarQualificationCoercion>,
            pub(crate) machine: TerminalMachine,
            pub(crate) source_calls: Vec<LoweredSourceCallOccurrence>,
            pub(crate) quotient_adaptations:
                Vec<crate::emission::operation_emission::buffer::SourceQuotientAdaptation>,
            pub(crate) quotient_calls:
                Vec<crate::emission::operation_emission::buffer::SourceQuotientCall>,
            pub(crate) selected_comparisons: Vec<LoweredSelectedComparisonOccurrence>,
            pub(crate) source_checked_obligations: Vec<semantic_vocabulary::ObligationId>,
        }

        pub(crate) fn emit(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            signature: &MachineSignature,
            plan: &CheckedUnitEffectMachinePlan,
            terminal_machine: MachineId,
            structural_types: &mut Vec<StructuralTypeDeclaration>,
            catalog: ClosureCatalog<'_>,
            counters: EmissionCounters<'_>,
        ) -> Result<EmittedMachine, LoweringError> {
            let ClosureCatalog {
                scalar_applications,
                type_ids,
                domain_ids,
                domain_declarations,
                service_ids,
                boundary_parameters: lowered_boundary_parameters,
                machine_ids,
                signatures: machine_signatures,
                scalar_requirement_counts,
                plans,
                closure,
                provider_candidate_plans,
                prepared_scalar_qualifications,
                dynamic_dispatch,
            } = catalog;
            let EmissionCounters {
                place: place_counter,
                value: value_counter,
                block: block_counter,
                operation: operation_counter,
                edge: edge_counter,
                call_obligation: call_obligation_counter,
            } = counters;
            let mut next_place = *place_counter;
            let mut next_value = *value_counter;
            let mut next_block = *block_counter;
            let mut next_operation = *operation_counter;
            let next_edge = *edge_counter;
            let mut next_call_obligation = *call_obligation_counter;
            let parameters = &signature.parameters;
            let scalar_parameters = signature.scalar_parameters.clone();
            let scalar_parameter_count = scalar_parameters.len();
            let runtime_requirements = &signature.runtime_requirements;
            let entry_claims = &signature.claims.entry_claims;
            let claim_bindings = &signature.claims.source_claims;
            let content_entry_claims = content_conservation::lower_whole_content_entry_claims(
                identities,
                checked,
                structural_types,
                &plan.structural_parameters,
                parameters,
                &plan.entry_claims,
                claim_bindings,
            )?;
            let local_places = plan
                .trivial_affine_locals
                .iter()
                .map(|local| {
                    Ok(StructuralPlaceDeclaration {
                        id: place_id(allocate_dense(&mut next_place)?),
                        kind: StructuralPlaceKind::TrivialAffineLocal {
                            declaration_ordinal: local.declaration_ordinal,
                            structural_type: lookup_type_id(type_ids, &local.type_identity)?,
                            construction: local
                                .construction
                                .as_ref()
                                .map(|element| {
                                    Ok(semantic_vocabulary::AffineConstructionElement {
                                        root_structural_type: lookup_type_id(
                                            type_ids,
                                            &element.root_type_identity,
                                        )?,
                                        index: element.index,
                                    })
                                })
                                .transpose()?,
                        },
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            let (private_places, call_literal_count, mut operations) = establish_call_literals(
                identities,
                checked,
                plan,
                plans,
                lowered_boundary_parameters,
                type_ids,
                domain_ids,
                structural_types,
                dynamic_dispatch,
                &mut next_place,
                &mut next_value,
                &mut next_call_obligation,
                next_operation,
            )?;
            let next_literal_argument = 0usize;
            let next_value_identity = next_value;
            let scalar_result_values = scalar_parameters.clone();
            let parameter_storage = primitive_locals::establish_parameter_storage(
                &plan.parameter_storage,
                &plan.scalar_parameters,
                &scalar_parameters,
                type_ids,
                primitive_locals::PrimitiveStorageCatalog {
                    types: structural_types,
                    domains: domain_declarations,
                    scalars: prepared_scalar_qualifications.catalog(),
                },
                &mut next_place,
                &mut next_call_obligation,
                &mut operations,
            )?;
            let primitive_local_places = parameter_storage
                .iter()
                .map(|storage| primitive_locals::PrimitiveLocal {
                    symbol: storage.local.symbol,
                    declaration: storage.local.declaration,
                })
                .collect::<Vec<_>>();
            let structural_result_places = Vec::<(Option<StructuralPlaceDeclaration>, bool)>::new();
            let mut evaluation = argument_evaluation::Evaluation::new(
                &mut next_block,
                std::sync::Arc::clone(scalar_applications),
            )?;
            evaluation.shared_qualifications = prepared_scalar_qualifications.clone();
            evaluation.machine = Some(terminal_machine);
            evaluation.erased_scalar_formals = signature.erased_scalar_parameters.clone();
            evaluation.place_observations =
                crate::scalar_lowering::computations::fields::prepare_place_observations(
                    identities,
                    checked,
                    plan.machine,
                    plan.state,
                    structural_types,
                );
            evaluation.record_fields = crate::scalar_lowering::computations::fields::prepare(
                identities,
                checked,
                plan.machine,
                &[plan.machine],
                structural_types,
            )?;
            evaluation.literals = crate::scalar_lowering::computations::literals::prepare(
                identities,
                checked,
                plan.machine,
                structural_types,
                &mut next_place,
            )?;
            evaluation.produced_values =
                prepare(identities, checked, &[plan.machine], structural_types)?;
            evaluation.observations = crate::scalar_lowering::computations::observations::prepare(
                checked,
                &[plan.machine],
                &mut next_place,
            )?;
            evaluation.subslices = crate::scalar_lowering::computations::subslices::prepare(
                identities,
                checked,
                &[plan.machine],
                structural_types,
                &mut next_place,
            )?;
            evaluation.cases = crate::scalar_lowering::computations::cases::prepare(
                identities,
                checked,
                &[plan.machine],
                structural_types,
                &mut next_place,
            )?;
            evaluation.bind_structural_parameters(
                plan.structural_parameters
                    .iter()
                    .zip(parameters)
                    .map(|(source, parameter)| (source.position, parameter.clone()))
                    .collect(),
                structural_types,
            );
            evaluation.current_claims.ingress(
                parameters
                    .iter()
                    .filter(|parameter| parameter.access == terminal_psi::StructuralAccess::Owned)
                    .map(|parameter| parameter.place),
                entry_claims,
            )?;
            evaluation.primitive_storage =
                crate::unit::attached_unit::parameters::primitive_reference_storage(
                    checked,
                    plan.state,
                    &plan.structural_parameters,
                    &evaluation.structural_parameters,
                )?;
            evaluation.scalar_bindings =
                crate::expression_preparation::bindings::ScalarBindings::entry(
                    scalar_parameters.iter().map(|value| value.scalar_type),
                    &evaluation.primitive_storage,
                    &evaluation.structural_parameters,
                    structural_types,
                    &evaluation.place_observations,
                );
            primitive_locals::register_mut_formal_storage_homes(
                &mut evaluation,
                &parameter_storage,
            )?;
            let staged_arguments = vec![
                super::argument_evaluation::ScalarArgumentCaptures::default(
                );
                plan.operations.len()
            ];
            let staged_subslices = vec![Vec::<(usize, PlaceId)>::new(); plan.operations.len()];
            let retained_scalar_prefix = None;
            let cleanup_receivers = machine_signatures
                .iter()
                .filter_map(|signature| {
                    signature
                        .cleanup_receiver
                        .map(|receiver| (signature.source, receiver))
                })
                .collect::<Vec<_>>();
            let call_structural_types = structural_types.clone();
            let mut emission = MachineEmission {
                checked,
                plan,
                plans,
                signature,
                parameters,
                scalar_parameter_count,
                borrowed_windows: Default::default(),
                structural_types,
                type_ids,
                domain_ids,
                domain_declarations,
                service_ids,
                lowered_boundary_parameters,
                machine_signatures,
                closure,
                next_place,
                next_value_identity,
                next_block,
                next_edge,
                next_call_obligation,
                operations,
                evaluation,
                scalar_result_values,
                primitive_local_places,
                structural_result_places,
                local_places,
                private_places,
                next_literal_argument,
                staged_arguments,
                staged_subslices,
                staged_selectors: vec![Vec::new(); plan.operations.len()],
                staged_structural_arguments: vec![Vec::new(); plan.operations.len()],
                retained_scalar_prefix,
                scalar_calls: CallEmissionContext {
                    terminal_machine,
                    service_ids,
                    domain_ids,
                    domain_declarations,
                    scalar_qualifications: prepared_scalar_qualifications.catalog(),
                    claims: crate::emission::claim_bindings::ClaimBindings::new(
                        claim_bindings.clone(),
                        dense_identity(claim_bindings.len())?,
                    ),
                    cleanup_receivers: &cleanup_receivers,
                    machine_ids,
                    signatures: machine_signatures,
                    structural_types: &call_structural_types,
                    requirement_counts: scalar_requirement_counts,
                    boundaries: lowered_boundary_parameters,
                    next_obligation_identity: next_call_obligation,
                    obligation_limit: u64::MAX,
                },
            };
            let mut schedule = argument_schedule::build(checked, plan)?
                .into_iter()
                .peekable();
            while let Some(step) = schedule.next() {
                let finishes_argument_group =
                    matches!(schedule.peek(), Some(argument_schedule::Step::End));
                emission.lower_step(identities, step, finishes_argument_group)?;
                if emission.operations.invocation_completion.is_some() {
                    break;
                }
            }
            let MachineEmission {
                structural_types,
                mut next_place,
                mut next_value_identity,
                mut next_block,
                mut next_edge,
                mut next_call_obligation,
                mut operations,
                mut evaluation,
                mut scalar_result_values,
                primitive_local_places,
                mut structural_result_places,
                local_places,
                mut private_places,
                next_literal_argument,
                mut borrowed_windows,
                mut scalar_calls,
                ..
            } = emission;
            let structural_result_homes =
                super::operation_frame::dense_result_homes(&structural_result_places)?;
            borrowed_windows.require_closed()?;

            if next_literal_argument != call_literal_count {
                return unsupported("byte-sequence literal argument consumption is incomplete");
            }
            let crash_routes = {
                let effective = crate::unit::effective_crash_routes(checked, plan.machine)?;
                if parameters.is_empty() {
                    lower_checked_crash_route_buckets(identities, &effective, &scalar_parameters)?
                } else {
                    lower_structural_crash_route_buckets(
                        identities,
                        &effective,
                        &scalar_parameters,
                        &signature.predicate_parameters,
                        structural_types,
                        runtime_requirements,
                    )?
                }
            };
            let declared_result = match &plan.result {
                checked_trees::CheckedControlResultPlan::Unit => TerminalMachineResult::Unit,
                checked_trees::CheckedControlResultPlan::Scalar { primitive_type } => {
                    let declared = super::parameters::unit_scalar_result_type(
                        checked,
                        plan.state,
                        terminal_scalar_type(*primitive_type)?,
                        prepared_scalar_qualifications,
                    )?;
                    TerminalMachineResult::Scalar(ValueDeclaration {
                        qualifications: declared.qualifications,
                        id: value_id(allocate_dense(&mut next_value_identity)?),
                        scalar_type: declared.scalar_type,
                    })
                }
                checked_trees::CheckedControlResultPlan::Structural(_) => {
                    TerminalMachineResult::Structural(lower_structural_result_declaration(
                        identities,
                        checked,
                        plan,
                        parameters,
                        structural_types,
                        type_ids,
                        domain_ids,
                        &mut next_place,
                    )?)
                }
            };
            let mut completes_normally = true;
            let mut parameter_cleanup = Vec::new();
            if let Some(terminator) = operations.take_invocation_completion(&mut next_edge)? {
                completes_normally = false;
                evaluation.blocks.push(Block {
                    id: evaluation.current,
                    parameters: evaluation.parameters,
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: evaluation.block_structural_parameters,
                    operations: operations[evaluation.operation_start..].to_vec(),
                    terminator,
                });
                next_value = next_value_identity;
            } else if let Some(control) = &plan.scalar_control {
                scalar_calls.next_obligation_identity = next_call_obligation;
                let Some(CheckedUnitEffectOperationPlan::Complete {
                    trivial_affine_local_discard_ordinals,
                    ..
                }) = plan.operations.last()
                else {
                    return unsupported("scalar control sequence has no checked completion plan");
                };
                let trivial_affine_discards = trivial_affine_local_discard_ordinals
                    .iter()
                    .map(|ordinal| {
                        local_places
                            .get(*ordinal as usize)
                            .map(|local| local.id)
                            .ok_or(LoweringError::Unsupported(
                                "scalar control local cleanup ordinal is not dense",
                            ))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let claim_bindings = scalar_calls.claims.bindings().to_vec();
                let mut cleanup_catalog = super::completion_cleanup::CompletionCleanupCatalog {
                    control: &checked.facts.flow.terminal_unit_effects.cleanup_control,
                    types: structural_types,
                    type_ids,
                    calls: &mut scalar_calls,
                };
                let cleanups = super::completion_cleanup::scalar_returns(
                    identities,
                    &claim_bindings,
                    checked,
                    super::completion_cleanup::NormalExitSource {
                        completion: plan.normal_exit_cleanups.completion,
                        cleanups: flow_facts(checked)
                            .terminal_unit_effects
                            .cleanup_control
                            .normal_exits(plan.normal_exit_cleanups),
                        trivial_affine_discards: &trivial_affine_discards,
                        operations: &plan.operations,
                        parameters: &plan.structural_parameters,
                    },
                    control,
                    &mut cleanup_catalog,
                    parameters,
                    &evaluation,
                    &operations,
                )?;
                let source = {
                    let (mut environment, emission) = OperationFrame {
                        checked,
                        machine: plan.machine,
                        state: plan.state,
                        scalar_result: plan.scalar_result.as_ref(),
                        scalar_parameter_count,
                        source_value_count: scalar_result_values.len(),
                        parameters: evaluation.current_parameters(parameters),
                        structural_types: StructuralTypeRoster::Published(structural_types),
                        type_ids,
                        service_ids,
                        primitive_locals: &primitive_local_places,
                        results: StructuralResults::Dense(&mut structural_result_places),
                        private_places: &mut private_places,
                        windows: &mut borrowed_windows,
                        next_place: &mut next_place,
                        evaluation: &mut evaluation,
                        values: &mut scalar_result_values,
                        next_value: &mut next_value_identity,
                        next_block: &mut next_block,
                        next_edge: &mut next_edge,
                        calls: &mut scalar_calls,
                        operations: &mut operations,
                        callees: Callees {
                            plans,
                            signatures: machine_signatures,
                            boundaries: lowered_boundary_parameters,
                            domain_ids,
                            domain_declarations,
                            closure,
                        },
                        caller: CallerCustody {
                            erased_scalar_parameters: &signature.erased_scalar_parameters,
                            erased_proof_parameters: &signature.erased_proof_parameters,
                            local_places: &local_places,
                        },
                    }
                    .split();
                    emission.evaluation.scalar_control_result(
                        identities,
                        checked,
                        plan.machine,
                        plan.state,
                        control,
                        &cleanups,
                        emission.values,
                        emission.next_value,
                        emission.next_block,
                        emission.next_edge,
                        emission.operations,
                        emission.calls,
                        &mut environment,
                    )
                }?;
                borrowed_windows.require_closed()?;
                next_call_obligation = scalar_calls.next_obligation_identity;
                let TerminalMachineResult::Scalar(_) = &declared_result else {
                    return unsupported("scalar completion differs from the declared result");
                };
                next_value = next_value_identity;
                let return_edge = edge_id(allocate_dense(&mut next_edge)?);
                operations.source_returns.push(SourceScalarReturn {
                    edge: return_edge,
                    exits: cleanups.iter().map(|row| row.exit).collect(),
                });
                evaluation.blocks.push(Block {
                    id: evaluation.current,
                    parameters: evaluation.parameters,
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: evaluation.block_structural_parameters,
                    operations: operations[evaluation.operation_start..].to_vec(),
                    terminator: Terminator::Return {
                        edge: return_edge,
                        value: source.id,
                        cleanup_actions: Vec::new(),
                    },
                });
            } else {
                let Some(CheckedUnitEffectOperationPlan::Complete {
                    trivial_affine_local_discard_ordinals,
                    trivial_affine_discards,
                    ..
                }) = plan.operations.last()
                else {
                    return Err(LoweringError::Unsupported(
                        "returning Unit sequence has no checked completion plan",
                    ));
                };
                let cleanup_control = &checked.facts.flow.terminal_unit_effects.cleanup_control;
                let [normal_exit] = cleanup_control.normal_exits(plan.normal_exit_cleanups) else {
                    return unsupported(
                        "ordinary sequence return has no unique source cleanup coordinate",
                    );
                };
                let local_discards = trivial_affine_local_discard_ordinals.iter().map(|ordinal| {
                    local_places
                        .get(usize::try_from(*ordinal).map_err(|_| {
                            LoweringError::Unsupported("Unit local cleanup ordinal exceeds usize")
                        })?)
                        .map(|local| local.id)
                        .ok_or(LoweringError::Unsupported(
                            "Unit local cleanup ordinal is not dense",
                        ))
                });
                let current_parameters = evaluation.current_parameters(parameters);
                parameter_cleanup = trivial_affine_discards
                    .iter()
                    .map(|parameter_index| {
                        current_parameters
                            .get(usize::try_from(*parameter_index).map_err(|_| {
                                LoweringError::Unsupported(
                                    "Unit affine discard parameter index exceeds usize",
                                )
                            })?)
                            .cloned()
                            .ok_or(LoweringError::Unsupported(
                                "Unit affine discard has an invalid parameter index",
                            ))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                parameter_cleanup.sort_by_key(|parameter| parameter.position);
                let moved_subtrees = projected_moves::moved_subtrees(&operations.operations);
                let claim_bindings = scalar_calls.claims.bindings().to_vec();
                let mut cleanup_catalog = super::completion_cleanup::CompletionCleanupCatalog {
                    control: cleanup_control,
                    types: structural_types,
                    type_ids,
                    calls: &mut scalar_calls,
                };
                let normal_regions = super::completion_cleanup::for_exit_regions(
                    identities,
                    &claim_bindings,
                    checked,
                    super::completion_cleanup::NormalExitSource {
                        completion: plan.normal_exit_cleanups.completion,
                        cleanups: cleanup_control.normal_exits(plan.normal_exit_cleanups),
                        trivial_affine_discards: &[],
                        operations: &plan.operations,
                        parameters: &plan.structural_parameters,
                    },
                    normal_exit.exit,
                    &mut cleanup_catalog,
                    parameters,
                    &evaluation,
                    &operations,
                )?;
                let mut selection_roster = structural_result_homes
                    .iter()
                    .rev()
                    .map(|(place, discard)| {
                        let retired = moved_subtrees.get(&place.id).is_some_and(|moved| {
                            projected_moves::retires(
                                structural_types,
                                &operations.operations,
                                place,
                                moved,
                            )
                        });
                        let contextual_disposal = normal_regions.iter().any(|region| {
                            region
                                .action
                                .physical()
                                .is_some_and(|action| action.place() == place.id)
                        });
                        (place.id, *discard && !retired && !contextual_disposal)
                    })
                    .collect::<Vec<_>>();
                let mut parameter_sources = Vec::new();
                for cleanup in &evaluation.selection_cleanups {
                    for source in &cleanup.sources {
                        if let Some((position, _)) = evaluation
                            .structural_parameters
                            .iter()
                            .find(|(_, declaration)| declaration.place == source.storage.place)
                        {
                            parameter_sources.push((*position, source.storage.place));
                        }
                    }
                }
                parameter_sources.sort_by_key(|(position, _)| std::cmp::Reverse(*position));
                parameter_sources.dedup_by_key(|(_, place)| *place);
                selection_roster.extend(
                    parameter_sources
                        .into_iter()
                        .map(|(_, place)| (place, false)),
                );
                let trivial_affine_discards = selection_roster
                    .into_iter()
                    .filter_map(|(place, discard)| discard.then_some(place))
                    .map(Ok)
                    .chain(local_discards)
                    .collect::<Result<Vec<_>, _>>()?;
                let mut scalar_return = plan
                    .scalar_result
                    .as_ref()
                    .map(|result| {
                        let position = scalar_parameter_count
                            .checked_add(result.binding_ordinal as usize)
                            .ok_or(LoweringError::Unsupported(
                                "scalar completion binding position overflows",
                            ))?;
                        let position = evaluation.scalar_bindings.immutable_position(position)?;
                        let source = scalar_result_values.get(position).ok_or(
                            LoweringError::Unsupported("scalar completion binding is absent"),
                        )?;
                        let scalar_type = terminal_scalar_type(result.primitive_type)?;
                        if source.scalar_type != scalar_type {
                            return unsupported(
                                "scalar completion binding has a different carrier",
                            );
                        }
                        let TerminalMachineResult::Scalar(declaration) = &declared_result else {
                            return unsupported("scalar binding differs from the declared result");
                        };
                        Ok::<_, LoweringError>((source.id, *declaration))
                    })
                    .transpose()?;
                next_value = next_value_identity;
                let structural_return = plan
                    .structural_result
                    .as_ref()
                    .map(|result| {
                        let (place, path) = match result.source {
                            CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                                binding_ordinal,
                            } => {
                                if let Some((_, binding)) = operations
                                    .structural_values
                                    .iter()
                                    .find(|(ordinal, _)| *ordinal == binding_ordinal)
                                {
                                    (binding.place(), binding.path().to_vec())
                                } else {
                                    let declaration = structural_result_places
                                        .get(binding_ordinal as usize)
                                        .and_then(|(declaration, _)| declaration.as_ref())
                                        .ok_or(LoweringError::Unsupported(
                                            "returned structural binding is absent",
                                        ))?;
                                    (declaration.id, Vec::new())
                                }
                            }
                            CheckedUnitStructuralArgumentSourcePlan::Parameter {
                                parameter_index,
                            } => {
                                let parameter = evaluation
                                    .structural_parameters
                                    .get(parameter_index as usize)
                                    .map(|(_, parameter)| parameter)
                                    .ok_or(LoweringError::Unsupported(
                                        "returned structural parameter is absent",
                                    ))?;
                                (parameter.place, Vec::new())
                            }
                            _ => {
                                return unsupported(
                                    "structural return source is not an owned value",
                                );
                            }
                        };
                        let source = terminal_psi::StructuralArgument {
                            place,
                            path,
                            access: terminal_psi::StructuralAccess::Owned,
                        };
                        let returned_claims = super::claims::lower_return_claims(
                            identities,
                            checked,
                            plan.machine,
                            plan.state,
                            result,
                            &source,
                            &operations,
                            &evaluation.current_claims,
                            scalar_calls.claims.bindings(),
                            lookup_type_id(type_ids, &result.type_identity)?,
                            structural_types,
                        )?;
                        Ok::<_, LoweringError>((
                            source,
                            match &declared_result {
                                TerminalMachineResult::Structural(declaration)
                                    if lookup_type_id(type_ids, &result.type_identity)?
                                        == declaration.structural_type =>
                                {
                                    declaration.clone()
                                }
                                _ => {
                                    return unsupported(
                                        "structural completion differs from the declared result",
                                    );
                                }
                            },
                            returned_claims,
                        ))
                    })
                    .transpose()?;
                let edge = edge_id(allocate_dense(&mut next_edge)?);
                let mut cleanup_regions = Vec::new();
                for place in trivial_affine_discards {
                    cleanup_regions.extend(evaluation.normal_discard_regions(
                        place,
                        structural_types,
                        &operations,
                    )?);
                }
                cleanup_regions.extend(normal_regions);
                super::completion_cleanup::selection(
                    plan.state,
                    &plan.operations,
                    &evaluation,
                    &operations,
                    &mut cleanup_regions,
                )?;
                let (terminator, arrival) =
                    if let Some((source, result, returned_claims)) = &structural_return {
                        super::structural_returns::materialize(
                            identities,
                            checked,
                            source,
                            result.structural_type,
                            result.multiplicity,
                            Terminator::ReturnStructural {
                                edge,
                                source: source.place,
                                returned_claims: returned_claims.clone(),
                                cleanup_actions: Vec::new(),
                            },
                            cleanup_regions,
                            &mut next_place,
                            &mut private_places,
                            &mut evaluation,
                            &mut scalar_result_values,
                            &mut next_value,
                            &mut next_block,
                            &mut next_edge,
                            &mut operations,
                        )?
                    } else {
                        let cleanup_actions = if cleanup_regions.iter().any(|region| {
                            region.partition_leaf.is_some()
                                || !region.conditions.is_empty()
                                || region.selected_region.is_some()
                        }) {
                            let return_position = scalar_return
                                .as_ref()
                                .map(|(source, _)| {
                                    scalar_result_values
                                        .iter()
                                        .position(|value| value.id == *source)
                                        .ok_or(LoweringError::Unsupported(
                                            "cleanup lost its captured scalar return",
                                        ))
                                })
                                .transpose()?;
                            evaluation.continue_cleanup_regions(
                                identities,
                                checked,
                                cleanup_regions,
                                &mut scalar_result_values,
                                &mut next_value,
                                &mut next_block,
                                &mut next_edge,
                                &mut operations,
                            )?;
                            if let Some(position) = return_position {
                                scalar_return
                                    .as_mut()
                                    .ok_or(LoweringError::Unsupported(
                                        "cleanup lost its return contract",
                                    ))?
                                    .0 = scalar_result_values[position].id;
                            }
                            Vec::new()
                        } else {
                            cleanup_regions
                                .into_iter()
                                .filter_map(|region| region.action.into_physical())
                                .collect()
                        };
                        let terminator = if let Some((source, _)) = &scalar_return {
                            operations.source_returns.push(SourceScalarReturn {
                                edge,
                                exits: vec![normal_exit.exit],
                            });
                            Terminator::Return {
                                edge,
                                value: *source,
                                cleanup_actions,
                            }
                        } else {
                            Terminator::ReturnUnit {
                                edge,
                                cleanup_actions,
                            }
                        };
                        (terminator, None)
                    };
                if let Some(arrival) = arrival {
                    evaluation.blocks.push(arrival);
                }
                evaluation.blocks.push(Block {
                    id: evaluation.current,
                    parameters: evaluation.parameters,
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: evaluation.block_structural_parameters,
                    operations: operations[evaluation.operation_start..].to_vec(),
                    terminator,
                });
                evaluation.blocks.sort_by_key(|block| block.id);
            }
            evaluation.blocks.sort_by_key(|block| block.id);
            next_operation = operations.next_identity;
            retain_literal_declarations(&operations, &mut private_places)?;
            let computed_array_places = evaluation
                .blocks
                .iter()
                .flat_map(|block| {
                    crate::scalar_lowering::computations::arrays::declarations(&block.operations)
                        .chain(crate::scalar_lowering::computations::cases::declarations(
                            &block.operations,
                        ))
                })
                .filter(|place| {
                    !structural_result_places.iter().any(|(existing, _)| {
                        existing.is_some_and(|existing| existing.id == place.id)
                    }) && !private_places
                        .iter()
                        .any(|existing| existing.id == place.id)
                })
                .collect::<Vec<_>>();
            let OperationBuffer {
                mut source_returns,
                claim_edge_bindings,
                mut block_entry_claims,
                mut claim_introductions,
                selected_cleanup_groups,
                mut completed_cleanup_frontiers,
                mut scalar_block_invariants,
                mut source_calls,
                quotient_adaptations,
                quotient_calls,
                loan_completions,
                mut selected_comparisons,
                mut source_checked_obligations,
                ..
            } = operations;
            let mut structural_places = parameters
                .iter()
                .map(|parameter| StructuralPlaceDeclaration {
                    id: parameter.place,
                    kind: StructuralPlaceKind::Parameter {
                        position: parameter.position,
                    },
                })
                .chain(local_places.iter().copied())
                .chain(primitive_local_places.iter().map(|local| local.declaration))
                .chain(private_places.iter().copied())
                .chain(
                    structural_result_homes
                        .iter()
                        .map(|(place, _)| place)
                        .filter(|place| {
                            !matches!(place.kind, StructuralPlaceKind::ByteSequenceLiteral { .. })
                        })
                        .copied(),
                )
                .chain(computed_array_places)
                .collect::<Vec<_>>();
            retain_block_parameters(evaluation.blocks.iter(), &mut structural_places)?;
            crate::emission::operation_emission::place_declarations::retain_operation_results(
                evaluation.blocks.iter().flat_map(|block| &block.operations),
                &mut structural_places,
            )?;
            if let TerminalMachineResult::Structural(result) = &declared_result {
                structural_places.push(StructuralPlaceDeclaration {
                    id: result.place,
                    kind: StructuralPlaceKind::Result,
                });
            }
            structural_places.sort_by_key(|place| place.id);
            let normal_guarantees = super::normal_guarantees::lower(
                identities,
                checked,
                plan.state,
                &scalar_parameters,
                &declared_result,
                &signature.erased_scalar_parameters,
                &crate::scalar_lowering::contracts::ContractViewNamespace {
                    scalar_applications,
                    constant_values: &checked.typed.constant_recipe_closure.values,
                    parameters: &evaluation.structural_parameters,
                    result: crate::scalar_lowering::contracts::ContractStructuralResult::Absent,
                    structural_types,
                },
                &mut next_call_obligation,
            )?;
            let nominal_cleanup_subject =
                crate::machine_lowering::nominal_cleanup_subject(checked, plan.machine, type_ids)?;
            crate::emission::claim_bindings::seal_block_entry_claims(&mut block_entry_claims)?;
            let mut machine = TerminalMachine {
                assembly_target_requirement: checked
                    .typed
                    .machine(plan.machine)
                    .map(|machine| machine.assembly_target_requirement)
                    .ok_or(crate::shared::lowering_error::LoweringError::Unsupported(
                        "assembly target requirement has no checked machine",
                    ))?,
                declaration: None,
                closed_reach_application: None,
                declared_service_reach: lower_declared_service_reach(
                    checked,
                    plan.machine,
                    service_ids,
                )?,
                id: terminal_machine,
                attachment: nominal_cleanup_subject,
                nominal_cleanup_subject,
                parameters: scalar_parameters.clone(),
                parameter_order: signature.parameter_order.clone(),
                structural_parameters: parameters.clone(),
                ranked_scc: None,
                result: declared_result,
                structural_places,
                entry_claims: entry_claims.clone(),
                block_entry_claims,
                loan_completions,
                published_service_ceiling: if let Some(provider) = provider_candidate_plans
                    .iter()
                    .find(|candidate| candidate.candidate == plan.machine)
                {
                    lower_provider_candidate_service_ceiling(
                        checked,
                        plans,
                        provider,
                        plan,
                        service_ids,
                    )?
                } else {
                    lower_installation_machine_service_ceiling(
                        checked,
                        plan.machine,
                        plan.contract_service_reach,
                        plan.service_reach,
                        service_ids,
                    )?
                },
                content_entry_claims,
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: evaluation.entry,
                blocks: evaluation.blocks,
                contract: MachineContract {
                    id: contract_id(terminal_machine.get()),
                    crash_routes,
                    erased_scalar_formals: signature.erased_scalar_parameters.clone(),
                    erased_proof_formals:
                        crate::scalar_lowering::contracts::erased_proof_formal_declarations(
                            identities,
                            checked,
                            &signature.erased_proof_parameters,
                        ),
                    requires: signature.requires.clone(),
                    source_checked_requires: signature.source_checked_requires.clone(),
                    ensures: normal_guarantees,
                    outcome_specific_ensures: Vec::new(),
                },
            };
            for block in &mut machine.blocks {
                crate::scalar_lowering::contracts::bind_invocation_proof_references(block);
            }
            crate::emission::continuation_specialization::complete(
                &mut machine,
                &selected_cleanup_groups,
                structural_types,
                crate::emission::continuation_specialization::CompanionRows {
                    completed_cleanup_frontiers: &mut completed_cleanup_frontiers,
                    source_returns: &mut source_returns,
                    continuation_value_uses: &[],
                    claim_introductions: &mut claim_introductions,
                    claim_edge_bindings: &claim_edge_bindings,
                    source_calls: &mut source_calls,
                    scalar_block_invariants: &mut scalar_block_invariants,
                    qualification_coercions: &mut evaluation.qualification_coercions,
                    selected_comparisons: &mut selected_comparisons,
                    source_checked_obligations: &mut source_checked_obligations,
                },
                crate::emission::continuation_specialization::Counters {
                    next_place: &mut next_place,
                    next_value: &mut next_value,
                    next_block: &mut next_block,
                    next_edge: &mut next_edge,
                    next_operation: &mut next_operation,
                    next_obligation: &mut next_call_obligation,
                },
            )?;
            if completes_normally {
                crate::emission::affine_cleanup::complete(
                    &completed_cleanup_frontiers,
                    &parameter_cleanup,
                    &machine.structural_parameters,
                    &crate::emission::affine_cleanup::no_code_type_ids(checked, type_ids),
                    &machine.structural_places,
                    machine.entry,
                    &mut machine.blocks,
                    structural_types,
                )?;
            }
            *place_counter = next_place;
            *value_counter = next_value;
            *block_counter = next_block;
            *operation_counter = next_operation;
            *edge_counter = next_edge;
            *call_obligation_counter = next_call_obligation;
            Ok(EmittedMachine {
                source_returns,
                scalar_block_invariants,
                qualification_coercions: evaluation.qualification_coercions,
                machine,
                source_calls,
                quotient_adaptations,
                quotient_calls,
                selected_comparisons,
                source_checked_obligations,
            })
        }

        #[allow(clippy::too_many_arguments)]
        fn lower_structural_result_declaration(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            plan: &CheckedUnitEffectMachinePlan,
            parameters: &[StructuralParameterDeclaration],
            structural_types: &[StructuralTypeDeclaration],
            type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
            domain_ids: &[StructuralDomainBinding],
            next_place: &mut u64,
        ) -> Result<terminal_psi::StructuralResultDeclaration, LoweringError> {
            let checked_trees::CheckedControlResultPlan::Structural(result) = &plan.result else {
                return unsupported("structural declaration has a different result kind");
            };
            let place = place_id(allocate_dense(next_place)?);
            let structural_type = lookup_type_id(type_ids, &result.type_identity)?;
            let state = typed_trees::state::state_by_symbol(
                checked
                    .typed
                    .machine_states(checked.typed.machine(plan.machine).ok_or(
                        LoweringError::Unsupported("structural result has no exact machine"),
                    )?),
                plan.state,
            )
            .ok_or(LoweringError::Unsupported(
                "structural result has no exact state",
            ))?;
            let return_type = state.return_type;
            let mut qualifications =
                validation::view_result_carrier_qualifications(&checked.typed, return_type)
                    .map_err(LoweringError::Unsupported)?
                    .iter()
                    .map(|domain| {
                        lookup_domain_id_for_carrier(domain_ids, *domain, structural_type)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
            qualifications.extend(
                parameters::authored_result_whole_domains(checked, plan.machine)
                    .iter()
                    .map(|domain| {
                        lookup_domain_id_for_carrier(domain_ids, *domain, structural_type)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            );
            qualifications.sort();
            qualifications.dedup();
            Ok(terminal_psi::StructuralResultDeclaration {
                place,
                structural_type,
                multiplicity: crate::shared::terminal_identities::structural_multiplicity(
                    result.multiplicity,
                ),
                qualifications,
                projected_qualifications: parameters::lower_projected_qualifications(
                    identities,
                    &parameters::result_projected_qualifications(
                        checked,
                        plan.machine,
                        return_type,
                    )?,
                    domain_ids,
                    structural_type,
                    structural_types,
                )?,
                reference_sources: parameters::lower_reference_result_sources(
                    identities,
                    checked,
                    plan.structural_result
                        .as_ref()
                        .map_or(&[][..], |result| result.reference_sources.as_slice()),
                    structural_type,
                    structural_types,
                    parameters,
                )?,
            })
        }

        fn establish_call_literals(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            plan: &CheckedUnitEffectMachinePlan,
            plans: UnitPlans<'_>,
            lowered_boundary_parameters: &[super::operation_frame::BoundaryParameters],
            type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
            domain_ids: &[StructuralDomainBinding],
            structural_types: &[terminal_psi::StructuralTypeDeclaration],
            dynamic_dispatch: &std::sync::Arc<crate::unit::dynamic_dispatch::DynamicDispatchIndex>,
            next_place: &mut u64,
            next_value: &mut u64,
            next_obligation: &mut u64,
            next_operation: u64,
        ) -> Result<(Vec<StructuralPlaceDeclaration>, usize, OperationBuffer), LoweringError>
        {
            let mut literal_arguments = Vec::new();
            for operation in &plan.operations {
                let Some(call) = operation.call() else {
                    continue;
                };
                let target_machine = call.target.machine;
                literal_arguments.extend(
                    call.structural_arguments
                        .iter()
                        .enumerate()
                        .filter(|(_, argument)| argument.byte_sequence_literal().is_some())
                        .map(|(index, argument)| (argument, target_machine, index)),
                );
                if matches!(
                    call.completion(),
                    semantic_vocabulary::BoundaryCompletion::External(_)
                ) {
                    break;
                }
            }
            let mut operand_places = Vec::new();
            let operation_identity_base =
                next_operation
                    .checked_sub(1)
                    .ok_or(LoweringError::Unsupported(
                        "terminal operation identity starts at one",
                    ))?;
            let mut operations = OperationBuffer::new(
                operation_identity_base,
                *next_place,
                std::sync::Arc::new(
                    crate::emission::reference_formations::ReferenceFormations::prepare(
                        checked,
                        plan.state,
                        plan.operations
                            .iter()
                            .flat_map(|operation| operation.with_value_calls()),
                        std::iter::empty(),
                    )?,
                ),
                std::sync::Arc::clone(dynamic_dispatch),
            );
            let mut private_places = literal_arguments
                .iter()
                .map(|(argument, target_machine, argument_index)| {
                    let qualifications = match lowered_boundary_parameters
                        .iter()
                        .find(|boundary| boundary.source == *target_machine)
                    {
                        Some(boundary) => boundary
                            .structural
                            .get(*argument_index)
                            .ok_or(LoweringError::Unsupported(
                                "literal call boundary target parameter is absent",
                            ))?
                            .qualifications
                            .clone(),
                        None if UnitBody::contains(plans, *target_machine) => {
                            UnitBody::find(plans, *target_machine)?
                                .entry()?
                                .structural_parameters
                                .get(*argument_index)
                                .ok_or(LoweringError::Unsupported(
                                    "literal call target parameter is absent",
                                ))?
                                .qualifications
                                .iter()
                                .map(|domain| lookup_domain_id_for_carrier(domain_ids, *domain, lookup_type_id(type_ids, &argument.type_identity)?))
                                .collect::<Result<Vec<_>, _>>()?
                        }
                        None => {
                            crate::scalar_lowering::call_closure::callee::CheckedScalarCallee::find_for_unit_call(
                                checked,
                                *target_machine,
                            )?
                            .structural_parameters()
                            .get(*argument_index)
                            .ok_or(LoweringError::Unsupported(
                                "literal call target parameter is absent",
                            ))?
                            .qualifications
                            .iter()
                            .map(|domain| lookup_domain_id_for_carrier(domain_ids, *domain, lookup_type_id(type_ids, &argument.type_identity)?))
                            .collect::<Result<Vec<_>, _>>()?
                        }
                    };
                    crate::emission::structural_byte_sequence_store::emit_call_literal(
                        identities,
                        argument.byte_sequence_literal().ok_or(LoweringError::Unsupported("literal call payload is absent"))?,
                        lookup_type_id(type_ids, &argument.type_identity)?,
                        qualifications,
                        structural_types,
                        &mut operand_places,
                        next_place,
                        next_value,
                        next_obligation,
                        &mut operations,
                    )
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            let argument_count = private_places.len();
            operand_places.retain(|place| {
                !private_places
                    .iter()
                    .any(|argument| argument.id == place.id)
            });
            private_places.extend(operand_places);
            Ok((private_places, argument_count, operations))
        }

        impl MachineEmission<'_> {
            fn lower_step(
                &mut self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                step: argument_schedule::Step,
                finishes_argument_group: bool,
            ) -> Result<(), LoweringError> {
                let checked = self.checked;
                let plan = self.plan;
                self.scalar_calls.next_obligation_identity = self.next_call_obligation;
                let (operation_index, staged) = match step {
                    argument_schedule::Step::CaptureBefore(statement) => {
                        self.evaluation.prepare_storage_before(
                            identities,
                            self.checked,
                            self.plan.state,
                            statement,
                            self.structural_types,
                            self.domain_declarations,
                            &self.scalar_result_values,
                            &mut self.next_place,
                            &mut self.next_value_identity,
                            &mut self.operations,
                            &mut self.private_places,
                        )?;
                        return Ok(());
                    }
                    argument_schedule::Step::Begin => {
                        if self
                            .retained_scalar_prefix
                            .replace(self.scalar_result_values.len())
                            .is_some()
                        {
                            return unsupported("nested argument staging has an unfinished group");
                        }
                        return Ok(());
                    }
                    argument_schedule::Step::End => {
                        let prefix = self.retained_scalar_prefix.take().ok_or(
                            LoweringError::Unsupported(
                                "nested argument staging has no active group",
                            ),
                        )?;
                        self.scalar_result_values.truncate(prefix);
                        return Ok(());
                    }
                    argument_schedule::Step::Argument { operation, ordinal } => {
                        let source_value_count =
                            self.retained_scalar_prefix
                                .ok_or(LoweringError::Unsupported(
                                    "staged scalar argument has no source prefix",
                                ))?;
                        let value = {
                            let (mut environment, emission) =
                                self.frame(source_value_count).split();
                            emission.evaluation.argument_at(
                                identities,
                                checked,
                                plan.machine,
                                plan.state,
                                &plan.operations[operation],
                                ordinal,
                                source_value_count,
                                emission.values,
                                emission.next_value,
                                emission.next_block,
                                emission.next_edge,
                                emission.operations,
                                emission.calls,
                                &mut environment,
                            )
                        }?;
                        self.next_call_obligation = self.scalar_calls.next_obligation_identity;
                        self.staged_arguments[operation]
                            .record(ordinal, self.scalar_result_values.len());
                        self.scalar_result_values.push(value);
                        return Ok(());
                    }
                    argument_schedule::Step::Selector {
                        operation,
                        argument,
                        segment,
                    } => {
                        let source_value_count =
                            self.retained_scalar_prefix
                                .ok_or(LoweringError::Unsupported(
                                    "selector staging has no source prefix",
                                ))?;
                        let value = {
                            let (mut environment, emission) =
                                self.frame(source_value_count).split();
                            emission.evaluation.call_selector(
                                identities,
                                checked,
                                plan.machine,
                                plan.state,
                                &plan.operations[operation],
                                argument,
                                segment,
                                source_value_count,
                                emission.values,
                                emission.next_value,
                                emission.next_block,
                                emission.next_edge,
                                emission.operations,
                                emission.calls,
                                &mut environment,
                            )
                        }?;
                        self.next_call_obligation = self.scalar_calls.next_obligation_identity;
                        self.staged_selectors[operation].push(self.scalar_result_values.len());
                        self.scalar_result_values.push(value);
                        return Ok(());
                    }
                    argument_schedule::Step::Structural {
                        operation,
                        argument,
                        complete,
                    } => {
                        let count =
                            self.retained_scalar_prefix
                                .ok_or(LoweringError::Unsupported(
                                    "structural operand staging has no source prefix",
                                ))?;
                        let arguments = plan.operations[operation].structural_operands();
                        let offset = arguments[..argument]
                            .iter()
                            .map(|source| source.call_selector_positions().count())
                            .sum::<usize>();
                        let selectors = self.staged_selectors[operation][offset..].to_vec();
                        if self.staged_structural_arguments[operation].is_empty() {
                            self.staged_structural_arguments[operation] =
                                vec![None; arguments.len()];
                        }
                        let previous = self.staged_structural_arguments[operation][argument].take();
                        let captured = self.frame(count).capture_argument(
                            identities,
                            &plan.operations[operation],
                            argument,
                            &selectors,
                            previous,
                            complete,
                        )?;
                        self.staged_structural_arguments[operation][argument] = Some(captured);
                        self.next_call_obligation = self.scalar_calls.next_obligation_identity;
                        return Ok(());
                    }
                    argument_schedule::Step::Subslice { operation, ordinal } => {
                        let source_value_count =
                            self.retained_scalar_prefix
                                .ok_or(LoweringError::Unsupported(
                                    "subslice staging has no source prefix",
                                ))?;
                        if self.staged_subslices[operation]
                            .iter()
                            .any(|(prior, _)| *prior == ordinal)
                        {
                            return unsupported("subslice argument is evaluated more than once");
                        }
                        let bindings = self
                            .evaluation
                            .scalar_bindings
                            .clone()
                            .with_structural_parameters(&self.evaluation.structural_parameters)
                            .with_structural_observations(self.structural_types)
                            .with_primitive_storage(&self.evaluation.primitive_storage)
                            .with_structural_locals(&self.evaluation.structural_locals)
                            .with_view_locals(&self.evaluation.view_locals)
                            .with_element_views(&self.evaluation.element_views);
                        let place = {
                            let (mut environment, emission) =
                                self.frame(source_value_count).split();
                            let context = byte_subslices::StagingContext {
                                state: plan.state,
                                structural_types: environment.structural_types,
                                type_ids: environment.type_ids,
                                bindings: &bindings,
                            };
                            byte_subslices::emit(
                                identities,
                                checked,
                                &context,
                                &plan.operations[operation],
                                ordinal,
                                emission.evaluation,
                                source_value_count,
                                emission.values,
                                emission.next_value,
                                emission.next_block,
                                emission.next_edge,
                                emission.operations,
                                emission.calls,
                                &mut environment,
                            )
                        }?;
                        self.next_call_obligation = self.scalar_calls.next_obligation_identity;
                        self.staged_subslices[operation].push((ordinal, place));
                        return Ok(());
                    }
                    argument_schedule::Step::Call(index)
                        if self.retained_scalar_prefix.is_some() =>
                    {
                        (index, true)
                    }
                    argument_schedule::Step::Operation(index)
                        if self.retained_scalar_prefix.is_some() =>
                    {
                        (index, true)
                    }
                    argument_schedule::Step::Ordinary(index)
                        if self.retained_scalar_prefix.is_none() =>
                    {
                        (index, false)
                    }
                    _ => {
                        return unsupported(
                            "call schedule disagrees with its active argument group",
                        );
                    }
                };
                let operation = &plan.operations[operation_index];
                if matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::ReleaseReference {
                        defer_to_exit: true,
                        ..
                    }
                ) {
                    return Ok(());
                }
                if matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::ContinuationCleanup { .. }
                ) {
                    let source_value_count = self.scalar_result_values.len();
                    return self.emit_through_frame(identities, operation, source_value_count);
                }
                let source_value_count = self
                    .retained_scalar_prefix
                    .unwrap_or(self.scalar_result_values.len());
                let evaluated_scalar_arguments = if staged {
                    Some(self.staged_arguments[operation_index].values(&self.scalar_result_values))
                } else {
                    {
                        let (mut environment, emission) = self.frame(source_value_count).split();
                        emission.evaluation.arguments(
                            identities,
                            checked,
                            plan.machine,
                            plan.state,
                            operation,
                            emission.values,
                            emission.next_value,
                            emission.next_block,
                            emission.next_edge,
                            emission.operations,
                            emission.calls,
                            &mut environment,
                        )
                    }?
                };
                self.next_call_obligation = self.scalar_calls.next_obligation_identity;
                let step = StepInputs {
                    operation_index,
                    staged,
                    finishes_argument_group,
                    source_value_count,
                    evaluated_scalar_arguments,
                    completed_structural_arguments: (!self.staged_structural_arguments
                        [operation_index]
                        .is_empty())
                    .then(|| self.staged_structural_arguments[operation_index].clone()),
                    evaluated_selectors: staged
                        .then(|| {
                            self.staged_selectors[operation_index]
                                .iter()
                                .map(|slot| {
                                    self.scalar_result_values.get(*slot).copied().ok_or(
                                        LoweringError::Unsupported(
                                            "staged selector lost its retained value",
                                        ),
                                    )
                                })
                                .collect::<Result<Vec<_>, _>>()
                        })
                        .transpose()?,
                };
                let kind = match operation {
                    CheckedUnitEffectOperationPlan::EvaluateStructuralValue { .. }
                    | CheckedUnitEffectOperationPlan::RegisterAccess { .. }
                    | CheckedUnitEffectOperationPlan::Store(_)
                    | CheckedUnitEffectOperationPlan::AtomicFence { .. }
                    | CheckedUnitEffectOperationPlan::AtomicAccess(_)
                    | CheckedUnitEffectOperationPlan::EstablishLocal(
                        checked_trees::CheckedUnitLocalBindingPlan::Scalar { .. },
                    )
                    | checked_trees::structural_field_write_pattern!()
                    | checked_trees::byte_sequence_establishment_pattern!()
                    | CheckedUnitEffectOperationPlan::EstablishLocal(
                        checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { .. },
                    )
                    | CheckedUnitEffectOperationPlan::EstablishLocal(
                        checked_trees::CheckedUnitLocalBindingPlan::Structural { .. },
                    )
                    | CheckedUnitEffectOperationPlan::EstablishLocal(
                        checked_trees::CheckedUnitLocalBindingPlan::ScalarArray { .. },
                    )
                    | checked_trees::reference_hold_pattern!() => {
                        return self.emit_through_frame(
                            identities,
                            operation,
                            step.source_value_count,
                        );
                    }
                    CheckedUnitEffectOperationPlan::Call(_)
                    | CheckedUnitEffectOperationPlan::GlobalDescriptorTable { .. } => {
                        return self.emit_call_through_frame(identities, operation, step);
                    }
                    CheckedUnitEffectOperationPlan::EstablishLocal(binding) => match binding {
                        checked_trees::CheckedUnitLocalBindingPlan::Primitive { .. } => {
                            let local = self
                                .frame(step.source_value_count)
                                .establish_primitive_local(identities, operation)?;
                            self.next_call_obligation = self.scalar_calls.next_obligation_identity;
                            self.primitive_local_places.push(local);
                            return Ok(());
                        }
                        checked_trees::CheckedUnitLocalBindingPlan::TrivialAffine { .. } => {
                            self.establish_trivial_affine_local(operation)?
                        }
                        _ => unreachable!("not an ordinary-machine local binding"),
                    },
                    CheckedUnitEffectOperationPlan::ContinuationCleanup { .. }
                    | CheckedUnitEffectOperationPlan::Complete { .. } => {
                        return unsupported("Unit return is not the final checked operation");
                    }
                };
                let Some(kind) = kind else {
                    return Ok(());
                };
                let id = self.operations.allocate();
                self.operations.push(Operation::unsuspended(
                    id,
                    terminal_psi::OperationResult::Unit,
                    kind,
                ));
                Ok(())
            }
        }

        impl<'a> MachineEmission<'a> {
            fn emit_through_frame(
                &mut self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                operation: &CheckedUnitEffectOperationPlan,
                source_value_count: usize,
            ) -> Result<(), LoweringError> {
                self.frame(source_value_count).emit(identities, operation)?;
                self.next_call_obligation = self.scalar_calls.next_obligation_identity;
                Ok(())
            }

            fn emit_call_through_frame(
                &mut self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                operation: &CheckedUnitEffectOperationPlan,
                step: StepInputs,
            ) -> Result<(), LoweringError> {
                let byte_places = byte_subslices::argument_places(
                    operation.structural_operands(),
                    &self.private_places,
                    &mut self.next_literal_argument,
                    &self.staged_subslices[step.operation_index],
                )?;
                if step.staged && step.finishes_argument_group {
                    self.scalar_result_values.truncate(step.source_value_count);
                }
                self.frame(step.source_value_count).emit_call(
                    identities,
                    operation,
                    CallInputs {
                        evaluated_scalar_arguments: step.evaluated_scalar_arguments.as_deref(),
                        byte_places: &byte_places,
                        evaluated_selectors: step.evaluated_selectors.as_deref(),
                        completed_structural_arguments: step
                            .completed_structural_arguments
                            .as_deref(),
                    },
                )?;
                self.next_call_obligation = self.scalar_calls.next_obligation_identity;
                if step.staged && step.finishes_argument_group {
                    self.retained_scalar_prefix = Some(self.scalar_result_values.len());
                }
                Ok(())
            }

            fn frame(&mut self, source_value_count: usize) -> OperationFrame<'_, 'a> {
                let plan = self.plan;
                OperationFrame {
                    checked: self.checked,
                    machine: plan.machine,
                    state: plan.state,
                    scalar_result: plan.scalar_result.as_ref(),
                    scalar_parameter_count: self.scalar_parameter_count,
                    source_value_count,
                    parameters: self.evaluation.current_parameters(self.parameters),
                    structural_types: StructuralTypeRoster::Owned(self.structural_types),
                    type_ids: self.type_ids,
                    service_ids: self.service_ids,
                    primitive_locals: &self.primitive_local_places,
                    results: StructuralResults::Dense(&mut self.structural_result_places),
                    private_places: &mut self.private_places,
                    windows: &mut self.borrowed_windows,
                    evaluation: &mut self.evaluation,
                    values: &mut self.scalar_result_values,
                    next_place: &mut self.next_place,
                    next_value: &mut self.next_value_identity,
                    next_block: &mut self.next_block,
                    next_edge: &mut self.next_edge,
                    calls: &mut self.scalar_calls,
                    operations: &mut self.operations,
                    callees: Callees {
                        plans: self.plans,
                        signatures: self.machine_signatures,
                        boundaries: self.lowered_boundary_parameters,
                        domain_ids: self.domain_ids,
                        domain_declarations: self.domain_declarations,
                        closure: self.closure,
                    },
                    caller: CallerCustody {
                        erased_scalar_parameters: &self.signature.erased_scalar_parameters,
                        erased_proof_parameters: &self.signature.erased_proof_parameters,
                        local_places: &self.local_places,
                    },
                }
            }
        }
    }
    pub(crate) mod quotients {
        use crate::emission::operation_emission::buffer::SourceQuotientAdaptation;
        use crate::shared::lowering_error::LoweringError;
        use crate::shared::terminal_identities::{
            lookup_machine_id, lookup_type_id, stored_structural_identity,
        };
        use checked_trees::{
            CheckedQuotientAdaptationLicense, CheckedStructuralValueKind, CheckedTrees,
        };
        use terminal_psi::{
            TerminalQuotientAdaptation, TerminalQuotientAdaptationLicense, TerminalQuotientCatalog,
            TerminalQuotientRepresentation,
        };

        pub(crate) fn retain(
            checked: &CheckedTrees,
            _identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            _exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
            structural_types: &[terminal_psi::StructuralTypeDeclaration],
            emitted: &[SourceQuotientAdaptation],
            calls: &[crate::emission::operation_emission::buffer::SourceQuotientCall],
            machine_ids: &[(symbols::SymbolHandle, semantic_vocabulary::MachineId)],
            type_ids: &[(
                typed_trees::type_identity::TypeIdentityId,
                semantic_vocabulary::StructuralTypeId,
            )],
        ) -> Result<TerminalQuotientCatalog, LoweringError> {
            let mut catalog = TerminalQuotientCatalog::default();
            for (reference, _) in checked.typed.type_reference_table.type_reference_rows() {
                let Some(definition) = checked.typed.closed_data_definition(reference) else {
                    continue;
                };
                let Some(quotient) = definition.quotient.as_ref() else {
                    continue;
                };
                let identity = validation::stored_structural_type_identity(
                    &checked.typed,
                    reference,
                    &[],
                    &[],
                );
                let Some((_, quotient_id)) = type_ids.iter().find(|(id, _)| *id == identity) else {
                    continue;
                };
                if catalog
                    .representations
                    .iter()
                    .any(|row| row.quotient == *quotient_id)
                {
                    continue;
                }
                let carrier = lookup_type_id(
                    type_ids,
                    &stored_structural_identity(checked, quotient.carrier),
                )?;
                let formation = validation::checked_quotient_relation_identity(
                    &checked.typed,
                    reference,
                    &checked.facts.quotient_formations,
                )
                .map_err(|error| LoweringError::InvalidQuotientCorrespondence(vec![error]))?;
                catalog
                    .representations
                    .push(TerminalQuotientRepresentation {
                        quotient_storage_identity: storage_identity(
                            structural_types,
                            *quotient_id,
                        )?,
                        carrier_storage_identity: storage_identity(structural_types, carrier)?,
                        quotient: *quotient_id,
                        carrier,
                        formation,
                    });
            }

            for call in calls {
                let node = checked.facts.values.structural_values.nodes.get(call.value);
                let CheckedStructuralValueKind::QuotientCall {
                    request,
                    representative_state,
                    ..
                } = &node.kind
                else {
                    return Err(LoweringError::Unsupported(
                        "quotient call occurrence has no checked invocation",
                    ));
                };
                let mut rows = checked.facts.quotient_operations.iter().filter(|row| {
                    row.request == *request
                        && row.public_machine == node.machine
                        && row.public_state == node.state
                        && row.representative_state == *representative_state
                });
                let row = rows.next().ok_or(LoweringError::Unsupported(
                    "quotient call lost its checked correspondence",
                ))?;
                if rows.next().is_some() {
                    return Err(LoweringError::Unsupported(
                        "quotient call has ambiguous correspondence",
                    ));
                }
                let (representative, _) =
                    validation::find_machine_by_entry_state(&checked.typed, *representative_state)
                        .ok_or(LoweringError::Unsupported(
                            "quotient representative is absent",
                        ))?;
                let (_, entry) =
                    validation::find_machine_by_entry_state(&checked.typed, *representative_state)
                        .ok_or(LoweringError::Unsupported(
                            "quotient representative is absent",
                        ))?;
                let input_relations = checked
                    .typed
                    .state_parameters(entry)
                    .iter()
                    .zip(&row.correspondence.input_relations)
                    .filter(|(parameter, _)| {
                        checked
                            .typed
                            .primitive_type_reference(parameter.type_reference)
                            .is_none()
                    })
                    .map(|(_, relation)| relation.clone())
                    .collect();
                catalog
                    .operations
                    .push(terminal_psi::TerminalQuotientOperation {
                        public_machine: lookup_machine_id(machine_ids, node.machine)?,
                        representative_machine: lookup_machine_id(
                            machine_ids,
                            representative.symbol,
                        )?,
                        call: call.call,
                        input_relations,
                        result_relation: row.correspondence.result_relation.clone(),
                    });
            }
            for row in emitted {
                let node = checked.facts.values.structural_values.nodes.get(row.value);
                let CheckedStructuralValueKind::QuotientAdaptation {
                    quotient, license, ..
                } = node.kind
                else {
                    return Err(LoweringError::Unsupported(
                        "quotient copy lost its checked adaptation",
                    ));
                };
                let formation = validation::checked_quotient_relation_identity(
                    &checked.typed,
                    quotient,
                    &checked.facts.quotient_formations,
                )
                .map_err(|error| LoweringError::InvalidQuotientCorrespondence(vec![error]))?;
                let quotient_id = lookup_type_id(
                    type_ids,
                    &validation::stored_structural_type_identity(
                        &checked.typed,
                        quotient,
                        &[],
                        &[],
                    ),
                )?;
                let (carrier, license) = match license {
                    CheckedQuotientAdaptationLicense::Mint => {
                        (row.source_type, TerminalQuotientAdaptationLicense::Mint)
                    }
                    CheckedQuotientAdaptationLicense::Input { request, position } => {
                        let operation = operation_index(checked, calls, node.machine, request)?;
                        let invocation = checked
                            .facts
                            .quotient_operations
                            .iter()
                            .find(|operation| {
                                operation.request == request
                                    && operation.public_machine == node.machine
                            })
                            .ok_or(LoweringError::Unsupported(
                                "quotient input lost its invocation",
                            ))?;
                        let (_, entry) = validation::find_machine_by_entry_state(
                            &checked.typed,
                            invocation.representative_state,
                        )
                        .ok_or(LoweringError::Unsupported(
                            "quotient input lost its representative",
                        ))?;
                        let parameters = checked.typed.state_parameters(entry);
                        let prefix = parameters.get(..position as usize).ok_or(
                            LoweringError::Unsupported("quotient input source position is absent"),
                        )?;
                        let position = u32::try_from(
                            prefix
                                .iter()
                                .filter(|parameter| {
                                    checked
                                        .typed
                                        .primitive_type_reference(parameter.type_reference)
                                        .is_none()
                                })
                                .count(),
                        )
                        .map_err(|_| {
                            LoweringError::Unsupported("quotient structural position overflow")
                        })?;
                        (
                            row.result_type,
                            TerminalQuotientAdaptationLicense::Input {
                                operation,
                                position,
                            },
                        )
                    }
                    CheckedQuotientAdaptationLicense::Result { request } => {
                        let operation = operation_index(checked, calls, node.machine, request)?;
                        (
                            row.source_type,
                            TerminalQuotientAdaptationLicense::Result { operation },
                        )
                    }
                };
                let representation = TerminalQuotientRepresentation {
                    quotient_storage_identity: storage_identity(structural_types, quotient_id)?,
                    carrier_storage_identity: storage_identity(structural_types, carrier)?,
                    quotient: quotient_id,
                    carrier,
                    formation,
                };
                let index = if let Some(index) = catalog
                    .representations
                    .iter()
                    .position(|existing| existing == &representation)
                {
                    index
                } else {
                    let index = catalog.representations.len();
                    catalog.representations.push(representation);
                    index
                };
                catalog.adaptations.push(TerminalQuotientAdaptation {
                    machine: lookup_machine_id(machine_ids, node.machine)?,
                    copy: row.copy,
                    source: row.source,
                    result: row.result,
                    source_type: row.source_type,
                    result_type: row.result_type,
                    representation: u32::try_from(index).map_err(|_| {
                        LoweringError::Unsupported("quotient representation roster overflow")
                    })?,
                    license,
                });
            }
            Ok(catalog)
        }

        fn operation_index(
            checked: &CheckedTrees,
            calls: &[crate::emission::operation_emission::buffer::SourceQuotientCall],
            machine: symbols::SymbolHandle,
            request: typed_trees::expression::ExpressionHandle,
        ) -> Result<u32, LoweringError> {
            let mut matches = calls.iter().enumerate().filter(|(_, call)| {
                let node = checked.facts.values.structural_values.nodes.get(call.value);
                node.machine == machine && matches!(&node.kind,
                    CheckedStructuralValueKind::QuotientCall { request: selected, .. } if *selected == request)
            });
            let (index, _) = matches.next().ok_or(LoweringError::Unsupported(
                "quotient adaptation has no emitted call",
            ))?;
            if matches.next().is_some() {
                return Err(LoweringError::Unsupported(
                    "quotient adaptation has ambiguous emitted calls",
                ));
            }
            u32::try_from(index)
                .map_err(|_| LoweringError::Unsupported("quotient operation roster overflow"))
        }

        fn storage_identity(
            declarations: &[terminal_psi::StructuralTypeDeclaration],
            id: semantic_vocabulary::StructuralTypeId,
        ) -> Result<terminal_psi::TerminalIdentityIndex, LoweringError> {
            let mut rows = declarations.iter().filter(|row| row.id == id);
            let row = rows.next().ok_or(LoweringError::Unsupported(
                "quotient storage declaration is absent",
            ))?;
            if rows.next().is_some() {
                return Err(LoweringError::Unsupported(
                    "quotient storage declaration is ambiguous",
                ));
            }
            Ok(row.identity)
        }
    }
    pub(crate) mod structural_returns {

        use crate::emission::operation_emission::buffer::OperationBuffer;
        use crate::shared::lowering_error::LoweringError;
        use crate::shared::terminal_identities::{allocate_dense, block_id, edge_id, place_id};
        use crate::unit::attached_unit::argument_evaluation::Evaluation;
        use crate::unit::unit_cleanup::CleanupRegion;
        use semantic_vocabulary::StructuralPlaceKind;
        use terminal_psi::{
            Block, StructuralAccess, StructuralArgument, StructuralParameterDeclaration,
            StructuralPlaceDeclaration, Terminator,
        };

        pub(crate) fn materialize(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &checked_trees::CheckedTrees,
            source: &StructuralArgument,
            structural_type: semantic_vocabulary::StructuralTypeId,
            multiplicity: terminal_psi::StructuralMultiplicity,
            completion: Terminator,
            regions: Vec<CleanupRegion>,
            next_place: &mut u64,
            temporary_places: &mut Vec<StructuralPlaceDeclaration>,
            evaluation: &mut Evaluation,
            values: &mut Vec<terminal_psi::ValueDeclaration>,
            next_value: &mut u64,
            next_block: &mut u64,
            next_edge: &mut u64,
            operations: &mut OperationBuffer,
        ) -> Result<(Terminator, Option<Block>), LoweringError> {
            if source.path.is_empty() {
                let mut completion = completion;
                let Terminator::ReturnStructural {
                    cleanup_actions, ..
                } = &mut completion
                else {
                    return Err(LoweringError::Unsupported(
                        "structural return lost its completion",
                    ));
                };
                *cleanup_actions = evaluation.emit_cleanup_regions(
                    identities, checked, regions, values, next_value, next_block, next_edge,
                    operations,
                )?;
                return Ok((completion, None));
            }
            let Terminator::ReturnStructural {
                source: returned_source,
                returned_claims,
                ..
            } = completion
            else {
                return Err(LoweringError::Unsupported(
                    "projected return lost its completion",
                ));
            };
            if returned_source != source.place {
                return Err(LoweringError::Unsupported(
                    "projected return lost its current root",
                ));
            }
            let block = block_id(allocate_dense(next_block)?);
            let place = place_id(allocate_dense(next_place)?);
            temporary_places.push(StructuralPlaceDeclaration {
                id: place,
                kind: StructuralPlaceKind::BlockParameter { block, position: 0 },
            });
            let mut arrival_block = block;
            let mut arrival_parameters = Vec::new();
            let mut structural_parameters = vec![StructuralParameterDeclaration {
                place,
                position: 0,
                structural_type,
                multiplicity,
                access: StructuralAccess::Owned,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                additional_input_qualifications: Vec::new(),
            }];
            let selected = regions.iter().any(|region| {
                region.partition_leaf.is_some()
                    || !region.conditions.is_empty()
                    || region.selected_region.is_some()
            });
            let capture_operation_end = operations.len();
            let mut operation_start = capture_operation_end;
            let cleanup_actions = if selected {
                crate::unit::attached_unit::argument_evaluation::cleanup_control::CleanupEmission {
                    structural_types: &evaluation.structural_types,
                    source_parameters: &evaluation.structural_parameters,
                    previous_parameter_homes: &evaluation.previous_parameter_homes,
                    checked,
                    current: &mut arrival_block,
                    parameters: &mut arrival_parameters,
                    structural_parameters: &mut structural_parameters,
                    block_erased_scalar_formals: &mut Vec::new(),
                    erased_proof_formals: &evaluation.erased_proof_formals,
                    operation_start: &mut operation_start,
                    blocks: &mut evaluation.blocks,
                }
                .emit(
                    identities,
                    &regions,
                    &mut Vec::new(),
                    next_value,
                    next_block,
                    next_edge,
                    operations,
                )?;
                Vec::new()
            } else {
                regions
                    .into_iter()
                    .filter_map(|region| region.action.into_physical())
                    .collect()
            };
            let arrival = Block {
                id: arrival_block,
                parameters: arrival_parameters,
                erased_scalar_formals: Vec::new(),
                erased_proof_formals:
                    crate::scalar_lowering::contracts::erased_proof_formal_declarations(
                        identities,
                        checked,
                        &evaluation.erased_proof_formals,
                    ),
                structural_parameters,
                operations: operations[operation_start..].to_vec(),
                terminator: Terminator::ReturnStructural {
                    edge: edge_id(allocate_dense(next_edge)?),
                    source: place,
                    returned_claims,
                    cleanup_actions: Vec::new(),
                },
            };
            operations.truncate(capture_operation_end);
            Ok((
                Terminator::Jump {
                    edge: edge_id(allocate_dense(next_edge)?),
                    target: block,
                    arguments: Vec::new(),
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: (0..evaluation.erased_proof_formals.len())
                        .map(|position| {
                            Ok(semantic_vocabulary::ProofTerm::Formal {
                                position: u32::try_from(position).map_err(|_| {
                                    LoweringError::Unsupported(
                                        "projected return erased-proof roster exceeds u32",
                                    )
                                })?,
                            })
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?,
                    structural_arguments: vec![source.clone()],
                    cleanup_actions,
                },
                Some(arrival),
            ))
        }
    }

    pub(crate) mod byte_locals {
        use crate::shared::content_classes::plain_owned_with_represented_constraints;
        use crate::shared::lowering_error::{LoweringError, unsupported};
        use checked_trees::{CheckedTrees, CheckedUnitEffectOperationPlan};

        pub(crate) fn validate(
            checked: &CheckedTrees,
            state: symbols::SymbolHandle,
            operation: &CheckedUnitEffectOperationPlan,
        ) -> Result<(), LoweringError> {
            let CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::ByteSequence { result, value, .. },
            ) = operation
            else {
                return unsupported("literal local has no establishment");
            };
            let reference = if result.local.is_named() {
                result.local.declared_type
            } else {
                let (_, authored) = crate::shared::declarations::state_declaration(checked, state)?;
                let shared_view =
                    validation::slice_view_shell(&checked.typed, authored.return_type, &[])
                        .is_some_and(|view| {
                            view.access == language_semantics::ReferenceAccess::Shared
                        });
                if !shared_view
                    && (crate::shared::content_classes::bounded_byte_capacity(
                        checked,
                        authored.return_type,
                    )
                    .is_none()
                        || !plain_owned_with_represented_constraints(checked, authored.return_type))
                {
                    return unsupported("literal completion is not an owned bounded byte result");
                }
                authored.return_type
            };
            let declared_reference = reference;
            let mut reference = reference;
            loop {
                match checked.type_reference_table.type_reference(reference) {
                    checked_trees::types::TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    } => {
                        for constraint in checked.type_reference_table.constraints(*constraints) {
                            match constraint {
                                checked_trees::types::TypeConstraintNode::Domain(domain)
                                    if domain.semantic_id.is_valid() => {}
                                checked_trees::types::TypeConstraintNode::ArithmeticDomain(_) => {}
                                _ => {
                                    return unsupported(
                                        "literal local has an unsupported qualification",
                                    );
                                }
                            }
                        }
                        reference = *base_type;
                    }
                    checked_trees::types::TypeReferenceNode::Reference { referee, .. } => {
                        reference = *referee
                    }
                    _ => break,
                }
            }
            let reference =
                super::parameters::structural_carrier_type(checked, declared_reference)?;
            if crate::shared::content_classes::bounded_byte_capacity(checked, declared_reference)
                .is_some()
            {
                return Ok(());
            }
            if value.literal_bytes().is_none() {
                return unsupported("borrowed byte local requires literal storage");
            }
            let checked_trees::types::TypeReferenceNode::Reference {
                referee,
                access: language_semantics::ReferenceAccess::Shared,
                ..
            } = checked.type_reference_table.type_reference(reference)
            else {
                return unsupported("literal local is not a shared byte view");
            };
            let carrier = super::parameters::structural_carrier_type(checked, *referee)?;
            let checked_trees::types::TypeReferenceNode::Slice { .. } =
                checked.type_reference_table.type_reference(carrier)
            else {
                return unsupported("literal local is not a byte slice");
            };
            Ok(())
        }
    }
    pub(crate) mod byte_subslices {
        use super::view_ranges::{self, ViewRangeSite};
        use super::{
            CheckedTrees, CheckedUnitEffectOperationPlan, LoweringError, PlaceId,
            StructuralPlaceDeclaration, StructuralTypeId, ValueDeclaration, allocate_dense,
            literal_argument_places, lookup_type_id, place_id, unsupported,
        };
        use crate::emission::operation_emission::buffer::OperationBuffer;
        use crate::expression_preparation::bindings::ScalarBindings;
        use checked_trees::CheckedUnitStructuralArgumentPlan;

        pub(crate) fn arguments(
            operation: &CheckedUnitEffectOperationPlan,
        ) -> &[CheckedUnitStructuralArgumentPlan] {
            operation.structural_operands()
        }

        pub(crate) use super::view_ranges::range;

        pub(crate) fn contains(operation: &CheckedUnitEffectOperationPlan) -> bool {
            arguments(operation)
                .iter()
                .any(|argument| range(argument).is_some())
        }

        fn locate<'a>(
            state: symbols::SymbolHandle,
            operation: &'a CheckedUnitEffectOperationPlan,
            ordinal: usize,
        ) -> Result<(&'a CheckedUnitStructuralArgumentPlan, ViewRangeSite<'a>), LoweringError>
        {
            let argument = arguments(operation)
                .get(ordinal)
                .ok_or(LoweringError::Unsupported(
                    "subslice has no checked argument",
                ))?;
            let range = range(argument).ok_or(LoweringError::Unsupported(
                "subslice schedule names a different argument kind",
            ))?;
            let Some(call) = operation.call() else {
                return unsupported("subslice schedule has no call coordinate");
            };
            if !matches!(
                argument.access,
                checked_trees::CheckedStructuralAccess::SharedBorrow
                    | checked_trees::CheckedStructuralAccess::MutableBorrow
            ) {
                return unsupported("subslice source or result changed immutable view custody");
            }
            let site = ViewRangeSite {
                state,
                statement: call.coordinate.statement_index,
                range: range.endpoints(),
            };
            Ok((argument, site))
        }

        pub(crate) struct StagingContext<'a> {
            pub(crate) state: symbols::SymbolHandle,
            pub(crate) structural_types: &'a [terminal_psi::StructuralTypeDeclaration],
            pub(crate) type_ids:
                &'a [(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
            pub(crate) bindings: &'a ScalarBindings,
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn emit(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            context: &StagingContext<'_>,
            operation: &CheckedUnitEffectOperationPlan,
            ordinal: usize,
            evaluation: &mut super::argument_evaluation::Evaluation,
            source_value_count: usize,
            values: &mut Vec<ValueDeclaration>,
            next_value: &mut u64,
            next_block: &mut u64,
            next_edge: &mut u64,
            operations: &mut OperationBuffer,
            calls: &mut crate::emission::operation_emission::calls::CallEmissionContext<'_>,
            environment: &mut super::OperationEnvironment<'_>,
        ) -> Result<PlaceId, LoweringError> {
            let (argument, site) = locate(context.state, operation, ordinal)?;
            let structural_type = lookup_type_id(context.type_ids, &argument.type_identity)?;
            let destination = place_id(allocate_dense(environment.next_place)?);
            let (source, establishment) = view_ranges::stage_source(
                identities,
                checked,
                context.state,
                site.statement,
                argument,
                context.structural_types,
                context.bindings,
                environment.next_place,
                values,
                next_value,
                operations,
            )?;
            if let Some(declaration) = establishment {
                environment.private_places.push(declaration);
            }
            let declaration = view_ranges::emit(
                identities,
                checked,
                site,
                source,
                structural_type,
                destination,
                evaluation,
                source_value_count,
                values,
                next_value,
                next_block,
                next_edge,
                operations,
                calls,
                environment,
            )?;
            environment.private_places.push(declaration);
            Ok(destination)
        }

        pub(crate) fn argument_places(
            arguments: &[CheckedUnitStructuralArgumentPlan],
            literals: &[StructuralPlaceDeclaration],
            next_literal: &mut usize,
            derived: &[(usize, PlaceId)],
        ) -> Result<Vec<PlaceId>, LoweringError> {
            let mut places = Vec::new();
            for (ordinal, argument) in arguments.iter().enumerate() {
                if argument.byte_sequence_literal().is_some() {
                    places.extend(literal_argument_places(
                        std::slice::from_ref(argument),
                        literals,
                        next_literal,
                    )?);
                } else if range(argument).is_some() {
                    places.push(
                        derived
                            .iter()
                            .find_map(|(index, place)| (*index == ordinal).then_some(*place))
                            .ok_or(LoweringError::Unsupported(
                                "subslice was not evaluated at its argument position",
                            ))?,
                    );
                }
            }
            Ok(places)
        }
    }
    pub(crate) mod ordinary_calls {

        use super::super::{
            BTreeMap, MachineId, ObligationId, Proposition, ScalarTerm, StructuralArgument,
            StructuralParameterDeclaration, StructuralTypeDeclaration,
            substitute_structural_crash_route_roots,
        };
        use super::parameters::call_index_selectors;
        use super::{
            CheckedTrees, CheckedUnitEffectOperationPlan, LoweringError, OperationResult, PlaceId,
            StructuralPlaceDeclaration, StructuralTypeId, ValueDeclaration, allocate_dense,
            argument_evaluation, lookup_type_id, lower_checked_crash_route_buckets,
            lower_structural_arguments, lower_structural_crash_route_buckets, place_id,
            primitive_locals, unsupported,
        };
        use crate::emission::claim_bindings::emitted_claim_transfers;
        use crate::emission::operation_emission::buffer::{
            OperationBuffer, SourceCallCoordinate, SourceInvocation,
        };
        use crate::emission::operation_emission::calls::CallEmissionContext;
        use crate::emission::operation_emission::place_declarations::operation_result_place;
        use crate::scalar_lowering::contracts::lowered_proof_term;
        use crate::shared::direct_calls::{
            DirectCall, DirectCallResult, ModuleInvocationArguments,
        };
        use crate::shared::terminal_identities::StructuralDomainBinding;
        use crate::unit::runtime_requirements::substitute_runtime_requirement_scalar_values;

        pub(crate) struct Target<'a> {
            pub parameters: &'a [StructuralParameterDeclaration],
            pub scalar_parameters: &'a [ValueDeclaration],
            pub erased_scalar_parameters: &'a [ValueDeclaration],
            pub predicate_parameters: &'a [StructuralParameterDeclaration],
            pub requires: &'a [Proposition],
            pub runtime_requirements: &'a [Proposition],
            pub source_checked_requires: &'a [Proposition],
        }

        impl ModuleInvocationArguments {
            #[allow(clippy::too_many_arguments)]
            pub(crate) fn emit(
                self,
                checked: &CheckedTrees,
                operations: &mut OperationBuffer,
                coordinate: impl Into<SourceInvocation>,
                record: crate::shared::direct_calls::CallRecord<'_>,
                result: OperationResult,
                callee: MachineId,
                result_kind: DirectCallResult,
            ) -> Result<semantic_vocabulary::OperationId, LoweringError> {
                let coordinate = coordinate.into();
                let (target, _, source_site) =
                    crate::shared::call_sites::checked_invocation(checked, coordinate)?;
                crate::shared::direct_calls::emit_direct_call(
                    operations,
                    coordinate,
                    source_site,
                    target,
                    record,
                    result,
                    DirectCall::new(
                        callee,
                        self.arguments,
                        self.erased_arguments,
                        self.erased_proof_arguments,
                        self.structural_arguments,
                        self.claim_transfers,
                        self.requirement_obligations,
                        self.crash_continuations,
                        result_kind,
                    ),
                )
            }
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn prepare(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            scalar_applications: &super::scalar_applications::SourceScalarApplications,
            state: symbols::SymbolHandle,
            operation: &CheckedUnitEffectOperationPlan,
            target: Target<'_>,
            evaluated_scalar_arguments: Option<&[ValueDeclaration]>,
            evaluated_selectors: Option<&[ValueDeclaration]>,
            completed_structural_arguments: Option<
                &[Option<super::operation_frame::CompletedStructuralArgument>],
            >,
            caller_scalar_values: &[ValueDeclaration],
            caller_erased_scalar_parameters: &[ValueDeclaration],
            caller_erased_proof_parameters: &[checked_trees::CheckedErasedProofParameterPlan],
            caller_observation_parameters: &[(u32, StructuralParameterDeclaration)],
            parameters: &[StructuralParameterDeclaration],
            local_places: &[StructuralPlaceDeclaration],
            structural_result_places: &[(Option<StructuralPlaceDeclaration>, bool)],
            primitive_local_places: &[primitive_locals::PrimitiveLocal],
            structural_locals: &[crate::expression_preparation::bindings::StructuralLocalBinding],
            scalar_bindings: &crate::expression_preparation::bindings::ScalarBindings,
            structural_types: &[StructuralTypeDeclaration],
            call_byte_places: &[PlaceId],
            calls: &mut CallEmissionContext<'_>,
            next_value: &mut u64,
            operations: &mut OperationBuffer,
            current_claims: &mut crate::emission::current_claims::CurrentClaimNames,
        ) -> Result<ModuleInvocationArguments, LoweringError> {
            let Some((call, erased_scalar_arguments, erased_proof_arguments)) = operation
                .call()
                .filter(|call| call.is_module())
                .and_then(|call| {
                    Some((
                        call,
                        call.erased_scalar_arguments()?,
                        call.erased_proof_arguments()?,
                    ))
                })
            else {
                return unsupported("ordinary call preparation has no call operation");
            };
            let target_machine = &call.target.machine;
            let scalar_arguments = &call.scalar_arguments;
            let structural_arguments = &call.structural_arguments;
            let claim_transfers = call.claim_transfers();
            if structural_arguments.len() != target.parameters.len() {
                return unsupported("ordinary call has an unsupported structural operand arity");
            }
            let erased_proof_arguments = erased_proof_arguments
                .iter()
                .map(|argument| {
                    crate::scalar_lowering::contracts::checked_proof_term(
                        identities,
                        checked,
                        argument,
                        caller_erased_proof_parameters,
                    )
                    .and_then(|term| {
                        lowered_proof_term(
                            checked,
                            &term,
                            caller_scalar_values,
                            caller_erased_scalar_parameters,
                        )
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            let erased_arguments = erased_scalar_arguments
                .iter()
                .map(|argument| {
                    let checked_trees::CheckedCallScalarArgument::Pure(expression) = argument
                    else {
                        return unsupported(
                            "erased Unit call actual must be a pure checked expression",
                        );
                    };
                    crate::scalar_lowering::contracts::erased_actual_term(
                        identities,
                        expression,
                        caller_scalar_values,
                        caller_erased_scalar_parameters,
                        &crate::scalar_lowering::contracts::ContractViewNamespace {
                            scalar_applications,
                            constant_values: &checked.typed.constant_recipe_closure.values,
                            parameters: caller_observation_parameters,
                            structural_types,
                            ..crate::scalar_lowering::contracts::ContractViewNamespace::EMPTY
                        },
                        structural_locals,
                    )
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            if scalar_arguments.len() != target.scalar_parameters.len() {
                return unsupported("Unit call scalar argument count disagrees with its target");
            }
            let terminal_scalar_values = argument_evaluation::validated_values(
                evaluated_scalar_arguments,
                &target
                    .scalar_parameters
                    .iter()
                    .map(|parameter| parameter.value_type())
                    .collect::<Vec<_>>(),
            )?;
            let terminal_scalar_arguments = terminal_scalar_values
                .iter()
                .map(|value| value.id)
                .collect();
            let mut selector_values =
                call_index_selectors(structural_arguments, evaluated_selectors)?.into_iter();

            let terminal_arguments = lower_structural_arguments(
                identities,
                checked,
                structural_arguments,
                completed_structural_arguments,
                structural_types,
                parameters,
                local_places,
                structural_result_places,
                &operations.structural_values.clone(),
                call_byte_places,
                primitive_local_places,
                structural_locals,
                Some(scalar_bindings),
                Some(caller_scalar_values),
                &mut selector_values,
                Some(calls),
                Some((next_value, operations)),
            )?;

            let claim_transfers = emitted_claim_transfers(
                identities,
                claim_transfers,
                calls.claims.bindings(),
                target.parameters,
                structural_types,
                &terminal_arguments,
                current_claims,
            )?;

            let caller_types = scalar_bindings
                .published_structural_types()?
                .with_completed(operations)?;
            let crash_continuations = prepare_crash_continuations(
                identities,
                checked,
                SourceCallCoordinate {
                    state,
                    statement_index: call.coordinate.statement_index as usize,
                    call_ordinal: call.coordinate.call_ordinal as usize,
                },
                &operations.loan_captures,
                &operations.operations,
                Target { ..target },
                &crate::unit::effective_crash_routes(checked, *target_machine)?,
                &terminal_scalar_values,
                &erased_arguments,
                &terminal_arguments,
                structural_types,
                &caller_types,
            )?;
            let requirement_obligations =
                prepare_requirement_obligations(&target, calls, operations)?;
            Ok(ModuleInvocationArguments {
                arguments: terminal_scalar_arguments,
                erased_arguments,
                erased_proof_arguments,
                structural_arguments: terminal_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            })
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn prepare_computed_module_call(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            coordinate: SourceInvocation,
            caller_types: &crate::expression_preparation::bindings::PublishedStructuralTypes,
            target_machine: symbols::SymbolHandle,
            arguments: &[ValueDeclaration],
            erased_arguments: Vec<semantic_vocabulary::ScalarTerm>,
            erased_proof_arguments: &[crate::scalar_lowering::contracts::LoweredProofTerm],
            structural_arguments: &[crate::emission::operation_emission::calls::PreparedStructuralArgument],
            claim_transfers: &[crate::emission::claim_bindings::PreparedClaimTransfer],
            crash_routes: &[checked_trees::CrashRouteBucket],
            source_values: &[ValueDeclaration],
            caller_erased_formals: &[ValueDeclaration],
            calls: &mut CallEmissionContext<'_>,
            next_value: &mut u64,
            operations: &mut OperationBuffer,
            current_claims: &mut crate::emission::current_claims::CurrentClaimNames,
        ) -> Result<ModuleInvocationArguments, LoweringError> {
            let signature = calls
                .signatures
                .iter()
                .find(|signature| signature.source == target_machine);
            let requirement_obligations = if let Some(signature) = signature {
                prepare_requirement_obligations(&signature.call_target(), calls, operations)?
            } else {
                let requirement_count = calls
                    .requirement_counts
                    .iter()
                    .find_map(|(source, count)| (*source == target_machine).then_some(*count))
                    .ok_or(LoweringError::Unsupported(
                        "computed call target has no prepared contract",
                    ))?;
                (0..requirement_count)
                    .map(|_| calls.allocate_requirement())
                    .collect::<Result<Vec<_>, _>>()?
            };
            let erased_proof_arguments = erased_proof_arguments
                .iter()
                .map(|term| lowered_proof_term(checked, term, source_values, caller_erased_formals))
                .collect::<Result<Vec<_>, LoweringError>>()?;
            let emitted_structural_arguments = structural_arguments
                .iter()
                .map(|argument| argument.emit(source_values, calls, next_value, operations))
                .collect::<Result<Vec<_>, _>>()?;
            let caller_types = caller_types.clone().with_completed(operations)?;
            let crash_continuations = match signature {
                Some(signature) => prepare_crash_continuations(
                    identities,
                    checked,
                    coordinate,
                    &operations.loan_captures,
                    &operations.operations,
                    signature.call_target(),
                    crash_routes,
                    arguments,
                    &erased_arguments,
                    &emitted_structural_arguments,
                    calls.structural_types,
                    &caller_types,
                )?,
                None if structural_arguments.is_empty() => {
                    lower_checked_crash_route_buckets(identities, crash_routes, arguments)?
                }
                None => {
                    return unsupported("computed call with structural operands has no signature");
                }
            };
            let claim_transfers = crate::emission::claim_bindings::emit_prepared_claim_transfers(
                claim_transfers,
                calls.claims.bindings(),
                &emitted_structural_arguments,
                current_claims,
            )?;
            Ok(ModuleInvocationArguments {
                arguments: arguments.iter().map(|argument| argument.id).collect(),
                erased_arguments,
                erased_proof_arguments,
                structural_arguments: emitted_structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            })
        }

        pub(crate) fn prepare_requirement_obligations(
            target: &Target<'_>,
            calls: &mut CallEmissionContext<'_>,
            operations: &mut OperationBuffer,
        ) -> Result<Vec<ObligationId>, LoweringError> {
            target
                .requires
                .iter()
                .map(|requirement| {
                    let obligation = calls.allocate_requirement()?;
                    if target.source_checked_requires.contains(requirement) {
                        operations.source_checked_obligations.push(obligation);
                    }
                    Ok(obligation)
                })
                .collect()
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn prepare_crash_continuations(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            coordinate: impl Into<SourceInvocation>,
            captures: &[lowered_psi::LoweredSourceLoanCapture],
            operations: &[terminal_psi::Operation],
            target: Target<'_>,
            routes: &[checked_trees::CrashRouteBucket],
            terminal_scalar_values: &[ValueDeclaration],
            erased_arguments: &[ScalarTerm],
            terminal_arguments: &[StructuralArgument],
            structural_types: &[StructuralTypeDeclaration],
            caller_types: &crate::expression_preparation::bindings::PublishedStructuralTypes,
        ) -> Result<Vec<terminal_psi::CrashRouteBucket>, LoweringError> {
            let coordinate = coordinate.into();
            if target.parameters.len() != terminal_arguments.len() {
                return unsupported("ordinary call has an unsupported structural operand arity");
            }
            if target.scalar_parameters.len() != terminal_scalar_values.len() {
                return unsupported("Unit call scalar requirement arity is inconsistent");
            }
            if target.erased_scalar_parameters.len() != erased_arguments.len() {
                return unsupported("call erased scalar requirement arity is inconsistent");
            }
            let mut scalar_substitutions = target
                .scalar_parameters
                .iter()
                .zip(terminal_scalar_values)
                .map(|(formal, actual)| {
                    if formal.scalar_type != actual.scalar_type {
                        return unsupported(
                            "Unit call scalar requirement parameter type is inconsistent",
                        );
                    }
                    Ok((formal.id, ScalarTerm::value(actual.id, actual.scalar_type)))
                })
                .collect::<Result<BTreeMap<_, _>, LoweringError>>()?;
            for (formal, actual) in target.erased_scalar_parameters.iter().zip(erased_arguments) {
                actual
                    .validate()
                    .map_err(LoweringError::InvalidCrashPredicate)?;
                if formal.scalar_type != actual.scalar_type() {
                    return unsupported(
                        "call erased scalar requirement parameter type is inconsistent",
                    );
                }
                if scalar_substitutions
                    .insert(formal.id, actual.clone())
                    .is_some()
                {
                    return unsupported("call requirement repeats a scalar formal identity");
                }
            }
            let mut crash_continuations = {
                let target_routes = routes.to_vec();
                if target.parameters.is_empty() {
                    lower_checked_crash_route_buckets(
                        identities,
                        &target_routes,
                        terminal_scalar_values,
                    )?
                } else {
                    let target_runtime_requirements = target
                        .runtime_requirements
                        .iter()
                        .map(|requirement| {
                            let mut requirement = requirement.clone();
                            substitute_runtime_requirement_scalar_values(
                                &mut requirement,
                                &scalar_substitutions,
                            )?;
                            Ok(requirement)
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                    lower_structural_crash_route_buckets(
                        identities,
                        &target_routes,
                        terminal_scalar_values,
                        target.predicate_parameters,
                        structural_types,
                        &target_runtime_requirements,
                    )?
                }
            };
            if !crash_continuations.is_empty() {
                substitute_structural_crash_route_roots(&mut crash_continuations, &mut |root| {
                    let Some((index, parameter)) = target
                        .parameters
                        .iter()
                        .enumerate()
                        .find(|(_, parameter)| parameter.place == root)
                    else {
                        return Ok(None);
                    };
                    let argument = crate::proofs::crash_routes::captured_crash_argument(
                        checked,
                        coordinate.statement().ok_or(LoweringError::Unsupported(
                            "structural crash capture has no statement custody",
                        ))?,
                        target
                            .predicate_parameters
                            .iter()
                            .find(|predicate_parameter| {
                                predicate_parameter.place == parameter.place
                            })
                            .ok_or(LoweringError::Unsupported(
                                "crash observation lost its authored formal position",
                            ))?
                            .position,
                        &terminal_arguments[index],
                        captures,
                        operations,
                        structural_types,
                        &mut |argument| caller_types.root_type(argument),
                    )?;
                    let prefix =
                        crate::proofs::crash_routes::structural_crash_route_prefix_from_type(
                            &argument,
                            caller_types.root_type(&argument)?,
                            structural_types,
                        )?;
                    Ok(Some((argument.place, prefix)))
                })?;
            }
            Ok(crash_continuations)
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn emit_structural(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            state: symbols::SymbolHandle,
            operation: &CheckedUnitEffectOperationPlan,
            prepared: ModuleInvocationArguments,
            callee: MachineId,
            type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
            structural_types: &[terminal_psi::StructuralTypeDeclaration],
            domain_ids: &[StructuralDomainBinding],
            claim_bindings: &mut crate::emission::claim_bindings::ClaimBindings<'_>,
            register_binding: bool,
            frontier_before_call: &lowered_psi::LoweredSourceFrontierBindings,
            next_place: &mut u64,
            operations: &mut OperationBuffer,
        ) -> Result<StructuralPlaceDeclaration, LoweringError> {
            let Some((call, result)) = operation
                .call()
                .filter(|call| call.is_module())
                .and_then(|call| Some((call, call.structural_result()?)))
            else {
                return unsupported("structural call emission requires an owned result call");
            };
            let coordinate = &call.coordinate;
            let place = place_id(allocate_dense(next_place)?);
            let structural_type = lookup_type_id(type_ids, &result.type_identity)?;
            let returned = super::call_results::prepare(
                identities,
                checked,
                call,
                place,
                structural_type,
                structural_types,
            )?
            .emit(
                identities,
                checked,
                state,
                *coordinate,
                domain_ids,
                structural_types,
                claim_bindings,
            )?;
            let id = prepared.emit(
                checked,
                operations,
                SourceCallCoordinate {
                    state,
                    statement_index: coordinate.statement_index as usize,
                    call_ordinal: coordinate.call_ordinal as usize,
                },
                crate::shared::direct_calls::CallRecord::Frontier(frontier_before_call),
                OperationResult::Structural(returned.clone()),
                callee,
                DirectCallResult::Structural {
                    multiplicity: returned.multiplicity,
                },
            )?;
            if register_binding
                && operations
                    .structural_values
                    .iter()
                    .any(|(ordinal, _)| *ordinal == result.binding_ordinal)
            {
                return unsupported("structural call result was published twice");
            }
            if register_binding {
                operations.bind_structural_value(result.binding_ordinal, returned.into());
            }
            Ok(operation_result_place(place, id, structural_type))
        }
    }
    pub(crate) mod parameters {

        use super::super::{
            BTreeSet, CheckedUnitStructuralPathSegment, ServiceId, ServiceReachId,
            ServiceReachInterface, ServiceReachPlan, StructuralAccess, StructuralArgument,
            StructuralParameterDeclaration, StructuralTypeDeclaration,
        };
        use super::{
            CheckedBoundaryMachinePlan, CheckedTrees, LoweringError, PlaceId, ScalarType,
            ServiceReachSummary, StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeId,
            ValueDeclaration, allocate_dense, lookup_service_id, lookup_type_id, place_id,
            require_valid_service_row, terminal_scalar_type, unsupported,
        };
        use crate::emission::operation_emission::buffer::OperationBuffer;
        use crate::emission::operation_emission::buffer::StructuralBinding;
        use crate::emission::operation_emission::calls::CallEmissionContext;
        use crate::emission::operation_emission::expressions::emit_direct_expression;
        use crate::expression_preparation::bindings::structural_fields::resolve_typed;
        use crate::expression_preparation::bindings::structural_paths::prepare_call_argument_path;
        use crate::expression_preparation::bindings::structural_paths::{
            lower_typed_argument_path, structural_access_can_supply, structural_argument,
            terminal_access,
        };
        use crate::expression_preparation::bindings::{
            StructuralLocalBinding, StructuralScalarFieldBinding,
        };
        use crate::shared::terminal_identities::structural_multiplicity;
        use crate::shared::terminal_identities::{
            StructuralDomainBinding, lookup_domain_id_for_carrier,
        };
        use semantic_vocabulary::ValueId;
        pub(crate) mod reference_sources {

            use super::{
                LoweringError, StructuralArgument, StructuralParameterDeclaration,
                StructuralPlaceDeclaration, StructuralTypeDeclaration, lower_structural_arguments,
                unsupported,
            };
            use crate::expression_preparation::bindings::StructuralLocalBinding;
            use crate::expression_preparation::bindings::structural_paths::lower_static_argument_path;
            use crate::expression_preparation::bindings::structural_paths::terminal_access;
            use checked_trees::{
                CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
                CheckedUnitStructuralPathSegment,
            };

            pub(crate) struct ReferenceSourceEvaluation<'scope, 'calls> {
                pub(crate) values: &'scope [terminal_psi::ValueDeclaration],
                pub(crate) selectors: &'scope mut dyn Iterator<Item = semantic_vocabulary::ValueId>,
                pub(crate) calls:
                    &'scope mut crate::emission::operation_emission::calls::CallEmissionContext<
                        'calls,
                    >,
                pub(crate) next_value: &'scope mut u64,
                pub(crate) operations:
                    &'scope mut crate::emission::operation_emission::buffer::OperationBuffer,
            }

            pub(crate) fn lower_reference_source(
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                checked: &checked_trees::CheckedTrees,
                source: &CheckedUnitStructuralArgumentPlan,
                structural_types: &[StructuralTypeDeclaration],
                parameters: &[StructuralParameterDeclaration],
                trivial_affine_locals: &[StructuralPlaceDeclaration],
                structural_results: &[(Option<StructuralPlaceDeclaration>, bool)],
                primitive_locals: &[super::super::primitive_locals::PrimitiveLocal],
                structural_locals: &[StructuralLocalBinding],
                evaluation: Option<ReferenceSourceEvaluation<'_, '_>>,
            ) -> Result<StructuralArgument, LoweringError> {
                if source.access == CheckedStructuralAccess::Owned {
                    return unsupported("reference source cannot transfer owned custody");
                }
                let mut prefix = source.clone();
                let has_leaf_marker = matches!(
                    prefix.path.last(),
                    Some(CheckedUnitStructuralPathSegment::Referent)
                );
                if has_leaf_marker {
                    prefix.path.pop();
                    if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocalBorrow {
                        root_reference_hops,
                        ..
                    } = &mut prefix.source
                        && usize::try_from(*root_reference_hops).ok() == Some(source.path.len())
                    {
                        *root_reference_hops =
                            root_reference_hops
                                .checked_sub(1)
                                .ok_or(LoweringError::Unsupported(
                                    "reference root marker count underflows",
                                ))?;
                    }
                }
                let mut empty_selectors = std::iter::empty();
                let (values, selectors, calls, emission) = match evaluation {
                    Some(evaluation) => (
                        Some(evaluation.values),
                        evaluation.selectors,
                        Some(evaluation.calls),
                        Some((evaluation.next_value, evaluation.operations)),
                    ),
                    None => (
                        None,
                        &mut empty_selectors
                            as &mut dyn Iterator<Item = semantic_vocabulary::ValueId>,
                        None,
                        None,
                    ),
                };
                let arguments = lower_structural_arguments(
                    identities,
                    checked,
                    std::slice::from_ref(&prefix),
                    None,
                    structural_types,
                    parameters,
                    trivial_affine_locals,
                    structural_results,
                    &[],
                    &[],
                    primitive_locals,
                    structural_locals,
                    None,
                    values,
                    selectors,
                    calls,
                    emission,
                )?;
                let [mut argument] =
                    <[StructuralArgument; 1]>::try_from(arguments).map_err(|_| {
                        LoweringError::Unsupported("reference source is not one argument")
                    })?;
                let retains_leaf_marker =
                    if has_leaf_marker && source.borrowed_root_type_identity().is_some() {
                        let mut roots = parameters
                            .iter()
                            .filter(|parameter| parameter.place == argument.place)
                            .map(|parameter| parameter.structural_type)
                            .chain(structural_results.iter().filter_map(|(declaration, _)| {
                                let declaration = declaration
                                    .as_ref()
                                    .filter(|row| row.id == argument.place)?;
                                match declaration.kind {
                                semantic_vocabulary::StructuralPlaceKind::OperationResult {
                                    structural_type,
                                    ..
                                }
                                | semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                                    structural_type,
                                    ..
                                } => Some(structural_type),
                                _ => None,
                            }
                            }))
                            .chain(structural_locals.iter().filter_map(|binding| {
                                (binding.argument.place == argument.place
                                    && binding.argument.path.is_empty())
                                .then_some(binding.structural_type)
                            }));
                        let root = roots.next().ok_or(LoweringError::Unsupported(
                            "reference source lost its published home type",
                        ))?;
                        if !roots.all(|candidate| candidate == root) {
                            return unsupported(
                                "reference source has conflicting published home types",
                            );
                        }
                        let endpoint = terminal_semantics::runtime_structural_path_tip(
                            structural_types.iter(),
                            root,
                            &argument.path,
                        )
                        .ok_or(LoweringError::Unsupported(
                            "reference source has no selected published endpoint",
                        ))?;
                        if matches!(
                            endpoint.shape,
                            terminal_psi::StructuralTypeShape::Reference { .. }
                        ) {
                            true
                        } else {
                            if endpoint.identity
                                != identities.intern(
                                    &checked
                                        .type_identity_text(source.type_identity)
                                        .unwrap_or_default(),
                                )
                            {
                                return unsupported(
                                    "reference source changes its checked endpoint type",
                                );
                            }
                            false
                        }
                    } else {
                        has_leaf_marker
                    };
                if retains_leaf_marker {
                    argument
                        .path
                        .push(terminal_psi::StructuralPathSegment::Referent);
                }
                Ok(argument)
            }

            pub(crate) fn lower_reference_result_sources(
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                checked: &checked_trees::CheckedTrees,
                references: &[checked_trees::CheckedReturnedReferencePlan],
                result_type: semantic_vocabulary::StructuralTypeId,
                structural_types: &[StructuralTypeDeclaration],
                parameters: &[StructuralParameterDeclaration],
            ) -> Result<Vec<terminal_psi::StructuralReferenceResultSource>, LoweringError>
            {
                let mut lowered = Vec::new();
                for reference in references {
                    if reference.source.access == CheckedStructuralAccess::Owned {
                        return unsupported("returned origin cannot transfer owned storage");
                    }
                    let result_path = lower_static_argument_path(
                        identities,
                        &reference.path,
                        result_type,
                        structural_types,
                    )?;
                    let field_carrier = terminal_semantics::runtime_structural_field(
                        structural_types.iter(),
                        result_type,
                        &result_path,
                    )
                    .and_then(|field| field.field_type.byte_sequence_carrier());
                    let leaf = terminal_semantics::runtime_structural_path_tip(
                        structural_types.iter(),
                        result_type,
                        &result_path,
                    );
                    if matches!(
                        field_carrier,
                        Some(terminal_psi::byte_sequence_view_carrier_pattern!())
                    ) || leaf.is_some_and(|leaf| {
                        matches!(
                            leaf.shape,
                            terminal_psi::StructuralTypeShape::ElementView { .. }
                                | terminal_psi::StructuralTypeShape::ByteSequence(
                                    terminal_psi::byte_sequence_view_carrier_pattern!()
                                )
                        )
                    }) {
                        continue;
                    }
                    if !leaf.is_some_and(|leaf| {
                        matches!(
                            leaf.shape,
                            terminal_psi::StructuralTypeShape::Reference { .. }
                        )
                    }) {
                        return unsupported("returned origin has no borrowed carrier leaf");
                    }
                    let (place, root_type, root_coordinate) = match reference.source.root {
                        checked_trees::CheckedReferenceOriginRoot::Parameter { index } => {
                            let parameter = parameters.get(index as usize).ok_or(
                                LoweringError::Unsupported(
                                    "returned origin lost its input parameter",
                                ),
                            )?;
                            let coordinate = if reference.source.entry_reference_hops == 0 {
                                crate::expression_preparation::bindings::structural_paths::ProjectionRoot::Published(parameter.structural_type)
                            } else {
                                crate::expression_preparation::bindings::structural_paths::ProjectionRoot::BorrowedEntry {
                                    published: parameter.structural_type,
                                    access: parameter.access,
                                    reference_hops: reference.source.entry_reference_hops,
                                }
                            };
                            (Some(parameter.place), parameter.structural_type, coordinate)
                        }
                        checked_trees::CheckedReferenceOriginRoot::Invocation { carrier_type } => {
                            let declaration =
                                crate::shared::terminal_identities::structural_declaration_by_identity(
                                    structural_types,
                                    identities,
                                    checked,
                                    carrier_type,
                                    "returned invocation origin lost its carrier type",
                                )?;
                            if reference.source.entry_reference_hops != 0 {
                                return unsupported(
                                    "invocation origin cannot erase a formal entry",
                                );
                            }
                            (None, declaration.id, crate::expression_preparation::bindings::structural_paths::ProjectionRoot::Published(declaration.id))
                        }
                    };
                    let path =
                        crate::expression_preparation::bindings::structural_paths::lower_reference_origin_path(
                            identities,
                            checked,
                            &reference.source.path,
                            root_coordinate,
                            terminal_access(reference.source.access),
                            structural_types,
                        )?;
                    let access = terminal_access(reference.source.access);
                    let source = match place {
                        Some(place) => terminal_psi::StructuralReferenceOrigin::Parameter(
                            terminal_psi::ReferenceOriginSource {
                                place,
                                extent: reference.source.extent,
                                path,
                                access,
                            },
                        ),
                        None => terminal_psi::StructuralReferenceOrigin::Invocation {
                            carrier_type: root_type,
                            extent: reference.source.extent,
                            path,
                            access,
                        },
                    };
                    lowered.push(terminal_psi::StructuralReferenceResultSource {
                        path: result_path,
                        sources: vec![source],
                    });
                }
                lowered.sort_by(|left, right| left.path.cmp(&right.path));
                let mut grouped: Vec<terminal_psi::StructuralReferenceResultSource> =
                    Vec::with_capacity(lowered.len());
                for reference in lowered {
                    if let Some(group) = grouped.last_mut()
                        && group.path == reference.path
                    {
                        group.sources.extend(reference.sources);
                    } else {
                        grouped.push(reference);
                    }
                }
                for group in &mut grouped {
                    group.sources.sort();
                    group.sources.dedup();
                }
                Ok(grouped)
            }
        }
        pub(crate) use reference_sources::ReferenceSourceEvaluation;
        pub(crate) use reference_sources::{
            lower_reference_result_sources, lower_reference_source,
        };

        pub(crate) fn structural_carrier_type(
            checked: &CheckedTrees,
            mut reference: checked_trees::types::TypeReferenceHandle,
        ) -> Result<checked_trees::types::TypeReferenceHandle, LoweringError> {
            let mut visited = Vec::new();
            loop {
                if !reference.is_valid() || visited.contains(&reference) {
                    return unsupported("structural carrier has an invalid type chain");
                }
                visited.push(reference);
                match checked.type_reference_table.type_reference(reference) {
                    checked_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                        reference = *base_type
                    }
                    _ => return Ok(reference),
                }
            }
        }

        pub(crate) fn lower_runtime_parameter_order(
            scalars: &[checked_trees::CheckedStructuralScalarParameterPlan],
            structural: &[checked_trees::CheckedUnitStructuralParameterPlan],
        ) -> Result<Vec<terminal_psi::BoundaryParameterKind>, LoweringError> {
            use terminal_psi::BoundaryParameterKind::{Scalar, Structural};
            if scalars
                .windows(2)
                .any(|pair| pair[0].source_position >= pair[1].source_position)
                || structural
                    .windows(2)
                    .any(|pair| pair[0].position >= pair[1].position)
            {
                return unsupported("runtime parameter lanes are not in strict source order");
            }
            let mut scalar = scalars.iter().peekable();
            let mut structure = structural.iter().peekable();
            let mut order = Vec::with_capacity(scalars.len() + structural.len());
            loop {
                let kind = match (scalar.peek(), structure.peek()) {
                    (Some(left), Some(right)) => match left.source_position.cmp(&right.position) {
                        std::cmp::Ordering::Less => Scalar,
                        std::cmp::Ordering::Greater => Structural,
                        std::cmp::Ordering::Equal => {
                            return unsupported("runtime parameter lanes share a source position");
                        }
                    },
                    (Some(_), None) => Scalar,
                    (None, Some(_)) => Structural,
                    (None, None) => break,
                };
                match kind {
                    Scalar => {
                        scalar.next();
                    }
                    Structural => {
                        structure.next();
                    }
                }
                order.push(kind);
            }
            Ok(order)
        }

        pub(crate) fn lower_unit_scalar_parameter_declarations(
            checked: &CheckedTrees,
            state: symbols::SymbolHandle,
            parameters: &[checked_trees::CheckedStructuralScalarParameterPlan],
            qualifications: &crate::expression_preparation::qualifications::PreparedScalarQualifications,
            next_value: &mut u64,
        ) -> Result<Vec<ValueDeclaration>, LoweringError> {
            let carriers = lower_unit_scalar_parameter_types(parameters, false)?;
            let (_, declaration) = crate::shared::declarations::state_declaration(checked, state)?;
            let authored = checked.state_parameters(declaration);
            parameters
                .iter()
                .zip(carriers)
                .map(|(parameter, carrier)| {
                    let authored = usize::try_from(parameter.source_position)
                        .ok()
                        .and_then(|position| authored.get(position))
                        .ok_or(LoweringError::Unsupported(
                            "Unit scalar formal has no authored parameter",
                        ))?;
                    let declared = qualifications.parameter_value_type(
                        checked,
                        state,
                        parameter.source_position,
                    )?;
                    if declared.scalar_type != carrier
                        || checked.primitive_type_reference(authored.type_reference)
                            != Some(parameter.primitive_type)
                    {
                        return unsupported("Unit scalar formal has a different authored carrier");
                    }
                    Ok(ValueDeclaration {
                        qualifications: declared.qualifications,
                        id: crate::shared::terminal_identities::value_id(allocate_dense(
                            next_value,
                        )?),
                        scalar_type: carrier,
                    })
                })
                .collect()
        }

        pub(crate) fn unit_scalar_result_type(
            checked: &CheckedTrees,
            state: symbols::SymbolHandle,
            carrier: ScalarType,
            qualifications: &crate::expression_preparation::qualifications::PreparedScalarQualifications,
        ) -> Result<semantic_vocabulary::QualifiedScalarType, LoweringError> {
            let (_, declaration) = crate::shared::declarations::state_declaration(checked, state)?;
            let declared = qualifications.signature_value_type(checked, declaration.return_type)?;
            if declared.scalar_type != carrier {
                return unsupported("Unit scalar result has a different authored carrier");
            }
            Ok(declared)
        }

        /*
            Scalar parameters and erased scalar lanes both arrive in source
            order. Runtime scalar parameters each own their authored position.
            An erased record formal owns one lane per scalar leaf, so its
            consecutive lanes share a position (`leaves_share_position`).
        */
        pub(crate) fn lower_unit_scalar_parameter_types(
            parameters: &[checked_trees::CheckedStructuralScalarParameterPlan],
            leaves_share_position: bool,
        ) -> Result<Vec<ScalarType>, LoweringError> {
            if parameters.windows(2).any(|pair| {
                pair[0].source_position > pair[1].source_position
                    || (!leaves_share_position
                        && pair[0].source_position == pair[1].source_position)
            }) {
                return unsupported("Unit scalar parameters are not in strict source order");
            }
            parameters
                .iter()
                .map(|parameter| terminal_scalar_type(parameter.primitive_type))
                .collect()
        }
        pub(crate) fn lower_unit_parameters(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
            type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
            structural_types: &[StructuralTypeDeclaration],
            domain_ids: &[StructuralDomainBinding],
            next_place: &mut u64,
        ) -> Result<Vec<StructuralParameterDeclaration>, LoweringError> {
            let mut positions = BTreeSet::new();
            parameters
                .iter()
                .enumerate()
                .map(|(dense_position, parameter)| {
                    if !positions.insert(parameter.position) {
                        return Err(LoweringError::Unsupported(
                            "Unit structural parameters contain duplicate source positions",
                        ));
                    }
                    let mut qualifications = parameter
                        .qualifications
                        .iter()
                        .map(|domain| {
                            lookup_domain_id_for_carrier(
                                domain_ids,
                                *domain,
                                lookup_type_id(type_ids, &parameter.type_identity)?,
                            )
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                    qualifications.sort();
                    qualifications.dedup();
                    if qualifications.len() != parameter.qualifications.len() {
                        return Err(LoweringError::Unsupported(
                            "Unit structural parameter repeats a qualification",
                        ));
                    }
                    Ok(StructuralParameterDeclaration {
                        place: place_id(allocate_dense(next_place)?),
                        position: u32::try_from(dense_position).map_err(|_| {
                            LoweringError::Unsupported(
                                "Unit structural parameter count exceeds u32",
                            )
                        })?,
                        structural_type: lookup_type_id(type_ids, &parameter.type_identity)?,
                        multiplicity: structural_multiplicity(parameter.multiplicity),
                        access: terminal_access(parameter.access),
                        qualifications,
                        projected_qualifications: lower_projected_qualifications(
                            identities,
                            &parameter.projected_qualifications,
                            domain_ids,
                            lookup_type_id(type_ids, &parameter.type_identity)?,
                            structural_types,
                        )?,
                        additional_input_qualifications: lower_qualification_rows(
                            identities,
                            &parameter.additional_input_qualifications,
                            domain_ids,
                            lookup_type_id(type_ids, &parameter.type_identity)?,
                            structural_types,
                        )?,
                    })
                })
                .collect()
        }

        pub(crate) fn lower_contract_service_ceiling(
            rows: &language_semantics::ServiceReachRowTable,
            contract: ServiceReachPlan,
            summary: ServiceReachSummary,
            service_ids: &[(ServiceReachId, ServiceId)],
        ) -> Result<Vec<ServiceId>, LoweringError> {
            if contract.checked_inferred != summary.transitive {
                return unsupported("Unit contract reach does not match checked transitive reach");
            }
            let source = match contract.interface {
                ServiceReachInterface::PublishedCeiling(row) => {
                    require_valid_service_row(row)?;
                    let ceiling = rows.services(row);
                    if rows
                        .services(summary.transitive)
                        .iter()
                        .any(|service| !ceiling.contains(service))
                    {
                        return unsupported(
                            "checked Unit service reach exceeds its published ceiling",
                        );
                    }
                    ceiling
                }
                ServiceReachInterface::InternalInferred => rows.services(summary.transitive),
            };
            let mut lowered = source
                .iter()
                .map(|service| lookup_service_id(service_ids, *service))
                .collect::<Result<Vec<_>, LoweringError>>()?;
            lowered.sort();
            lowered.dedup();
            if lowered.len() != source.len() {
                return unsupported("Unit published service ceiling contains duplicates");
            }
            Ok(lowered)
        }

        pub(crate) fn lower_declared_service_reach(
            checked: &CheckedTrees,
            machine: symbols::SymbolHandle,
            service_ids: &[(ServiceReachId, ServiceId)],
        ) -> Result<Vec<ServiceId>, LoweringError> {
            let mut sources = checked
                .typed
                .machines()
                .iter()
                .filter(|source| source.symbol == machine);
            let source = sources.next().ok_or(LoweringError::Unsupported(
                "service declaration lost its source machine",
            ))?;
            if sources.next().is_some() {
                return unsupported("service declaration has ambiguous source machines");
            }
            if source.service_reach_is_installation_bound {
                return Ok(Vec::new());
            }
            let declared = checked
                .typed
                .service_reach_rows
                .services(source.service_reach_row);
            crate::shared::terminal_identities::lower_service_reach_sorted(declared, service_ids)
        }

        pub(crate) fn lower_fixed_boundary_service_reach(
            checked: &CheckedTrees,
            boundary: &CheckedBoundaryMachinePlan,
            service_ids: &[(ServiceReachId, ServiceId)],
        ) -> Result<Vec<ServiceId>, LoweringError> {
            let source = validation::fixed_installation_boundary_service_reach(
                &checked.typed,
                boundary.state,
            )
            .ok_or(LoweringError::Unsupported(
                "fixed boundary reach has no exact typed requirement",
            ))?;
            crate::shared::terminal_identities::lower_service_reach_sorted(&source, service_ids)
        }

        pub(crate) fn lower_published_service_ceiling(
            rows: &language_semantics::ServiceReachRowTable,
            contract: ServiceReachPlan,
            summary: ServiceReachSummary,
            service_ids: &[(ServiceReachId, ServiceId)],
        ) -> Result<Vec<ServiceId>, LoweringError> {
            if matches!(contract.interface, ServiceReachInterface::InternalInferred) {
                return unsupported("public Unit contract has no published service ceiling");
            }
            lower_contract_service_ceiling(rows, contract, summary, service_ids)
        }

        pub(crate) fn lower_installation_machine_service_ceiling(
            checked: &CheckedTrees,
            machine: symbols::SymbolHandle,
            contract: ServiceReachPlan,
            summary: ServiceReachSummary,
            service_ids: &[(ServiceReachId, ServiceId)],
        ) -> Result<Vec<ServiceId>, LoweringError> {
            if let Some(reach) = checked.facts.service_reaches.for_machine(machine)
                && matches!(contract.interface, ServiceReachInterface::InternalInferred)
                && !reach.unresolved_installation_reaches.is_empty()
            {
                let source = checked
                    .facts
                    .service_reaches
                    .rows
                    .services(summary.transitive);
                let mut lowered = source
                    .iter()
                    .map(|service| lookup_service_id(service_ids, *service))
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                lowered.sort();
                lowered.dedup();
                if lowered.len() != source.len() {
                    return unsupported("installation-bound service ceiling contains duplicates");
                }
                return Ok(lowered);
            }
            lower_contract_service_ceiling(
                &checked.facts.service_reaches.rows,
                contract,
                summary,
                service_ids,
            )
        }

        pub(crate) fn literal_argument_places(
            arguments: &[checked_trees::CheckedUnitStructuralArgumentPlan],
            literals: &[StructuralPlaceDeclaration],
            next_literal: &mut usize,
        ) -> Result<Vec<PlaceId>, LoweringError> {
            let count = arguments
                .iter()
                .filter(|argument| argument.byte_sequence_literal().is_some())
                .count();
            let end = next_literal
                .checked_add(count)
                .ok_or(LoweringError::Unsupported(
                    "byte-sequence literal argument count overflows usize",
                ))?;
            let places = literals
                .get(*next_literal..end)
                .ok_or(LoweringError::Unsupported(
                    "byte-sequence literal argument place is absent",
                ))?
                .iter()
                .map(|place| place.id)
                .collect();
            *next_literal = end;
            Ok(places)
        }

        pub(crate) fn call_index_selectors(
            arguments: &[checked_trees::CheckedUnitStructuralArgumentPlan],
            evaluated: Option<&[ValueDeclaration]>,
        ) -> Result<Vec<ValueId>, LoweringError> {
            let count = arguments
                .iter()
                .flat_map(|argument| argument.call_selector_positions())
                .count();
            let evaluated = evaluated.unwrap_or_default();
            if evaluated.len() != count || evaluated.iter().any(|value|
                !matches!(value.scalar_type, ScalarType::Integer(integer) if !integer.is_address())) {
                return unsupported("captured call selectors disagree with their argument paths");
            }
            Ok(evaluated.iter().map(|value| value.id).collect())
        }

        pub(crate) fn mint_runtime_index_selectors(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            arguments: &[checked_trees::CheckedUnitStructuralArgumentPlan],
            fields: &[StructuralScalarFieldBinding],
            bindings: &crate::expression_preparation::bindings::ScalarBindings,
            caller_scalar_values: &[ValueDeclaration],
            next_value: &mut u64,
            operations: &mut OperationBuffer,
        ) -> Result<Vec<ValueId>, LoweringError> {
            let mut values = Vec::new();
            for argument in arguments {
                for segment in &argument.path {
                    let observation = match segment {
                        CheckedUnitStructuralPathSegment::RuntimeIndex(
                            checked_trees::CheckedRuntimeIndex::StorageRead { symbol },
                        ) => bindings.runtime_index_storage(*symbol)?,
                        CheckedUnitStructuralPathSegment::RuntimeIndex(
                            checked_trees::CheckedRuntimeIndex::StructuralField(field),
                        ) => resolve_typed(
                            identities,
                            fields,
                            field.parameter_position,
                            &field.path,
                        )?,
                        _ => continue,
                    };
                    if !matches!(observation.scalar_type(), ScalarType::Integer(integer) if !integer.is_address())
                    {
                        return unsupported("runtime index field selector is not an integer read");
                    }
                    values.push(emit_direct_expression(
                        &observation,
                        caller_scalar_values,
                        next_value,
                        operations,
                    ));
                }
            }
            Ok(values)
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn lower_structural_arguments(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &checked_trees::CheckedTrees,
            arguments: &[checked_trees::CheckedUnitStructuralArgumentPlan],
            completed: Option<&[Option<super::operation_frame::CompletedStructuralArgument>]>,
            structural_types: &[StructuralTypeDeclaration],
            parameters: &[StructuralParameterDeclaration],
            trivial_affine_locals: &[StructuralPlaceDeclaration],
            structural_results: &[(Option<StructuralPlaceDeclaration>, bool)],
            completed_results: &[(u32, StructuralBinding)],
            byte_argument_places: &[PlaceId],
            primitive_locals: &[super::primitive_locals::PrimitiveLocal],
            structural_locals: &[StructuralLocalBinding],
            scalar_bindings: Option<&crate::expression_preparation::bindings::ScalarBindings>,
            caller_scalar_values: Option<&[ValueDeclaration]>,
            selector_values: &mut dyn Iterator<Item = ValueId>,
            mut calls: Option<&mut CallEmissionContext<'_>>,
            mut emission: Option<(&mut u64, &mut OperationBuffer)>,
        ) -> Result<Vec<StructuralArgument>, LoweringError> {
            if completed.is_some_and(|values| values.len() != arguments.len()) {
                return unsupported("completed structural operand roster has the wrong arity");
            }
            let mut next_byte_argument = 0;
            let mut lowered = Vec::with_capacity(arguments.len());
            for (ordinal, argument) in arguments.iter().enumerate() {
                if let Some(Some(captured)) = completed.and_then(|values| values.get(ordinal)) {
                    if captured.progress.is_some()
                        || captured.source != *argument
                        || captured.argument.access != terminal_access(argument.access)
                    {
                        return unsupported(
                            "completed structural operand changes its checked source or access",
                        );
                    }
                    for _ in argument.call_selector_positions() {
                        selector_values.next().ok_or(LoweringError::Unsupported(
                            "completed address lost its selector receipt",
                        ))?;
                    }
                    lowered.push(captured.argument.clone());
                    continue;
                }
                let prepared = prepare_structural_argument(
                    identities,
                    checked,
                    argument,
                    structural_types,
                    parameters,
                    trivial_affine_locals,
                    structural_results,
                    completed_results,
                    byte_argument_places,
                    &mut next_byte_argument,
                    primitive_locals,
                    structural_locals,
                    scalar_bindings,
                )?;
                let source = prepared.source.known_place()?;
                let path =
                    crate::expression_preparation::bindings::structural_paths::emit_captured_argument_path(
                        &prepared.path,
                        source,
                        caller_scalar_values,
                        &mut *selector_values,
                        calls.as_deref_mut(),
                        emission
                            .as_mut()
                            .map(|(next, operations)| (&mut **next, &mut **operations)),
                    )?;
                lowered.push(structural_argument(source, path, prepared.access));
            }
            if next_byte_argument != byte_argument_places.len() {
                return unsupported("byte-sequence place count disagrees with arguments");
            }
            Ok(lowered)
        }

        pub(crate) fn prepare_structural_argument(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &checked_trees::CheckedTrees,
            argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
            structural_types: &[StructuralTypeDeclaration],
            parameters: &[StructuralParameterDeclaration],
            trivial_affine_locals: &[StructuralPlaceDeclaration],
            structural_results: &[(Option<StructuralPlaceDeclaration>, bool)],
            completed_results: &[(u32, StructuralBinding)],
            byte_argument_places: &[PlaceId],
            next_byte_argument: &mut usize,
            primitive_locals: &[super::primitive_locals::PrimitiveLocal],
            structural_locals: &[StructuralLocalBinding],
            scalar_bindings: Option<&crate::expression_preparation::bindings::ScalarBindings>,
        ) -> Result<
            crate::emission::operation_emission::calls::PreparedStructuralArgument,
            LoweringError,
        > {
            use crate::emission::operation_emission::calls::PreparedStructuralArgument;
            use crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment;
            if argument.access != checked_trees::CheckedStructuralAccess::Owned
                && argument.structural_storage_root().is_some()
                && let Some(bindings) = scalar_bindings
            {
                return bindings
                    .clone()
                    .with_structural_locals(structural_locals)
                    .with_structural_observations(structural_types)
                    .prepared_borrowed_argument(
                        identities,
                        checked,
                        argument,
                        &[],
                        structural_types,
                    );
            }
            let mut prepare = |place: PlaceId,
                               root: StructuralTypeId,
                               prefix: &[terminal_psi::StructuralPathSegment],
                               root_type: Option<StructuralTypeId>|
             -> Result<PreparedStructuralArgument, LoweringError> {
                let suffix = prepare_call_argument_path(
                    identities,
                    checked,
                    argument,
                    root,
                    structural_types,
                )?;
                let path = prefix
                    .iter()
                    .cloned()
                    .map(PreparedStructuralPathSegment::Static)
                    .chain(suffix)
                    .collect();
                Ok(PreparedStructuralArgument {
                    source: place.into(),
                    root_type,
                    access: terminal_access(argument.access),
                    path,
                    selector_positions: Vec::new(),
                })
            };
            if let Some(root) = argument.structural_storage_root() {
                let mut bindings = structural_locals
                    .iter()
                    .filter(|binding| binding.root == root);
                let binding = bindings.next().ok_or(LoweringError::Unsupported(
                    "structural argument has no established storage",
                ))?;
                let access = terminal_access(argument.access);
                if bindings.next().is_some()
                    || !structural_access_can_supply(binding.argument.access, access)
                {
                    return unsupported(
                        "structural argument changes its established storage custody",
                    );
                }
                let root_type = scalar_bindings
                    .and_then(|bindings| bindings.structural_place_type(binding.argument.place))
                    .or_else(|| {
                        binding
                            .argument
                            .path
                            .is_empty()
                            .then_some(binding.structural_type)
                    });
                return prepare(
                    binding.argument.place,
                    binding.structural_type,
                    &binding.argument.path,
                    root_type,
                );
            }
            if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal {
                symbol,
            } = argument.source
            {
                let local = super::primitive_locals::find(primitive_locals, symbol)?;
                if !argument.path.is_empty() {
                    return unsupported("primitive local borrow has a projection");
                }
                if argument.access == checked_trees::CheckedStructuralAccess::Owned {
                    return unsupported("primitive local cannot transfer owned custody");
                }
                return PreparedStructuralArgument::from_static(
                    structural_argument(
                        local.declaration.id,
                        Vec::new(),
                        terminal_access(argument.access),
                    ),
                    None,
                );
            }
            if argument.byte_sequence_literal().is_some()
                || matches!(
                    argument.source,
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice { .. }
                        | checked_trees::CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice { .. }
                )
            {
                let place =
                    *byte_argument_places
                        .get(*next_byte_argument)
                        .ok_or(LoweringError::Unsupported(
                            "byte-sequence argument place is absent",
                        ))?;
                *next_byte_argument += 1;
                return PreparedStructuralArgument::from_static(
                    structural_argument(place, Vec::new(), terminal_access(argument.access)),
                    None,
                );
            }
            if let Some(ordinal) = argument.source_local_declaration_ordinal() {
                let source = trivial_affine_locals
                    .get(usize::try_from(ordinal).map_err(|_| {
                        LoweringError::Unsupported("Unit local argument ordinal exceeds usize")
                    })?)
                    .ok_or(LoweringError::Unsupported(
                        "Unit local argument has an invalid declaration ordinal",
                    ))?;
                return PreparedStructuralArgument::from_static(
                    structural_argument(source.id, Vec::new(), StructuralAccess::Owned),
                    None,
                );
            }
            if let Some(ordinal) = argument.source_structural_result_binding_ordinal() {
                if let Some(alias) = projected_alias(completed_results, ordinal) {
                    return prepare(
                        alias.place,
                        alias.structural_type,
                        alias.path,
                        scalar_bindings
                            .and_then(|bindings| bindings.structural_place_type(alias.place)),
                    );
                }
                let source = structural_result_source(structural_results, ordinal, argument)?;
                let structural_type = match source.kind {
                    StructuralPlaceKind::OperationResult {
                        structural_type, ..
                    }
                    | StructuralPlaceKind::ByteSequenceLiteral {
                        structural_type, ..
                    } => structural_type,
                    StructuralPlaceKind::BlockParameter { .. } => {
                        let mut bindings = completed_results
                            .iter()
                            .filter(|(candidate, _)| *candidate == ordinal);
                        let (_, binding) = bindings.next().ok_or(LoweringError::Unsupported(
                            "structural block arrival has no typed binding",
                        ))?;
                        if bindings.next().is_some()
                            || binding.place() != source.id
                            || !binding.path().is_empty()
                        {
                            return unsupported(
                                "structural block arrival disagrees with its typed binding",
                            );
                        }
                        binding.structural_type()
                    }
                    _ => return unsupported("structural result argument has no declared type"),
                };
                return prepare(source.id, structural_type, &[], Some(structural_type));
            }
            if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::ScalarParameter {
                ..
            } = argument.source
            {
                let source = scalar_bindings
                    .ok_or(LoweringError::Unsupported(
                        "borrowed scalar formal has no scalar namespace",
                    ))?
                    .scalar_parameter_borrow(identities, checked, argument, structural_types)?;
                return PreparedStructuralArgument::from_static(source, None);
            }
            let index = argument
                .source_parameter_index()
                .ok_or(LoweringError::Unsupported(
                    "Unit structural argument is neither a parameter nor a lowered literal",
                ))?;
            let parameter = parameters
                .get(usize::try_from(index).map_err(|_| {
                    LoweringError::Unsupported("Unit structural argument index exceeds usize")
                })?)
                .ok_or(LoweringError::Unsupported(
                    "Unit structural argument has an invalid caller parameter index",
                ))?;
            prepare(
                parameter.place,
                parameter.structural_type,
                &[],
                Some(parameter.structural_type),
            )
        }

        struct ProjectedAliasBinding<'results> {
            place: PlaceId,
            path: &'results [terminal_psi::StructuralPathSegment],
            structural_type: StructuralTypeId,
        }

        fn projected_alias<'results>(
            completed_results: &'results [(u32, StructuralBinding)],
            binding_ordinal: u32,
        ) -> Option<ProjectedAliasBinding<'results>> {
            let (_, binding) = completed_results
                .iter()
                .find(|(ordinal, _)| *ordinal == binding_ordinal)?;
            let StructuralBinding::ProjectedAlias {
                place,
                path,
                structural_type,
                ..
            } = binding
            else {
                return None;
            };
            Some(ProjectedAliasBinding {
                place: *place,
                path,
                structural_type: *structural_type,
            })
        }

        fn structural_result_source<'results>(
            results: &'results [(Option<StructuralPlaceDeclaration>, bool)],
            binding_ordinal: u32,
            argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
        ) -> Result<&'results StructuralPlaceDeclaration, LoweringError> {
            let (source, discard) = results
                .get(usize::try_from(binding_ordinal).map_err(|_| {
                    LoweringError::Unsupported(
                        "Unit structural result binding ordinal exceeds usize",
                    )
                })?)
                .ok_or(LoweringError::Unsupported(
                    "Unit structural result binding is not produced",
                ))?;
            let source = source.as_ref().ok_or(LoweringError::Unsupported(
                "Unit structural result binding is a projected alias",
            ))?;
            if (*discard
                && argument.access == checked_trees::CheckedStructuralAccess::Owned
                && argument.path.is_empty())
                || !matches!(
                    source.kind,
                    StructuralPlaceKind::OperationResult { .. }
                        | StructuralPlaceKind::ByteSequenceLiteral { .. }
                        | StructuralPlaceKind::BlockParameter { .. }
                )
            {
                return unsupported(
                    "Unit structural result argument disagrees with its producer cleanup",
                );
            }
            Ok(source)
        }

        pub(crate) fn authored_result_qualification_domains(
            checked: &CheckedTrees,
            machine: symbols::SymbolHandle,
        ) -> Vec<language_semantics::SemanticDomainId> {
            let mut qualifications = Vec::new();
            let Some(contract) = checked.facts.contract_plans.for_machine(machine) else {
                return qualifications;
            };
            for clause in contract.closed_scalar_values.ensures() {
                let Some(checked_trees::ClosedScalarContractValue::SourceCheckedGuarantee {
                    result_qualifications,
                    ..
                }) = clause
                else {
                    continue;
                };
                qualifications.extend(result_qualifications.iter().map(|row| row.domain));
            }
            qualifications.sort_by_key(|domain| domain.0);
            qualifications.dedup();
            qualifications
        }

        fn authored_result_paths(
            checked: &CheckedTrees,
            machine: symbols::SymbolHandle,
        ) -> Vec<checked_trees::CheckedStructuralPathQualification> {
            let mut rows = Vec::new();
            let Some(contract) = checked.facts.contract_plans.for_machine(machine) else {
                return rows;
            };
            for clause in contract.closed_scalar_values.ensures() {
                let Some(checked_trees::ClosedScalarContractValue::SourceCheckedGuarantee {
                    result_qualifications,
                    ..
                }) = clause
                else {
                    continue;
                };
                rows.extend(result_qualifications.iter().cloned());
            }
            rows.sort_by(|left, right| {
                left.path
                    .cmp(&right.path)
                    .then(left.domain.0.cmp(&right.domain.0))
            });
            rows.dedup();
            rows
        }

        pub(crate) fn authored_result_whole_domains(
            checked: &CheckedTrees,
            machine: symbols::SymbolHandle,
        ) -> Vec<language_semantics::SemanticDomainId> {
            authored_result_paths(checked, machine)
                .into_iter()
                .filter(|row| row.path.is_empty())
                .map(|row| row.domain)
                .collect()
        }

        pub(crate) fn result_projected_qualifications(
            checked: &CheckedTrees,
            machine: symbols::SymbolHandle,
            return_type: checked_trees::types::TypeReferenceHandle,
        ) -> Result<Vec<checked_trees::CheckedStructuralPathQualification>, LoweringError> {
            let mut rows =
                validation::structural_result_projected_qualifications(&checked.typed, return_type)
                    .map_err(LoweringError::Unsupported)?;
            rows.extend(
                authored_result_paths(checked, machine)
                    .into_iter()
                    .filter(|row| !row.path.is_empty()),
            );
            rows.sort_by(|left, right| {
                left.path
                    .cmp(&right.path)
                    .then(left.domain.0.cmp(&right.domain.0))
            });
            rows.dedup();
            Ok(rows)
        }

        pub(crate) fn lower_projected_qualifications(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            rows: &[checked_trees::CheckedStructuralPathQualification],
            domain_ids: &[StructuralDomainBinding],
            root: StructuralTypeId,
            structural_types: &[StructuralTypeDeclaration],
        ) -> Result<Vec<terminal_psi::StructuralPathQualification>, LoweringError> {
            if rows.iter().any(|row| row.path.is_empty()) {
                return unsupported("projected qualification has an empty path");
            }
            lower_qualification_rows(identities, rows, domain_ids, root, structural_types)
        }

        fn lower_qualification_rows(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            rows: &[checked_trees::CheckedStructuralPathQualification],
            domain_ids: &[StructuralDomainBinding],
            root: StructuralTypeId,
            structural_types: &[StructuralTypeDeclaration],
        ) -> Result<Vec<terminal_psi::StructuralPathQualification>, LoweringError> {
            let mut qualifications = rows
                .iter()
                .map(|row| {
                    let mut path = Vec::new();
                    let mut selected = root;
                    for (ordinal, chunk) in row
                        .path
                        .split(|step| {
                            matches!(
                                step,
                                checked_trees::CheckedUnitStructuralPathSegment::AllElements
                            )
                        })
                        .enumerate()
                    {
                        if ordinal > 0 {
                            let declaration =
                                terminal_psi::find_structural_type(structural_types, selected)
                                    .ok_or(LoweringError::Unsupported(
                                        "qualification collection type is absent",
                                    ))?;
                            selected = match declaration.shape {
                                terminal_psi::StructuralTypeShape::FixedArray {
                                    element, ..
                                }
                                | terminal_psi::StructuralTypeShape::ElementView {
                                    element, ..
                                } => element,
                                _ => {
                                    return unsupported(
                                        "universal qualification names no element collection",
                                    );
                                }
                            };
                            path.push(terminal_psi::StructuralPathSegment::AllElements);
                        }
                        let suffix = lower_typed_argument_path(
                            identities,
                            chunk,
                            selected,
                            structural_types,
                            None,
                            &mut std::iter::empty(),
                            None,
                        )?;
                        selected = terminal_semantics::runtime_structural_path_tip(
                            structural_types.iter(),
                            selected,
                            &suffix,
                        )
                        .ok_or(LoweringError::Unsupported(
                            "qualification has no exact selected carrier",
                        ))?
                        .id;
                        path.extend(suffix);
                    }
                    let carrier = selected;
                    Ok(terminal_psi::StructuralPathQualification {
                        path,
                        domain: lookup_domain_id_for_carrier(domain_ids, row.domain, carrier)?,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            qualifications.sort();
            if qualifications.windows(2).any(|pair| pair[0] == pair[1]) {
                return unsupported("projected qualifications contain duplicate domain paths");
            }
            Ok(qualifications)
        }

        pub(crate) fn primitive_reference_storage(
            checked: &CheckedTrees,
            state: symbols::SymbolHandle,
            plans: &[checked_trees::CheckedUnitStructuralParameterPlan],
            parameters: &[(u32, StructuralParameterDeclaration)],
        ) -> Result<
            Vec<(
                symbols::SymbolHandle,
                semantic_vocabulary::PlaceId,
                ScalarType,
            )>,
            LoweringError,
        > {
            let (_, authored) = crate::shared::declarations::state_declaration(checked, state)?;
            let formals = checked.state_parameters(authored);
            let mut storage = Vec::new();
            for (plan, (_, parameter)) in plans.iter().zip(parameters) {
                let Some(primitive) = plan.primitive_referent else {
                    continue;
                };
                if primitive == checked_trees::types::PrimitiveType::Addr
                    || !matches!(
                        plan.access,
                        checked_trees::CheckedStructuralAccess::SharedBorrow
                            | checked_trees::CheckedStructuralAccess::MutableBorrow
                    )
                {
                    continue;
                }
                let formal =
                    formals
                        .get(plan.position as usize)
                        .ok_or(LoweringError::Unsupported(
                            "primitive reference formal lost its authored position",
                        ))?;
                storage.push((
                    formal.symbol,
                    parameter.place,
                    terminal_scalar_type(primitive)?,
                ));
            }
            Ok(storage)
        }
    }
    pub(crate) mod primitive_locals {
        use super::operation_frame::{OperationFrame, StructuralTypeRoster};
        use super::{
            CheckedScalarExpressionRole, CheckedTrees, CheckedUnitEffectOperationPlan,
            LoweringError, Operation, OperationKind, StructuralMultiplicity,
            StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeShape, allocate_dense,
            lookup_type_id, place_id, terminal_scalar_type, unsupported,
        };
        use crate::emission::operation_emission::buffer::structural_result_declaration;
        use crate::emission::operation_emission::place_declarations::operation_result_place;
        use crate::expression_preparation::bindings::StructuralLocalBinding;
        use crate::shared::terminal_identities::obligation_id;
        use crate::unit::StructuralTypeDeclaration;

        #[derive(Clone, Copy)]
        pub(crate) struct PrimitiveStorageCatalog<'a> {
            pub(crate) types: &'a [StructuralTypeDeclaration],
            pub(crate) domains: &'a [terminal_psi::StructuralDomainDeclaration],
            pub(crate) scalars: &'a terminal_psi::ScalarQualificationCatalog,
        }

        #[derive(Clone, Copy)]
        pub(crate) struct PrimitiveLocal {
            pub symbol: symbols::SymbolHandle,
            pub declaration: StructuralPlaceDeclaration,
        }

        pub(crate) fn completed_scalar(
            value: semantic_vocabulary::ValueId,
            parameters: &[terminal_psi::ValueDeclaration],
            operations: &crate::emission::operation_emission::buffer::OperationBuffer,
        ) -> Result<terminal_psi::ValueDeclaration, LoweringError> {
            parameters
                .iter()
                .chain(
                    operations
                        .operations
                        .iter()
                        .filter_map(|operation| operation.result.scalar_ref()),
                )
                .find(|declaration| declaration.id == value)
                .copied()
                .ok_or(LoweringError::Unsupported(
                    "primitive initializer lost its completed SSA declaration",
                ))
        }

        pub(crate) struct ParameterStorage {
            pub(crate) position: usize,
            pub(crate) local: PrimitiveLocal,
            pub(crate) scalar_type: semantic_vocabulary::ScalarType,
        }

        pub(crate) fn register_mut_formal_storage_homes(
            evaluation: &mut super::argument_evaluation::Evaluation,
            parameter_storage: &[ParameterStorage],
        ) -> Result<(), LoweringError> {
            for storage in parameter_storage {
                evaluation.scalar_bindings.register_scalar_parameter_home(
                    u32::try_from(storage.position).map_err(|_| {
                        LoweringError::Unsupported("parameter storage position exceeds u32")
                    })?,
                    storage.local.declaration.id,
                    storage.scalar_type,
                    terminal_psi::StructuralAccess::MutableBorrow,
                )?;
                let StructuralPlaceKind::OperationResult {
                    structural_type, ..
                } = storage.local.declaration.kind
                else {
                    return unsupported("mutable formal storage has no established declaration");
                };
                let root = facts::PlaceRoot::Symbol(storage.local.symbol);
                if evaluation
                    .structural_locals
                    .iter()
                    .any(|home| home.root == root)
                {
                    return unsupported("mutable formal has duplicate original storage homes");
                }
                evaluation.bind_structural_local(StructuralLocalBinding {
                    result_binding_ordinal: None,
                    root,
                    structural_type,
                    argument: terminal_psi::StructuralArgument {
                        place: storage.local.declaration.id,
                        path: Vec::new(),
                        access: terminal_psi::StructuralAccess::Owned,
                    },
                });
            }
            Ok(())
        }

        pub(crate) fn initialization_obligation(
            declarations: &[StructuralTypeDeclaration],
            structural_type: semantic_vocabulary::StructuralTypeId,
            next_obligation: &mut u64,
        ) -> Result<Option<semantic_vocabulary::ObligationId>, LoweringError> {
            let mut matching = declarations.iter().filter(|row| row.id == structural_type);
            let declaration = matching.next().ok_or(LoweringError::Unsupported(
                "primitive storage has no receiving declaration",
            ))?;
            if matching.next().is_some() {
                return unsupported("primitive storage receiving declaration is ambiguous");
            }
            match &declaration.scalar_restriction {
                None => Ok(None),
                Some(restriction)
                    if terminal_semantics::scalar_field_requires_evidence(restriction) =>
                {
                    Ok(Some(obligation_id(allocate_dense(next_obligation)?)))
                }
                _ => unsupported("primitive storage has no supported initialization restriction"),
            }
        }

        pub(crate) fn push_established_local(
            next_place: &mut u64,
            operations: &mut crate::emission::operation_emission::buffer::OperationBuffer,
            value: terminal_psi::ValueDeclaration,
            structural_type: semantic_vocabulary::StructuralTypeId,
            catalog: PrimitiveStorageCatalog<'_>,
            range_obligation: Option<semantic_vocabulary::ObligationId>,
        ) -> Result<(semantic_vocabulary::PlaceId, StructuralPlaceDeclaration), LoweringError>
        {
            let place = place_id(allocate_dense(next_place)?);
            let declaration = establish_at(
                place,
                operations,
                value,
                structural_type,
                catalog,
                range_obligation,
            )?;
            Ok((place, declaration))
        }

        pub(crate) fn establish_at(
            place: semantic_vocabulary::PlaceId,
            operations: &mut crate::emission::operation_emission::buffer::OperationBuffer,
            value: terminal_psi::ValueDeclaration,
            structural_type: semantic_vocabulary::StructuralTypeId,
            catalog: PrimitiveStorageCatalog<'_>,
            range_obligation: Option<semantic_vocabulary::ObligationId>,
        ) -> Result<StructuralPlaceDeclaration, LoweringError> {
            let declarations = catalog.types;
            let mut matching = declarations.iter().filter(|row| row.id == structural_type);
            let declaration = matching.next().ok_or(LoweringError::Unsupported(
                "primitive home has no declared receiving type",
            ))?;
            if matching.next().is_some()
                || !matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(_))
                || declaration.scalar_restriction.is_some() != range_obligation.is_some()
            {
                return unsupported("primitive home lost its receiving restriction obligation");
            }
            let qualifications = terminal_semantics::primitive_scalar_memberships(
                catalog.types.iter(),
                catalog.domains.iter(),
                catalog.scalars,
                structural_type,
                &value,
            )
            .ok_or(LoweringError::Unsupported(
                "primitive initializer lost its exact scalar membership correspondence",
            ))?;
            let producer = operations.allocate();
            let mut result = structural_result_declaration(
                place,
                structural_type,
                StructuralMultiplicity::Unrestricted,
            );
            let terminal_psi::OperationResult::Structural(declared) = &mut result else {
                unreachable!("structural declaration");
            };
            declared.qualifications = qualifications;
            operations.push(Operation::unsuspended(
                producer,
                result,
                OperationKind::EstablishPrimitiveLocal {
                    value: value.id,
                    range_obligation,
                },
            ));
            Ok(operation_result_place(place, producer, structural_type))
        }

        pub(crate) fn establish_parameter_storage(
            storage: &[checked_trees::CheckedUnitParameterStoragePlan],
            scalar_parameters: &[checked_trees::CheckedStructuralScalarParameterPlan],
            values: &[terminal_psi::ValueDeclaration],
            type_ids: &[(
                typed_trees::type_identity::TypeIdentityId,
                semantic_vocabulary::StructuralTypeId,
            )],
            catalog: PrimitiveStorageCatalog<'_>,
            next_place: &mut u64,
            next_obligation: &mut u64,
            operations: &mut crate::emission::operation_emission::buffer::OperationBuffer,
        ) -> Result<Vec<ParameterStorage>, LoweringError> {
            let declarations = catalog.types;
            storage
                .iter()
                .map(|storage| {
                    let position = scalar_parameters
                        .iter()
                        .position(|parameter| parameter.source_position == storage.source_position)
                        .ok_or(LoweringError::Unsupported(
                            "parameter storage names no scalar formal",
                        ))?;
                    let value = values.get(position).ok_or(LoweringError::Unsupported(
                        "parameter storage has no incoming value",
                    ))?;
                    let scalar_type = terminal_scalar_type(storage.primitive_type)?;
                    let structural_type = lookup_type_id(type_ids, &storage.type_identity)?;
                    if !storage.symbol.is_valid()
                        || value.scalar_type != scalar_type
                        || !declarations.iter().any(|declaration| {
                            declaration.id == structural_type
                                && declaration.shape
                                    == StructuralTypeShape::PrimitiveScalar(scalar_type)
                        })
                    {
                        return unsupported("parameter storage and its incoming value disagree");
                    }
                    let range_obligation =
                        initialization_obligation(declarations, structural_type, next_obligation)?;
                    let (_, declaration) = push_established_local(
                        next_place,
                        operations,
                        *value,
                        structural_type,
                        catalog,
                        range_obligation,
                    )?;
                    Ok(ParameterStorage {
                        position,
                        local: PrimitiveLocal {
                            symbol: storage.symbol,
                            declaration,
                        },
                        scalar_type,
                    })
                })
                .collect()
        }

        pub(crate) fn find(
            locals: &[PrimitiveLocal],
            symbol: symbols::SymbolHandle,
        ) -> Result<&PrimitiveLocal, LoweringError> {
            let mut matching = locals.iter().filter(|local| local.symbol == symbol);
            let local = matching.next().ok_or(LoweringError::Unsupported(
                "primitive local has not been established",
            ))?;
            if matching.next().is_some() {
                return unsupported("primitive local establishment is duplicated");
            }
            Ok(local)
        }

        impl OperationFrame<'_, '_> {
            pub(crate) fn establish_primitive_local(
                mut self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                operation: &CheckedUnitEffectOperationPlan,
            ) -> Result<PrimitiveLocal, LoweringError> {
                let CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::Primitive {
                        statement_index,
                        symbol,
                        type_identity,
                        primitive_type,
                        value,
                    },
                ) = operation
                else {
                    return unsupported("primitive local establishment received another operation");
                };
                if self
                    .primitive_locals
                    .iter()
                    .any(|local| local.symbol == *symbol)
                    || self
                        .evaluation
                        .structural_locals
                        .iter()
                        .any(|local| local.root == facts::PlaceRoot::Symbol(*symbol))
                    || self
                        .evaluation
                        .primitive_storage
                        .iter()
                        .any(|(storage, _, _)| storage == symbol)
                {
                    return unsupported("primitive local is established more than once");
                }
                let role = CheckedScalarExpressionRole::StorageInitializer;
                let initializer = self.source_value(identities, *statement_index, role, value)?;
                let structural_type = lookup_type_id(self.type_ids, type_identity)?;
                let declarations: &[StructuralTypeDeclaration] = match &self.structural_types {
                    StructuralTypeRoster::Owned(types) => types,
                    StructuralTypeRoster::Published(types) => types,
                };
                if initializer.scalar_type != terminal_scalar_type(*primitive_type)?
                    || !declarations.iter().any(|declaration| {
                        declaration.id == structural_type
                            && declaration.shape
                                == StructuralTypeShape::PrimitiveScalar(initializer.scalar_type)
                    })
                {
                    return unsupported("primitive local initializer and referent types disagree");
                }
                let range_obligation = initialization_obligation(
                    declarations,
                    structural_type,
                    &mut self.calls.next_obligation_identity,
                )?;
                let (place, declaration) = push_established_local(
                    self.next_place,
                    self.operations,
                    initializer,
                    structural_type,
                    PrimitiveStorageCatalog {
                        types: declarations,
                        domains: self.calls.domain_declarations,
                        scalars: self.calls.scalar_qualifications,
                    },
                    range_obligation,
                )?;
                self.operations.register_structural_materialization(
                    place,
                    self.state,
                    *statement_index,
                )?;
                self.evaluation
                    .bind_structural_local(StructuralLocalBinding {
                        result_binding_ordinal: None,
                        root: facts::PlaceRoot::Symbol(*symbol),
                        structural_type,
                        argument: terminal_psi::StructuralArgument {
                            place,
                            path: Vec::new(),
                            access: terminal_psi::StructuralAccess::Owned,
                        },
                    });
                Ok(PrimitiveLocal {
                    symbol: *symbol,
                    declaration,
                })
            }
        }

        impl super::argument_evaluation::Evaluation {
            #[allow(clippy::too_many_arguments)]
            pub(crate) fn prepare_borrowed_argument(
                &mut self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                checked: &CheckedTrees,
                state: symbols::SymbolHandle,
                source: &checked_trees::CheckedUnitStructuralArgumentPlan,
                values: &[terminal_psi::ValueDeclaration],
                types: &[StructuralTypeDeclaration],
                domains: &[terminal_psi::StructuralDomainDeclaration],
                next_place: &mut u64,
                operations: &mut crate::emission::operation_emission::buffer::OperationBuffer,
                places: &mut Vec<StructuralPlaceDeclaration>,
            ) -> Result<(), LoweringError> {
                if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::ScalarParameter {
                    parameter_index,
                } = source.source
                {
                    let (_, authored) =
                        crate::shared::declarations::state_declaration(checked, state)?;
                    let parameter = checked
                        .state_parameters(authored)
                        .iter()
                        .filter(|parameter| {
                            !parameter.is_const
                                && !parameter.relevance.is_erased()
                                && checked
                                    .primitive_type_reference(parameter.type_reference)
                                    .is_some()
                        })
                        .nth(parameter_index as usize)
                        .ok_or(LoweringError::Unsupported(
                            "primitive reference ingress is absent",
                        ))?;
                    let scalar_type = terminal_scalar_type(
                        checked
                            .primitive_type_reference(parameter.type_reference)
                            .ok_or(LoweringError::Unsupported(
                                "primitive reference ingress lost its carrier",
                            ))?,
                    )?;
                    let position = self
                        .scalar_bindings
                        .immutable_position(parameter_index as usize)?;
                    let value = values.get(position).ok_or(LoweringError::Unsupported(
                        "primitive ingress value is absent",
                    ))?;
                    let identity = checked
                        .typed
                        .normalized_type_identity_text(parameter.type_reference);
                    let mut carriers = types
                        .iter()
                        .filter(|actual| actual.identity == identities.intern(&identity));
                    let carrier = carriers.next().ok_or(LoweringError::Unsupported(
                        "primitive ingress has no original storage carrier",
                    ))?;
                    if carriers.next().is_some()
                        || carrier.shape != StructuralTypeShape::PrimitiveScalar(scalar_type)
                        || value.scalar_type != scalar_type
                    {
                        return unsupported("primitive ingress value and storage carrier disagree");
                    }
                    let place = if let Some(place) = self
                        .scalar_bindings
                        .scalar_parameter_home(parameter_index, scalar_type)?
                    {
                        place
                    } else if parameter.is_mutable {
                        return unsupported("borrowed `mut` scalar formal has no storage");
                    } else {
                        self.immutable_primitive_home(
                            parameter.symbol,
                            value,
                            carrier.id,
                            types,
                            domains,
                            next_place,
                            operations,
                            places,
                        )?
                    };
                    self.scalar_bindings.register_scalar_parameter_home(
                        parameter_index,
                        place,
                        scalar_type,
                        if parameter.is_mutable {
                            terminal_psi::StructuralAccess::MutableBorrow
                        } else {
                            terminal_psi::StructuralAccess::SharedBorrow
                        },
                    )?;
                } else {
                    self.prepare_immutable_local_borrow(
                        identities,
                        checked,
                        state,
                        source.structural_storage_root(),
                        values,
                        types,
                        domains,
                        next_place,
                        operations,
                        places,
                    )?;
                }
                Ok(())
            }
            #[allow(clippy::too_many_arguments)]
            pub(crate) fn prepare_immutable_local_borrow(
                &mut self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                checked: &CheckedTrees,
                state_symbol: symbols::SymbolHandle,
                root: Option<facts::PlaceRoot>,
                values: &[terminal_psi::ValueDeclaration],
                structural_types: &[StructuralTypeDeclaration],
                domains: &[terminal_psi::StructuralDomainDeclaration],
                next_place: &mut u64,
                operations: &mut crate::emission::operation_emission::buffer::OperationBuffer,
                places: &mut Vec<StructuralPlaceDeclaration>,
            ) -> Result<bool, LoweringError> {
                let Some(facts::PlaceRoot::Symbol(symbol)) = root else {
                    return Ok(false);
                };
                let locals = &checked.facts.state_locals;
                let Some(local) = locals.named(state_symbol, symbol) else {
                    return Ok(false);
                };
                let Some(primitive) = checked.primitive_type_reference(local.declared_type) else {
                    return Ok(false);
                };
                if local.is_mutable {
                    return Ok(false);
                }
                let scalar_type = crate::emission::scalar_types::terminal_scalar_type(primitive)?;
                let ordinal = locals.scalar_locals_before(state_symbol, local.statement_index);
                let bindings = &self.scalar_bindings;
                let position = bindings.immutable_local_value(ordinal as usize, scalar_type)?;
                let value = values.get(position).ok_or(LoweringError::Unsupported(
                    "immutable primitive borrow lost its completed scalar value",
                ))?;
                if value.scalar_type != scalar_type {
                    return unsupported(
                        "immutable primitive borrow changes its completed scalar type",
                    );
                }
                let identity =
                    identities.intern(&checked.normalized_type_identity_text(local.declared_type));
                let mut carriers = structural_types
                    .iter()
                    .filter(|declaration| declaration.identity == identity);

                let structural_type = carriers
                    .next()
                    .ok_or(LoweringError::Unsupported(
                        "immutable primitive home has no storage carrier",
                    ))?
                    .id;
                if carriers.next().is_some()
                    || terminal_psi::find_structural_type(structural_types, structural_type)
                        .is_none_or(|declaration| {
                            declaration.shape != StructuralTypeShape::PrimitiveScalar(scalar_type)
                        })
                {
                    return unsupported(
                        "immutable primitive home lost its exact declared storage carrier",
                    );
                }
                let place = self.immutable_primitive_home(
                    symbol,
                    value,
                    structural_type,
                    structural_types,
                    domains,
                    next_place,
                    operations,
                    places,
                )?;
                operations.register_structural_materialization(
                    place,
                    state_symbol,
                    local.statement_index,
                )?;
                let home = StructuralLocalBinding {
                    result_binding_ordinal: None,
                    root: facts::PlaceRoot::Symbol(symbol),
                    structural_type,
                    argument: terminal_psi::StructuralArgument {
                        place,
                        path: Vec::new(),
                        access: terminal_psi::StructuralAccess::Owned,
                    },
                };
                let mut established = self
                    .structural_locals
                    .iter()
                    .filter(|binding| binding.root == home.root);
                if let Some(binding) = established.next() {
                    if established.next().is_some()
                        || binding.result_binding_ordinal.is_some()
                        || binding.structural_type != home.structural_type
                        || binding.argument != home.argument
                    {
                        return unsupported(
                            "immutable primitive home changes its established custody",
                        );
                    }
                } else {
                    self.bind_structural_local(home);
                }
                self.primitive_storage.retain(|row| row.0 != symbol);
                self.scalar_bindings.register_local_storage(
                    ordinal as usize,
                    symbol,
                    scalar_type,
                )?;
                Ok(true)
            }

            #[allow(clippy::too_many_arguments)]
            pub(crate) fn immutable_primitive_home(
                &mut self,
                symbol: symbols::SymbolHandle,
                value: &terminal_psi::ValueDeclaration,
                structural_type: semantic_vocabulary::StructuralTypeId,
                declarations: &[StructuralTypeDeclaration],
                domains: &[terminal_psi::StructuralDomainDeclaration],
                next_place: &mut u64,
                operations: &mut crate::emission::operation_emission::buffer::OperationBuffer,
                places: &mut Vec<StructuralPlaceDeclaration>,
            ) -> Result<semantic_vocabulary::PlaceId, LoweringError> {
                let mut types = declarations.iter().filter(|row| row.id == structural_type);
                if !symbol.is_valid()
                    || types.next().map(|row| &row.shape)
                        != Some(&StructuralTypeShape::PrimitiveScalar(value.scalar_type))
                    || types.next().is_some()
                {
                    return unsupported("primitive value home lost its exact storage declaration");
                }
                let mut homes = self.primitive_storage.iter().filter(|row| row.0 == symbol);
                if let Some((_, place, scalar_type)) = homes.next() {
                    if homes.next().is_some() || *scalar_type != value.scalar_type {
                        return unsupported("primitive value has ambiguous original storage");
                    }
                    return Ok(*place);
                }
                let mut homes = self
                    .structural_locals
                    .iter()
                    .filter(|row| row.root == facts::PlaceRoot::Symbol(symbol));
                if let Some(home) = homes.next() {
                    if homes.next().is_some()
                        || home.structural_type != structural_type
                        || !home.argument.path.is_empty()
                        || home.argument.access != terminal_psi::StructuralAccess::Owned
                    {
                        return unsupported("primitive value changes its original storage custody");
                    }
                    return Ok(home.argument.place);
                }
                let (place, declaration) = push_established_local(
                    next_place,
                    operations,
                    *value,
                    structural_type,
                    PrimitiveStorageCatalog {
                        types: declarations,
                        domains,
                        scalars: self.shared_qualifications.catalog(),
                    },
                    None,
                )?;
                places.push(declaration);
                self.primitive_storage
                    .push((symbol, place, value.scalar_type));
                Ok(place)
            }
        }
    }
    pub(crate) mod reference_results {
        use super::super::StructuralArgument;
        use super::{
            LoweringError, Operation, OperationKind, StructuralMultiplicity,
            StructuralPlaceDeclaration, StructuralTypeId, allocate_dense, lookup_type_id, place_id,
        };
        use crate::emission::operation_emission::buffer::{
            OperationBuffer, structural_result_declaration,
        };
        use crate::emission::operation_emission::place_declarations::operation_result_place;
        use crate::emission::scalar_types::terminal_scalar_type;
        use crate::retention::access_lifecycles::storage_fields;
        use crate::shared::terminal_identities::lookup_domain_id_for_carrier;
        use crate::shared::terminal_identities::{
            stored_structural_identity, structural_declaration_by_identity,
        };
        use checked_trees::CheckedUnitStructuralResultBindingPlan;

        pub(crate) struct Formation<'checked> {
            pub(crate) checked: &'checked checked_trees::CheckedTrees,
            pub(crate) statement_index: usize,
            pub(crate) expression: checked_trees::expression::ExpressionHandle,
            pub(crate) backing_type: typed_trees::types::TypeReferenceHandle,
            pub(crate) source_contract: &'checked checked_trees::CheckedReferenceSourceContract,
            pub(crate) qualification_decisions:
                arena::HandleSpan<checked_trees::CheckedValueQualification>,
            pub(crate) domain_ids:
                &'checked [crate::shared::terminal_identities::StructuralDomainBinding],
        }

        #[derive(Debug, Clone, Copy)]
        pub(crate) enum PublishedReferenceStorage {
            Declared,
            RepresentationView(terminal_psi::ReferenceStorageView),
            Access,
            ResidentAccess {
                extent: symbols::SymbolHandle,
                granted: language_semantics::SemanticDomainId,
                resident: language_semantics::SemanticDomainId,
            },
        }

        pub(crate) fn publish_storage(
            storage: checked_trees::CheckedReferenceStorage,
            next_value: &mut u64,
            operations: &mut OperationBuffer,
        ) -> Result<
            (
                PublishedReferenceStorage,
                Option<terminal_psi::ValueDeclaration>,
            ),
            LoweringError,
        > {
            let checked_trees::CheckedReferenceStorage::View { stored_scalar } = storage else {
                return Ok((
                    match storage {
                        checked_trees::CheckedReferenceStorage::Access => {
                            PublishedReferenceStorage::Access
                        }
                        checked_trees::CheckedReferenceStorage::ResidentAccess {
                            extent,
                            granted,
                            resident,
                        } => PublishedReferenceStorage::ResidentAccess {
                            extent,
                            granted,
                            resident,
                        },
                        _ => PublishedReferenceStorage::Declared,
                    },
                    None,
                ));
            };
            let byte_offset =
                crate::shared::terminal_identities::value_id(allocate_dense(next_value)?);
            let value = terminal_psi::ValueDeclaration {
                id: byte_offset,
                scalar_type: crate::unit::terminal_scalar_type(crate::unit::PrimitiveType::U64)?,
                qualifications: Default::default(),
            };
            let id = operations.allocate();
            operations.push(Operation::unsuspended(
                id,
                terminal_psi::OperationResult::Scalar(value),
                OperationKind::IntegerConstant {
                    value: semantic_vocabulary::IntegerValue::Unsigned(0),
                },
            ));
            Ok((
                PublishedReferenceStorage::RepresentationView(terminal_psi::ReferenceStorageView {
                    byte_offset,
                    stored_scalar,
                }),
                Some(value),
            ))
        }

        pub(crate) fn emit(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            formation: Formation<'_>,
            result: &CheckedUnitStructuralResultBindingPlan,
            source: StructuralArgument,
            storage: PublishedReferenceStorage,
            types: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
            declarations: &[terminal_psi::StructuralTypeDeclaration],
            evaluation: &super::argument_evaluation::Evaluation,
            next_place: &mut u64,
            operations: &mut OperationBuffer,
        ) -> Result<StructuralPlaceDeclaration, LoweringError> {
            let structural_type = lookup_type_id(types, &result.type_identity)?;
            emit_carrier(
                identities,
                formation,
                structural_type,
                source,
                storage,
                declarations,
                evaluation,
                place_id(allocate_dense(next_place)?),
                operations,
            )
        }

        pub(crate) fn emit_carrier(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            formation: Formation<'_>,
            structural_type: StructuralTypeId,
            source: StructuralArgument,
            storage: PublishedReferenceStorage,
            declarations: &[terminal_psi::StructuralTypeDeclaration],
            evaluation: &super::argument_evaluation::Evaluation,
            place: semantic_vocabulary::PlaceId,
            operations: &mut OperationBuffer,
        ) -> Result<StructuralPlaceDeclaration, LoweringError> {
            let id = operations.allocate();
            let parameters = evaluation.structural_root_parameters().collect::<Vec<_>>();
            let resident_source = if let PublishedReferenceStorage::ResidentAccess {
                extent,
                granted,
                resident,
            } = storage
            {
                let (root, _, _, _) =
                    super::structural_values::member_memberships::source_memberships_in(
                        source.place,
                        parameters.iter().copied(),
                        operations.iter().chain(
                            evaluation
                                .blocks
                                .iter()
                                .flat_map(|block| block.operations.iter()),
                        ),
                    )
                    .ok_or(LoweringError::Unsupported(
                        "resident reference lost its supplying declaration",
                    ))?;
                let carrier = terminal_semantics::runtime_structural_path_tip(
                    declarations.iter(),
                    root,
                    &source.path,
                )
                .ok_or(LoweringError::Unsupported(
                    "resident reference lost its supplying owner",
                ))?;
                let (base, length) =
                    storage_fields(identities, formation.checked, extent, carrier)?;
                let carrier = carrier.id;
                Some(terminal_psi::ResidentReferenceSource {
                    base,
                    length,
                    granted: lookup_domain_id_for_carrier(formation.domain_ids, granted, carrier)?,
                    resident: lookup_domain_id_for_carrier(
                        formation.domain_ids,
                        resident,
                        carrier,
                    )?,
                })
            } else {
                None
            };
            let declaration = terminal_psi::find_structural_type(declarations, structural_type)
                .ok_or(LoweringError::Unsupported("loan carrier type is missing"))?;
            let (kind, multiplicity) = match declaration.shape {
                terminal_psi::StructuralTypeShape::Reference { .. } => (
                    match storage {
                        PublishedReferenceStorage::Access
                        | PublishedReferenceStorage::ResidentAccess { .. } => {
                            OperationKind::EstablishAccessReference {
                                source: source.clone(),
                                resident: resident_source,
                            }
                        }
                        PublishedReferenceStorage::Declared => OperationKind::EstablishReference {
                            source: source.clone(),
                            storage: None,
                        },
                        PublishedReferenceStorage::RepresentationView(view) => {
                            OperationKind::EstablishReference {
                                source: source.clone(),
                                storage: Some(view),
                            }
                        }
                    },
                    StructuralMultiplicity::Affine,
                ),
                terminal_psi::StructuralTypeShape::ByteSequence(
                    terminal_psi::byte_sequence_view_carrier_pattern!(),
                )
                | terminal_psi::StructuralTypeShape::ElementView { .. }
                    if matches!(storage, PublishedReferenceStorage::Declared) =>
                {
                    let element = match declaration.shape {
                        terminal_psi::StructuralTypeShape::ElementView { element, .. } => element,
                        _ => {
                            let scalar =
                                terminal_scalar_type(checked_trees::types::PrimitiveType::U8)?;
                            declarations
                                .iter()
                                .find(|declaration| {
                                    declaration.scalar_restriction.is_none()
                                        && declaration.shape
                                            == terminal_psi::StructuralTypeShape::PrimitiveScalar(
                                                scalar,
                                            )
                                })
                                .ok_or(LoweringError::Unsupported(
                                    "byte loan carrier has no u8 declaration",
                                ))?
                                .id
                        }
                    };
                    let region = terminal_psi::ElementViewRegion::WholeCollection;
                    match source.access {
                        terminal_psi::StructuralAccess::SharedBorrow => (
                            OperationKind::EstablishElementView {
                                destination: place,
                                source: source.clone(),
                                element,
                                region,
                            },
                            StructuralMultiplicity::Unrestricted,
                        ),
                        terminal_psi::StructuralAccess::MutableBorrow => (
                            OperationKind::EstablishMutableElementView {
                                destination: place,
                                source: source.clone(),
                                element,
                                region,
                            },
                            StructuralMultiplicity::Affine,
                        ),
                        _ => {
                            return Err(LoweringError::Unsupported(
                                "byte loan carrier has no readable access",
                            ));
                        }
                    }
                }
                _ => {
                    return Err(LoweringError::Unsupported(
                        "loan carrier has no formation operation",
                    ));
                }
            };
            let mut result = structural_result_declaration(place, structural_type, multiplicity);
            if resident_source.is_none()
                && matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::Reference { .. }
                )
            {
                let terminal_psi::OperationResult::Structural(result) = &mut result else {
                    return Err(LoweringError::Unsupported(
                        "reference result is not structural",
                    ));
                };
                if let Some((root, whole, projected, additional)) =
                    super::structural_values::member_memberships::source_memberships_in(
                        source.place,
                        parameters.iter().copied(),
                        operations.iter().chain(
                            evaluation
                                .blocks
                                .iter()
                                .flat_map(|block| block.operations.iter()),
                        ),
                    )
                {
                    let path = terminal_semantics::reference_source_storage_path(
                        declarations.iter(),
                        root,
                        &source.path,
                    );
                    let (captured, additional_input) =
                        terminal_semantics::reference_carrier_membership_contract(
                            declarations.iter(),
                            root,
                            whole,
                            projected,
                            additional,
                            path,
                            source.access,
                            structural_type,
                            match &storage {
                                PublishedReferenceStorage::RepresentationView(view) => Some(view),
                                _ => None,
                            },
                        )
                        .map_err(LoweringError::Unsupported)?;
                    result.projected_qualifications.extend(captured);
                    result.additional_input_qualifications = additional_input;
                    if let Some(field) = terminal_semantics::runtime_structural_field(
                        declarations.iter(),
                        root,
                        path,
                    ) && field.field_type.byte_sequence_carrier().is_some()
                    {
                        if !formation.backing_type.is_valid() {
                            return Err(LoweringError::Unsupported(
                                "inline field loan lost its supplying type",
                            ));
                        }
                        let terminal_psi::StructuralTypeShape::Reference { referent, .. } =
                            declaration.shape
                        else {
                            return Err(LoweringError::Unsupported(
                                "field loan lost its reference carrier",
                            ));
                        };
                        let decisions = formation
                            .checked
                            .facts
                            .qualifications
                            .value_formations
                            .span(formation.qualification_decisions)
                            .ok_or(LoweringError::Unsupported(
                                "field loan membership decisions are stale",
                            ))?;
                        for domain in decisions.iter().map(|decision| decision.semantic_domain) {
                            result.projected_qualifications.push(
                                terminal_psi::StructuralPathQualification {
                                    path: vec![terminal_psi::StructuralPathSegment::Referent],
                                    domain: lookup_domain_id_for_carrier(
                                        formation.domain_ids,
                                        domain,
                                        referent,
                                    )?,
                                },
                            );
                        }
                    }
                }
                if matches!(
                    storage,
                    PublishedReferenceStorage::Declared
                        | PublishedReferenceStorage::RepresentationView(_)
                ) {
                    let required = source_contract_memberships(
                        identities,
                        &formation,
                        declaration,
                        source.access,
                        storage,
                        declarations,
                    )?;
                    result.additional_input_qualifications.retain(|row| {
                        !terminal_psi::StructuralPathQualification::select(
                            &[],
                            &required,
                            &row.path,
                        )
                        .0
                        .contains(&row.domain)
                    });
                    result.projected_qualifications.extend(required);
                }
                result.projected_qualifications.sort();
                result.projected_qualifications.dedup();
                result.additional_input_qualifications.sort();
                result.additional_input_qualifications.dedup();
            }
            for loan in operations
                .reference_formations
                .loans_for(formation.statement_index, formation.expression)
            {
                operations
                    .loan_captures
                    .push(lowered_psi::LoweredSourceLoanCapture {
                        loan,
                        formation: id,
                        carrier: place,
                        source: source.clone(),
                    });
            }
            operations.reference_backings.record(place, source);
            operations.push(Operation::unsuspended(id, result, kind));
            Ok(operation_result_place(place, id, structural_type))
        }

        fn source_contract_memberships(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            formation: &Formation<'_>,
            declaration: &terminal_psi::StructuralTypeDeclaration,
            access: terminal_psi::StructuralAccess,
            storage: PublishedReferenceStorage,
            declarations: &[terminal_psi::StructuralTypeDeclaration],
        ) -> Result<Vec<terminal_psi::StructuralPathQualification>, LoweringError> {
            let refuse = |reason: &str| LoweringError::InvalidReferenceContract {
                supplying_type: formation.backing_type,
                reason: reason.to_owned(),
            };
            let contract = match formation.source_contract {
                checked_trees::CheckedReferenceSourceContract::Standing { contract, .. } => {
                    contract
                }
                checked_trees::CheckedReferenceSourceContract::Unavailable(reason) => {
                    return Err(refuse(reason));
                }
                checked_trees::CheckedReferenceSourceContract::SeparateStorage => {
                    return Err(refuse("pointer formation has no supplying contract"));
                }
            };
            if contract.qualifications.is_empty() && contract.projected_qualifications.is_empty() {
                return Ok(Vec::new());
            }
            if !formation.backing_type.is_valid() {
                return Err(refuse("reference formation lost its supplying type"));
            }
            let supplying = structural_declaration_by_identity(
                declarations,
                identities,
                formation.checked,
                stored_structural_identity(formation.checked, formation.backing_type),
                "reference formation lost its supplying carrier",
            )?;
            let terminal_psi::StructuralTypeShape::Reference { referent, .. } = declaration.shape
            else {
                return Err(refuse("reference formation lost its reference carrier"));
            };
            if matches!(storage, PublishedReferenceStorage::Declared) && supplying.id != referent {
                return Err(refuse(
                    "reference formation source has a non-matching referent carrier",
                ));
            }
            let projected = super::parameters::lower_projected_qualifications(
                identities,
                &contract.projected_qualifications,
                formation.domain_ids,
                supplying.id,
                declarations,
            )?;
            let whole = contract
                .qualifications
                .iter()
                .map(|domain| {
                    lookup_domain_id_for_carrier(formation.domain_ids, *domain, supplying.id)
                })
                .collect::<Result<Vec<_>, _>>()?;
            terminal_semantics::reference_carrier_memberships(
                declarations.iter(),
                supplying.id,
                &whole,
                &projected,
                &[],
                access,
                declaration.id,
                match &storage {
                    PublishedReferenceStorage::RepresentationView(view) => Some(view),
                    _ => None,
                },
            )
            .map_err(refuse)
        }
    }
    pub(crate) mod scalar_arrays {
        use super::{
            LoweringError, Operation, OperationKind, StructuralMultiplicity,
            StructuralPlaceDeclaration, StructuralTypeId, ValueDeclaration, allocate_dense,
            lookup_type_id, place_id,
        };
        use crate::emission::operation_emission::buffer::{
            OperationBuffer, structural_result_declaration,
        };
        use crate::emission::operation_emission::place_declarations::operation_result_place;
        use checked_trees::CheckedUnitStructuralResultBindingPlan;

        /// Whether a receiver can replay every one of these memberships from
        /// the literal operands of an array construction.
        ///
        /// Construction itself establishes no qualification evidence. The only
        /// memberships it may state are byte predicates, which a receiver
        /// re-evaluates over the literal octets. Any other checked membership,
        /// such as a predicate over `u16` elements, is added by `qualify`.
        pub(crate) fn replayed_at_construction(
            domains: &[terminal_psi::StructuralDomainDeclaration],
            qualifications: &[semantic_vocabulary::StructuralDomainId],
        ) -> bool {
            qualifications.iter().all(|identity| {
                domains
                    .iter()
                    .any(|domain| domain.id == *identity && domain.byte_predicate.is_some())
            })
        }

        /// Adds checked memberships to a completed array by a whole-value
        /// copy, the same transfer that qualifies a local built by stores.
        pub(crate) fn qualify(
            constructed: semantic_vocabulary::PlaceId,
            structural_type: StructuralTypeId,
            multiplicity: StructuralMultiplicity,
            mut qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
            next_place: &mut u64,
            operations: &mut OperationBuffer,
        ) -> Result<StructuralPlaceDeclaration, LoweringError> {
            qualifications.sort();
            qualifications.dedup();
            let copy = operations.allocate();
            let qualified = place_id(allocate_dense(next_place)?);
            let mut result =
                structural_result_declaration(qualified, structural_type, multiplicity);
            if let terminal_psi::OperationResult::Structural(result) = &mut result {
                result.qualifications = qualifications;
            }
            operations.push(Operation::unsuspended(
                copy,
                result,
                OperationKind::StructuralLeafCopy {
                    source: constructed,
                    path: Vec::new(),
                },
            ));
            Ok(operation_result_place(qualified, copy, structural_type))
        }

        /// Emits one array construction and returns the place that holds the
        /// checked value. `private_places` receives the construction place
        /// when a qualifying copy follows it.
        #[allow(clippy::too_many_arguments)]
        pub(crate) fn emit(
            result: &CheckedUnitStructuralResultBindingPlan,
            mut qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
            domains: &[terminal_psi::StructuralDomainDeclaration],
            elements: &[ValueDeclaration],
            types: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
            next_place: &mut u64,
            private_places: &mut Vec<StructuralPlaceDeclaration>,
            operations: &mut OperationBuffer,
        ) -> Result<StructuralPlaceDeclaration, LoweringError> {
            let structural_type = lookup_type_id(types, &result.type_identity)?;
            let id = operations.allocate();
            let place = place_id(allocate_dense(next_place)?);
            qualifications.sort();
            qualifications.dedup();
            let replayed = replayed_at_construction(domains, &qualifications);
            let mut constructed = structural_result_declaration(
                place,
                structural_type,
                StructuralMultiplicity::Unrestricted,
            );
            if replayed
                && let terminal_psi::OperationResult::Structural(constructed) = &mut constructed
            {
                constructed.qualifications = qualifications.clone();
            }
            operations.push(Operation::unsuspended(
                id,
                constructed,
                OperationKind::EstablishScalarArray {
                    elements: elements.iter().map(|element| element.id).collect(),
                },
            ));
            let construction = operation_result_place(place, id, structural_type);
            if replayed {
                return Ok(construction);
            }
            private_places.push(construction);
            qualify(
                place,
                structural_type,
                StructuralMultiplicity::Unrestricted,
                qualifications,
                next_place,
                operations,
            )
        }
    }
    pub(crate) mod view_ranges {
        use super::{CheckedTrees, LoweringError, allocate_dense, place_id, unsupported};
        use crate::emission::operation_emission::buffer::{
            OperationBuffer, structural_result_declaration,
        };
        use crate::emission::operation_emission::expressions::emit_direct_expression;
        use crate::emission::operation_emission::expressions::{
            LoweredDirectExpression, LoweredElementPathSegment, TERMINAL_NESTED_SELECTOR_LIMIT,
            emit_prepared_path, path_with_current_bounds,
        };
        use crate::emission::operation_emission::place_declarations::operation_result_place;
        use crate::emission::operation_emission::view_subslice::{self, ViewFamily};
        use crate::emission::scalar_types::terminal_scalar_type;
        use crate::expression_preparation::bindings::ScalarBindings;
        use crate::expression_preparation::bindings::structural_paths::prepare_projection_path;
        use crate::expression_preparation::bindings::structural_paths::terminal_access;
        use crate::expression_preparation::prepare_expression::exact_u64_index;
        use crate::shared::checked_facts::flow_facts;
        use crate::shared::terminal_identities::obligation_id;
        use checked_trees::expression::ExpressionHandle;
        use checked_trees::{
            CheckedCallScalarArgument, CheckedScalarExpressionRole, CheckedSubsliceSite,
        };
        use checked_trees::{
            CheckedStorageRoot, CheckedUnitStructuralArgumentPlan,
            CheckedUnitStructuralArgumentSourcePlan,
        };
        use semantic_vocabulary::{PlaceId, StructuralTypeId};
        use terminal_psi::{StructuralPlaceDeclaration, ValueDeclaration};

        #[derive(Clone, Copy)]
        pub(crate) struct Range<'a> {
            pub(crate) root: CheckedStorageRoot,
            pub(crate) family: ViewFamily,
            pub(crate) expression: ExpressionHandle,
            pub(crate) start: &'a Option<CheckedCallScalarArgument>,
            pub(crate) end: &'a Option<CheckedCallScalarArgument>,
            pub(crate) end_inclusive: bool,
            pub(crate) site: CheckedSubsliceSite,
        }

        #[derive(Clone, Copy)]
        pub(crate) struct RangeEndpoints<'a> {
            pub(crate) start: &'a Option<CheckedCallScalarArgument>,
            pub(crate) end: &'a Option<CheckedCallScalarArgument>,
            pub(crate) end_inclusive: bool,
            pub(crate) site: CheckedSubsliceSite,
        }

        impl<'a> Range<'a> {
            pub(crate) fn endpoints(self) -> RangeEndpoints<'a> {
                RangeEndpoints {
                    start: self.start,
                    end: self.end,
                    end_inclusive: self.end_inclusive,
                    site: self.site,
                }
            }
        }

        pub(crate) fn range(argument: &CheckedUnitStructuralArgumentPlan) -> Option<Range<'_>> {
            let (root, family, expression, start, end, end_inclusive, site) = match &argument.source
            {
                CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
                    root,
                    expression,
                    start,
                    end,
                    end_inclusive,
                    site,
                    storage_type_identity: _,
                    root_type_identity: _,
                } => (
                    *root,
                    ViewFamily::Bytes,
                    *expression,
                    start,
                    end,
                    *end_inclusive,
                    *site,
                ),
                CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
                    root,
                    expression,
                    start,
                    end,
                    end_inclusive,
                    site,
                    storage_type_identity: _,
                    root_type_identity: _,
                } => (
                    *root,
                    ViewFamily::Elements,
                    *expression,
                    start,
                    end,
                    *end_inclusive,
                    *site,
                ),
                _ => return None,
            };
            Some(Range {
                root,
                family,
                expression,
                start,
                end,
                end_inclusive,
                site,
            })
        }

        #[derive(Clone)]
        pub(crate) struct RangeStorage {
            source: CheckedUnitStructuralArgumentPlan,
            structural_type: StructuralTypeId,
            family: ViewFamily,
            collection_element: Option<StructuralTypeId>,
            access: checked_trees::CheckedStructuralAccess,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct CollectionView {
            pub(crate) source: PlaceId,
            pub(crate) access: terminal_psi::StructuralAccess,
            pub(crate) path: Vec<LoweredElementPathSegment>,
            pub(crate) element: StructuralTypeId,
            pub(crate) place: PlaceId,
            pub(crate) structural_type: StructuralTypeId,
            pub(crate) byte_region: Option<(LoweredDirectExpression, LoweredDirectExpression)>,
        }

        pub(crate) fn element_type(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            structural_type: StructuralTypeId,
            declarations: &[terminal_psi::StructuralTypeDeclaration],
        ) -> Result<StructuralTypeId, LoweringError> {
            let declaration = declarations
                .iter()
                .find(|declaration| declaration.id == structural_type)
                .ok_or(LoweringError::Unsupported(
                    "element view lost its descriptor carrier",
                ))?;
            match declaration.shape {
                terminal_psi::StructuralTypeShape::ElementView { element, .. } => Ok(element),
                terminal_psi::StructuralTypeShape::ByteSequence(_) => {
                    let identity = crate::shared::terminal_identities::byte_view_element_identity(
                        flow_facts(checked)
                            .terminal_unit_effects
                            .structural_types
                            .iter(),
                    )
                    .ok_or(LoweringError::Unsupported(
                        "byte view lost its element identity",
                    ))?;
                    crate::shared::terminal_identities::structural_declaration_by_identity(
                        declarations,
                        identities,
                        checked,
                        identity,
                        "byte view lost its element carrier",
                    )
                    .map(|declaration| declaration.id)
                }
                _ => unsupported("element view has no descriptor carrier"),
            }
        }

        impl RangeStorage {
            pub(crate) fn requires_establishment(&self) -> bool {
                self.collection_element.is_some()
            }

            pub(crate) fn bind(
                &self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,

                checked: &CheckedTrees,
                state: symbols::SymbolHandle,
                bindings: &ScalarBindings,
                destination: Option<PlaceId>,
                declarations: &[terminal_psi::StructuralTypeDeclaration],
            ) -> Result<(ViewRangeSource, Option<CollectionView>), LoweringError> {
                let (_, authored) = crate::shared::declarations::state_declaration(checked, state)?;
                let resolved = bindings.resolve_borrowed_argument(
                    checked,
                    &self.source,
                    checked.state_parameters(authored),
                )?;
                let argument = &resolved.argument;
                let source = &resolved.source;
                let root =
                    bindings
                        .structural_argument_type(source)
                        .ok_or(LoweringError::Unsupported(
                            "range source lost its published storage type",
                        ))?;
                let mut prepared = prepare_projection_path(
                    identities,
                    checked,
                    &argument.path,
                    crate::expression_preparation::bindings::structural_paths::ProjectionRoot::Borrowed {
                        published: root,
                        logical: resolved.root_type_identity
                            .or_else(|| self.source.subslice_root_type_identity())
                            .ok_or(
                            LoweringError::Unsupported("range source lost its logical root"),
                        )?,
                        spelled_reference_hops: 0,
                    },
                    self.source
                        .subslice_storage_type_identity()
                        .ok_or(LoweringError::Unsupported(
                            "range source lost its collection declaration",
                        ))?,
                    terminal_access(argument.access),
                    declarations,
                )?;
                crate::expression_preparation::bindings::structural_paths::retain_selector_evaluations(
                    &argument.path,
                    argument.projection_evaluations,
                    &checked.facts.values.scalar_computations.operands,
                    &mut prepared,
                )?;
                resolved.retain_captured_coordinates(&mut prepared)?;
                let path = source
                    .path
                    .iter()
                    .cloned()
                    .map(LoweredElementPathSegment::Static)
                    .chain(collection_path(
                        identities,
                        prepared,
                        bindings,
                        source.place,
                    )?)
                    .collect::<Vec<_>>();
                let (place, path, establishment) = if let Some(element) = self.collection_element {
                    let destination = destination.ok_or(LoweringError::Unsupported(
                        "collection range has no reserved whole-view descriptor",
                    ))?;
                    (
                        destination,
                        Vec::new(),
                        Some(CollectionView {
                            source: source.place,
                            access: terminal_access(self.access),
                            path,
                            element,
                            place: destination,
                            structural_type: self.structural_type,
                            byte_region: None,
                        }),
                    )
                } else {
                    (source.place, path, None)
                };
                Ok((
                    ViewRangeSource {
                        place,
                        path,
                        structural_type: self.structural_type,
                        family: self.family,
                        access: terminal_access(self.access),
                    },
                    establishment,
                ))
            }
        }

        fn collection_path(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            path: Vec<
                crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment,
            >,
            bindings: &ScalarBindings,
            source: PlaceId,
        ) -> Result<Vec<LoweredElementPathSegment>, LoweringError> {
            use crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment;
            let mut runtime_indices = 0;
            let mut dynamic_extent = false;
            path.into_iter()
                .map(|segment| {
                    if matches!(
                        &segment,
                        PreparedStructuralPathSegment::RuntimeIndex(_)
                            | PreparedStructuralPathSegment::CapturedIndex(_)
                    ) {
                        dynamic_extent = false;
                        runtime_indices += 1;
                        if runtime_indices > TERMINAL_NESTED_SELECTOR_LIMIT {
                            return unsupported(
                                "collection view exceeds the runtime selector namespace",
                            );
                        }
                    }
                    match segment {
                    PreparedStructuralPathSegment::Static(segment) => {
                        if dynamic_extent
                            && let terminal_psi::StructuralPathSegment::FixedIndex(index) = segment
                        {
                            dynamic_extent = false;
                            runtime_indices += 1;
                            if runtime_indices > TERMINAL_NESTED_SELECTOR_LIMIT {
                                return unsupported(
                                    "collection view exceeds the runtime selector namespace",
                                );
                            }
                            return Ok(LoweredElementPathSegment::RuntimeIndex {
                                selector: LoweredDirectExpression::IntegerLiteral {
                                    value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(
                                        index,
                                    )),
                                    scalar_type: terminal_scalar_type(
                                        checked_trees::types::PrimitiveType::U64,
                                    )?,
                                },
                                extent: None,
                                bound_decided: false,
                            });
                        }
                        Ok(LoweredElementPathSegment::Static(segment))
                    }
                    PreparedStructuralPathSegment::ViewExtent { bytes } => {
                        dynamic_extent = true;
                        Ok(LoweredElementPathSegment::ViewExtent { source, bytes })
                    }
                    PreparedStructuralPathSegment::ComputedIndex(_)
                    | PreparedStructuralPathSegment::EvaluatedFixedIndex { .. } => {
                        unsupported("computed view selector requires its owning operand evaluation")
                    }
                    PreparedStructuralPathSegment::CapturedIndex(place) => {
                        Ok(LoweredElementPathSegment::RuntimeIndex {
                            selector: LoweredDirectExpression::PrimitiveRead {
                                qualifications: Default::default(),
                                source: crate::emission::operation_emission::expressions::PrimitiveReadSource::Place(place),
                                path: Vec::new(),
                                scalar_type: terminal_scalar_type(
                                    checked_trees::types::PrimitiveType::U64,
                                )?,
                            },
                            extent: None,
                            bound_decided: false,
                        })
                    }
                    PreparedStructuralPathSegment::RuntimeIndex(index) => {
                        let selector = match index {
                            checked_trees::CheckedRuntimeIndex::StorageRead { symbol } => {
                                bindings.runtime_index_storage(symbol)?
                            }
                            checked_trees::CheckedRuntimeIndex::Parameter { position } => {
                                bindings.runtime_index_parameter(position)?
                            }
                            checked_trees::CheckedRuntimeIndex::Local { position } => {
                                bindings.runtime_index_local(position)?
                            }
                            checked_trees::CheckedRuntimeIndex::StructuralField(field) => {
                                bindings.runtime_index_field(identities, &field)?
                            }
                            _ => {
                                return unsupported(
                                    "collection view cannot borrow an assignment selector",
                                );
                            }
                        };
                        Ok(LoweredElementPathSegment::RuntimeIndex {
                            selector: exact_u64_index(selector)?,
                            extent: None,
                            bound_decided: false,
                        })
                    }
                    }
                })
                .collect()
        }

        pub(crate) fn storage(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            _state: symbols::SymbolHandle,
            _statement: u32,
            argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
            declarations: &[terminal_psi::StructuralTypeDeclaration],
        ) -> Result<RangeStorage, LoweringError> {
            let range = range(argument).ok_or(LoweringError::Unsupported(
                "range storage has no range operation",
            ))?;
            if !matches!(
                argument.access,
                checked_trees::CheckedStructuralAccess::SharedBorrow
                    | checked_trees::CheckedStructuralAccess::MutableBorrow
            ) {
                return unsupported("range storage requires a readable loan");
            }
            let (checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
                storage_type_identity,
                ..
            }
            | checked_trees::CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
                storage_type_identity,
                ..
            }) = &argument.source
            else {
                return unsupported("range storage has no range operation");
            };
            let identity = checked
                .type_identity_text(*storage_type_identity)
                .unwrap_or_default();
            let collection = terminal_psi::structural_type_by_identity(
                declarations,
                identities.intern(&identity),
            )
            .ok_or(LoweringError::Unsupported(
                "range collection has no exact Terminal type",
            ))?;
            let view = crate::shared::terminal_identities::structural_declaration_by_identity(
                declarations,
                identities,
                checked,
                argument.type_identity,
                "range result has no exact Terminal type",
            )?;
            let collection_element = match collection.shape {
                terminal_psi::StructuralTypeShape::FixedArray { element, .. } => Some(element),
                terminal_psi::StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                    | terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                ) if range.family == ViewFamily::Bytes && view.shape.is_borrowed_byte_view() => {
                    let byte = terminal_scalar_type(checked_trees::types::PrimitiveType::U8)?;
                    Some(
                        declarations
                            .iter()
                            .find(|declaration| {
                                declaration.shape
                                    == terminal_psi::StructuralTypeShape::PrimitiveScalar(byte)
                            })
                            .map(|declaration| declaration.id)
                            .ok_or(LoweringError::Unsupported(
                                "bounded byte backing has no exact primitive element declaration",
                            ))?,
                    )
                }
                terminal_psi::StructuralTypeShape::ElementView { element, .. }
                    if range.family == ViewFamily::Elements
                        && matches!(view.shape, terminal_psi::StructuralTypeShape::ElementView { element: actual, .. } if actual == element) =>
                {
                    None
                }
                terminal_psi::StructuralTypeShape::ByteSequence(
                    terminal_psi::byte_sequence_view_carrier_pattern!(),
                ) if range.family == ViewFamily::Bytes && view.shape.is_borrowed_byte_view() => {
                    None
                }
                _ => {
                    return unsupported(
                        "range collection is neither backing nor its exact descriptor",
                    );
                }
            };
            Ok(RangeStorage {
                source: argument.clone(),
                structural_type: view.id,
                family: range.family,
                collection_element,
                access: argument.access,
            })
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn stage_source(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            state: symbols::SymbolHandle,
            statement: u32,
            argument: &CheckedUnitStructuralArgumentPlan,
            declarations: &[terminal_psi::StructuralTypeDeclaration],
            bindings: &ScalarBindings,
            next_place: &mut u64,
            values: &[ValueDeclaration],
            next_value: &mut u64,
            operations: &mut OperationBuffer,
        ) -> Result<(ViewRangeSource, Option<StructuralPlaceDeclaration>), LoweringError> {
            let storage = storage(
                identities,
                checked,
                state,
                statement,
                argument,
                declarations,
            )?;
            let whole = if storage.requires_establishment() {
                Some(place_id(allocate_dense(next_place)?))
            } else {
                None
            };
            let (source, establishment) =
                storage.bind(identities, checked, state, bindings, whole, declarations)?;
            let declaration = establishment
                .map(|establishment| establish(&establishment, values, next_value, operations))
                .transpose()?;
            Ok((source, declaration))
        }

        pub(crate) fn establish(
            view: &CollectionView,
            values: &[ValueDeclaration],
            next_value: &mut u64,
            operations: &mut OperationBuffer,
        ) -> Result<StructuralPlaceDeclaration, LoweringError> {
            let path = emit_prepared_path(&view.path, 0, values, next_value, operations);
            let endpoints = view.byte_region.as_ref().map(|(start, end)| {
                (
                    emit_direct_expression(start, values, next_value, operations),
                    emit_direct_expression(end, values, next_value, operations),
                )
            });
            let producer = operations.allocate();
            let source = terminal_psi::StructuralArgument {
                place: view.source,
                access: view.access,
                path: path_with_current_bounds(&path, producer.get()),
            };
            let region = match endpoints {
                None => terminal_psi::ElementViewRegion::WholeCollection,
                Some((start, end)) => terminal_psi::ElementViewRegion::RepresentationBytes {
                    start,
                    end,
                    obligation: obligation_id(producer.get().checked_add(1).ok_or(
                        LoweringError::Unsupported(
                            "representation view obligation identity overflows",
                        ),
                    )?),
                },
            };
            Ok(emit_view(
                view.place,
                view.structural_type,
                source,
                view.element,
                region,
                if view.access == terminal_psi::StructuralAccess::MutableBorrow {
                    terminal_psi::StructuralMultiplicity::Affine
                } else {
                    terminal_psi::StructuralMultiplicity::Unrestricted
                },
                producer,
                operations,
            ))
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn emit_view(
            place: PlaceId,
            structural_type: StructuralTypeId,
            source: terminal_psi::StructuralArgument,
            element: StructuralTypeId,
            region: terminal_psi::ElementViewRegion,
            multiplicity: terminal_psi::StructuralMultiplicity,
            producer: semantic_vocabulary::OperationId,
            operations: &mut OperationBuffer,
        ) -> StructuralPlaceDeclaration {
            let access = source.access;
            operations.push(terminal_psi::Operation::unsuspended(
                producer,
                structural_result_declaration(place, structural_type, multiplicity),
                if access == terminal_psi::StructuralAccess::MutableBorrow {
                    terminal_psi::OperationKind::EstablishMutableElementView {
                        destination: place,
                        source,
                        element,
                        region,
                    }
                } else {
                    terminal_psi::OperationKind::EstablishElementView {
                        destination: place,
                        source,
                        element,
                        region,
                    }
                },
            ));
            operation_result_place(place, producer, structural_type)
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct ViewRangeSource {
            pub(crate) place: PlaceId,
            pub(crate) path: Vec<LoweredElementPathSegment>,
            pub(crate) structural_type: StructuralTypeId,
            pub(crate) family: ViewFamily,
            pub(crate) access: terminal_psi::StructuralAccess,
        }

        pub(crate) struct ViewRangeSite<'a> {
            pub(crate) state: symbols::SymbolHandle,
            pub(crate) statement: u32,
            pub(crate) range: RangeEndpoints<'a>,
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn emit(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            site: ViewRangeSite<'_>,
            source: ViewRangeSource,
            result_type: StructuralTypeId,
            destination: PlaceId,
            evaluation: &mut super::argument_evaluation::Evaluation,
            source_value_count: usize,
            values: &mut Vec<ValueDeclaration>,
            next_value: &mut u64,
            next_block: &mut u64,
            next_edge: &mut u64,
            operations: &mut OperationBuffer,
            calls: &mut crate::emission::operation_emission::calls::CallEmissionContext<'_>,
            environment: &mut super::OperationEnvironment<'_>,
        ) -> Result<StructuralPlaceDeclaration, LoweringError> {
            let (start, end) = (site.range.start.clone(), site.range.end.clone());
            let (machine, _) = crate::shared::declarations::state_declaration(checked, site.state)?;
            let path = emit_prepared_path(&source.path, 0, values, next_value, operations);
            let retained_count = values.len();
            let mut positions = [None, None];
            for (ordinal, (value, role)) in [
                (
                    start.as_ref(),
                    CheckedScalarExpressionRole::SubsliceStart {
                        site: site.range.site,
                    },
                ),
                (
                    end.as_ref(),
                    CheckedScalarExpressionRole::SubsliceEnd {
                        site: site.range.site,
                    },
                ),
            ]
            .into_iter()
            .enumerate()
            {
                if let Some(value) = value {
                    let completed = evaluation.source_value(
                        identities,
                        checked,
                        machine.symbol,
                        site.state,
                        site.statement,
                        role,
                        value,
                        source_value_count,
                        values,
                        next_value,
                        next_block,
                        next_edge,
                        operations,
                        calls,
                        environment,
                    )?;
                    positions[ordinal] = Some(values.len());
                    values.push(completed);
                }
            }
            let [start, end] = positions.map(|position| {
                position
                    .map(|position| {
                        view_subslice::coordinate(LoweredDirectExpression::Parameter {
                            position,
                            scalar_type: values[position].scalar_type,
                        })
                    })
                    .transpose()
            });
            let (start, end) = (start?, end?);
            let end = exclusive_end(site.range.end_inclusive, end)?;
            let first_operation = operations.operations.len();
            let declaration = view_subslice::emit(
                source.family,
                terminal_psi::StructuralArgument {
                    place: source.place,
                    access: source.access,
                    path,
                },
                start.as_ref(),
                end.as_ref(),
                destination,
                result_type,
                values,
                next_value,
                operations,
            )?;
            evaluation
                .current_claims
                .publish_operations(&operations.operations[first_operation..])?;
            values.truncate(retained_count);
            Ok(declaration)
        }

        pub(crate) fn exclusive_end(
            end_inclusive: bool,
            end: Option<LoweredDirectExpression>,
        ) -> Result<Option<LoweredDirectExpression>, LoweringError> {
            let end = end.map(view_subslice::coordinate).transpose()?;
            if !end_inclusive {
                return Ok(end);
            }
            let end = end.ok_or(LoweringError::Unsupported(
                "inclusive range has no final endpoint",
            ))?;
            let scalar_type = terminal_scalar_type(checked_trees::types::PrimitiveType::U64)?;
            Ok(Some(LoweredDirectExpression::IntegerBinary {
                kind:
                    crate::emission::operation_emission::integer::LoweredIntegerBinaryKind::ExactAdd,
                scalar_type,
                left: Box::new(end),
                right: Box::new(LoweredDirectExpression::IntegerLiteral {
                    value: semantic_vocabulary::IntegerValue::Unsigned(1),
                    scalar_type,
                }),
                formation_decided: true,
            }))
        }
    }

    pub(crate) mod scalar_completion {
        use super::{CheckedTrees, LoweringError};

        pub(crate) mod control {

            use super::{CheckedTrees, LoweringError};
            use crate::shared::checked_facts::flow_facts;
            use checked_trees::CheckedScalarBranchDestination;

            pub(crate) fn returning_exits<'checked>(
                checked: &'checked CheckedTrees,
                arms: arena::HandleSpan<checked_trees::CheckedScalarGuardedExit>,
                fallback: Option<&CheckedScalarBranchDestination>,
            ) -> Result<&'checked [checked_trees::CheckedScalarGuardedExit], LoweringError>
            {
                flow_facts(checked)
                    .guarded_exits
                    .returning_exits(arms, fallback)
                    .map_err(LoweringError::Unsupported)
            }
        }
    }

    pub(crate) use bodies::{UnitBody, UnitPlans};
    pub(crate) use boundaries::{lower_boundary_result, retain_unit_boundary};
    pub(crate) use parameters::{lower_declared_service_reach, lower_fixed_boundary_service_reach};

    pub(crate) use call_closure::{
        checked_scalar_call_closure_with_structural_roots, unique_unit_boundary,
    };
    pub(crate) use call_closure::{
        checked_unit_boundary_identity, checked_unit_call_closure_including,
    };
    pub(crate) use catalog::lower_root_service_reach;
    pub(crate) use catalog::require_valid_service_row;
    pub(crate) use composed_control::live as unit_graph_live_states;
    pub(crate) use parameters::{
        literal_argument_places, lower_installation_machine_service_ceiling,
        lower_published_service_ceiling, lower_runtime_parameter_order, lower_structural_arguments,
        lower_unit_parameters,
    };
    pub(crate) use providers::checked_unit_provider_candidates;

    #[derive(Clone, Copy)]
    pub(crate) struct UnitClosureRequest<'a> {
        pub(crate) plans: UnitPlans<'a>,
        pub(crate) entry: symbols::SymbolHandle,
        pub(crate) unit_roots: &'a [symbols::SymbolHandle],
        pub(crate) program_roots: crate::TerminalProgramRoots<'a>,
        pub(crate) exact_toolchain_sources: &'a [(source::SourceId, [u8; 32])],
        pub(crate) scalar_entry: bool,
    }

    impl<'a> UnitClosureRequest<'a> {
        pub(crate) fn unit(
            checked: &'a CheckedTrees,
            entry: symbols::SymbolHandle,
            roots: &'a [symbols::SymbolHandle],
            program_roots: crate::TerminalProgramRoots<'a>,
            exact_toolchain_sources: &'a [(source::SourceId, [u8; 32])],
        ) -> Self {
            Self {
                plans: UnitPlans::published(&checked.facts.flow),
                entry,
                unit_roots: roots,
                program_roots,
                exact_toolchain_sources,
                scalar_entry: false,
            }
        }
    }

    pub(crate) fn lower_unit_effect_closure(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        entry: symbols::SymbolHandle,
        program_roots: crate::TerminalProgramRoots<'_>,
        exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    ) -> Result<crate::shared::producer_result::SourceMappedLowered, LoweringError> {
        lower_unit_closure(
            identities,
            checked,
            &UnitClosureRequest::unit(
                checked,
                entry,
                &[entry],
                program_roots,
                exact_toolchain_sources,
            ),
        )
    }

    pub(crate) fn lower_scalar_effect_closure(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        entry: symbols::SymbolHandle,
        program_roots: crate::TerminalProgramRoots<'_>,
        exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    ) -> Result<crate::shared::producer_result::SourceMappedLowered, LoweringError> {
        lower_unit_closure(
            identities,
            checked,
            &UnitClosureRequest {
                scalar_entry: true,
                ..UnitClosureRequest::unit(
                    checked,
                    entry,
                    &[],
                    program_roots,
                    exact_toolchain_sources,
                )
            },
        )
    }

    pub(crate) fn lower_unit_closure(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        request: &UnitClosureRequest<'_>,
    ) -> Result<crate::shared::producer_result::SourceMappedLowered, LoweringError> {
        let UnitClosureRequest {
            plans,
            entry,
            unit_roots,
            program_roots,
            exact_toolchain_sources,
            scalar_entry,
        } = *request;
        let ordinary_entry = unit_roots.first().copied();
        if !scalar_entry && ordinary_entry.is_some_and(|root| root != entry) {
            return unsupported("ordinary Unit closure must begin with its exact entry");
        }

        let scalar_applications = std::sync::Arc::new(
            scalar_applications::SourceScalarApplications::prepare(checked)?,
        );
        let mut retained_roots = program_roots.machines.to_vec();
        retained_roots.extend(scalar_applications.evaluation_roots());
        retained_roots.sort_unstable_by_key(|root| (root.arena_index(), root.generation()));
        retained_roots.dedup();
        let program_roots = crate::TerminalProgramRoots {
            machines: &retained_roots,
            boundaries: program_roots.boundaries,
        };

        let call_catalog = call_catalog::discover(
            checked,
            plans,
            entry,
            unit_roots,
            program_roots,
            scalar_entry,
        )?;
        let closure = call_catalog.operations;
        let provider_candidate_plans = call_catalog.providers;

        let mut boundaries = Vec::<(&CheckedBoundaryMachinePlan, BoundaryIdentity)>::new();
        for symbol in call_catalog.boundaries {
            retain_unit_boundary(checked, plans, &mut boundaries, symbol)?;
        }
        let admitted_bodies = admission::admit(checked, plans, &closure, &mut boundaries)?;
        boundaries.sort_by_key(|(_, identity)| identity.order_key());
        if let Some(pair) = boundaries.windows(2).find(|pair| pair[0].1 == pair[1].1) {
            return Err(LoweringError::InvalidUnitMachinePlan {
                machine: format!(
                    "{} and {} ({:?}: {})",
                    checked.symbols.display_path(pair[0].0.machine, "::"),
                    checked.symbols.display_path(pair[1].0.machine, "::"),
                    pair[0].1,
                    pair[0].1.render(checked)?
                ),
                reason: "boundary Unit closure contains duplicate canonical identities",
                gap: None,
            });
        }

        let (mut structural_types, type_ids, structural_referents, native_records) =
            catalog::lower_unit_structural_types(
                identities,
                checked,
                &scalar_applications,
                plans,
                &closure,
                &boundaries,
                &scalar_applications.type_roots().collect::<Vec<_>>(),
            )?;
        let calling_abi_signatures = calling_signatures::lower(identities, plans, &type_ids)?;
        let mut opaque_representations = Vec::new();
        for relation in plans.opaque_representations() {
            let Some((_, opaque)) = type_ids
                .iter()
                .find(|(source, _)| *source == relation.opaque)
            else {
                continue;
            };
            opaque_representations.push(terminal_psi::TerminalOpaqueRepresentation {
                opaque: *opaque,
                carrier: lookup_type_id(&type_ids, &relation.carrier)?,
                opaque_storage_identity: identities.intern(
                    &checked.type_identity_text(relation.opaque).ok_or(
                        LoweringError::Unsupported(
                            "opaque representation lost its portable storage identity",
                        ),
                    )?,
                ),
                carrier_storage_identity: identities.intern(
                    &checked.type_identity_text(relation.carrier).ok_or(
                        LoweringError::Unsupported(
                            "opaque representation lost its carrier storage identity",
                        ),
                    )?,
                ),
                application: relation.application.clone(),
            });
        }
        opaque_representations.sort_by_key(|row| row.opaque);
        for machine_symbol in &closure {
            for operation in UnitBody::find(plans, *machine_symbol)?.operations() {
                prepare_operation(identities, &mut structural_types, operation)?;
                if operation.call().is_some_and(|call| {
                    call.structural_arguments.iter().any(|argument| {
                        argument.byte_sequence_literal().is_some()
                            && argument.access == checked_trees::CheckedStructuralAccess::Owned
                    })
                }) || crate::expression_preparation::structural_value_requirements::collect_operation_requirements(
                    checked,
                    *machine_symbol,
                    operation,
                    &mut Vec::new(),
                )? {
                    crate::emission::structural_byte_sequence_store::literal_view_type(identities, &mut structural_types)?;
                }
                if let Some(value) = operation.stored_bytes() {
                    prepare_types(identities, value, &mut structural_types)?;
                }
                if let CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::ByteSequence {
                        result, value, ..
                    },
                ) = operation
                    && let Ok(declared) = lookup_type_id(&type_ids, &result.type_identity)
                    && structural_types.iter().any(|declaration| {
                        declaration.id == declared
                            && declaration.shape.is_bounded_owned_byte_sequence()
                    })
                {
                    prepare_types(identities, value, &mut structural_types)?;
                }
            }
        }
        let (structural_domains, domain_ids) = catalog::lower_unit_structural_domains(
            identities,
            checked,
            plans,
            &closure,
            &boundaries,
            &type_ids,
            &structural_types,
        )?;
        let (services, service_ids) = catalog::lower_unit_services(
            identities,
            checked,
            plans,
            &closure,
            &boundaries,
            &provider_candidate_plans,
        )?;
        let root_service_reach =
            lower_root_service_reach(identities, checked, entry, &service_ids)?;

        let boundary_signatures = boundaries.iter().map(|(plan, _)| *plan).collect::<Vec<_>>();
        let scalar_fields =
            catalog::scalar_field_types(identities, checked, plans, &type_ids, &structural_types)?;
        let qualifications =
            crate::expression_preparation::qualifications::PreparedScalarQualifications::prepare(
                identities,
                checked,
                &scalar_applications,
                &[],
                &call_catalog.source_machines,
                &boundary_signatures,
                &scalar_fields,
            )?;
        let mut next_place = 1_u64;
        let (boundary_machines, lowered_boundary_parameters) = boundaries::lower_declarations(
            identities,
            checked,
            &scalar_applications,
            exact_toolchain_sources,
            &boundaries,
            &qualifications,
            &type_ids,
            &structural_types,
            &domain_ids,
            &service_ids,
            &mut next_place,
        )?;
        let mut next_value = 1_u64;
        let machine_signatures = signatures::lower(
            identities,
            checked,
            &scalar_applications,
            &admitted_bodies,
            &qualifications,
            &type_ids,
            &domain_ids,
            &structural_types,
            &mut next_place,
            &mut next_value,
        )?;
        let machine_ids = closure
            .iter()
            .enumerate()
            .map(|(index, symbol)| Ok((*symbol, machine_id(dense_identity(index)?))))
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let mut access_view_inputs = checked
            .facts
            .access_view_inputs
            .iter()
            .filter(|input| {
                machine_ids
                    .iter()
                    .any(|(symbol, _)| *symbol == input.machine)
                    && flow_facts(checked)
                        .terminal_unit_effects
                        .callable_entry_for_machine(input.machine)
                        .is_some_and(|entry| entry.state == input.state)
            })
            .map(|input| {
                lower_access_view_input(
                    identities,
                    checked,
                    exact_toolchain_sources,
                    input,
                    lookup_machine_id(&machine_ids, input.machine)?,
                )
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        access_view_inputs.sort();
        let scalar_requirement_counts = machine_signatures
            .iter()
            .map(|signature| (signature.source, signature.requires.len()))
            .collect::<Vec<_>>();

        let mut scalar_block_invariants = Vec::new();
        let mut next_operation = 1_u64;
        let mut next_edge = 1_u64;
        let mut next_block = 1_u64;
        let mut next_call_obligation = TERMINAL_UNIT_CALL_OBLIGATION_BASE;
        let mut machines = Vec::with_capacity(closure.len());
        let mut occurrences = ClosureOccurrences::default();
        let dynamic_dispatch = std::sync::Arc::new(DynamicDispatchIndex::new(checked));
        let catalog = ordinary_machine::ClosureCatalog {
            scalar_applications: &scalar_applications,
            type_ids: &type_ids,
            domain_ids: &domain_ids,
            domain_declarations: &structural_domains,
            service_ids: &service_ids,
            boundary_parameters: &lowered_boundary_parameters,
            machine_ids: &machine_ids,
            signatures: &machine_signatures,
            scalar_requirement_counts: &scalar_requirement_counts,
            plans,
            closure: &closure,
            provider_candidate_plans: &provider_candidate_plans,
            prepared_scalar_qualifications: &qualifications,
            dynamic_dispatch: &dynamic_dispatch,
        };
        let mut scalar_qualification_coercions = Vec::new();
        for (signature, admitted) in machine_signatures.iter().zip(admitted_bodies) {
            let body = admitted.source();
            let plan = body.entry()?;
            let terminal_machine = lookup_machine_id(&machine_ids, plan.machine)?;
            let attributed = |error| match error {
                LoweringError::Unsupported(reason) => LoweringError::InvalidUnitMachinePlan {
                    machine: checked.symbols.display_path(plan.machine, "::"),
                    reason,
                    gap: None,
                },
                LoweringError::InvalidArgumentProjection(mut failure) => {
                    failure
                        .machine
                        .get_or_insert_with(|| checked.symbols.display_path(plan.machine, "::"));
                    LoweringError::InvalidArgumentProjection(failure)
                }
                error => error,
            };
            let parameters = &signature.parameters;
            let scalar_parameters = signature.scalar_parameters.clone();
            if let admission::AdmittedBody::Composed {
                source: source_plan,
                body: admitted,
            } = admitted
            {
                let (machine, composed, mut invariants, mut coercions) = {
                    composed_control::callable::emit(
                        identities,
                        checked,
                        source_plan,
                        admitted,
                        parameters.clone(),
                        scalar_parameters,
                        composed_control::callable::SharedCatalog {
                            scalar_applications: &scalar_applications,
                            structural_types: &structural_types,
                            type_ids: &type_ids,
                            domain_ids: &domain_ids,
                            domain_declarations: &structural_domains,
                            service_ids: &service_ids,
                            boundary_parameters: &lowered_boundary_parameters,
                            machine_ids: &machine_ids,
                            signatures: &machine_signatures,
                            scalar_requirement_counts: &scalar_requirement_counts,
                            closure: &closure,
                            prepared_scalar_qualifications: &qualifications,
                            dynamic_dispatch: &dynamic_dispatch,
                        },
                        composed_control::callable::EmissionCounters {
                            place: &mut next_place,
                            value: &mut next_value,
                            block: &mut next_block,
                            operation: &mut next_operation,
                            edge: &mut next_edge,
                            call_obligation: &mut next_call_obligation,
                        },
                    )
                }
                .map_err(attributed)?;
                occurrences
                    .quotient_adaptations
                    .extend(composed.quotient_adaptations);
                occurrences.quotient_calls.extend(composed.quotient_calls);
                occurrences.source_returns.extend(composed.source_returns);
                machines.push(machine);
                occurrences.retain(composed.source_calls, composed.selected_comparisons);
                occurrences
                    .source_checked_obligations
                    .extend(composed.source_checked_obligations);
                occurrences.state_blocks.extend(composed.state_blocks);
                scalar_block_invariants.append(&mut invariants);
                scalar_qualification_coercions.append(&mut coercions);
                continue;
            }
            let plan = body.ordinary()?;
            let emitted = {
                ordinary_machine::emit(
                    identities,
                    checked,
                    signature,
                    plan,
                    terminal_machine,
                    &mut structural_types,
                    catalog,
                    composed_control::callable::EmissionCounters {
                        place: &mut next_place,
                        value: &mut next_value,
                        block: &mut next_block,
                        operation: &mut next_operation,
                        edge: &mut next_edge,
                        call_obligation: &mut next_call_obligation,
                    },
                )
            }
            .map_err(attributed)?;
            scalar_block_invariants.extend(emitted.scalar_block_invariants);
            scalar_qualification_coercions.extend(emitted.qualification_coercions);
            occurrences
                .quotient_adaptations
                .extend(emitted.quotient_adaptations);
            occurrences.quotient_calls.extend(emitted.quotient_calls);
            machines.push(emitted.machine);
            occurrences.source_returns.extend(emitted.source_returns);
            occurrences.retain(emitted.source_calls, emitted.selected_comparisons);
            occurrences
                .source_checked_obligations
                .extend(emitted.source_checked_obligations);
        }
        super::unit_cleanup::bind_nominal_cleanup_subjects(
            checked,
            &mut machines,
            &type_ids,
            &machine_ids,
        )?;
        let provider_candidates = providers::conformances(
            identities,
            checked,
            exact_toolchain_sources,
            &provider_candidate_plans,
            &lowered_boundary_parameters,
            &boundary_machines,
            &machine_ids,
            &machines,
        )?;
        let provider_instance_requirements = providers::instance_requirements(
            identities,
            checked,
            exact_toolchain_sources,
            &lowered_boundary_parameters,
        )?;

        let folded_constants = folded_constants::lower(
            checked,
            identities,
            exact_toolchain_sources,
            &provider_candidate_plans,
            &provider_candidates,
            &machine_ids,
        )?;

        let target_machine_families =
            target_families::lower(checked, &machine_ids, &lowered_boundary_parameters)?;
        let target_layout_applications =
            target_layouts::lower(checked, plans, &structural_types, &type_ids, identities)?;
        let (native_layouts, source_records) =
            native_layouts::lower(checked, &native_records, identities)?;
        let mut scalar_qualifications = qualifications.catalog().clone();
        crate::expression_preparation::qualifications::retain_returns(
            identities,
            checked,
            &machine_ids,
            &machines,
            &occurrences.source_returns,
            &mut scalar_qualifications,
        )?;
        scalar_qualifications
            .coercions
            .append(&mut scalar_qualification_coercions);
        scalar_qualifications
            .float_entry_ranges
            .sort_by_key(|range| (range.machine, range.parameter));
        scalar_qualifications
            .integer_entry_ranges
            .sort_by_key(|range| (range.machine, range.parameter));
        scalar_qualifications
            .coercions
            .sort_by_key(|coercion| (coercion.machine, coercion.edge, coercion.argument_ordinal));
        scalar_block_invariants.sort_by_key(|invariant| (invariant.machine, invariant.header));

        let mut retained_roots = Vec::with_capacity(program_roots.machines.len());
        for root in program_roots.machines {
            if let terminal_psi::TargetMachineCallable::Machine(machine) =
                target_families::callable(*root, &machine_ids, &lowered_boundary_parameters)?
            {
                retained_roots.push(machine);
            }
        }
        let quotient_catalog = quotients::retain(
            checked,
            identities,
            exact_toolchain_sources,
            &structural_types,
            &occurrences.quotient_adaptations,
            &occurrences.quotient_calls,
            &machine_ids,
            &type_ids,
        )?;
        for representation in &quotient_catalog.representations {
            let declaration = structural_types
                .iter_mut()
                .find(|row| row.id == representation.quotient)
                .ok_or(LoweringError::Unsupported("quotient declaration is absent"))?;
            declaration.quotient_carrier = Some(representation.carrier);
        }
        let mathematical_carriers =
            crate::scalar_lowering::contracts::mathematical_carriers(checked, identities)?;
        let indexed_theorems =
            indexed_theorems(checked, identities, exact_toolchain_sources, &machine_ids)?;
        let identity_formations =
            crate::scalar_lowering::contracts::identity_formations(checked, identities)?;
        let (scalar_applications, source_proof_declarations) = scalar_applications.publish(
            &type_ids,
            &machine_ids,
            &lowered_boundary_parameters,
            scalar_applications::EmittedScalars {
                checked,
                identities,
                sources: exact_toolchain_sources,
                calls: &occurrences.source_calls,
                machines: &machines,
            },
        )?;
        let lowered = LoweredPsi {
            source_proof_declarations,
            source_records,
            source_boundary_declarations: Vec::new(),
            source_machine_declarations: Vec::new(),
            build_provider_requirements: Vec::new(),
            semantic_module: TerminalModule {
                scalar_applications,
                folded_constants,
                indexed_theorems,
                mathematical_carriers,
                identity_formations,
                callback_applications: Vec::new(),
                installed_interrupt_registrations: Vec::new(),
                boundary_application_candidates: Vec::new(),
                boundary_application_demands: Vec::new(),
                scalar_qualifications,
                scalar_block_invariants,
                operation_crash_contracts: Vec::new(),
                vocabulary_marker: VocabularyMarker::CURRENT,
                identities: identities.snapshot(),
                entry: entry
                    .is_valid()
                    .then(|| {
                        target_families::callable(entry, &machine_ids, &lowered_boundary_parameters)
                    })
                    .transpose()?,
                structural_types,
                structural_referents,
                structural_domains,
                services,
                root_service_reach,
                access_view_inputs,
                access_view_plans: Vec::new(),
                access_lifecycle_plans: Vec::new(),
                reborrow_restored_call_uses: Vec::new(),
                boundary_machines,
                calling_signatures: Vec::new(),
                calling_abi_signatures,
                calling_policy_requests: Vec::new(),
                opaque_representations,
                unoptimized_behavior: Vec::new(),
                external_providers: Vec::new(),
                provider_candidates,
                provider_instance_requirements,
                dynamic_dispatch: Default::default(),
                callables: Default::default(),
                quotients: quotient_catalog,
                float_meaning_projections: Vec::new(),
                float_meaning_equalities: Vec::new(),
                proposition_declarations: Vec::new(),
                proposition_applications: Vec::new(),
                evidence_terms: Vec::new(),
                evidence_contract_lanes: Vec::new(),
                proof_output_calls: Vec::new(),
                proof_recursive_components: Vec::new(),
                closed_conformance_applications: Vec::new(),
                suspension_call_plan_count: 0,
                suspension_call_sites: Vec::new(),
                suspension_call_plans: Vec::new(),
                machines,
                target_machine_families,
                target_layout_applications,
                native_layouts,
                task_activation_demands: Default::default(),
            },
            debug_map: None,
            retained_roots,
            source_call_occurrences: occurrences.source_calls,
            selected_comparison_occurrences: occurrences.selected_comparisons,
            state_blocks: occurrences.state_blocks,
        };
        crate::shared::producer_result::SourceMappedLowered::new(
            lowered,
            machine_ids,
            lowered_boundary_parameters
                .iter()
                .map(|boundary| (boundary.source, boundary.id))
                .collect(),
        )
    }

    #[derive(Default)]
    pub(crate) struct ClosureOccurrences {
        source_returns: Vec<crate::expression_preparation::qualifications::SourceScalarReturn>,
        quotient_adaptations:
            Vec<crate::emission::operation_emission::buffer::SourceQuotientAdaptation>,
        quotient_calls: Vec<crate::emission::operation_emission::buffer::SourceQuotientCall>,
        source_calls: Vec<lowered_psi::LoweredSourceCallOccurrence>,
        selected_comparisons: Vec<lowered_psi::LoweredSelectedComparisonOccurrence>,
        source_checked_obligations: Vec<semantic_vocabulary::ObligationId>,
        state_blocks: Vec<lowered_psi::LoweredStateBlock>,
    }

    impl ClosureOccurrences {
        fn retain(
            &mut self,
            source_calls: Vec<lowered_psi::LoweredSourceCallOccurrence>,
            selected_comparisons: Vec<lowered_psi::LoweredSelectedComparisonOccurrence>,
        ) {
            self.source_calls.extend(source_calls);
            self.selected_comparisons.extend(selected_comparisons);
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct CheckedUnitProviderCandidate {
        provider: symbols::SymbolHandle,
        boundary: symbols::SymbolHandle,
        candidate: symbols::SymbolHandle,
        receiving: arena::Handle<checked_trees::CheckedProviderReceivingApplication>,
        requirement_identity: typed_trees::type_identity::CallableIdentityId,
        provider_identity: String,
        candidate_identity: typed_trees::type_identity::CallableIdentityId,
    }
}
