/*
Semantic judgments for access views, lifecycle transitions, RAM origins, and structural access
formation in `TerminalModule`.
*/

pub(crate) mod access_lifecycles {

    use terminal_psi::{
        BoundaryDeclarationRole, BoundaryMachineResult, StructuralAccess, StructuralTypeShape,
        TerminalAccessLifecycleExecution, TerminalAccessLifecycleOperation, TerminalModule,
    };

    pub(crate) mod result_contract {

        use semantic_vocabulary::{IntegerSign, StructuralDomainId, StructuralTypeId};
        use terminal_psi::{
            BoundaryMachineResult, BoundaryStructuralResultDeclaration, StructuralCaseDeclaration,
            StructuralFieldDeclaration, StructuralMultiplicity, StructuralPathQualification,
            StructuralPathSegment, StructuralTypeShape, TerminalAccessLifecycleOperation,
            TerminalAccessLifecyclePlan, TerminalModule,
        };

        pub enum AccessResultContract<'module> {
            View {
                result: &'module BoundaryStructuralResultDeclaration,
                ready: &'module StructuralCaseDeclaration,
                rejected: &'module StructuralCaseDeclaration,
                failures: &'module [StructuralCaseDeclaration; 3],
                carrier: StructuralTypeId,
                access: terminal_psi::StructuralAccess,
            },
            Initialize {
                result: &'module BoundaryStructuralResultDeclaration,
                ready: &'module StructuralCaseDeclaration,
                rejected: &'module StructuralCaseDeclaration,
                returned: &'module [StructuralFieldDeclaration; 2],
                failures: &'module [StructuralCaseDeclaration; 3],
            },
            MoveOut {
                result: &'module BoundaryStructuralResultDeclaration,
                moved: &'module StructuralCaseDeclaration,
            },
            Relocate {
                result: &'module BoundaryStructuralResultDeclaration,
                ready: &'module StructuralCaseDeclaration,
                rejected: &'module StructuralCaseDeclaration,
                moved: &'module [StructuralFieldDeclaration; 2],
                returned: &'module [StructuralFieldDeclaration; 2],
                failures: &'module [StructuralCaseDeclaration; 3],
            },
        }

        pub fn access_view_reference_slot(
            types: &[terminal_psi::StructuralTypeDeclaration],
            result: StructuralTypeId,
        ) -> Option<(Vec<StructuralPathSegment>, StructuralTypeId)> {
            let shape = &terminal_psi::find_structural_type(types, result)?.shape;
            let StructuralTypeShape::Sum { cases } = shape else {
                return None;
            };
            let [ready, _] = cases.as_slice() else {
                return None;
            };
            let [field] = ready.fields.as_slice() else {
                return None;
            };
            let StructuralTypeShape::Reference {
                referent,
                access: terminal_psi::reading_borrow_pair_pattern!(),
            } = terminal_psi::find_structural_type(types, field.field_type.as_structural()?)?.shape
            else {
                return None;
            };
            Some((
                terminal_psi::structural_case_initializer_path(shape, ready.id, 0, field.id)?,
                referent,
            ))
        }

        pub fn access_result_contract<'module>(
            module: &'module TerminalModule,
            plan: &TerminalAccessLifecyclePlan,
        ) -> Result<AccessResultContract<'module>, &'static str> {
            let boundary = module
                .boundary_machine(plan.boundary)
                .ok_or("placement result lost its boundary declaration")?;
            let BoundaryMachineResult::Structural(result) = &boundary.result else {
                return Err("access lifecycle must return its owned structural outcome");
            };
            if result.multiplicity != StructuralMultiplicity::Affine
                || !result.qualifications.is_empty()
            {
                return Err("placement outcome changed its core ownership or root qualifications");
            }
            let cases = sum_cases(module, result.structural_type)?;
            match plan.operation {
                TerminalAccessLifecycleOperation::View => {
                    let [ready, rejected] = cases else {
                        return Err(
                            "borrowed placement requires exactly Ready and Rejected outcomes",
                        );
                    };
                    let [view] = ready.fields.as_slice() else {
                        return Err("borrowed placement Ready must carry one reference");
                    };
                    let Some(StructuralTypeShape::Reference { referent, access }) = module
                        .structural_type(structural_type(view)?)
                        .map(|declaration| &declaration.shape)
                    else {
                        return Err(
                            "borrowed placement Ready does not retain its reference carrier",
                        );
                    };
                    let storage = boundary
                        .structural_parameters
                        .first()
                        .ok_or("borrowed placement lost its exact storage parameter")?;
                    if !matches!(access, terminal_psi::reading_borrow_pair_pattern!())
                        || *access != storage.access
                        || !module.access_view_plans.iter().any(|view| {
                            view.carrier == *referent
                                && view.schema == plan.schema
                                && view.policy_identity == plan.policy_identity
                                && view.placement == plan.placement
                        })
                    {
                        return Err(
                            "borrowed placement Ready changed its exact placed interpretation",
                        );
                    }
                    let [returned, reason] = rejected.fields.as_slice() else {
                        return Err(
                            "borrowed placement Rejected must retain its empty return and reason",
                        );
                    };
                    let returned = module
                        .structural_type(structural_type(returned)?)
                        .ok_or("borrowed placement return record is absent")?;
                    if returned.owns_linear_claim
                        || !matches!(&returned.shape, StructuralTypeShape::Record { fields, .. } if fields.is_empty())
                        || !result.projected_qualifications.is_empty()
                    {
                        return Err(
                            "borrowed placement must not manufacture returned storage or qualifications",
                        );
                    }
                    Ok(AccessResultContract::View {
                        result,
                        ready,
                        rejected,
                        failures: failure_cases(module, reason)?,
                        carrier: *referent,
                        access: *access,
                    })
                }
                TerminalAccessLifecycleOperation::Initialize => {
                    let (vacant, resident_domain) = plan.storage.owned_content()?;
                    let [ready, rejected] = cases else {
                        return Err(
                            "placement initialization requires exactly Ready and Rejected outcomes",
                        );
                    };
                    let [resident] = ready.fields.as_slice() else {
                        return Err("placement Ready must carry its single resident extent");
                    };
                    let [returned_field, reason] = rejected.fields.as_slice() else {
                        return Err("placement Rejected must carry its returned inputs and reason");
                    };
                    require_type(resident, plan.storage.owner_carrier())?;
                    let returned_type = structural_type(returned_field)?;
                    let Some(StructuralTypeShape::Record { fields, .. }) = module
                        .structural_type(returned_type)
                        .map(|declaration| &declaration.shape)
                    else {
                        return Err("placement rejection lost its returned-input record");
                    };
                    let returned: &[StructuralFieldDeclaration; 2] =
                        fields.as_slice().try_into().map_err(
                            |_| "placement rejection must return exactly storage and value",
                        )?;
                    require_type(&returned[0], plan.storage.carrier)?;
                    require_type(&returned[1], plan.schema)?;
                    let failures = failure_cases(module, reason)?;
                    let owner_path = [
                        StructuralPathSegment::Case(ready.id),
                        StructuralPathSegment::Field(resident.identity),
                    ];
                    match plan.storage.owner {
                        terminal_psi::TerminalAccessOwner::Extent => storage_qualifications(
                            result,
                            &owner_path,
                            plan.storage.granted,
                            resident_domain,
                        )?,
                        terminal_psi::TerminalAccessOwner::Run {
                            carrier,
                            storage,
                            singleton,
                            ..
                        } => {
                            let singleton = singleton.ok_or(
                                "typed run initialization lost its singleton qualification",
                            )?;
                            let (whole, descendants) = StructuralPathQualification::select(
                                &result.qualifications,
                                &result.projected_qualifications,
                                &owner_path,
                            );
                            let Some(StructuralTypeShape::Record { fields, .. }) = module
                                .structural_type(carrier)
                                .map(|declaration| &declaration.shape)
                            else {
                                return Err("typed run initialization lost its owner record");
                            };
                            let backing = fields
                                .iter()
                                .find(|field| field.id == storage)
                                .ok_or("typed run initialization lost its backing field")?;
                            let (backing_domains, nested) = StructuralPathQualification::select(
                                &whole,
                                &descendants,
                                &[StructuralPathSegment::Field(backing.identity)],
                            );
                            if whole.len() != 2
                                || !whole.contains(&resident_domain)
                                || !whole.contains(&singleton)
                                || backing_domains.as_slice() != [plan.storage.granted]
                                || !nested.is_empty()
                                || descendants.len() != 1
                            {
                                return Err(
                                    "typed run initialization changed its live owner or nested backing qualifications",
                                );
                            }
                        }
                    }
                    storage_qualifications(
                        result,
                        &[
                            StructuralPathSegment::Case(rejected.id),
                            StructuralPathSegment::Field(returned_field.identity),
                            StructuralPathSegment::Field(returned[0].identity),
                        ],
                        plan.storage.granted,
                        vacant,
                    )?;
                    let value = boundary
                        .structural_parameters
                        .get(1)
                        .ok_or("placement rejection lost its original value contract")?;
                    let (whole, descendants) = StructuralPathQualification::select(
                        &result.qualifications,
                        &result.projected_qualifications,
                        &[
                            StructuralPathSegment::Case(rejected.id),
                            StructuralPathSegment::Field(returned_field.identity),
                            StructuralPathSegment::Field(returned[1].identity),
                        ],
                    );
                    if whole != value.qualifications
                        || descendants != value.projected_qualifications
                    {
                        return Err(
                            "placement rejection changed the original value qualifications",
                        );
                    }
                    Ok(AccessResultContract::Initialize {
                        result,
                        ready,
                        rejected,
                        returned,
                        failures,
                    })
                }
                TerminalAccessLifecycleOperation::Relocate => {
                    let (vacant, live) = plan.storage.owned_content()?;
                    let [ready, rejected] = cases else {
                        return Err("run relocation requires Ready and Rejected outcomes");
                    };
                    let [moved_field] = ready.fields.as_slice() else {
                        return Err(
                            "run relocation Ready must carry its moved run and vacated backing",
                        );
                    };
                    let [returned_field, reason] = rejected.fields.as_slice() else {
                        return Err("run relocation Rejected must retain both inputs and a reason");
                    };
                    let moved = record_pair(module, moved_field)?;
                    let returned = record_pair(module, returned_field)?;
                    require_type(&moved[0], plan.storage.owner_carrier())?;
                    require_type(&moved[1], plan.storage.carrier)?;
                    require_type(&returned[0], plan.storage.owner_carrier())?;
                    require_type(&returned[1], plan.storage.carrier)?;
                    let owner_path = [
                        StructuralPathSegment::Case(ready.id),
                        StructuralPathSegment::Field(moved_field.identity),
                        StructuralPathSegment::Field(moved[0].identity),
                    ];
                    let (whole, descendants) = StructuralPathQualification::select(
                        &result.qualifications,
                        &result.projected_qualifications,
                        &owner_path,
                    );
                    let backing_path = plan
                        .storage
                        .owner_storage_path(&module.structural_types)
                        .ok_or("run relocation lost its backing projection")?;
                    let (backing, nested) =
                        StructuralPathQualification::select(&whole, &descendants, &backing_path);
                    if !matches!(
                        plan.storage.owner,
                        terminal_psi::TerminalAccessOwner::Run { .. }
                    ) || whole.as_slice() != [live]
                        || backing.as_slice() != [plan.storage.granted]
                        || !nested.is_empty()
                        || descendants.len() != 1
                    {
                        return Err(
                            "run relocation changed its live owner or backing qualifications",
                        );
                    }
                    storage_qualifications(
                        result,
                        &[
                            StructuralPathSegment::Case(ready.id),
                            StructuralPathSegment::Field(moved_field.identity),
                            StructuralPathSegment::Field(moved[1].identity),
                        ],
                        plan.storage.granted,
                        vacant,
                    )?;
                    for (position, field) in returned.iter().enumerate() {
                        let parameter = boundary
                            .structural_parameters
                            .get(position)
                            .ok_or("run relocation lost an original owner")?;
                        let (whole, descendants) = StructuralPathQualification::select(
                            &result.qualifications,
                            &result.projected_qualifications,
                            &[
                                StructuralPathSegment::Case(rejected.id),
                                StructuralPathSegment::Field(returned_field.identity),
                                StructuralPathSegment::Field(field.identity),
                            ],
                        );
                        if whole != parameter.qualifications
                            || descendants != parameter.projected_qualifications
                        {
                            return Err(
                                "run relocation rejection changed an original owner's qualifications",
                            );
                        }
                    }
                    Ok(AccessResultContract::Relocate {
                        result,
                        ready,
                        rejected,
                        moved,
                        returned,
                        failures: failure_cases(module, reason)?,
                    })
                }
                TerminalAccessLifecycleOperation::MoveOut => {
                    let (vacant, _) = plan.storage.owned_content()?;
                    let [moved] = cases else {
                        return Err("placement extraction requires exactly its Moved outcome");
                    };
                    let [storage, value] = moved.fields.as_slice() else {
                        return Err("placement extraction must return exactly storage and value");
                    };
                    require_type(storage, plan.storage.carrier)?;
                    require_type(value, plan.schema)?;
                    storage_qualifications(
                        result,
                        &[
                            StructuralPathSegment::Case(moved.id),
                            StructuralPathSegment::Field(storage.identity),
                        ],
                        plan.storage.granted,
                        vacant,
                    )?;
                    Ok(AccessResultContract::MoveOut { result, moved })
                }
            }
        }

        fn record_pair<'module>(
            module: &'module TerminalModule,
            field: &StructuralFieldDeclaration,
        ) -> Result<&'module [StructuralFieldDeclaration; 2], &'static str> {
            let Some(StructuralTypeShape::Record { fields, .. }) = module
                .structural_type(structural_type(field)?)
                .map(|declaration| &declaration.shape)
            else {
                return Err("relocation result must retain its two-owner record");
            };
            fields
                .as_slice()
                .try_into()
                .map_err(|_| "relocation result must retain exactly two owners")
        }

        fn failure_cases<'module>(
            module: &'module TerminalModule,
            reason: &StructuralFieldDeclaration,
        ) -> Result<&'module [StructuralCaseDeclaration; 3], &'static str> {
            let failures: &[StructuralCaseDeclaration; 3] =
                sum_cases(module, structural_type(reason)?)?
                    .try_into()
                    .map_err(|_| "placement rejection lost its three core failure cases")?;
            let [range, alignment, incompatible] = failures;
            for case in [range, alignment] {
                let [first, second] = case.fields.as_slice() else {
                    return Err("placement failure must carry its two unsigned coordinates");
                };
                for field in [first, second] {
                    if field.relevance.is_erased()
                        || field
                            .field_type
                            .scalar_type()
                            .and_then(|scalar| scalar.as_integer())
                            .is_none_or(|integer| {
                                integer.is_address()
                                    || integer.sign() != IntegerSign::Unsigned
                                    || integer.bits() != Some(64)
                            })
                    {
                        return Err("placement failure coordinate is not a represented u64");
                    }
                }
            }
            if !incompatible.fields.is_empty() {
                return Err("incompatible placement supply must have an empty payload");
            }
            Ok(failures)
        }

        fn sum_cases(
            module: &TerminalModule,
            structural_type: StructuralTypeId,
        ) -> Result<&[StructuralCaseDeclaration], &'static str> {
            match module.structural_type(structural_type) {
                Some(declaration) if !declaration.owns_linear_claim => match &declaration.shape {
                    StructuralTypeShape::Sum { cases } => Ok(cases),
                    _ => Err("placement result requires its exact core sum declaration"),
                },
                _ => Err("placement result sum declaration is absent or has changed ownership"),
            }
        }

        fn structural_type(
            field: &StructuralFieldDeclaration,
        ) -> Result<StructuralTypeId, &'static str> {
            if field.relevance.is_erased() {
                return Err("placement result cannot erase an owned payload");
            }
            field
                .field_type
                .as_structural()
                .ok_or("placement result lost its structural payload")
        }

        fn require_type(
            field: &StructuralFieldDeclaration,
            expected: StructuralTypeId,
        ) -> Result<(), &'static str> {
            if structural_type(field)? != expected {
                return Err("placement result payload differs from its exact storage or schema");
            }
            Ok(())
        }

        fn storage_qualifications(
            result: &BoundaryStructuralResultDeclaration,
            path: &[StructuralPathSegment],
            granted: StructuralDomainId,
            content: StructuralDomainId,
        ) -> Result<(), &'static str> {
            let (whole, descendants) = StructuralPathQualification::select(
                &result.qualifications,
                &result.projected_qualifications,
                path,
            );
            if whole.len() != 2
                || !whole.contains(&granted)
                || !whole.contains(&content)
                || !descendants.is_empty()
            {
                return Err(
                    "placement result storage lost its exact grant and content qualifications",
                );
            }
            Ok(())
        }
    }
    pub use result_contract::{
        AccessResultContract, access_result_contract, access_view_reference_slot,
    };
    use terminal_psi::StructuralFieldType;
    use terminal_psi::StructuralMultiplicity;
    use terminal_psi::find_structural_type;

    pub fn validate_access_lifecycle_plans(module: &TerminalModule) -> Result<(), &'static str> {
        for boundary in &module.boundary_machines {
            if let Some(operation) = boundary
                .declaration
                .as_ref()
                .and_then(|declaration| declaration.execution.placement_operation())
                && !module.access_lifecycle_plans.iter().any(|plan| {
                    plan.boundary == boundary.id
                        && plan.operation == operation
                        && matches!(
                            plan.execution,
                            TerminalAccessLifecycleExecution::CompilerPrimitive { .. }
                        )
                })
            {
                return Err("compiler placement primitive lost its mandatory lifecycle obligation");
            }
        }
        let mut previous = None;
        for (position, plan) in module.access_lifecycle_plans.iter().enumerate() {
            let key = (
                plan.boundary,
                plan.target.as_ref().map(|target| target.target.as_str()),
            );
            if previous.is_some_and(|previous| previous >= key) {
                return Err("access lifecycle boundary/target pairs are not unique and ordered");
            }
            previous = Some(key);
            if let Some(target) = &plan.target {
                layout_plans::LayoutTargetFacts {
                    profile: target.target.clone(),
                    pointer_size: target.pointer_size,
                    pointer_alignment: target.pointer_alignment,
                    case_tag_size: target.case_tag_size,
                    case_tag_alignment: target.case_tag_alignment,
                    little_endian: target.little_endian,
                }
                .validate()?;
            }
            let boundary = module
                .boundary_machine(plan.boundary)
                .ok_or("access lifecycle boundary is absent")?;
            if boundary.role != BoundaryDeclarationRole::Callable {
                return Err("access lifecycle names non-callable contract evidence");
            }
            let schema = find_structural_type(&module.structural_types, plan.schema)
                .ok_or("access lifecycle schema is absent")?;
            if !matches!(schema.shape, StructuralTypeShape::Record { .. }) {
                return Err("access lifecycle schema is not a record");
            }
            for identity in [
                plan.policy_identity,
                plan.policy_plan_machine_identity,
                plan.schema_identity,
            ] {
                if module.identities.text(identity).is_none_or(str::is_empty) {
                    return Err("access lifecycle declaration identity is absent");
                }
            }
            if !boundary.scalar_parameters.is_empty() {
                return Err("access lifecycle requires structural inputs");
            }
            let profile_type = match &plan.execution {
                TerminalAccessLifecycleExecution::SelectedProvider => {
                    if boundary.kind != semantic_vocabulary::BoundaryRequirementKind::Trait {
                        return Err("provider lifecycle does not name a trait requirement");
                    }
                    None
                }
                TerminalAccessLifecycleExecution::CompilerPrimitive {
                    profile_type,
                    profile_requirements,
                } => {
                    if boundary.kind != semantic_vocabulary::BoundaryRequirementKind::Machine {
                        return Err("compiler placement primitive does not name an exact machine");
                    }
                    if boundary
                        .declaration
                        .as_ref()
                        .and_then(|declaration| declaration.execution.placement_operation())
                        != Some(plan.operation)
                    {
                        return Err("placement primitive obligation differs from its declaration");
                    }
                    match profile_type {
                        Some(profile_type) => {
                            let expected = plan.operation.profile_operands().len();
                            if profile_requirements.len() != expected {
                                return Err(
                                    "placement operation lost its exact supply-role roster",
                                );
                            }
                            for requirement in profile_requirements {
                                let supply = module.boundary_machine(requirement.boundary).ok_or(
                                    "placement initialization lost its profile supply requirement",
                                )?;
                                let [storage] = supply.structural_parameters.as_slice() else {
                                    return Err("profile supply lost its borrowed extent input");
                                };
                                if supply.role != BoundaryDeclarationRole::Callable
                                    || supply.kind
                                        != semantic_vocabulary::BoundaryRequirementKind::Trait
                                    || !requirement.owner.same_identity_as(
                                        &module.identities,
                                        &supply.owner,
                                        &module.identities,
                                    )
                                    || module
                                        .identities
                                        .text(requirement.callable)
                                        .filter(|identity| !identity.is_empty())
                                        .is_none_or(|identity| {
                                            module.identities.text(supply.identity)
                                                != Some(identity)
                                        })
                                    || !supply.scalar_parameters.is_empty()
                                    || storage.structural_type != plan.storage.carrier
                                    || storage.access != StructuralAccess::SharedBorrow
                                    || !storage.qualifications.contains(&plan.storage.granted)
                                    || !matches!(&supply.result, BoundaryMachineResult::Structural(result)
                                    if result.structural_type == *profile_type)
                                {
                                    return Err(
                                        "profile supply differs from its exact core declaration or extent/result contract",
                                    );
                                }
                            }
                            if find_structural_type(&module.structural_types, *profile_type)
                                .is_none_or(|declaration| {
                                    !matches!(declaration.shape, StructuralTypeShape::Record { .. })
                                })
                            {
                                return Err(
                                    "placement primitive profile is not a retained record type",
                                );
                            }
                        }
                        _ => {
                            return Err("placement primitive operation and profile input disagree");
                        }
                    }
                    *profile_type
                }
            };
            match plan.operation {
                TerminalAccessLifecycleOperation::Relocate => {
                    let [run, destination, source_profile, destination_profile] =
                        boundary.structural_parameters.as_slice()
                    else {
                        return Err("run relocation lost its owners or their profile inputs");
                    };
                    let (vacant, _) = plan.storage.owned_content()?;
                    if !matches!(
                        plan.storage.owner,
                        terminal_psi::TerminalAccessOwner::Run { .. }
                    ) || run.access != StructuralAccess::Owned
                        || destination.access != StructuralAccess::Owned
                        || destination.multiplicity != StructuralMultiplicity::Linear
                        || destination.structural_type != plan.storage.carrier
                        || destination.qualifications.len() != 2
                        || !destination.qualifications.contains(&plan.storage.granted)
                        || !destination.qualifications.contains(&vacant)
                        || !destination.projected_qualifications.is_empty()
                        || [source_profile, destination_profile].iter().any(|profile| {
                            profile.access != StructuralAccess::SharedBorrow
                                || Some(profile.structural_type) != profile_type
                        })
                    {
                        return Err(
                            "run relocation must own live and vacant backing and borrow both original profiles",
                        );
                    }
                }
                TerminalAccessLifecycleOperation::View => {
                    let [storage, profile] = boundary.structural_parameters.as_slice() else {
                        return Err("borrowed placement lost its storage and profile inputs");
                    };
                    if !matches!(storage.access, terminal_psi::reading_borrow_pair_pattern!())
                        || profile.access != StructuralAccess::SharedBorrow
                        || Some(profile.structural_type) != profile_type
                    {
                        return Err(
                            "borrowed placement must retain its storage loan and original shared profile loan",
                        );
                    }
                }
                TerminalAccessLifecycleOperation::Initialize => {
                    let parameters = boundary.structural_parameters.as_slice();
                    let expected = if profile_type.is_some() { 3 } else { 2 };
                    if parameters.len() != expected {
                        return Err("placement initialization lost its storage/value inputs");
                    }
                    let value = &parameters[1];
                    if value.structural_type != plan.schema
                        || parameters[..2]
                            .iter()
                            .any(|parameter| parameter.access != StructuralAccess::Owned)
                    {
                        return Err("placement initialization value differs from its schema");
                    }
                    if let Some(profile_type) = profile_type
                        && (parameters[2].structural_type != profile_type
                            || parameters[2].access != StructuralAccess::SharedBorrow)
                    {
                        return Err(
                            "placement initialization profile differs from its exact input type",
                        );
                    }
                }
                TerminalAccessLifecycleOperation::MoveOut => {
                    let parameters = &boundary.structural_parameters;
                    if parameters.len() != if profile_type.is_some() { 2 } else { 1 } {
                        return Err("placement extraction lost its storage input");
                    }
                    if boundary.structural_parameters[0].access != StructuralAccess::Owned {
                        return Err("placement extraction must own its resident storage");
                    }
                    if let Some(profile_type) = profile_type
                        && (parameters[1].structural_type != profile_type
                            || parameters[1].access != StructuralAccess::SharedBorrow)
                    {
                        return Err("placement extraction must borrow its original profile");
                    }
                }
            }
            validate_storage(module, plan)?;
            if plan.operation == TerminalAccessLifecycleOperation::View {
                validate_access_view_content(module, plan)?;
            }
            access_result_contract(module, plan)?;
            validate_image_fields(module, plan)?;
            for view in module.access_view_plans.iter().filter(|view| {
                view.policy_identity == plan.policy_identity
                    && view.schema_identity == plan.schema_identity
            }) {
                if view.schema != plan.schema
                    || view.policy_plan_machine_identity != plan.policy_plan_machine_identity
                    || view.placement != plan.placement
                {
                    return Err("access lifecycle and field view disagree on normalized policy");
                }
                for field in &view.fields {
                    let image_field = plan
                        .image_fields
                        .get(field.access_slot as usize)
                        .and_then(|identity| match &schema.shape {
                            StructuralTypeShape::Record { fields, .. } => {
                                fields.iter().find(|field| field.id == *identity)
                            }
                            _ => None,
                        })
                        .ok_or("placement view lost its whole-value field correspondence")?;
                    if image_field.identity != field.schema_field {
                        return Err("placement view and whole-value image select different fields");
                    }
                }
            }
            for earlier in module.access_lifecycle_plans[..position]
                .iter()
                .filter(|earlier| {
                    earlier.policy_identity == plan.policy_identity
                        && earlier.schema_identity == plan.schema_identity
                        && earlier.target.as_ref().map(|target| target.target.as_str())
                            == plan.target.as_ref().map(|target| target.target.as_str())
                })
            {
                if earlier.schema != plan.schema
                    || earlier.image_fields != plan.image_fields
                    || earlier.storage.carrier != plan.storage.carrier
                    || earlier.storage.base != plan.storage.base
                    || earlier.storage.length != plan.storage.length
                    || earlier.storage.granted != plan.storage.granted
                    || (earlier.operation != TerminalAccessLifecycleOperation::View
                        && plan.operation != TerminalAccessLifecycleOperation::View
                        && earlier.storage.owner_carrier() == plan.storage.owner_carrier()
                        && earlier.storage.owned_content()? != plan.storage.owned_content()?)
                    || earlier.policy_plan_machine_identity != plan.policy_plan_machine_identity
                    || earlier.placement != plan.placement
                    || earlier.target != plan.target
                {
                    return Err("access lifecycle operations disagree on normalized policy");
                }
            }
        }
        Ok(())
    }

    pub fn validate_access_view_content(
        module: &TerminalModule,
        plan: &terminal_psi::TerminalAccessLifecyclePlan,
    ) -> Result<(), &'static str> {
        fn scalar(value: semantic_vocabulary::ScalarType) -> Result<(), &'static str> {
            match value {
                semantic_vocabulary::ScalarType::Integer(integer) if !integer.is_address() => {
                    Ok(())
                }
                _ => Err("nonresident access View requires total scalar decoding evidence"),
            }
        }
        fn visit(
            module: &TerminalModule,
            identity: semantic_vocabulary::StructuralTypeId,
            active: &mut Vec<semantic_vocabulary::StructuralTypeId>,
        ) -> Result<(), &'static str> {
            if active.contains(&identity) {
                return Err("nonresident access View cannot decode cyclic inline content");
            }
            let declaration = module
                .structural_type(identity)
                .ok_or("nonresident access View lost its exact content declaration")?;
            if declaration.owns_linear_claim
                || declaration.quotient_carrier.is_some()
                || declaration.scalar_restriction.is_some()
            {
                return Err(
                    "nonresident access View requires content validity or custody evidence",
                );
            }
            active.push(identity);
            match &declaration.shape {
                StructuralTypeShape::PrimitiveScalar(value) => scalar(*value)?,
                StructuralTypeShape::FixedArray { element, .. } => visit(module, *element, active)?,
                StructuralTypeShape::Record { fields, .. } => {
                    for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                        match field.field_type {
                            StructuralFieldType::Scalar(value) => scalar(value)?,
                            StructuralFieldType::IeeeFloat(_) => {}
                            StructuralFieldType::Structural(child) => visit(module, child, active)?,
                            _ => {
                                return Err(
                                    "nonresident access View requires total field decoding evidence",
                                );
                            }
                        }
                    }
                }
                _ => {
                    return Err(
                        "nonresident access View requires explicit content validity or custody evidence",
                    );
                }
            }
            active.pop();
            Ok(())
        }
        visit(module, plan.schema, &mut Vec::new())
    }

    fn validate_image_fields(
        module: &TerminalModule,
        plan: &terminal_psi::TerminalAccessLifecyclePlan,
    ) -> Result<(), &'static str> {
        let Some(StructuralTypeShape::Record { fields, .. }) = module
            .structural_type(plan.schema)
            .map(|declaration| &declaration.shape)
        else {
            return Err("placement image schema is absent");
        };
        if plan.image_fields.len() != plan.placement.access().plan().entries().len() {
            return Err("placement image fields differ from the complete normalized slot roster");
        }
        for (position, identity) in plan.image_fields.iter().enumerate() {
            let field = fields
                .iter()
                .find(|field| field.id == *identity)
                .ok_or("placement image names an absent schema field")?;
            if field.relevance.is_erased() || plan.image_fields[..position].contains(identity) {
                return Err("placement image repeats a field or represents erased custody");
            }
        }
        let carriers = plan
            .target
            .as_ref()
            .map(|target| {
                Ok::<_, &'static str>(semantic_vocabulary::TargetCarrierLayout {
                    pointer_byte_size: target.pointer_size,
                    pointer_byte_alignment: target.pointer_alignment,
                    descriptor_byte_size: target
                        .pointer_size
                        .checked_mul(2)
                        .ok_or("placement image descriptor footprint overflows")?,
                    descriptor_byte_alignment: target.pointer_alignment,
                    case_tag_byte_size: target.case_tag_size,
                    case_tag_byte_alignment: target.case_tag_alignment,
                })
            })
            .transpose()?;
        let mut geometry =
            crate::ReferenceStorageGeometry::new(module.structural_types.iter(), |application| {
                Err(crate::StorageGeometryError::UnresolvedExtent(application))
            })
            .with_target_carriers(carriers);
        for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
            let size = if let Some(scalar) = field.field_type.scalar_type() {
                crate::reference_storage_byte_width(scalar).map(u64::from)
            } else {
                let child = field
                    .field_type
                    .as_structural()
                    .ok_or("placement image field has no structural carrier")?;
                Some(
                    geometry
                        .layout(child)
                        .map_err(|error| match error {
                            crate::StorageGeometryError::MissingCarrierLayout => {
                                "placement image requires explicit target carrier geometry"
                            }
                            crate::StorageGeometryError::InvalidCarrierLayout => {
                                "placement image has invalid target carrier geometry"
                            }
                            crate::StorageGeometryError::Overflow => {
                                "placement image field geometry overflows"
                            }
                            crate::StorageGeometryError::UnresolvedExtent(_) => {
                                "placement image field has an unresolved array extent"
                            }
                            crate::StorageGeometryError::UnresolvedLayout(_) => {
                                "placement image field has an unresolved record layout"
                            }
                            _ => {
                                "placement image field has no supported closed physical representation"
                            }
                        })?
                        .0,
                )
            }
            .ok_or("placement image field has no closed physical representation")?;
            if (size != 0) != plan.image_fields.contains(&field.id) {
                return Err(
                    "placement image omits represented content or represents a zero-byte field",
                );
            }
        }
        Ok(())
    }

    fn validate_storage(
        module: &TerminalModule,
        plan: &terminal_psi::TerminalAccessLifecyclePlan,
    ) -> Result<(), &'static str> {
        let storage = &plan.storage;
        crate::references::resident_references::validate_resident_owner_geometry(
            module,
            storage.carrier,
            storage.base,
            storage.length,
        )?;
        let backing_path = match storage.owner {
            terminal_psi::TerminalAccessOwner::Extent => Vec::new(),
            terminal_psi::TerminalAccessOwner::Run {
                carrier,
                storage: backing,
                count,
                singleton,
            } => {
                if plan.operation == TerminalAccessLifecycleOperation::View
                    || singleton.is_some()
                        != (plan.operation == TerminalAccessLifecycleOperation::Initialize)
                {
                    return Err("typed run owner disagrees with its lifecycle operation");
                }
                let owner = module
                    .structural_type(carrier)
                    .ok_or("typed run owner is absent")?;
                let StructuralTypeShape::Record { fields, .. } = &owner.shape else {
                    return Err("typed run owner must be a linear record");
                };
                let backing = fields
                    .iter()
                    .find(|field| field.id == backing)
                    .ok_or("typed run backing field is absent")?;
                let count = fields
                    .iter()
                    .find(|field| field.id == count)
                    .ok_or("typed run count field is absent")?;
                if !owner.owns_linear_claim
                    || backing.id == count.id
                    || backing.relevance.is_erased()
                    || count.relevance.is_erased()
                    || backing.field_type.as_structural() != Some(storage.carrier)
                    || count
                        .field_type
                        .scalar_type()
                        .and_then(|scalar| scalar.as_integer())
                        .is_none_or(|integer| {
                            integer.is_address()
                                || integer.sign() != semantic_vocabulary::IntegerSign::Unsigned
                                || integer.bits() != Some(64)
                        })
                {
                    return Err("typed run changed its owned backing or u64 count roles");
                }
                if let Some(singleton) = singleton
                    && !module
                        .structural_domains
                        .iter()
                        .any(|domain| domain.id == singleton && domain.carrier == carrier)
                {
                    return Err("typed run singleton qualification belongs to another carrier");
                }
                vec![terminal_psi::StructuralPathSegment::Field(backing.identity)]
            }
        };
        let mut domains = vec![(storage.granted, storage.carrier)];
        let content = match plan.operation {
            TerminalAccessLifecycleOperation::View => {
                if storage.vacant.is_some() || storage.resident.is_some() {
                    return Err(
                        "borrowed placement cannot declare owned content qualification roles",
                    );
                }
                None
            }
            operation => {
                let (vacant, resident) = storage.owned_content()?;
                domains.extend([
                    (vacant, storage.carrier),
                    (resident, storage.owner_carrier()),
                ]);
                Some(
                    if operation == TerminalAccessLifecycleOperation::Initialize {
                        vacant
                    } else {
                        resident
                    },
                )
            }
        };
        if domains.iter().enumerate().any(|(position, (identity, _))| {
            domains[..position]
                .iter()
                .any(|(previous, _)| previous == identity)
        }) || domains.iter().any(|(identity, carrier)| {
            !module
                .structural_domains
                .iter()
                .any(|domain| domain.id == *identity && domain.carrier == *carrier)
        }) {
            return Err("placement extent qualification roles are absent or aliased");
        }
        let boundary = module
            .boundary_machine(plan.boundary)
            .ok_or("access lifecycle boundary is absent")?;
        let parameter = boundary
            .structural_parameters
            .first()
            .ok_or("placement extent input is absent")?;
        let input_multiplicity = match plan.operation {
            TerminalAccessLifecycleOperation::View => StructuralMultiplicity::Unrestricted,
            TerminalAccessLifecycleOperation::Initialize
            | TerminalAccessLifecycleOperation::MoveOut
            | TerminalAccessLifecycleOperation::Relocate => StructuralMultiplicity::Linear,
        };
        let extracting = matches!(
            plan.operation,
            TerminalAccessLifecycleOperation::MoveOut | TerminalAccessLifecycleOperation::Relocate
        );
        let input_carrier = if extracting {
            storage.owner_carrier()
        } else {
            storage.carrier
        };
        let (backing_qualifications, _) = terminal_psi::StructuralPathQualification::select(
            &parameter.qualifications,
            &parameter.projected_qualifications,
            if extracting { &backing_path } else { &[] },
        );
        if parameter.structural_type != input_carrier
            || parameter.multiplicity != input_multiplicity
            || !backing_qualifications.contains(&storage.granted)
            || content.is_some_and(|content| !parameter.qualifications.contains(&content))
        {
            return Err("placement extent input does not retain its exact grant and content");
        }
        Ok(())
    }
}
pub(crate) mod access_ram_correspondence {

    use access_plans::{
        AtomicCapability, BoundaryReach, ExternalCapability, PeerWritability, ResourceProfile,
        ResourceRegion, StableCapability, validate_access_resources, validate_resource_profile,
    };
    use semantic_vocabulary::StructuralTypeId;
    use terminal_psi::{
        StructuralAccess, StructuralPathSegment, StructuralTypeShape, TerminalModule,
    };

    pub fn access_reference_correspondence(
        module: &TerminalModule,
        source_schema: StructuralTypeId,
        result_reference: StructuralTypeId,
        source_access: StructuralAccess,
    ) -> Result<AccessRamCorrespondence, String> {
        let Some(StructuralTypeShape::Reference { referent, access }) = module
            .structural_type(result_reference)
            .map(|row| &row.shape)
        else {
            return Err(
                "placed establishment requires an ordinary reference descriptor".to_owned(),
            );
        };
        if !matches!(
            source_access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        ) || !matches!(
            access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        ) || !access.admits(source_access)
        {
            return Err(
                "placed establishment cannot acquire access from its result type".to_owned(),
            );
        }
        access_ram_correspondence(module, *referent, source_schema)
    }

    pub use terminal_psi::{AccessRamCorrespondence, AccessRamFieldCorrespondence};

    pub fn access_ram_correspondence(
        module: &TerminalModule,
        carrier: StructuralTypeId,
        source_schema: StructuralTypeId,
    ) -> Result<AccessRamCorrespondence, String> {
        crate::validate_access_view_plans(module).map_err(|reason| {
            format!("RAM interpretation has an invalid placement catalog: {reason:?}")
        })?;
        let mut plans = module
            .access_view_plans
            .iter()
            .filter(|plan| plan.carrier == carrier);
        let (Some(plan), None) = (plans.next(), plans.next()) else {
            return Err("RAM interpretation requires one exact retained placement".to_owned());
        };
        access_ram_plan_correspondence(
            &module.structural_types,
            &module.access_view_plans,
            plan,
            source_schema,
        )
    }

    pub fn access_ram_plan_correspondence(
        declarations: &[terminal_psi::StructuralTypeDeclaration],
        plans: &[terminal_psi::TerminalAccessViewPlan],
        plan: &terminal_psi::TerminalAccessViewPlan,
        source_schema: StructuralTypeId,
    ) -> Result<AccessRamCorrespondence, String> {
        let carrier = plan.carrier;
        if plan.schema != source_schema
            || crate::contains_access_carrier_in_catalog(declarations, plans, source_schema)
        {
            return Err("RAM interpretation must retain its ordinary source schema".to_owned());
        }
        let (source_byte_length, _) =
            crate::reference_storage_layout(declarations.iter(), source_schema).ok_or_else(
                || "RAM source has no complete contiguous declared storage".to_owned(),
            )?;
        let StructuralTypeShape::Record {
            fields: schema_fields,
            ..
        } = &declarations
            .iter()
            .find(|row| row.id == source_schema)
            .ok_or_else(|| "RAM source schema is missing".to_owned())?
            .shape
        else {
            return Err("RAM interpretation requires its exact record schema".to_owned());
        };
        let mut fields = Vec::new();
        for field in &plan.fields {
            let access = plan
                .placement
                .access()
                .plan()
                .entries()
                .get(field.access_slot as usize)
                .filter(|entry| entry.key().slot() == field.access_slot)
                .ok_or_else(|| "RAM interpretation lost its exact access slot".to_owned())?;
            if access.access() == &access_plans::FieldAccess::Inaccessible {
                continue;
            }
            let mut selected = schema_fields.iter().filter(|candidate| {
                candidate.identity == field.schema_field && !candidate.relevance.is_erased()
            });
            let (Some(source_field), None) = (selected.next(), selected.next()) else {
                return Err("RAM interpretation lost its unique source field".to_owned());
            };
            let mut path = vec![StructuralPathSegment::Field(source_field.identity)];
            if let Some(region) = plan.placement.access().field_region(access.key()) {
                let source_region =
                    crate::reference_storage_region(declarations.iter(), source_schema, &path)
                        .ok_or_else(|| {
                            "RAM array source has no complete storage region".to_owned()
                        })?;
                if source_region.byte_offset != region.byte_offset()
                    || source_region.byte_length != region.byte_length()
                {
                    return Err(
                        "RAM placement region differs from its initialized source array".to_owned(),
                    );
                }
                let mut child = source_field
                    .field_type
                    .as_structural()
                    .ok_or_else(|| "RAM region source is not structural".to_owned())?;
                loop {
                    match declarations
                        .iter()
                        .find(|row| row.id == child)
                        .map(|row| &row.shape)
                    {
                        Some(StructuralTypeShape::FixedArray {
                            element,
                            length: terminal_psi::ArrayExtent::Literal(length),
                        }) if *length != 0 => {
                            path.push(StructuralPathSegment::FixedIndex(0));
                            child = *element;
                        }
                        Some(StructuralTypeShape::PrimitiveScalar(_)) => break,
                        _ => return Err("RAM region source has no scalar array leaf".to_owned()),
                    }
                }
            }
            let source =
                crate::reference_storage_projection(declarations.iter(), source_schema, &path)
                    .ok_or_else(|| {
                        "RAM source field has no complete scalar storage projection".to_owned()
                    })?;
            let source_width = u16::from(
                crate::reference_storage_byte_width(source.stored_scalar)
                    .ok_or_else(|| "RAM source scalar has no fixed byte width".to_owned())?,
            )
            .checked_mul(8)
            .ok_or_else(|| "RAM source scalar width overflows".to_owned())?;
            if let Some(descriptor) = plan.placement.access().field_descriptor(access.key()) {
                let [fragment] = descriptor.logical_extent().fragments() else {
                    return Err(
                        "RAM interpretation must preserve one complete source scalar encoding"
                            .to_owned(),
                    );
                };
                if descriptor.container_byte_offset() != source.byte_offset
                    || descriptor.transfer_width_bits() != source_width
                    || fragment.layout_bit_offset()
                        != source
                            .byte_offset
                            .checked_mul(8)
                            .ok_or_else(|| "RAM source field bit offset overflows".to_owned())?
                    || fragment.source_bit_offset() != 0
                    || fragment.width_bits() != u64::from(source_width)
                {
                    return Err(
                        "RAM placement transfer differs from the initialized source field encoding"
                            .to_owned(),
                    );
                }
            }
            fields.push(AccessRamFieldCorrespondence {
                access_slot: field.access_slot,
                source_field: source_field.id,
                byte_offset: source.byte_offset,
                transfer_width_bits: source_width,
                logical_scalar: source.logical_scalar,
                stored_scalar: source.stored_scalar,
            });
        }
        let profile = validate_resource_profile(
            ResourceProfile {
                regions: vec![ResourceRegion {
                    offset: 0,
                    length: source_byte_length,
                    peer: PeerWritability::Exclusive,
                    stable: StableCapability::ReadWrite,
                    external: ExternalCapability::None,
                    atomic: AtomicCapability::None,
                    reach: BoundaryReach::default(),
                }],
            },
            source_byte_length,
        )
        .map_err(|reason| reason.to_string())?;
        let resources = validate_access_resources(&plan.placement, &profile)
            .map_err(|reason| reason.to_string())?;
        Ok(AccessRamCorrespondence {
            schema: source_schema,
            carrier,
            source_byte_length,
            required_base: resources.base_congruence(),
            fields,
        })
    }
}
pub(crate) mod access_ram_origins {
    use crate::descriptors::descriptor_lifetimes::{
        DescriptorLocation, DescriptorPosition, descriptor_location,
    };
    use crate::descriptors::descriptor_results::{
        DescriptorAnalysis, DescriptorCallOrigin, DescriptorDefinition, DescriptorMachine,
        DescriptorSourceOrigin,
    };
    use semantic_vocabulary::{
        BoundaryMachineId, MachineId, OperationId, PlaceId, StructuralTypeId,
    };
    use std::collections::BTreeSet;
    use terminal_psi::{
        OperationKind, StructuralArgument, StructuralTypeShape, TerminalAccessorKind,
        TerminalModule,
    };

    #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub struct AccessRamFormation {
        pub machine: MachineId,
        pub operation: OperationId,
        pub place: PlaceId,
        pub carrier: StructuralTypeId,
    }

    #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub enum AccessRamOrigin {
        Formation(AccessRamFormation),
        ProviderFormation(AccessRamFormation),
        PrivateEntry {
            input: terminal_psi::TerminalAccessViewInput,
            carrier: StructuralTypeId,
        },
    }
    impl AccessRamOrigin {
        fn carrier(&self) -> StructuralTypeId {
            match self {
                Self::Formation(formation) | Self::ProviderFormation(formation) => {
                    formation.carrier
                }
                Self::PrivateEntry { carrier, .. } => *carrier,
            }
        }
    }

    pub struct AccessRamOriginAnalysis<'a> {
        module: &'a TerminalModule,
        graph: &'a [DescriptorMachine],
        analysis: DescriptorAnalysis<'a>,
        reachable: &'a BTreeSet<MachineId>,
        entry: MachineId,
        private_entry_roots: Vec<terminal_psi::TerminalAccessViewInput>,
    }
    impl<'a> AccessRamOriginAnalysis<'a> {
        pub fn new(
            module: &'a TerminalModule,
            graph: &'a [DescriptorMachine],
            reachable: &'a BTreeSet<MachineId>,
            entry: MachineId,
        ) -> Self {
            Self {
                module,
                graph,
                analysis: DescriptorAnalysis::with_access_origins(&module.structural_types, graph),
                reachable,
                entry,
                private_entry_roots: Vec::new(),
            }
        }
        pub fn with_private_entry_roots(
            mut self,
            roots: impl IntoIterator<Item = terminal_psi::TerminalAccessViewInput>,
        ) -> Self {
            self.private_entry_roots = roots.into_iter().collect();
            self
        }

        pub fn boundary_formations(
            &mut self,
            machine: MachineId,
            position: DescriptorPosition,
            boundary: BoundaryMachineId,
            arguments: &[StructuralArgument],
        ) -> Option<BTreeSet<AccessRamOrigin>> {
            let [argument] = arguments else {
                return None;
            };
            let mut accesses = self.module.access_view_plans.iter().flat_map(|plan| {
                plan.fields.iter().flat_map(move |field| {
                    field
                        .operations
                        .iter()
                        .filter(move |operation| operation.boundary == boundary)
                        .map(move |operation| (plan, field, operation.kind))
                })
            });
            let (Some((plan, field, kind)), None) = (accesses.next(), accesses.next()) else {
                return None;
            };
            if !matches!(
                kind,
                TerminalAccessorKind::Read | TerminalAccessorKind::Write
            ) {
                return None;
            }
            let current = self.module.machine(machine)?;
            let root = current
                .structural_places
                .iter()
                .find(|place| place.id == argument.place)?;
            let root_type = match root.kind {
                semantic_vocabulary::StructuralPlaceKind::OperationResult {
                    structural_type,
                    ..
                } => structural_type,
                _ => {
                    current
                        .structural_parameters
                        .iter()
                        .chain(
                            current
                                .blocks
                                .iter()
                                .flat_map(|block| block.structural_parameters.iter()),
                        )
                        .find(|parameter| parameter.place == argument.place)?
                        .structural_type
                }
            };
            let mut descriptor_prefix = argument.path.as_slice();
            for prefix_length in 0..=argument.path.len() {
                let selected = crate::structural_window_projection(
                    self.module.structural_types.iter(),
                    root_type,
                    &argument.path[..prefix_length],
                    None,
                )?;
                if selected.structural_type == field.accessor {
                    let leaf = crate::structural_window_projection(
                        self.module.structural_types.iter(),
                        field.accessor,
                        &argument.path[prefix_length..],
                        None,
                    )?;
                    let declaration = self.module.boundary_machine(boundary)?;
                    if declaration.structural_parameters.first()?.structural_type
                        != leaf.structural_type
                    {
                        return None;
                    }
                    descriptor_prefix = &argument.path[..prefix_length];
                    break;
                }
            }
            let location = descriptor_location(
                &self.module.structural_types,
                argument.place,
                root_type,
                descriptor_prefix,
                None,
            )?;
            let formations = self.resolve(machine, position, location, &mut Vec::new())?;
            if formations
                .iter()
                .any(|origin| !matches!(origin, AccessRamOrigin::ProviderFormation(_)))
            {
                crate::access_ram_plan_correspondence(
                    &self.module.structural_types,
                    &self.module.access_view_plans,
                    plan,
                    plan.schema,
                )
                .ok()?;
            }
            (!formations.is_empty()
                && formations
                    .iter()
                    .all(|formation| formation.carrier() == plan.carrier))
            .then_some(formations)
        }
        fn resolve(
            &mut self,
            machine: MachineId,
            position: DescriptorPosition,
            location: DescriptorLocation,
            pending: &mut Vec<(MachineId, DescriptorPosition, DescriptorSourceOrigin)>,
        ) -> Option<BTreeSet<AccessRamOrigin>> {
            let origins = self
                .analysis
                .symbolic_source_origins(machine, position, &location)?;
            self.resolve_origins(machine, position, origins, pending)
        }
        fn resolve_origins(
            &mut self,
            machine: MachineId,
            position: DescriptorPosition,
            origins: Vec<DescriptorSourceOrigin>,
            pending: &mut Vec<(MachineId, DescriptorPosition, DescriptorSourceOrigin)>,
        ) -> Option<BTreeSet<AccessRamOrigin>> {
            if origins.is_empty() {
                return None;
            }
            let mut complete = BTreeSet::new();
            for origin in origins {
                let query = (machine, position, origin.clone());
                if pending.contains(&query) {
                    return None;
                }
                pending.push(query);
                match origin {
                    DescriptorSourceOrigin::Backing(DescriptorCallOrigin::AccessFormation {
                        machine,
                        operation,
                        place,
                        carrier,
                        projection: _,
                    }) => {
                        let source = self
                            .module
                            .machine(machine)?
                            .blocks
                            .iter()
                            .flat_map(|block| block.operations.iter())
                            .find(|row| row.id == operation)?;
                        let result = source.result.structural()?;
                        if result.place != place {
                            return None;
                        }
                        let provider_backing = match source.kind {
                            OperationKind::EstablishAccessReference { resident, .. } => {
                                if !matches!(self.module.structural_type(result.structural_type)?.shape,
                                    StructuralTypeShape::Reference { referent, .. } if referent == carrier)
                                {
                                    return None;
                                }
                                resident.is_some()
                            }
                            OperationKind::BoundaryCall { boundary, .. } => {
                                let plan = self
                                    .module
                                    .access_lifecycle_plans
                                    .iter()
                                    .find(|plan| plan.boundary == boundary)?;
                                let crate::AccessResultContract::View {
                                    carrier: formed, ..
                                } = crate::access_result_contract(self.module, plan).ok()?
                                else {
                                    return None;
                                };
                                if formed != carrier {
                                    return None;
                                }
                                true
                            }
                            _ => return None,
                        };
                        let formation = AccessRamFormation {
                            machine,
                            operation,
                            place,
                            carrier,
                        };
                        complete.insert(if provider_backing {
                            AccessRamOrigin::ProviderFormation(formation)
                        } else {
                            AccessRamOrigin::Formation(formation)
                        });
                    }
                    incoming @ (DescriptorSourceOrigin::IncomingSlot(_)
                    | DescriptorSourceOrigin::IncomingContents { .. }) => {
                        let incoming_root = match &incoming {
                            DescriptorSourceOrigin::IncomingSlot(location) => location.root,
                            DescriptorSourceOrigin::IncomingContents { query, .. } => query.root,
                            _ => return None,
                        };
                        let callback_entry = self
                            .module
                            .machine(machine)?
                            .declaration
                            .as_ref()
                            .is_some_and(|declaration| {
                                self.module.callback_applications.iter().any(|application| {
                                    declaration.same_identity_as(
                                        &self.module.identities,
                                        &application.body,
                                        &self.module.identities,
                                    )
                                })
                            });
                        if callback_entry {
                            return None;
                        }
                        if machine == self.entry {
                            let row = self.private_entry_roots.iter().find(|row| {
                                row.machine == machine
                                    && crate::access_view_input_parameter(self.module, row)
                                        .is_ok_and(|parameter| parameter.place == incoming_root)
                            })?;
                            if !self.module.access_view_inputs.contains(row) {
                                return None;
                            }
                            let parameter =
                                crate::access_view_input_parameter(self.module, row).ok()?;
                            complete.insert(AccessRamOrigin::PrivateEntry {
                                input: row.clone(),
                                carrier: parameter.structural_type,
                            });
                            pending.pop();
                            continue;
                        }
                        let current = self.graph.iter().find(|row| row.id == machine)?;
                        let parameter = current
                            .places
                            .iter()
                            .find(|row| row.place == incoming_root)?;
                        let DescriptorDefinition::Parameter { .. } = parameter.definition else {
                            return None;
                        };
                        let callers = self
                            .graph
                            .iter()
                            .filter(|caller| self.reachable.contains(&caller.id))
                            .flat_map(|caller| {
                                caller
                                    .calls
                                    .iter()
                                    .filter(move |(_, call)| call.callee() == Some(machine))
                                    .map(move |(position, _)| (caller.id, *position))
                            })
                            .collect::<Vec<_>>();
                        if callers.is_empty() {
                            return None;
                        }
                        for (caller, position) in callers {
                            let actual = self
                                .analysis
                                .substitute_call_source(caller, position, &incoming)?;
                            complete
                                .extend(self.resolve_origins(caller, position, actual, pending)?);
                        }
                    }
                    _ => return None,
                }
                pending.pop();
            }
            (!complete.is_empty()).then_some(complete)
        }
    }
}
pub(crate) mod access_view_plans {
    use semantic_vocabulary::AuthoredFieldStorage;
    use semantic_vocabulary::StructuralTypeId;
    use std::collections::BTreeSet;
    use terminal_psi::StructuralFieldType;
    use terminal_psi::{
        BoundaryMachineResult, StructuralAccess, StructuralTypeShape, TerminalAccessorKind,
        TerminalModule,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum AccessViewPlanError {
        DuplicateCarrier,
        DuplicateAccessor,
        CarrierMissing,
        SchemaMissing,
        CarrierNotRecord,
        SchemaNotRecord,
        IdentityMissing,
        ViewIdentityMismatch,
        InputPlanMismatch,
        InputMissing {
            machine: semantic_vocabulary::MachineId,
            runtime_position: usize,
        },
        InputDuplicate {
            machine: semantic_vocabulary::MachineId,
            runtime_position: u32,
        },
        FieldSlotMismatch,
        ViewFieldMismatch,
        SchemaFieldMismatch,
        AccessorMissing,
        DuplicateBoundary,
        BoundaryMissing,
        BoundaryCarrierMismatch,
        OperationNotAdmitted,
        OperationSignatureMismatch,
        CarrierStorageMismatch,
    }

    pub(crate) fn access_field_leaf(
        declarations: &[terminal_psi::StructuralTypeDeclaration],
        value: &StructuralFieldType,
        mut accessor: StructuralTypeId,
    ) -> Result<(StructuralFieldType, StructuralTypeId), AccessViewPlanError> {
        let mut value = value.clone();
        let mut visited = BTreeSet::new();
        loop {
            if !visited.insert(accessor) {
                return Err(AccessViewPlanError::AccessorMissing);
            }
            if value.scalar_type().is_some() {
                return Ok((value, accessor));
            }
            let source = value
                .as_structural()
                .and_then(|identity| declarations.iter().find(|row| row.id == identity))
                .ok_or(AccessViewPlanError::SchemaFieldMismatch)?;
            let view = declarations
                .iter()
                .find(|row| row.id == accessor)
                .ok_or(AccessViewPlanError::AccessorMissing)?;
            match (&source.shape, &view.shape) {
                (
                    StructuralTypeShape::FixedArray {
                        element: source_element,
                        length: terminal_psi::ArrayExtent::Literal(source_length),
                    },
                    StructuralTypeShape::FixedArray {
                        element: view_element,
                        length: terminal_psi::ArrayExtent::Literal(view_length),
                    },
                ) if source_length == view_length && *source_length != 0 => {
                    let element = declarations
                        .iter()
                        .find(|row| row.id == *source_element)
                        .ok_or(AccessViewPlanError::SchemaFieldMismatch)?;
                    value = match element.shape {
                        StructuralTypeShape::PrimitiveScalar(scalar) => {
                            StructuralFieldType::Scalar(scalar)
                        }
                        _ => StructuralFieldType::Structural(*source_element),
                    };
                    accessor = *view_element;
                }
                _ => return Err(AccessViewPlanError::SchemaFieldMismatch),
            }
        }
    }

    fn contains_accessor_leaf(
        declarations: &[terminal_psi::StructuralTypeDeclaration],
        mut root: StructuralTypeId,
        leaf: StructuralTypeId,
    ) -> bool {
        let mut visited = BTreeSet::new();
        while visited.insert(root) {
            if root == leaf {
                return true;
            }
            match declarations
                .iter()
                .find(|row| row.id == root)
                .map(|row| &row.shape)
            {
                Some(StructuralTypeShape::FixedArray { element, .. }) => root = *element,
                _ => return false,
            }
        }
        false
    }

    pub(crate) fn is_access_projection_shape(
        declarations: &[terminal_psi::StructuralTypeDeclaration],
        mut root: StructuralTypeId,
    ) -> bool {
        let mut visited = BTreeSet::new();
        while visited.insert(root) {
            match declarations
                .iter()
                .find(|row| row.id == root)
                .map(|row| &row.shape)
            {
                Some(StructuralTypeShape::FixedArray { element, .. }) => root = *element,
                Some(StructuralTypeShape::Record {
                    storage: terminal_psi::StructuralRecordStorage::Accessor(_),
                    fields,
                }) => return fields.is_empty(),
                _ => return false,
            }
        }
        false
    }

    pub fn validate_access_view_catalog(
        declarations: &[terminal_psi::StructuralTypeDeclaration],
        identities: &terminal_psi::TerminalIdentityTable,
        plans: &[terminal_psi::TerminalAccessViewPlan],
        boundaries: &[terminal_psi::BoundaryMachineDeclaration],
    ) -> Result<(), AccessViewPlanError> {
        let mut carriers = BTreeSet::new();
        let mut seen_boundaries = BTreeSet::new();
        let mut accessors = BTreeSet::new();
        let mut operation_leaves = BTreeSet::new();
        for declaration in declarations {
            if matches!(
                declaration.shape,
                StructuralTypeShape::Record {
                    storage: terminal_psi::StructuralRecordStorage::AccessView(_)
                        | terminal_psi::StructuralRecordStorage::Accessor(_),
                    ..
                }
            ) && !plans.iter().any(|plan| {
                plan.carrier == declaration.id
                    || plan.fields.iter().any(|field| {
                        contains_accessor_leaf(declarations, field.accessor, declaration.id)
                    })
            }) {
                return Err(AccessViewPlanError::CarrierStorageMismatch);
            }
        }
        for plan in plans {
            if !carriers.insert(plan.carrier) {
                return Err(AccessViewPlanError::DuplicateCarrier);
            }
            let view = declarations
                .iter()
                .find(|row| row.id == plan.carrier)
                .ok_or(AccessViewPlanError::CarrierMissing)?;
            let schema = declarations
                .iter()
                .find(|row| row.id == plan.schema)
                .ok_or(AccessViewPlanError::SchemaMissing)?;
            let StructuralTypeShape::Record {
                fields: view_fields,
                storage: terminal_psi::StructuralRecordStorage::AccessView(geometry),
            } = &view.shape
            else {
                return Err(AccessViewPlanError::CarrierNotRecord);
            };
            if Some(geometry.byte_size) != plan.placement.layout().size
                || geometry.byte_alignment != plan.placement.layout().align
                || geometry.byte_alignment == 0
                || !geometry.byte_alignment.is_power_of_two()
            {
                return Err(AccessViewPlanError::CarrierStorageMismatch);
            }
            let StructuralTypeShape::Record {
                fields: schema_fields,
                ..
            } = &schema.shape
            else {
                return Err(AccessViewPlanError::SchemaNotRecord);
            };
            let text = |identity| {
                identities
                    .text(identity)
                    .filter(|text| !text.is_empty())
                    .ok_or(AccessViewPlanError::IdentityMissing)
            };
            let expected = terminal_psi::canonical_access_view_identity(
                text(plan.policy_identity)?,
                text(plan.schema_identity)?,
            );
            if text(plan.view_identity)? != expected {
                return Err(AccessViewPlanError::ViewIdentityMismatch);
            }
            text(plan.policy_plan_machine_identity)?;
            let mut slots = BTreeSet::new();
            for field in &plan.fields {
                if !accessors.insert(field.accessor) {
                    return Err(AccessViewPlanError::DuplicateAccessor);
                }
                if !slots.insert(field.access_slot) {
                    return Err(AccessViewPlanError::FieldSlotMismatch);
                }
                let access = plan
                    .placement
                    .access()
                    .plan()
                    .entries()
                    .get(field.access_slot as usize)
                    .filter(|entry| entry.key().slot() == field.access_slot)
                    .filter(|entry| {
                        !matches!(entry.access(), access_plans::FieldAccess::Inaccessible)
                    })
                    .ok_or(AccessViewPlanError::FieldSlotMismatch)?;
                let view_field = view_fields
                    .iter()
                    .find(|row| {
                        row.identity == field.view_field
                            && row.field_type.as_structural() == Some(field.accessor)
                    })
                    .ok_or(AccessViewPlanError::ViewFieldMismatch)?;
                let schema_field = schema_fields
                    .iter()
                    .find(|row| row.identity == field.schema_field)
                    .ok_or(AccessViewPlanError::SchemaFieldMismatch)?;
                if view_field.identity != schema_field.identity {
                    return Err(AccessViewPlanError::SchemaFieldMismatch);
                }
                let layout_entry = plan
                    .placement
                    .layout()
                    .entries
                    .iter()
                    .find(|entry| entry.field == access.field())
                    .ok_or(AccessViewPlanError::FieldSlotMismatch)?;
                let canonical_member = layout_entry.member_identity.map_or_else(
                    || layout_entry.field.clone(),
                    |identity| format!("#{identity}"),
                );
                if text(field.schema_field)? != canonical_member {
                    return Err(AccessViewPlanError::SchemaFieldMismatch);
                }
                let (value_leaf, accessor_leaf) =
                    access_field_leaf(declarations, &schema_field.field_type, field.accessor)?;
                if !operation_leaves.insert(accessor_leaf) {
                    return Err(AccessViewPlanError::DuplicateAccessor);
                }
                let accessor = declarations
                    .iter()
                    .find(|row| row.id == accessor_leaf)
                    .ok_or(AccessViewPlanError::AccessorMissing)?;
                let StructuralTypeShape::Record {
                    fields: accessor_fields,
                    storage: terminal_psi::StructuralRecordStorage::Accessor(accessor_geometry),
                } = &accessor.shape
                else {
                    return Err(AccessViewPlanError::AccessorMissing);
                };
                let descriptor = plan.placement.access().field_descriptor(access.key());
                let region = plan.placement.access().field_region(access.key());
                if let Some(region) = region {
                    let source_type = schema_field
                        .field_type
                        .as_structural()
                        .ok_or(AccessViewPlanError::SchemaFieldMismatch)?;
                    if crate::reference_storage_layout(declarations.iter(), source_type)
                        != Some((region.byte_length(), region.byte_alignment()))
                    {
                        return Err(AccessViewPlanError::CarrierStorageMismatch);
                    }
                }
                let expected_size = if region.is_some() {
                    u64::from(
                        crate::reference_storage_byte_width(
                            value_leaf
                                .scalar_type()
                                .ok_or(AccessViewPlanError::SchemaFieldMismatch)?,
                        )
                        .ok_or(AccessViewPlanError::SchemaFieldMismatch)?,
                    )
                } else {
                    descriptor.map_or(0, |descriptor| descriptor.effect_footprint().length_bytes())
                };
                if region.is_some_and(|region| {
                    u64::from(region.transfer_width_bits()) != expected_size * 8
                }) {
                    return Err(AccessViewPlanError::SchemaFieldMismatch);
                }
                let expected_storage = descriptor
                    .map(|descriptor| AuthoredFieldStorage::Direct {
                        byte_offset: descriptor.container_byte_offset(),
                    })
                    .or_else(|| {
                        region.map(|region| AuthoredFieldStorage::Direct {
                            byte_offset: region.byte_offset(),
                        })
                    });
                if !accessor_fields.is_empty()
                    || accessor_geometry.byte_size != expected_size
                    || accessor_geometry.byte_alignment != 1
                    || view_field.storage != expected_storage
                {
                    return Err(AccessViewPlanError::CarrierStorageMismatch);
                }
                for operation in &field.operations {
                    if !seen_boundaries.insert(operation.boundary) {
                        return Err(AccessViewPlanError::DuplicateBoundary);
                    }
                    let boundary = boundaries
                        .iter()
                        .find(|row| row.id == operation.boundary)
                        .ok_or(AccessViewPlanError::BoundaryMissing)?;
                    let name = match operation.kind {
                        TerminalAccessorKind::Read => "read",
                        TerminalAccessorKind::Take => "take",
                        TerminalAccessorKind::Write => "write",
                    };
                    if !access.access().operation_names().contains(&name) {
                        return Err(AccessViewPlanError::OperationNotAdmitted);
                    }
                    let Some(parameter) = boundary.structural_parameters.first() else {
                        return Err(AccessViewPlanError::BoundaryCarrierMismatch);
                    };
                    let expected_access = if operation.kind == TerminalAccessorKind::Read {
                        StructuralAccess::SharedBorrow
                    } else {
                        StructuralAccess::MutableBorrow
                    };
                    if parameter.structural_type != accessor_leaf
                        || parameter.access != expected_access
                    {
                        return Err(AccessViewPlanError::BoundaryCarrierMismatch);
                    }
                    let signature_matches = if let Some(scalar) = value_leaf.scalar_type() {
                        match operation.kind {
                            TerminalAccessorKind::Read | TerminalAccessorKind::Take => {
                                boundary.structural_parameters.len() == 1
                                    && boundary.scalar_parameters.is_empty()
                                    && boundary.result
                                        == BoundaryMachineResult::Scalar(scalar.into())
                            }
                            TerminalAccessorKind::Write => {
                                boundary.structural_parameters.len() == 1
                                    && boundary.scalar_parameters == [scalar]
                                    && boundary.result == BoundaryMachineResult::Unit
                            }
                        }
                    } else if let Some(structural) = schema_field.field_type.as_structural() {
                        match operation.kind {
                            TerminalAccessorKind::Read | TerminalAccessorKind::Take => {
                                boundary.structural_parameters.len() == 1
                                    && boundary.scalar_parameters.is_empty()
                                    && matches!(&boundary.result,BoundaryMachineResult::Structural(result) if result.structural_type==structural)
                            }
                            TerminalAccessorKind::Write => {
                                boundary.structural_parameters.len() == 2
                                    && boundary.structural_parameters[1].structural_type
                                        == structural
                                    && boundary.scalar_parameters.is_empty()
                                    && boundary.result == BoundaryMachineResult::Unit
                            }
                        }
                    } else {
                        false
                    };
                    if !signature_matches {
                        return Err(AccessViewPlanError::OperationSignatureMismatch);
                    }
                }
            }
            if slots.len()
                != plan
                    .placement
                    .access()
                    .plan()
                    .entries()
                    .iter()
                    .filter(|entry| {
                        !matches!(entry.access(), access_plans::FieldAccess::Inaccessible)
                    })
                    .count()
                || view_fields.len() != plan.fields.len()
            {
                return Err(AccessViewPlanError::FieldSlotMismatch);
            }
        }
        Ok(())
    }

    pub fn validate_access_view_plans(module: &TerminalModule) -> Result<(), AccessViewPlanError> {
        validate_access_view_catalog(
            &module.structural_types,
            &module.identities,
            &module.access_view_plans,
            &module.boundary_machines,
        )?;
        let carriers = module
            .access_view_plans
            .iter()
            .map(|plan| plan.carrier)
            .collect::<BTreeSet<_>>();
        let mut inputs = BTreeSet::new();
        for input in &module.access_view_inputs {
            if !inputs.insert((input.machine, input.runtime_position)) {
                return Err(AccessViewPlanError::InputDuplicate {
                    machine: input.machine,
                    runtime_position: input.runtime_position,
                });
            }
            let parameter = crate::access_view_input_parameter(module, input)
                .map_err(|_| AccessViewPlanError::InputPlanMismatch)?;
            let plan = module
                .access_view_plans
                .iter()
                .find(|plan| plan.carrier == parameter.structural_type)
                .ok_or(AccessViewPlanError::InputPlanMismatch)?;
            if (
                input.view_identity,
                input.policy_identity,
                input.policy_plan_machine_identity,
                input.schema_identity,
                input.placement_commitment,
                input.placement_report_fingerprint,
            ) != (
                plan.view_identity,
                plan.policy_identity,
                plan.policy_plan_machine_identity,
                plan.schema_identity,
                plan.placement.content_interpretation().commitment(),
                plan.placement.identity().compatibility_fingerprint(),
            ) {
                return Err(AccessViewPlanError::InputPlanMismatch);
            }
        }
        for machine in &module.machines {
            for (runtime_position, parameter) in machine.structural_parameters.iter().enumerate() {
                if parameter.access != StructuralAccess::Owned
                    && carriers.contains(&parameter.structural_type)
                    && !u32::try_from(runtime_position)
                        .ok()
                        .is_some_and(|position| inputs.contains(&(machine.id, position)))
                {
                    return Err(AccessViewPlanError::InputMissing {
                        machine: machine.id,
                        runtime_position,
                    });
                }
            }
        }
        Ok(())
    }

    pub fn contains_access_carrier(module: &TerminalModule, root: StructuralTypeId) -> bool {
        contains_access_carrier_in_catalog(
            &module.structural_types,
            &module.access_view_plans,
            root,
        )
    }

    pub fn contains_access_carrier_in_catalog(
        declarations: &[terminal_psi::StructuralTypeDeclaration],
        plans: &[terminal_psi::TerminalAccessViewPlan],
        root: StructuralTypeId,
    ) -> bool {
        let mut pending = vec![root];
        let mut visited = BTreeSet::new();
        while let Some(identity) = pending.pop() {
            if !visited.insert(identity) {
                continue;
            }
            if plans.iter().any(|plan| {
                plan.carrier == identity
                    || plan.fields.iter().any(|field| field.accessor == identity)
            }) {
                return true;
            }
            let Some(declaration) = declarations.iter().find(|row| row.id == identity) else {
                return true;
            };
            let mut fields = Vec::new();
            match &declaration.shape {
                StructuralTypeShape::Record {
                    storage:
                        terminal_psi::StructuralRecordStorage::AccessView(_)
                        | terminal_psi::StructuralRecordStorage::Accessor(_),
                    ..
                } => return true,
                StructuralTypeShape::Record { fields: rows, .. } => fields.extend(rows),
                StructuralTypeShape::Mixed {
                    fields: rows,
                    cases,
                } => {
                    fields.extend(rows);
                    for case in cases {
                        fields.extend(&case.fields);
                    }
                }
                StructuralTypeShape::Sum { cases } => {
                    for case in cases {
                        fields.extend(&case.fields);
                    }
                }
                StructuralTypeShape::FixedArray { element, length }
                    if length.literal() != Some(0) =>
                {
                    pending.push(*element)
                }
                _ => {}
            }
            for field in fields
                .into_iter()
                .filter(|field| !field.relevance.is_erased())
            {
                if let Some(child) = field.field_type.as_structural() {
                    pending.push(child);
                }
            }
        }
        false
    }
}
pub(crate) mod access_view_referent {

    use crate::exactly_one::exactly_one;
    use semantic_vocabulary::{StructuralDomainId, StructuralTypeId};
    use terminal_psi::{
        StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
        TerminalModule,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum AccessViewReferentError {
        QualificationsNonCanonical,
        BackingUndeclared(StructuralTypeId),
        RangeUnresolved,
        QualificationUndeclared(StructuralDomainId),
        QualificationCarrier(StructuralDomainId),
    }

    pub fn validate_access_view_referent(
        module: &TerminalModule,
        structural_type: StructuralTypeId,
        path: &[StructuralPathSegment],
        qualifications: &[StructuralDomainId],
    ) -> Result<(), AccessViewReferentError> {
        if qualifications.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(AccessViewReferentError::QualificationsNonCanonical);
        }
        if declared_structural_type(module, structural_type).is_none() {
            return Err(AccessViewReferentError::BackingUndeclared(structural_type));
        }
        if resolve_referent_place(module, structural_type, path).is_none() {
            return Err(AccessViewReferentError::RangeUnresolved);
        }
        for domain in qualifications {
            let declaration = module
                .structural_domain(*domain)
                .ok_or(AccessViewReferentError::QualificationUndeclared(*domain))?;
            if declaration.carrier != structural_type {
                return Err(AccessViewReferentError::QualificationCarrier(*domain));
            }
        }
        Ok(())
    }

    fn declared_structural_type(
        module: &TerminalModule,
        structural_type: StructuralTypeId,
    ) -> Option<&StructuralTypeDeclaration> {
        let mut matching = module
            .structural_types
            .iter()
            .filter(|declaration| declaration.id == structural_type);
        let declaration = matching.next()?;
        matching.next().is_none().then_some(declaration)
    }

    fn resolve_referent_place(
        module: &TerminalModule,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<StructuralTypeId> {
        let mut structural_type = root;
        for segment in path {
            let declaration = declared_structural_type(module, structural_type)?;
            structural_type = match (segment, &declaration.shape) {
                (
                    StructuralPathSegment::Field(identity),
                    terminal_psi::record_or_mixed_shape_pattern!(fields),
                ) => {
                    let field = exactly_one(fields.iter(), |field| {
                        field.identity == *identity && !field.relevance.is_erased()
                    })?;
                    match &field.field_type {
                        StructuralFieldType::Structural(next) => *next,
                        leaf => {
                            leaf.canonical_leaf_type()?
                                .leaf_declaration(module.structural_types.iter())?
                                .id
                        }
                    }
                }
                (
                    StructuralPathSegment::FixedIndex(index),
                    StructuralTypeShape::FixedArray {
                        element,
                        length: terminal_psi::ArrayExtent::Literal(length),
                    },
                ) if index < length => *element,
                (
                    StructuralPathSegment::Referent,
                    StructuralTypeShape::Reference { referent, .. },
                ) => *referent,
                _ => return None,
            };
        }
        Some(structural_type)
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum AccessViewInputError {
        MachineMissing,
        DeclarationMissing,
        MachineIdentityMismatch,
        StateIdentityMismatch,
        CarrierIdentityMissing,
        ParameterMissing,
        CarrierMismatch,
        AccessMismatch,
    }

    pub fn access_view_input_parameter<'module>(
        module: &'module TerminalModule,
        row: &terminal_psi::TerminalAccessViewInput,
    ) -> Result<&'module terminal_psi::StructuralParameterDeclaration, AccessViewInputError> {
        let machine = module
            .machine(row.machine)
            .ok_or(AccessViewInputError::MachineMissing)?;
        let declaration = machine
            .declaration
            .as_ref()
            .ok_or(AccessViewInputError::DeclarationMissing)?;
        if declaration.machine.identity() != row.source_machine_identity {
            return Err(AccessViewInputError::MachineIdentityMismatch);
        }
        if !module
            .structural_types
            .iter()
            .any(|carrier| carrier.identity == row.carrier_identity)
        {
            return Err(AccessViewInputError::CarrierIdentityMissing);
        }
        if declaration.state.identity() != row.source_state_identity {
            return Err(AccessViewInputError::StateIdentityMismatch);
        }
        let parameter = machine
            .structural_parameters
            .get(row.runtime_position as usize)
            .ok_or(AccessViewInputError::ParameterMissing)?;
        let carrier = module
            .structural_types
            .iter()
            .find(|carrier| carrier.id == parameter.structural_type)
            .ok_or(AccessViewInputError::CarrierIdentityMissing)?;
        if carrier.identity != row.carrier_identity {
            return Err(AccessViewInputError::CarrierMismatch);
        }
        if parameter.access != row.access || row.access == terminal_psi::StructuralAccess::Owned {
            return Err(AccessViewInputError::AccessMismatch);
        }
        Ok(parameter)
    }
}
pub(crate) mod accessor_scalar_boundaries {

    use semantic_vocabulary::BoundaryMachineId;
    use terminal_psi::{
        BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind, StructuralAccess,
        StructuralTypeDeclaration, StructuralTypeShape, TerminalAccessViewPlan,
        TerminalAccessorKind, TerminalIdentityTable,
    };

    pub use terminal_psi::AccessorScalarBoundaryDemand;

    pub fn access_region_projection(
        declarations: &[StructuralTypeDeclaration],
        plan: &TerminalAccessViewPlan,
        field: &terminal_psi::TerminalAccessFieldPlan,
        path: &[terminal_psi::StructuralPathSegment],
    ) -> Result<crate::ReferenceStorageProjection, String> {
        let entry = plan
            .placement
            .access()
            .plan()
            .entries()
            .get(field.access_slot as usize)
            .ok_or_else(|| "placed region has no exact parent field".to_owned())?;
        let region = plan
            .placement
            .access()
            .field_region(entry.key())
            .ok_or_else(|| "placed projection requires a repeated parent region".to_owned())?;
        let schema = declarations
            .iter()
            .find(|row| row.id == plan.schema)
            .ok_or_else(|| "placed projection lost its schema".to_owned())?;
        let StructuralTypeShape::Record { fields, .. } = &schema.shape else {
            return Err("placed projection schema is not a record".to_owned());
        };
        let source = fields
            .iter()
            .find(|row| row.identity == field.schema_field)
            .and_then(|row| row.field_type.as_structural())
            .ok_or_else(|| "placed region has no structural source field".to_owned())?;
        let mut projection = crate::reference_storage_projection(declarations.iter(), source, path)
            .ok_or_else(|| {
                "placed region path does not select one bounded scalar leaf".to_owned()
            })?;
        let width = u64::from(
            crate::reference_storage_byte_width(projection.stored_scalar)
                .ok_or_else(|| "placed region leaf has no fixed storage width".to_owned())?,
        );
        if projection.logical_scalar != projection.stored_scalar
            || width * 8 != u64::from(region.transfer_width_bits())
            || projection
                .byte_offset
                .checked_add(width)
                .is_none_or(|end| end > region.byte_length())
        {
            return Err(
                "placed region leaf exceeds its admitted parent or changes its codec".to_owned(),
            );
        }
        projection.byte_offset = region
            .byte_offset()
            .checked_add(projection.byte_offset)
            .ok_or_else(|| "placed region leaf displacement overflows".to_owned())?;
        Ok(projection)
    }

    pub fn accessor_scalar_boundary_demand(
        declarations: &[StructuralTypeDeclaration],
        identities: &TerminalIdentityTable,
        plans: &[TerminalAccessViewPlan],
        boundaries: &[BoundaryMachineDeclaration],
        boundary: BoundaryMachineId,
    ) -> Result<AccessorScalarBoundaryDemand, String> {
        crate::validate_access_view_catalog(declarations, identities, plans, boundaries)
            .map_err(|reason| format!("placed scalar catalog is malformed: {reason:?}"))?;
        let mut matches = plans.iter().flat_map(|plan| {
            plan.fields.iter().flat_map(move |field| {
                field.operations.iter().filter_map(move |operation| {
                    (operation.boundary == boundary).then_some((plan, field, operation.kind))
                })
            })
        });
        let (Some((plan, field, operation)), None) = (matches.next(), matches.next()) else {
            return Err("placed scalar boundary has no unique sealed operation".to_owned());
        };
        let declaration = boundaries
            .iter()
            .find(|row| row.id == boundary)
            .ok_or_else(|| "placed scalar boundary declaration is missing".to_owned())?;
        let schema = declarations
            .iter()
            .find(|row| row.id == plan.schema)
            .ok_or_else(|| "placed scalar schema is missing".to_owned())?;
        let StructuralTypeShape::Record { fields, .. } = &schema.shape else {
            return Err("placed scalar source schema is not a record".to_owned());
        };
        let source_field = fields
            .iter()
            .find(|row| row.identity == field.schema_field)
            .ok_or_else(|| "placed scalar field is missing".to_owned())?;
        let (value_leaf, accessor_leaf) = crate::access::access_view_plans::access_field_leaf(
            declarations,
            &source_field.field_type,
            field.accessor,
        )
        .map_err(|reason| format!("placed scalar projection is malformed: {reason:?}"))?;
        let scalar = value_leaf
            .scalar_type()
            .ok_or_else(|| "placed scalar field has no exact scalar codec".to_owned())?;
        let access = if operation == TerminalAccessorKind::Read {
            StructuralAccess::SharedBorrow
        } else {
            StructuralAccess::MutableBorrow
        };
        let expected_order: &[BoundaryParameterKind] = if operation == TerminalAccessorKind::Write {
            &[
                BoundaryParameterKind::Structural,
                BoundaryParameterKind::Scalar,
            ]
        } else {
            &[BoundaryParameterKind::Structural]
        };
        let signature_matches = match operation {
            TerminalAccessorKind::Read | TerminalAccessorKind::Take => {
                declaration.scalar_parameters.is_empty()
                    && declaration.result == BoundaryMachineResult::Scalar(scalar.into())
            }
            TerminalAccessorKind::Write => {
                declaration.scalar_parameters == [scalar]
                    && declaration.result == BoundaryMachineResult::Unit
            }
        };
        if !signature_matches
            || declaration.parameter_order != expected_order
            || declaration.structural_parameters.len() != 1
            || declaration.structural_parameters[0].structural_type != accessor_leaf
            || declaration.structural_parameters[0].access != access
            || declaration.role != terminal_psi::BoundaryDeclarationRole::Callable
            || declaration.completion != semantic_vocabulary::BoundaryCompletion::NormalReturn
        {
            return Err(
                "placed scalar boundary substituted its parameter, polarity or codec".to_owned(),
            );
        }
        let entry = plan
            .placement
            .access()
            .plan()
            .entries()
            .get(field.access_slot as usize)
            .ok_or_else(|| "placed scalar access slot is missing".to_owned())?;
        let (byte_offset, transfer_width_bits, observation) =
            match (
                plan.placement.access().field_descriptor(entry.key()),
                plan.placement.access().field_region(entry.key()),
            ) {
                (Some(descriptor), None) => (
                    descriptor.container_byte_offset(),
                    descriptor.transfer_width_bits(),
                    descriptor.observation(),
                ),
                (None, Some(region)) => (
                    region.byte_offset(),
                    u16::from(crate::reference_storage_byte_width(scalar).ok_or_else(|| {
                        "placed scalar leaf has no fixed storage width".to_owned()
                    })?) * 8,
                    region.observation(),
                ),
                _ => return Err("placed scalar field is inaccessible".to_owned()),
            };
        let extent = plan
            .placement
            .layout()
            .size
            .ok_or_else(|| "placed scalar placement has no complete extent".to_owned())?;
        Ok(AccessorScalarBoundaryDemand {
            boundary,
            carrier: plan.carrier,
            schema: plan.schema,
            accessor: accessor_leaf,
            view_field: field.view_field,
            access_slot: field.access_slot,
            placement_commitment: plan.placement.content_interpretation().commitment(),
            operation,
            access,
            scalar,
            byte_offset,
            transfer_width_bits,
            placement_byte_extent: extent,
            observation,
        })
    }
}
