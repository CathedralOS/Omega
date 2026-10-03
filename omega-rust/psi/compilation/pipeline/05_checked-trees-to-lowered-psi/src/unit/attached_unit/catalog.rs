//! Structural type, domain, and service catalog publication for attached Unit
//! closures.
use super::super::{
    BTreeSet, CheckedUnitStructuralFieldType, CheckedUnitStructuralTypeShape, DomainSemanticId,
    InstallationReachDependency, ProgramLocalRootIntroductionSchema, ServiceDeclaration, ServiceId,
    ServiceReachId, ServiceReachInterface, ServiceReachPlan, ServiceReachRowId, StructuralAccess,
    StructuralCaseDeclaration, StructuralCaseId, StructuralDomainDeclaration,
    StructuralFieldDeclaration, StructuralFieldType, StructuralParameterDeclaration,
    StructuralTypeDeclaration, lower_mixed_cases, lower_mixed_fields,
    program_local_root_introduction_compatibility_report_identity, service_id,
    structural_domain_id, structural_field_id, structural_type_id, terminal_byte_sequence_carrier,
    terminal_structural_field_type,
};
use super::bodies::UnitPlans;
use super::{
    CheckedBoundaryMachinePlan, CheckedTrees, CheckedUnitEffectMachinePlan,
    CheckedUnitEffectOperationPlan, CheckedUnitProviderCandidate, LoweringError, SemanticDomainId,
    ServiceReachSummary, StructuralTypeId, StructuralTypeShape, UnitBody, allocate_dense,
    content_conservation, dense_identity, lookup_domain_id, lookup_service_id, lookup_type_id,
    terminal_scalar_type, unique_unit_boundary, unsupported,
};
use crate::terminal_identities::BYTE_VIEW_ELEMENT_TYPE_IDENTITY;
use crate::terminal_identities::{
    StructuralDomainBinding, lookup_domain_id_for_carrier, structural_domain_identity,
};
use checked_trees::{CheckedBoundaryMachineResultPlan, CheckedUnitStructuralTypePlan};

/// Root the leaf type identity of every structural argument plan reachable
/// inside a structural value tree: member/fixed-index leaf reads mint the
/// projected leaf's own identity (`slice(named(u8))` for a `&'a [u8]`
/// member), which no signature root names, while emission resolves that
/// identity against the unit catalog.
fn collect_value_roots(
    checked: &CheckedTrees,
    value: checked_trees::CheckedStructuralValueHandle,
    roots: &mut Vec<String>,
) -> Result<(), LoweringError> {
    let node = checked.facts.values.structural_values.nodes.get(value);
    match &node.kind {
        checked_trees::CheckedStructuralValueKind::Place(source)
        | checked_trees::CheckedStructuralValueKind::Reference { source }
        | checked_trees::CheckedStructuralValueKind::ScalarCasePlace { source, .. }
        | checked_trees::CheckedStructuralValueKind::BorrowedSliceView { source } => {
            roots.push(source.type_identity.clone());
        }
        checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace { source, leaf } => {
            // The source retains qualifications; catalog roots name storage.
            // Already-projected view identities keep their existing spelling.
            let identity =
                if checked.normalized_type_identity(*leaf).as_str() == source.type_identity {
                    let carrier = super::structural_carrier_type(checked, *leaf)?;
                    checked.normalized_type_identity(carrier).into_string()
                } else {
                    source.type_identity.clone()
                };
            roots.push(identity);
        }
        checked_trees::CheckedStructuralValueKind::Projection {
            source,
            type_identity,
            ..
        } => {
            roots.push(type_identity.clone());
            // A copied projection emits a leaf copy through the existing
            // referent. Its source node is provenance, not a materialized
            // reference carrier; the signature or local already owns the
            // backing type. Owned projections still materialize their root.
            if !matches!(
                checked
                    .facts
                    .values
                    .structural_values
                    .nodes
                    .get(*source)
                    .kind,
                checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace { .. }
            ) {
                collect_value_roots(checked, *source, roots)?;
            }
        }
        checked_trees::CheckedStructuralValueKind::Record { fields, .. }
        | checked_trees::CheckedStructuralValueKind::StructuralCase { fields, .. } => {
            for field in checked
                .facts
                .values
                .structural_values
                .record_fields
                .span_or_empty(*fields)
            {
                if let checked_trees::CheckedStructuralRecordFieldValue::Structural(nested) =
                    field.value
                {
                    // A nested view's node names its backing storage, never
                    // the view itself: `source.type_identity` is the array or
                    // record the range reads from. The field's declared type
                    // carries the view, so peel it to the view carrier the way
                    // the checked identity already spells it.
                    if matches!(
                        checked
                            .facts
                            .values
                            .structural_values
                            .nodes
                            .get(nested)
                            .kind,
                        checked_trees::CheckedStructuralValueKind::BorrowedSliceView { .. }
                            | checked_trees::CheckedStructuralValueKind::MutableSliceView { .. }
                    ) {
                        let mut view = field.type_reference;
                        while let checked_trees::types::TypeReferenceNode::Reference {
                            referee,
                            ..
                        }
                        | checked_trees::types::TypeReferenceNode::Constrained {
                            base_type: referee,
                            ..
                        } = checked.type_reference_table.type_reference(view)
                        {
                            view = *referee;
                        }
                        roots.push(
                            checked
                                .type_identity(
                                    typed_trees::type_identity::TypeIdentityRequest::ordinary(view),
                                )
                                .into_string(),
                        );
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
        checked_trees::CheckedStructuralValueKind::Dispatch { arms, .. } => {
            for arm in checked
                .facts
                .values
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

/// A borrowed `&[u8]` view established through the element-view op family
/// names the `u8` row that op's element resolves. The byte carrier keeps no
/// element field, so the establishment roots that row: a slice-view value,
/// directly or in a selection arm, whose result is a borrowed byte carrier.
/// A byte view reached any other way (a parameter, a literal, a subslice)
/// lowers without it, so its module keeps exactly its own rows.
fn establishes_byte_element_view(
    checked: &CheckedTrees,
    plans: UnitPlans<'_>,
    result_identity: &str,
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
        selected_declaration(checked, plans, result_identity)?.shape,
        CheckedUnitStructuralTypeShape::ByteSequence(
            checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. }
        )
    ))
}

pub(super) fn lower_program_local_root_introductions(
    checked: &CheckedTrees,
    plan: &CheckedBoundaryMachinePlan,
    requirement_identity: &str,
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
        let Some(domain) = checked
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == projection.domain)
        else {
            // Synthetic and older checked fixtures may retain a qualification
            // row without the authored declaration that could authorize root
            // introduction. Preserve the ordinary requirement, but emit no
            // producer schema and therefore no authority-bearing candidate.
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
        let qualification = lookup_domain_id_for_carrier(domain_ids, requirement.domain, carrier)?;
        let qualification_identity = checked
            .facts
            .flow
            .terminal_unit_effects
            .structural_domains
            .iter()
            .find(|candidate| {
                candidate.domain == requirement.domain
                    && candidate.carrier_type_identity == parameter.type_identity
            })
            .map(|candidate| {
                structural_domain_identity(&candidate.identity, &candidate.carrier_type_identity)
            })
            .ok_or(LoweringError::Unsupported(
                "program-local root qualification has no normalized identity",
            ))?;
        let owner_projection = content_conservation::lower_structural_content_projection(
            checked,
            requirement.domain,
            &parameter.type_identity,
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
                requirement_identity,
                &qualification_identity,
                &parameter.type_identity,
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

pub(super) fn lower_unit_structural_types(
    checked: &CheckedTrees,
    plans: UnitPlans<'_>,
    closure: &[symbols::SymbolHandle],
    boundaries: &[(&CheckedBoundaryMachinePlan, String)],
) -> Result<
    (
        Vec<StructuralTypeDeclaration>,
        Vec<(String, StructuralTypeId)>,
    ),
    LoweringError,
> {
    lower_unit_structural_types_including(checked, plans, closure, boundaries, &[])
}

pub(super) fn lower_unit_structural_types_including(
    checked: &CheckedTrees,
    plans: UnitPlans<'_>,
    closure: &[symbols::SymbolHandle],
    boundaries: &[(&CheckedBoundaryMachinePlan, String)],
    additional_roots: &[String],
) -> Result<
    (
        Vec<StructuralTypeDeclaration>,
        Vec<(String, StructuralTypeId)>,
    ),
    LoweringError,
> {
    let mut roots = additional_roots.to_vec();
    for symbol in closure {
        roots.extend(
            crate::scalar_graph::scalar_computations::operand_types::type_roots(checked, *symbol)?,
        );
        let body = UnitBody::find(plans, *symbol)?;
        if !matches!(body, UnitBody::Ordinary(plan) if plan.scalar_result.is_some() || plan.scalar_control.is_some())
            && let checked_trees::CheckedControlResultPlan::Structural(result) = body.result()?
        {
            roots.push(result.type_identity);
        }
        roots.extend(body.attachment().map(str::to_owned));
        roots.extend(
            body.structural_parameters()
                .map(|parameter| parameter.type_identity.clone()),
        );
        for local in match body {
            UnitBody::Ordinary(machine) => machine.trivial_affine_locals.as_slice(),
            UnitBody::Composed(_) => &[],
        } {
            roots.push(local.type_identity.clone());
            if let Some(construction) = &local.construction {
                roots.push(construction.root_type_identity.clone());
            }
        }
        for operation in body.operations() {
            match operation {
                // A local reference can be the sole use of both its original
                // backing carrier and its viewed reference carrier. Neither
                // identity need occur in a function signature.
                CheckedUnitEffectOperationPlan::EstablishReference { result, source } => {
                    roots.push(result.type_identity.clone());
                    roots.push(source.type_identity.clone());
                }
                // A view subslice's result can be the body's only owner of
                // its view type: a range over a fixed-array field narrows a
                // view no earlier binding or parameter declared.
                CheckedUnitEffectOperationPlan::EstablishScalarArray { result, .. }
                | CheckedUnitEffectOperationPlan::EstablishByteSequence { result, .. }
                | CheckedUnitEffectOperationPlan::EstablishViewSubslice { result, .. } => {
                    roots.push(result.type_identity.clone());
                }
                // A leaf observed through a shared-borrowed place (`source.f`
                // on `&'a` storage) projects a type that no signature,
                // parameter or result owns: its argument plan mints the leaf's
                // own identity, and emission resolves that identity against
                // this shared catalog, so the leaf type must be rooted here.
                CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                    result, value, ..
                } => {
                    roots.push(result.type_identity.clone());
                    collect_value_roots(checked, *value, &mut roots)?;
                    if establishes_byte_element_view(checked, plans, &result.type_identity, *value)?
                    {
                        roots.push(BYTE_VIEW_ELEMENT_TYPE_IDENTITY.to_owned());
                    }
                }
                CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal {
                    type_identity, ..
                } => roots.push(type_identity.clone()),
                // An ordinary structural call into a Unit body roots its
                // callee's structural signature.
                CheckedUnitEffectOperationPlan::Call(call)
                    if let (
                        checked_trees::CheckedUnitCallRoute::Module { .. },
                        checked_trees::CheckedUnitCallResult::Structural {
                            binding: result, ..
                        },
                    ) = (call.route(), call.result())
                        && UnitBody::contains(plans, call.target.machine) =>
                {
                    let target_state = &call.target.state;
                    let target = UnitBody::find(plans, call.target.machine)?;
                    let checked_trees::CheckedControlResultPlan::Structural(signature) =
                        target.result()?
                    else {
                        return unsupported(
                            "structural graph call has no structural result catalog",
                        );
                    };
                    if target.entry()?.state != *target_state
                        || signature.type_identity != result.type_identity
                    {
                        return unsupported("structural graph call result catalog drifted");
                    }
                    roots.push(signature.type_identity);
                    roots.extend(target.attachment().map(str::to_owned));
                    roots.extend(
                        target
                            .structural_parameters()
                            .map(|parameter| parameter.type_identity.clone()),
                    );
                }
                // Any other ordinary structural call is a claim-free
                // realization owned by its structural return catalog.
                CheckedUnitEffectOperationPlan::Call(call)
                    if !call.is_boundary() && call.structural_result().is_some() =>
                {
                    let realization_machine = &call.target.machine;
                    let realization_state = &call.target.state;
                    let realizations = checked
                        .facts
                        .flow
                        .terminal_structural_returns
                        .claim_free_affine_machines
                        .iter()
                        .filter(|plan| {
                            plan.machine == *realization_machine && plan.state == *realization_state
                        })
                        .collect::<Vec<_>>();
                    let [realization] = realizations.as_slice() else {
                        return unsupported(
                            "selected structural-result Unit operation has no exact type catalog owner",
                        );
                    };
                    roots.extend(realization.attachment_type_identity.iter().cloned());
                    roots.push(realization.structural_parameter.type_identity.clone());
                    roots.push(realization.result.type_identity.clone());
                }
                CheckedUnitEffectOperationPlan::Call(call)
                    if let Some(result) = call.structural_result()
                        && call.is_boundary() =>
                {
                    roots.push(result.type_identity.clone());
                }
                _ => {}
            }
        }
        if let UnitBody::Composed(machine) = body {
            // A projected loan's carrier declares the `&'a V` view no
            // parameter, result, or operation names; it must reach the
            // lowered table before the edge emits the EstablishReference op.
            roots.extend(
                machine
                    .states
                    .iter()
                    .flat_map(|state| state.successors())
                    .flat_map(|successor| &successor.transfers)
                    .filter_map(|transfer| {
                        match &transfer.source {
                        checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan {
                            carrier_identity,
                            ..
                        } if !carrier_identity.is_empty() => Some(carrier_identity.clone()),
                        _ => None,
                    }
                    }),
            );
        }
    }
    for (boundary, _) in boundaries {
        roots.extend(boundary.attachment_type_identity.iter().cloned());
        roots.extend(
            boundary
                .structural_parameters
                .iter()
                .map(|parameter| parameter.type_identity.clone()),
        );
        roots.extend(boundary.result.structural_identity().map(str::to_owned));
    }
    lower_unit_structural_type_roots(checked, plans, &roots)
}

fn select_from<'a>(
    roster: impl Iterator<Item = &'a CheckedUnitStructuralTypePlan>,
    identity: &str,
    selected: &mut Option<&'a CheckedUnitStructuralTypePlan>,
) -> Result<(), LoweringError> {
    let mut matches = roster.filter(|plan| plan.identity == identity);
    let Some(plan) = matches.next() else {
        return Ok(());
    };
    if matches.next().is_some() {
        return unsupported("Unit closure contains a duplicate or empty structural type identity");
    }
    if selected.is_some_and(|previous| previous != plan) {
        return unsupported("Unit closure structural type declarations conflict across owners");
    }
    *selected = Some(plan);
    Ok(())
}

/// The one checked declaration of `identity` that every roster able to own
/// it agrees on.
fn selected_declaration<'a>(
    checked: &'a CheckedTrees,
    plans: UnitPlans<'a>,
    identity: &str,
) -> Result<&'a CheckedUnitStructuralTypePlan, LoweringError> {
    if identity.is_empty() {
        return unsupported("Unit closure contains a duplicate or empty structural type identity");
    }
    let mut selected = None;
    select_from(
        checked
            .facts
            .flow
            .terminal_scalar_graphs
            .structural_types
            .iter(),
        identity,
        &mut selected,
    )?;
    select_from(plans.structural_types(), identity, &mut selected)?;
    for roster in [
        &checked
            .facts
            .flow
            .terminal_structural_returns
            .structural_types,
        &checked
            .facts
            .flow
            .terminal_boundary_scalar_returns
            .structural_types,
        &checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .structural_types,
    ] {
        select_from(roster.iter(), identity, &mut selected)?;
    }
    selected.ok_or(LoweringError::Unsupported(
        "Unit closure references a missing structural type",
    ))
}

pub(crate) fn lower_unit_structural_type_roots(
    checked: &CheckedTrees,
    plans: UnitPlans<'_>,
    roots: &[String],
) -> Result<
    (
        Vec<StructuralTypeDeclaration>,
        Vec<(String, StructuralTypeId)>,
    ),
    LoweringError,
> {
    fn collect<'a>(
        checked: &'a CheckedTrees,
        plans: UnitPlans<'a>,
        identity: &str,
        active: &mut Vec<String>,
        selected: &mut Vec<&'a CheckedUnitStructuralTypePlan>,
    ) -> Result<(), LoweringError> {
        if active.iter().any(|candidate| candidate == identity) {
            return unsupported("recursive structural type is outside the Unit terminal slice");
        }
        if selected
            .iter()
            .any(|candidate| candidate.identity == identity)
        {
            return Ok(());
        }
        let plan = selected_declaration(checked, plans, identity)?;
        active.push(identity.to_owned());
        match &plan.shape {
            CheckedUnitStructuralTypeShape::Reference {
                referent_identity, ..
            } => {
                collect(checked, plans, referent_identity, active, selected)?;
            }
            // Bounded byte backing has an implicit u8 element. Preserve
            // that declared dependency just like a fixed array's explicit
            // element; range establishment uses the same collection operation
            // even when no local structural-value node constructs a view.
            CheckedUnitStructuralTypeShape::ByteSequence(
                checked_trees::CheckedByteSequenceCarrier::BoundedView { .. },
            ) => collect(
                checked,
                plans,
                BYTE_VIEW_ELEMENT_TYPE_IDENTITY,
                active,
                selected,
            )?,
            // Dynamic byte descriptors have no implicit backing extent.
            // Their element-view establishment roots u8 through the operation.
            CheckedUnitStructuralTypeShape::FusedServiceBinding
            | CheckedUnitStructuralTypeShape::PrimitiveScalar(_)
            | CheckedUnitStructuralTypeShape::ByteSequence(_) => {}
            // A borrowed `&[T]` view is the runtime-length descriptor over an
            // element type that must itself reach the catalog.
            CheckedUnitStructuralTypeShape::BorrowedSliceView {
                element_type_identity,
            } => {
                collect(checked, plans, element_type_identity, active, selected)?;
            }
            CheckedUnitStructuralTypeShape::Record { fields } => {
                for field in fields {
                    if let CheckedUnitStructuralFieldType::Structural { type_identity }
                    | CheckedUnitStructuralFieldType::FusedServiceBacked {
                        provider_type_identity: type_identity,
                        ..
                    } = &field.field_type
                    {
                        collect(checked, plans, type_identity, active, selected)?;
                    }
                }
            }
            CheckedUnitStructuralTypeShape::FixedArray {
                element_type_identity,
                ..
            } => {
                collect(checked, plans, element_type_identity, active, selected)?;
            }
            CheckedUnitStructuralTypeShape::Sum { cases } => {
                for field in cases.iter().flat_map(|case| &case.fields) {
                    if let CheckedUnitStructuralFieldType::Structural { type_identity }
                    | CheckedUnitStructuralFieldType::FusedServiceBacked {
                        provider_type_identity: type_identity,
                        ..
                    } = &field.field_type
                    {
                        collect(checked, plans, type_identity, active, selected)?;
                    }
                }
            }
            CheckedUnitStructuralTypeShape::Mixed { fields, cases } => {
                for field in fields
                    .iter()
                    .chain(cases.iter().flat_map(|case| &case.fields))
                {
                    if let CheckedUnitStructuralFieldType::Structural { type_identity }
                    | CheckedUnitStructuralFieldType::FusedServiceBacked {
                        provider_type_identity: type_identity,
                        ..
                    } = &field.field_type
                    {
                        collect(checked, plans, type_identity, active, selected)?;
                    }
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
        collect(checked, plans, identity, &mut active, &mut selected)?;
    }
    selected.sort_by(|left, right| left.identity.cmp(&right.identity));
    let type_ids = selected
        .iter()
        .enumerate()
        .map(|(index, plan)| {
            Ok((
                plan.identity.clone(),
                structural_type_id(dense_identity(index)?),
            ))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let mut next_field = 1_u64;
    let mut next_case = 1_u64;
    let mut declarations = Vec::with_capacity(selected.len());
    for plan in selected {
        let identity = plan.identity.clone();
        let shape = match &plan.shape {
            CheckedUnitStructuralTypeShape::FusedServiceBinding => {
                StructuralTypeShape::FusedServiceBinding
            }
            CheckedUnitStructuralTypeShape::Reference {
                referent_identity,
                access,
            } => {
                let access = match *access {
                    checked_trees::CheckedStructuralAccess::MutableBorrow => {
                        StructuralAccess::MutableBorrow
                    }
                    // A `&'a V` shared view loans its referent the same way —
                    // immutably.
                    checked_trees::CheckedStructuralAccess::SharedBorrow => {
                        StructuralAccess::SharedBorrow
                    }
                    _ => {
                        return unsupported("reference type has no retained mutable access");
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
            CheckedUnitStructuralTypeShape::ByteSequence(carrier) => {
                StructuralTypeShape::ByteSequence(terminal_byte_sequence_carrier(*carrier))
            }
            CheckedUnitStructuralTypeShape::BorrowedSliceView {
                element_type_identity,
            } => StructuralTypeShape::ElementView {
                element: lookup_type_id(&type_ids, element_type_identity)?,
            },
            CheckedUnitStructuralTypeShape::Record { fields } => {
                let mut field_identities = BTreeSet::new();
                let fields = fields.iter().map(|field| {
                if field.identity.is_empty() || !field_identities.insert(field.identity.as_str()) {
                    return Err(LoweringError::Unsupported(
                        "Unit structural type contains an empty or duplicate field identity",
                    ));
                }
                let (relevance, field_type) = match &field.field_type {
                    CheckedUnitStructuralFieldType::BoundedInteger(integer) => {
                        (field.relevance, StructuralFieldType::BoundedInteger(*integer))
                    }
                    CheckedUnitStructuralFieldType::Scalar(primitive) => {
                        (field.relevance, terminal_structural_field_type(*primitive)?)
                    }
                    CheckedUnitStructuralFieldType::PredicateByteSequence { carrier, predicates } => {
                        (field.relevance, StructuralFieldType::PredicateByteSequence { carrier: terminal_byte_sequence_carrier(*carrier), predicates: *predicates })
                    }
                    CheckedUnitStructuralFieldType::ByteSequence(carrier) => {
                        (field.relevance, StructuralFieldType::ByteSequence(terminal_byte_sequence_carrier(*carrier)))
                    }
                    CheckedUnitStructuralFieldType::Structural { type_identity } => {
                        (field.relevance, StructuralFieldType::Structural(lookup_type_id(&type_ids, type_identity)?))
                    }
                    CheckedUnitStructuralFieldType::ProviderBacked {
                        provider_type_identity,
                    } => (field.relevance, StructuralFieldType::Erased {
                        type_identity: provider_type_identity.clone(),
                    }),
                    CheckedUnitStructuralFieldType::FusedServiceBacked {
                        provider_type_identity,
                        erasure,
                    } => {
                        if !erasure.requirement.is_valid()
                            || checked.fused_service_erasure(erasure.requirement).is_none()
                        {
                            return unsupported(
                                "fused Service erasure lacks an exact requirement receipt",
                            );
                        }
                        // Payload erases, not the semantic occurrence. A field loan
                        // retains this exact path; the child marker prohibits record
                        // construction and still requires entry/transfer establishment.
                        (field.relevance, StructuralFieldType::Structural(
                            lookup_type_id(&type_ids, provider_type_identity)?,
                        ))
                    }
                    CheckedUnitStructuralFieldType::Erased { type_identity } => {
                        (field.relevance, StructuralFieldType::Erased {
                            type_identity: type_identity.clone(),
                        })
                    }
                };
                let declaration = StructuralFieldDeclaration {
                    id: structural_field_id(allocate_dense(&mut next_field)?),
                    identity: field.identity.clone(),
                    relevance,
                    field_type,
                };
                Ok(declaration)
                }).collect::<Result<Vec<_>, LoweringError>>()?;
                StructuralTypeShape::Record { fields }
            }
            CheckedUnitStructuralTypeShape::FixedArray {
                element_type_identity,
                length,
            } => StructuralTypeShape::FixedArray {
                element: lookup_type_id(&type_ids, element_type_identity)?,
                length: *length,
            },
            CheckedUnitStructuralTypeShape::Sum { cases } => {
                let mut case_identities = BTreeSet::new();
                let cases = cases
                    .iter()
                    .map(|case| {
                        if case.identity.is_empty()
                            || !case_identities.insert(case.identity.as_str())
                        {
                            return Err(LoweringError::Unsupported(
                                "Unit structural type contains an empty or duplicate case identity",
                            ));
                        }
                        Ok(StructuralCaseDeclaration {
                            id: StructuralCaseId::new(allocate_dense(&mut next_case)?)
                                .expect("allocated structural case identity is nonzero"),
                            identity: case.identity.clone(),
                            fields: {
                                let mut field_identities = BTreeSet::new();
                                case.fields
                                    .iter()
                                    .map(|field| {
                                        if field.identity.is_empty()
                                            || !field_identities.insert(field.identity.as_str())
                                        {
                                            return Err(LoweringError::Unsupported(
                                                "Unit structural sum case contains an empty or duplicate payload field identity",
                                            ));
                                        }
                                        let field_type = match &field.field_type {
                                            CheckedUnitStructuralFieldType::BoundedInteger(integer) => StructuralFieldType::BoundedInteger(*integer),
                                            CheckedUnitStructuralFieldType::Scalar(primitive) => {
                                                terminal_structural_field_type(*primitive)?
                                            }
                                            CheckedUnitStructuralFieldType::PredicateByteSequence { carrier, predicates } => StructuralFieldType::PredicateByteSequence { carrier: terminal_byte_sequence_carrier(*carrier), predicates: *predicates },
                                            CheckedUnitStructuralFieldType::ByteSequence(
                                                carrier,
                                            ) => StructuralFieldType::ByteSequence(
                                                terminal_byte_sequence_carrier(*carrier),
                                            ),
                                            CheckedUnitStructuralFieldType::Structural {
                                                type_identity,
                                            } => StructuralFieldType::Structural(lookup_type_id(
                                                &type_ids,
                                                type_identity,
                                            )?),
                                            CheckedUnitStructuralFieldType::ProviderBacked { .. }
                                            | CheckedUnitStructuralFieldType::FusedServiceBacked { .. } => {
                                                return unsupported("provider-backed attachment fields are valid only on records");
                                            }
                                            CheckedUnitStructuralFieldType::Erased {
                                                type_identity,
                                            } => StructuralFieldType::Erased {
                                                type_identity: type_identity.clone(),
                                            },
                                        };
                                        Ok(StructuralFieldDeclaration {
                                            id: structural_field_id(allocate_dense(
                                                &mut next_field,
                                            )?),
                                            identity: field.identity.clone(),
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
            CheckedUnitStructuralTypeShape::Mixed { fields, cases } => StructuralTypeShape::Mixed {
                fields: lower_mixed_fields(fields, &type_ids, &mut next_field)?,
                cases: lower_mixed_cases(cases, &type_ids, &mut next_field, &mut next_case)?,
            },
        };
        declarations.push(StructuralTypeDeclaration {
            id: lookup_type_id(&type_ids, &identity)?,
            identity,
            shape,
        });
    }
    Ok((declarations, type_ids))
}

pub(super) fn lower_unit_structural_domains_including(
    checked: &CheckedTrees,
    plans: UnitPlans<'_>,
    closure: &[symbols::SymbolHandle],
    boundaries: &[(&CheckedBoundaryMachinePlan, String)],
    type_ids: &[(String, StructuralTypeId)],
    additional_domains: &[SemanticDomainId],
) -> Result<
    (
        Vec<StructuralDomainDeclaration>,
        Vec<StructuralDomainBinding>,
    ),
    LoweringError,
> {
    let mut selected = Vec::new();
    for domain in additional_domains {
        if !selected.contains(domain) {
            selected.push(*domain);
        }
    }
    for symbol in closure {
        let body = UnitBody::find(plans, *symbol)?;
        for operation in body.operations() {
            crate::expression_preparation::source_custody::structural::byte_literals::collect_operation_requirements(
                checked, *symbol, operation, &mut selected,
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
                CheckedUnitEffectOperationPlan::EstablishByteSequence {
                    qualifications, ..
                } => qualifications.as_slice(),
                _ => &[],
            }))
        {
            if !selected.contains(domain) {
                selected.push(*domain);
            }
        }
    }
    for (boundary, _) in boundaries {
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
            .chain(match &boundary.result {
                CheckedBoundaryMachineResultPlan::Structural { qualifications, .. } => {
                    qualifications.iter()
                }
                CheckedBoundaryMachineResultPlan::Unit
                | CheckedBoundaryMachineResultPlan::Scalar(_) => [].iter(),
            })
        {
            if !selected.contains(domain) {
                selected.push(*domain);
            }
        }
    }
    let mut selected_plans: Vec<&checked_trees::CheckedUnitStructuralDomainPlan> = Vec::new();
    for domain in selected {
        let mut found = false;
        let wrappers = if additional_domains.contains(&domain) {
            checked
                .facts
                .flow
                .terminal_boundary_scalar_returns
                .structural_domains
                .as_slice()
        } else {
            &[]
        };
        for catalog in [plans.structural_domains(), wrappers] {
            let mut carriers = Vec::new();
            for plan in catalog.iter().filter(|plan| {
                plan.domain == domain
                    && type_ids
                        .iter()
                        .any(|(identity, _)| *identity == plan.carrier_type_identity)
            }) {
                found = true;
                if carriers.contains(&plan.carrier_type_identity.as_str()) {
                    return unsupported("Unit closure has duplicate structural domain ownership");
                }
                carriers.push(plan.carrier_type_identity.as_str());
                if !domain.is_valid()
                    || plan.identity.is_empty()
                    || plan.carrier_type_identity.is_empty()
                {
                    return Err(LoweringError::Unsupported(
                        "Unit structural domain is duplicate, null, or incomplete",
                    ));
                }
                if let Some(existing) = selected_plans.iter().find(|existing| {
                    existing.domain == domain
                        && existing.carrier_type_identity == plan.carrier_type_identity
                }) {
                    if *existing != plan {
                        return unsupported(
                            "Unit and boundary-wrapper structural domains disagree",
                        );
                    }
                    continue;
                }
                selected_plans.push(plan);
            }
        }
        if !found {
            return unsupported("Unit closure references a missing structural domain carrier");
        }
    }
    // Semantic family and exact storage carrier are independent identities.
    // Keep every reachable instantiation; never select one capacity by source
    // declaration order or let it authorize another capacity's membership.
    selected_plans.sort_by(|left, right| {
        (&left.identity, &left.carrier_type_identity)
            .cmp(&(&right.identity, &right.carrier_type_identity))
    });
    if selected_plans.windows(2).any(|pair| {
        pair[0].identity == pair[1].identity
            && pair[0].carrier_type_identity == pair[1].carrier_type_identity
    }) {
        return unsupported("Unit structural domains have duplicate canonical identities");
    }
    let domain_ids = selected_plans
        .iter()
        .enumerate()
        .map(|(index, plan)| {
            Ok(StructuralDomainBinding {
                semantic_domain: plan.domain,
                carrier: lookup_type_id(type_ids, &plan.carrier_type_identity)?,
                domain: structural_domain_id(dense_identity(index)?),
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let declarations = selected_plans
        .into_iter()
        .map(|plan| {
            super::domain_replay::validate(checked, plan)?;
            let content_projection = content_conservation::lower_structural_content_projection(
                checked,
                plan.domain,
                &plan.carrier_type_identity,
            )?;
            let establishment_routes =
                lower_establishment_routes(checked, &plan.establishment_routes)?;
            Ok(StructuralDomainDeclaration {
                byte_predicate: plan.byte_predicate,
                establishment_routes,
                id: lookup_domain_id_for_carrier(
                    &domain_ids,
                    plan.domain,
                    lookup_type_id(type_ids, &plan.carrier_type_identity)?,
                )?,
                semantic_domain: DomainSemanticId::new(u64::from(plan.domain.0))
                    .ok_or(LoweringError::InvalidContentDomainIdentity)?,
                identity: structural_domain_identity(&plan.identity, &plan.carrier_type_identity),
                carrier: lookup_type_id(type_ids, &plan.carrier_type_identity)?,
                content_projection,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    Ok((declarations, domain_ids))
}

fn establishment_requirement_identity(
    checked: &CheckedTrees,
    trait_symbol: symbols::SymbolHandle,
    requirement_symbol: symbols::SymbolHandle,
) -> Result<String, LoweringError> {
    let definition = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.symbol == trait_symbol)
        .ok_or(LoweringError::Unsupported(
            "domain establishment route names an absent trait",
        ))?;
    let signature = checked
        .typed
        .trait_machine_signatures(definition)
        .iter()
        .find(|signature| signature.symbol == requirement_symbol)
        .ok_or(LoweringError::Unsupported(
            "domain establishment route names an absent requirement",
        ))?;
    Ok(checked
        .typed
        .normalized_trait_requirement_overload_identity(definition, signature)
        .identity())
}

fn lower_establishment_route(
    checked: &CheckedTrees,
    route: language_semantics::DomainEstablishmentRoute,
) -> Result<terminal_psi::StructuralEstablishmentRoute, LoweringError> {
    Ok(match route {
        language_semantics::DomainEstablishmentRoute::CheckedRequirement {
            trait_definition,
            requirement,
        } => terminal_psi::StructuralEstablishmentRoute::Requirement {
            requirement: establishment_requirement_identity(
                checked,
                trait_definition,
                requirement,
            )?,
        },
        language_semantics::DomainEstablishmentRoute::BoundaryRequirement {
            boundary_trait,
            requirement,
        } => terminal_psi::StructuralEstablishmentRoute::BoundaryRequirement {
            requirement: establishment_requirement_identity(checked, boundary_trait, requirement)?,
        },
        language_semantics::DomainEstablishmentRoute::ExactMachine { machine } => {
            let declaration = checked
                .typed
                .machines()
                .iter()
                .find(|declaration| declaration.symbol == machine)
                .ok_or(LoweringError::Unsupported(
                    "domain establishment route names an absent machine",
                ))?;
            let machine = checked
                .typed
                .normalized_machine_overload_identity(declaration)
                .map(|identity| identity.identity())
                .ok_or(LoweringError::Unsupported(
                    "domain establishment route machine has no normalized identity",
                ))?;
            terminal_psi::StructuralEstablishmentRoute::ExactMachine { machine }
        }
    })
}

/// Normalize one authored establishment catalog to its source-free form.
/// The emitted rows are strictly ordered; each identity is the declaration's
/// canonical normalized overload identity, so private issuers retain their
/// exact names without gaining consumer authority.
pub(super) fn lower_establishment_routes(
    checked: &CheckedTrees,
    routes: &[language_semantics::DomainEstablishmentRoute],
) -> Result<Vec<terminal_psi::StructuralEstablishmentRoute>, LoweringError> {
    let mut lowered = routes
        .iter()
        .map(|route| lower_establishment_route(checked, *route))
        .collect::<Result<Vec<_>, _>>()?;
    lowered.sort();
    if lowered.windows(2).any(|pair| pair[0] == pair[1]) {
        return unsupported("domain establishment catalog retains a duplicate route");
    }
    Ok(lowered)
}

/// Bind each route-authorized membership on one emitted boundary call result
/// to the exact catalog row authorizing it. Only occurrences whose checked
/// evidence at this call coordinate records the boundary requirement's
/// admitted receipt carry a row; every other membership lowers bare.
///
/// Requirement and ExactMachine routes retain only their catalog rows: an
/// attached-unit module publishes no conformance application or callable
/// registry for an ordinary callee, so no replay authority exists for an
/// ordinary call's issuer identity and no binding is emitted rather than one
/// the verifier could not check.
pub(super) fn call_result_qualification_establishments(
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
        statement_index: usize::try_from(coordinate.statement_index)
            .map_err(|_| LoweringError::Unsupported("call statement index exceeds usize"))?,
        call_ordinal: usize::try_from(coordinate.call_ordinal)
            .map_err(|_| LoweringError::Unsupported("call ordinal exceeds usize"))?,
    };
    let mut establishments = Vec::new();
    for domain in result_domains {
        let Some(plan) = checked
            .facts
            .flow
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
        let catalog = lower_establishment_routes(checked, &plan.establishment_routes)?;
        for route in &plan.establishment_routes {
            let authorized = matches!(
                route,
                language_semantics::DomainEstablishmentRoute::BoundaryRequirement {
                    requirement,
                    ..
                } if *requirement == boundary_callee
            );
            if !authorized {
                continue;
            }
            let row = lower_establishment_route(checked, *route)?;
            let Some(index) = catalog.iter().position(|candidate| *candidate == row) else {
                continue;
            };
            establishments.push(terminal_psi::ResultQualificationEstablishment {
                domain: lookup_domain_id(domain_ids, *domain)?,
                route: u32::try_from(index).map_err(|_| {
                    LoweringError::Unsupported("establishment route index exceeds u32")
                })?,
            });
            break;
        }
    }
    establishments.sort_by_key(|binding| (binding.domain, binding.route));
    Ok(establishments)
}

pub(super) fn lower_unit_services(
    checked: &CheckedTrees,
    plans: UnitPlans<'_>,
    closure: &[symbols::SymbolHandle],
    boundaries: &[(&CheckedBoundaryMachinePlan, String)],
    provider_candidates: &[CheckedUnitProviderCandidate],
) -> Result<(Vec<ServiceDeclaration>, Vec<(ServiceReachId, ServiceId)>), LoweringError> {
    lower_unit_services_including(
        checked,
        plans,
        closure,
        boundaries,
        provider_candidates,
        &[],
    )
}

pub(super) fn lower_unit_services_including(
    checked: &CheckedTrees,
    plans: UnitPlans<'_>,
    closure: &[symbols::SymbolHandle],
    boundaries: &[(&CheckedBoundaryMachinePlan, String)],
    provider_candidates: &[CheckedUnitProviderCandidate],
    additional_roots: &[ServiceReachId],
) -> Result<(Vec<ServiceDeclaration>, Vec<(ServiceReachId, ServiceId)>), LoweringError> {
    let facts = &checked.facts.service_reaches;
    let mut selected = additional_roots.to_vec();
    for symbol in closure {
        let body = UnitBody::find(plans, *symbol)?;
        if let Some(provider) = provider_candidates
            .iter()
            .find(|candidate| candidate.candidate == *symbol)
        {
            collect_provider_candidate_services(&facts.rows, plans, provider, body, &mut selected)?;
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
                    collect_service_summary(&facts.rows, call.target.service_reach, &mut selected)?;
                }
                CheckedUnitEffectOperationPlan::PortWrite { service_reach, .. } => {
                    collect_service_summary(&facts.rows, *service_reach, &mut selected)?;
                }
                CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { .. }
                | CheckedUnitEffectOperationPlan::EstablishScalarArray { .. }
                | CheckedUnitEffectOperationPlan::EstablishReference { .. }
                | CheckedUnitEffectOperationPlan::ReleaseReference { .. }
                | CheckedUnitEffectOperationPlan::EstablishByteSequence { .. }
                | CheckedUnitEffectOperationPlan::EstablishViewSubslice { .. }
                | CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
                | CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal { .. }
                | CheckedUnitEffectOperationPlan::EstablishStructuralLocal { .. }
                | CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. }
                | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
                | CheckedUnitEffectOperationPlan::RepresentationViewStore(_)
                | CheckedUnitEffectOperationPlan::AtomicAccess(_)
                | CheckedUnitEffectOperationPlan::ByteSequenceStore(_)
                | CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
                | CheckedUnitEffectOperationPlan::ElementViewWrite(_)
                | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
                | CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
                | CheckedUnitEffectOperationPlan::StructuralCaseFieldStore(_)
                | CheckedUnitEffectOperationPlan::StructuralCaseLiteralFieldStore(_)
                | CheckedUnitEffectOperationPlan::MoveStructuralField { .. }
                | CheckedUnitEffectOperationPlan::StoreStructuralField { .. }
                | CheckedUnitEffectOperationPlan::CallContinuationCleanup { .. }
                | CheckedUnitEffectOperationPlan::Complete { .. } => {}
            }
        }
    }
    for (boundary, _) in boundaries {
        collect_published_contract_services(
            &facts.rows,
            boundary.contract_service_reach,
            boundary.service_reach,
            &mut selected,
        )?;
    }
    lower_selected_unit_services(checked, selected)
}

pub(super) fn lower_selected_unit_services(
    checked: &CheckedTrees,
    mut selected: Vec<ServiceReachId>,
) -> Result<(Vec<ServiceDeclaration>, Vec<(ServiceReachId, ServiceId)>), LoweringError> {
    let facts = &checked.facts.service_reaches;
    let mut next = 0_usize;
    while let Some(service) = selected.get(next).copied() {
        next += 1;
        let definition = facts
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
    let mut selected_definitions = selected
        .iter()
        .map(|service| {
            facts
                .services
                .definition(*service)
                .map(|definition| (*service, definition))
                .ok_or(LoweringError::Unsupported(
                    "Unit closure references an unknown checked service",
                ))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    selected_definitions.sort_by(|left, right| left.1.name.cmp(&right.1.name));
    if selected_definitions
        .iter()
        .any(|(_, definition)| definition.name.is_empty())
        || selected_definitions
            .windows(2)
            .any(|pair| pair[0].1.name == pair[1].1.name)
    {
        return unsupported("Unit services have empty or duplicate canonical identities");
    }
    let service_ids = selected_definitions
        .iter()
        .enumerate()
        .map(|(index, (source, _))| Ok((*source, service_id(dense_identity(index)?))))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let declarations = selected_definitions
        .into_iter()
        .map(|(source, definition)| {
            let mut parents = definition
                .parents
                .iter()
                .map(|parent| lookup_service_id(&service_ids, *parent))
                .collect::<Result<Vec<_>, LoweringError>>()?;
            parents.sort();
            parents.dedup();
            Ok(ServiceDeclaration {
                id: lookup_service_id(&service_ids, source)?,
                identity: definition.name.clone(),
                parents,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    Ok((declarations, service_ids))
}

pub(crate) fn lower_root_service_reach(
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
                        .trait_machine_signatures(owner)
                        .iter()
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
                    .normalized_trait_requirement_overload_identity(owner, requirement)
                    .identity(),
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
                        .normalized_machine_overload_identity(machine)
                        .ok_or({
                            LoweringError::Unsupported(
                                "top-level installation reach requirement has no normalized machine overload identity",
                            )
                        })?
                        .identity()
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
                requirement_identity,
                upper_bound,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    installation_dependencies
        .sort_by(|left, right| left.requirement_identity.cmp(&right.requirement_identity));
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
        if contract.checked_inferred != summary.transitive
            || reach.inferred_transitive != summary.transitive
        {
            return unsupported(
                "installation-bound machine reach disagrees with its checked transitive row",
            );
        }
        return collect_service_summary(&checked.facts.service_reaches.rows, summary, selected);
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
            return unsupported("checked provider boundary has no published service ceiling");
        }
    };
    if rows
        .services(reach.transitive)
        .iter()
        .any(|service| !rows.services(ceiling).contains(service))
    {
        return unsupported("checked provider adapter reach exceeds its boundary requirement");
    }
    Ok(())
}

pub(super) fn lower_provider_candidate_service_ceiling(
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
    let mut lowered = source
        .iter()
        .map(|service| lookup_service_id(service_ids, *service))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    lowered.sort();
    lowered.dedup();
    if lowered.len() != source.len() {
        return unsupported("checked provider adapter reach contains duplicates");
    }
    Ok(lowered)
}

pub(crate) fn checked_unit_target_reach_matches(
    call: ServiceReachSummary,
    target_contract: ServiceReachPlan,
) -> bool {
    let expected = match target_contract.interface {
        ServiceReachInterface::PublishedCeiling(row) => row,
        ServiceReachInterface::InternalInferred => target_contract.checked_inferred,
    };
    call.transitive == expected
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
        return unsupported("Unit direct service reach is not contained in transitive reach");
    }
    for service in rows.services(summary.direct).iter().chain(transitive) {
        if !selected.contains(service) {
            selected.push(*service);
        }
    }
    Ok(())
}

pub(super) fn require_valid_service_row(row: ServiceReachRowId) -> Result<(), LoweringError> {
    if row.is_valid() {
        Ok(())
    } else {
        unsupported("Unit closure contains a null checked service row")
    }
}
