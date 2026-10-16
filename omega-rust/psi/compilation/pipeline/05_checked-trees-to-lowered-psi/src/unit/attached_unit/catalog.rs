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
};
use super::bodies::UnitPlans;
use super::{
    CheckedBoundaryMachinePlan, CheckedTrees, CheckedUnitEffectMachinePlan,
    CheckedUnitEffectOperationPlan, CheckedUnitProviderCandidate, LoweringError, SemanticDomainId,
    ServiceReachSummary, StructuralTypeId, StructuralTypeShape, UnitBody, allocate_dense,
    content_conservation, dense_identity, lookup_domain_id, lookup_service_id, lookup_type_id,
    terminal_scalar_type, unique_unit_boundary, unsupported,
};
use crate::terminal_identities::{
    BYTE_VIEW_ELEMENT_TYPE_IDENTITY, StructuralDomainBinding, lookup_domain_id_for_carrier,
    service_declaration_identity, structural_domain_identity,
};
use crate::unit::attached_unit::call_closure::BoundaryIdentity;
use checked_trees::{CheckedBoundaryMachineResultPlan, CheckedUnitStructuralTypePlan};
use terminal_psi::StructuralReferentIdentity;

mod operation_types;
pub(super) use operation_types::collect_operation_type_roots;

/// Root the leaf type identity of every structural argument plan reachable
/// inside a structural value tree: member/fixed-index leaf reads mint the
/// projected leaf's own identity (`slice(named(u8))` for a `&'a [u8]`
/// member), which no signature root names, while emission resolves that
/// identity against the unit catalog. A reference's computed backing also
/// roots its complete storage type, independently of the projected referent.
/// Scalar and Unit entrances consume this same inventory.
pub(crate) fn collect_value_roots(
    checked: &CheckedTrees,
    value: checked_trees::CheckedStructuralValueHandle,
    roots: &mut Vec<typed_trees::type_identity::TypeIdentityId>,
) -> Result<(), LoweringError> {
    let node = checked.facts.values.structural_values.nodes.get(value);
    match &node.kind {
        checked_trees::CheckedStructuralValueKind::Reference {
            source,
            source_value,
            ..
        } => {
            roots.push(source.type_identity);
            if source_value.is_valid() {
                roots.push(validation::stored_structural_type_identity(
                    &checked.typed,
                    checked
                        .facts
                        .values
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
        checked_trees::CheckedStructuralValueKind::Place(source)
        | checked_trees::CheckedStructuralValueKind::BorrowedSliceView { source, .. }
        | checked_trees::CheckedStructuralValueKind::MutableSliceView { source, .. } => {
            roots.push(source.type_identity);
        }
        checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace { source, .. } => {
            // The producer retains the qualified leaf separately and publishes
            // the exact stored carrier, including reference descriptors/views.
            roots.push(source.type_identity);
        }
        checked_trees::CheckedStructuralValueKind::Projection {
            source,
            type_identity,
            ..
        } => {
            roots.push(*type_identity);
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
                    // A view's node names the backing it reads, never the view
                    // itself, so the field roots its declared carrier, including
                    // the view's access shell. Every other initializer roots its
                    // own carrier; a scalar field (a zeroed `bool`, say) has no
                    // structural plan to root.
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
                        roots.push(validation::stored_structural_type_identity(
                            &checked.typed,
                            field.type_reference,
                            &[],
                            &[],
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

pub(super) fn lower_program_local_root_introductions(
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

pub(super) fn lower_unit_structural_types(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    plans: UnitPlans<'_>,
    closure: &[symbols::SymbolHandle],
    boundaries: &[(&CheckedBoundaryMachinePlan, BoundaryIdentity)],
) -> Result<
    (
        Vec<StructuralTypeDeclaration>,
        Vec<(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)>,
        Vec<StructuralReferentIdentity>,
    ),
    LoweringError,
> {
    lower_unit_structural_types_including(identities, checked, plans, closure, boundaries, &[])
}

pub(super) fn lower_unit_structural_types_including(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    plans: UnitPlans<'_>,
    closure: &[symbols::SymbolHandle],
    boundaries: &[(&CheckedBoundaryMachinePlan, BoundaryIdentity)],
    additional_roots: &[typed_trees::type_identity::TypeIdentityId],
) -> Result<
    (
        Vec<StructuralTypeDeclaration>,
        Vec<(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)>,
        Vec<StructuralReferentIdentity>,
    ),
    LoweringError,
> {
    let mut roots = additional_roots.to_vec();
    let mut literal_view = false;
    for symbol in closure {
        let operands = crate::scalar_lowering::computations::operand_types::type_requirements(
            checked, *symbol,
        )?;
        roots.extend(operands.identities);
        literal_view |= operands.literal_view;
        let body = UnitBody::find(plans, *symbol)?;
        if !matches!(body, UnitBody::Ordinary(plan) if plan.scalar_result.is_some() || plan.scalar_control.is_some())
            && let checked_trees::CheckedControlResultPlan::Structural(result) = body.result()?
        {
            roots.push(result.type_identity);
        }
        roots.extend(body.attachment());
        if let UnitBody::Composed(machine) = body {
            roots.extend(
                machine
                    .states
                    .iter()
                    .flat_map(|state| state.successors())
                    .flat_map(|edge| &edge.transfers)
                    .filter_map(|transfer| {
                        match &transfer.source {
                    checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan {
                        carrier_type_identity, ..
                    } if carrier_type_identity.is_valid() => Some(*carrier_type_identity),
                    _ => None,
                }
                    }),
            );
        }
        roots.extend(
            body.structural_parameters()
                .map(|parameter| parameter.type_identity),
        );
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
        // Scalar computations can own structural producers and their operand
        // temporaries. Their operation dependencies need the same catalog walk
        // as statement-owned operations, not just their call signatures.
        let value_operations =
            crate::expression_preparation::computation_graph::value_call_operations(
                checked, *symbol, None,
            )?;
        for operation in body.operations().chain(value_operations) {
            collect_operation_type_roots(checked, plans, operation, &mut roots)?;
            match operation {
                // An ordinary structural call into a Unit body roots its
                // callee's structural signature.
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
                    roots.extend(target.attachment());
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
        roots.extend(
            slot.structural_parameters
                .iter()
                .map(|parameter| parameter.type_identity),
        );
    }
    for (boundary, _) in boundaries {
        roots.extend(boundary.attachment_type_identity);
        roots.extend(
            boundary
                .structural_parameters
                .iter()
                .map(|parameter| parameter.type_identity),
        );
        roots.extend(boundary.result.structural_identity());
    }
    let (mut declarations, mut identity_ids, referents) =
        lower_unit_structural_type_roots(identities, checked, plans, &roots)?;
    if literal_view {
        crate::emission::structural_byte_sequence_store::literal_view_type(
            identities,
            &mut declarations,
        )?;
    }
    // A captured coordinate is compiler-owned primitive storage, even when
    // its source is an inline field and no authored signature roots u64.
    // Publish its ordinary carrier before any emission route borrows this
    // shared catalog. Loan preparation supplies the need; it grants no access.
    let scalar = terminal_scalar_type(checked_trees::types::PrimitiveType::U64)?;
    if !declarations
        .iter()
        .any(|declaration| declaration.shape == StructuralTypeShape::PrimitiveScalar(scalar))
    {
        let mut needs_coordinate = false;
        for machine in closure {
            let states = match UnitBody::find(plans, *machine)? {
                UnitBody::Ordinary(body) => vec![body.state],
                UnitBody::Composed(body) => body.states.iter().map(|state| state.state).collect(),
            };
            for state in states {
                let loans =
                    crate::scalar_lowering::computations::fields::prepare_place_observations(
                        identities,
                        checked,
                        *machine,
                        state,
                        &declarations,
                    );
                needs_coordinate |= loans.iter().any(|loan| {
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
                quotient_carrier: None,
                no_code_affine_cleanup: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id,
                identity: identities.intern(identity),
                shape: StructuralTypeShape::PrimitiveScalar(scalar),
            });
        }
    }
    Ok((declarations, identity_ids, referents))
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
        return unsupported("Unit closure contains a duplicate or empty structural type identity");
    }
    if selected.is_some_and(|previous| previous != plan) {
        return unsupported("Unit closure structural type declarations conflict across owners");
    }
    *selected = Some(plan);
    Ok(())
}

/// The one checked declaration of `identity` in the Unit roster.
pub(super) fn selected_declaration<'a>(
    plans: UnitPlans<'a>,
    identity: typed_trees::type_identity::TypeIdentityId,
) -> Result<&'a CheckedUnitStructuralTypePlan, LoweringError> {
    if !identity.is_valid() {
        return unsupported("Unit closure contains a duplicate or empty structural type identity");
    }
    let mut selected = None;
    select_from(plans.structural_types(), identity, &mut selected)?;
    selected.ok_or_else(|| LoweringError::MissingUnitStructuralType(format!("{identity:?}")))
}

pub(crate) fn lower_unit_structural_type_roots(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    plans: UnitPlans<'_>,
    roots: &[typed_trees::type_identity::TypeIdentityId],
) -> Result<
    (
        Vec<StructuralTypeDeclaration>,
        Vec<(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)>,
        Vec<StructuralReferentIdentity>,
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
        if active.iter().any(|candidate| *candidate == identity) {
            return unsupported("recursive structural type is outside the Unit terminal slice");
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
        match &plan.shape {
            CheckedUnitStructuralTypeShape::BindingCarrier => {}
            // Pointer transport names a closed referent, not its storage.
            // Actual backing and dereference operations root concrete shapes
            // independently. Following this edge enumerates an infinite set
            // for otherwise finite values such as Chain<T> -> &Chain<[T; 2]>.
            CheckedUnitStructuralTypeShape::Reference { .. } => {}
            // Byte backing has an implicit u8 element, owned or
            // viewed. Preserve that declared dependency just like a fixed
            // array's explicit element; range establishment uses the same
            // collection operation even when no local structural-value node
            // constructs a view.
            CheckedUnitStructuralTypeShape::ByteSequence(_) => collect(
                checked,
                plans,
                checked.type_identity_by_text(BYTE_VIEW_ELEMENT_TYPE_IDENTITY),
                active,
                selected,
            )?,
            CheckedUnitStructuralTypeShape::PrimitiveScalar(_) => {}
            // A borrowed `&[T]` view is the runtime-length descriptor over an
            // element type that must itself reach the catalog.
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
                | checked_trees::CheckedUnitRecordStorage::PlacedView {
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
    // Declaration order and the referent/block namespace preserve the
    // spelling-sorted ordering the emitted ids were allocated in.
    let identity_text = |identity: typed_trees::type_identity::TypeIdentityId| {
        checked.type_identity_text(identity).unwrap_or_default()
    };
    selected
        .sort_by(|left, right| identity_text(left.identity).cmp(&identity_text(right.identity)));
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
    referent_ids.sort_by(|left, right| identity_text(*left).cmp(&identity_text(*right)));
    referent_ids.dedup();
    // Identity-only leaves precede definitions in the shared namespace. Later
    // generated carriers allocate after the largest definition id, so they
    // cannot collide with a referent that owns no storage row.
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
            Ok(StructuralReferentIdentity {
                id: lookup_type_id(&type_ids, &identity)?,
                identity: identities.intern(&identity_text(identity)),
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let mut next_field = 1_u64;
    let mut next_case = 1_u64;
    let mut next_layout_application = 1_u32;
    let mut declarations = Vec::with_capacity(selected.len());
    for plan in selected {
        let identity = plan.identity;
        let shape = match &plan.shape {
            CheckedUnitStructuralTypeShape::BindingCarrier => StructuralTypeShape::BindingCarrier,
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
            CheckedUnitStructuralTypeShape::ByteSequence(carrier) => {
                StructuralTypeShape::ByteSequence(terminal_byte_sequence_carrier(*carrier))
            }
            CheckedUnitStructuralTypeShape::BorrowedSliceView {
                element_type_identity,
                access,
            } => StructuralTypeShape::ElementView {
                element: lookup_type_id(&type_ids, element_type_identity)?,
                access: access.map(
                    crate::expression_preparation::bindings::structural_paths::terminal_access,
                ),
            },
            CheckedUnitStructuralTypeShape::Record { fields, storage } => {
                let mut field_identities = BTreeSet::new();
                let fields = fields.iter().map(|field| {
                if !field.identity.is_valid() || !field_identities.insert(field.identity.spelling()) {
                    return Err(LoweringError::Unsupported(
                        "Unit structural type contains an empty or duplicate field identity",
                    ));
                }
                let (relevance, field_type) = match &field.field_type {
                    CheckedUnitStructuralFieldType::ProviderBacked {
                        provider_type_identity,
                    } => (field.relevance, StructuralFieldType::Erased {
                        type_identity: identities.intern(&checked
                            .type_identity_text(*provider_type_identity)
                            .unwrap_or_default()),
                    }),
                    CheckedUnitStructuralFieldType::BindingBacked {
                        provider_type_identity,
                        receipt,
                    } => {
                        if !receipt.requirement.is_valid()
                        {
                            return unsupported(
                                "Binding carrier lacks an exact requirement receipt",
                            );
                        }
                        // Retain the semantic occurrence independently of storage. A field loan
                        // retains this exact path; the child marker prohibits record
                        // construction and still requires entry/transfer establishment.
                        (field.relevance, StructuralFieldType::Structural(
                            lookup_type_id(&type_ids, provider_type_identity)?,
                        ))
                    }
                    field_type => (field.relevance, crate::structural_types::lower_field_type(identities, checked, field_type, &type_ids)?),
                };
                let declaration = StructuralFieldDeclaration {
                    storage: field.storage.clone(),
                    id: structural_field_id(allocate_dense(&mut next_field)?),
                    identity: identities.intern(&field.identity.spelling()),
                    relevance,
                    field_type,
                };
                Ok(declaration)
                }).collect::<Result<Vec<_>, LoweringError>>()?;
                StructuralTypeShape::Record {
                    fields,
                    storage: match storage {
                        checked_trees::CheckedUnitRecordStorage::PlacedView { storage, .. } => {
                            terminal_psi::StructuralRecordStorage::PlacedView(*storage)
                        }
                        checked_trees::CheckedUnitRecordStorage::PlacedAccessor(storage) => {
                            terminal_psi::StructuralRecordStorage::PlacedAccessor(*storage)
                        }
                        checked_trees::CheckedUnitRecordStorage::Packed => {
                            terminal_psi::StructuralRecordStorage::Packed
                        }
                        checked_trees::CheckedUnitRecordStorage::Authored(storage) => {
                            terminal_psi::StructuralRecordStorage::Authored(*storage)
                        }
                        checked_trees::CheckedUnitRecordStorage::TargetLayout { .. } => {
                            let id = terminal_psi::LayoutApplicationId(next_layout_application);
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
                // Terminal production runs on a closed program copy: every
                // recipe extent carries its value under this target capsule.
                length: length
                    .closed(&checked.typed)
                    .ok_or(LoweringError::Unsupported(
                        "fixed array extent is a constant recipe the target capsule has not closed",
                    ))?,
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
                                        let field_type = crate::structural_types::lower_field_type(identities, checked, &field.field_type, &type_ids)?;
                                        Ok(StructuralFieldDeclaration {
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
            CheckedUnitStructuralTypeShape::Mixed { fields, cases } => StructuralTypeShape::Mixed {
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
            },
        };
        declarations.push(StructuralTypeDeclaration {
            quotient_carrier: None,
            no_code_affine_cleanup: plan.no_code_affine_cleanup,
            descriptor_cleanup: plan.descriptor_cleanup,
            scalar_restriction: plan
                .scalar_restriction
                .as_ref()
                .map(|restriction| {
                    crate::structural_types::lower_field_type(
                        identities,
                        checked,
                        restriction,
                        &type_ids,
                    )
                })
                .transpose()?,
            id: lookup_type_id(&type_ids, &identity)?,
            identity: identities.intern(&identity_text(identity)),
            shape,
        });
    }
    Ok((declarations, type_ids, referents))
}

pub(super) fn lower_unit_structural_domains(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    plans: UnitPlans<'_>,
    closure: &[symbols::SymbolHandle],
    boundaries: &[(&CheckedBoundaryMachinePlan, BoundaryIdentity)],
    type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
) -> Result<
    (
        Vec<StructuralDomainDeclaration>,
        Vec<StructuralDomainBinding>,
    ),
    LoweringError,
> {
    let mut selected = Vec::new();
    for symbol in closure {
        let body = UnitBody::find(plans, *symbol)?;
        // A callable publishes its result contract even when its only caller
        // observes a field immediately. Register that signature's domains here,
        // not incidentally through a receiving local or another parameter.
        if !body.entry()?.returns_unit && body.scalar_result_type().is_none() {
            let (_, state) = crate::declarations::state_declaration(checked, body.entry()?.state)?;
            let mut result_domains =
                validation::view_result_carrier_qualifications(&checked.typed, state.return_type)
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
        for operation in body.operations() {
            crate::expression_preparation::structural_value_requirements::collect_operation_requirements(
                checked,
                *symbol,
                operation,
                &mut selected,
            )?;
        }
        // Values the body's computed operands construct carry the
        // memberships checking decided for them, as operation values do.
        for value in crate::expression_preparation::computation_graph::reachable_structural_values(
            checked, *symbol,
        )? {
            crate::expression_preparation::structural_value_requirements::collect_value_requirements(
                checked,
                *symbol,
                value,
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
                        qualifications, ..
                    },
                )
                | CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::Structural {
                        qualifications, ..
                    },
                ) => qualifications.as_slice(),
                _ => &[],
            }))
        {
            if !selected.contains(domain) {
                selected.push(*domain);
            }
        }
        // A call's completed value carries the roster its receiving result
        // publishes: the callee's checked custody domains and its authored
        // `ensures result in Domain` clauses. They are catalog dependencies
        // even when no parameter or local declaration in this body names
        // them (for example, a copied field returned directly). Selection
        // follows the same roster the emitted result spells, not the
        // operation that originally established those bytes. The body's own
        // result declaration spells its authored clauses the same way.
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
    let mut selected_plans: Vec<&checked_trees::CheckedUnitStructuralDomainPlan> = Vec::new();
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
                return unsupported("Unit closure has duplicate structural domain ownership");
            }
            carriers.push(plan.carrier_type_identity);
            if !domain.is_valid() || plan.name.is_empty() || !plan.carrier_type_identity.is_valid()
            {
                return Err(LoweringError::Unsupported(
                    "Unit structural domain is duplicate, null, or incomplete",
                ));
            }
            selected_plans.push(plan);
        }
        if !found {
            return unsupported("Unit closure references a missing structural domain carrier");
        }
    }
    // Semantic family and exact storage carrier are independent identities.
    // Keep every reachable instantiation; never select one capacity by source
    // declaration order or let it authorize another capacity's membership.
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
            let content_projection = content_conservation::lower_structural_content_projection(
                checked,
                plan.domain,
                &checked
                    .type_identity_text(plan.carrier_type_identity)
                    .unwrap_or_default(),
            )?;
            let establishment_routes =
                lower_establishment_routes(identities, checked, &plan.establishment_routes)?;
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
                identity: identities.intern(&structural_domain_identity(
                    &plan.name,
                    &checked
                        .type_identity_text(plan.carrier_type_identity)
                        .unwrap_or_default(),
                )),
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
        .trait_definition_by_symbol(trait_symbol)
        .ok_or(LoweringError::Unsupported(
            "domain establishment route names an absent trait",
        ))?;
    let signature = checked
        .typed
        .trait_machine_signature_by_symbol(definition, requirement_symbol)
        .ok_or(LoweringError::Unsupported(
            "domain establishment route names an absent requirement",
        ))?;
    Ok(checked
        .typed
        .trait_requirement_callable_identity_text(definition, signature))
}

fn lower_establishment_route(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    route: language_semantics::DomainEstablishmentRoute,
) -> Result<terminal_psi::StructuralEstablishmentRoute, LoweringError> {
    Ok(match route {
        language_semantics::DomainEstablishmentRoute::CheckedRequirement {
            trait_definition,
            requirement,
        } => terminal_psi::StructuralEstablishmentRoute::Requirement {
            requirement: identities.intern(&establishment_requirement_identity(
                checked,
                trait_definition,
                requirement,
            )?),
        },
        language_semantics::DomainEstablishmentRoute::BoundaryRequirement {
            boundary_trait,
            requirement,
        } => terminal_psi::StructuralEstablishmentRoute::BoundaryRequirement {
            requirement: identities.intern(&establishment_requirement_identity(
                checked,
                boundary_trait,
                requirement,
            )?),
        },
        language_semantics::DomainEstablishmentRoute::ExactMachine { machine } => {
            let declaration = checked
                .typed
                .machine(machine)
                .ok_or(LoweringError::Unsupported(
                    "domain establishment route names an absent machine",
                ))?;
            let machine = checked
                .typed
                .machine_callable_identity_text(declaration)
                .ok_or(LoweringError::Unsupported(
                    "domain establishment route machine has no normalized identity",
                ))?;
            terminal_psi::StructuralEstablishmentRoute::ExactMachine {
                machine: identities.intern(&machine),
            }
        }
    })
}

/// Normalize one authored establishment catalog to its source-free form.
/// The emitted rows are strictly ordered; each identity is the declaration's
/// canonical normalized overload identity, so private issuers retain their
/// exact names without gaining consumer authority.
pub(super) fn lower_establishment_routes(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    routes: &[language_semantics::DomainEstablishmentRoute],
) -> Result<Vec<terminal_psi::StructuralEstablishmentRoute>, LoweringError> {
    let mut lowered = routes
        .iter()
        .map(|route| lower_establishment_route(identities, checked, *route))
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
        let catalog = lower_establishment_routes(identities, checked, &plan.establishment_routes)?;
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
            let row = lower_establishment_route(identities, checked, *route)?;
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
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    plans: UnitPlans<'_>,
    closure: &[symbols::SymbolHandle],
    boundaries: &[(&CheckedBoundaryMachinePlan, BoundaryIdentity)],
    provider_candidates: &[CheckedUnitProviderCandidate],
) -> Result<(Vec<ServiceDeclaration>, Vec<(ServiceReachId, ServiceId)>), LoweringError> {
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

pub(super) fn lower_unit_services_including(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    plans: UnitPlans<'_>,
    closure: &[symbols::SymbolHandle],
    boundaries: &[(&CheckedBoundaryMachinePlan, BoundaryIdentity)],
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
                CheckedUnitEffectOperationPlan::PortWrite { service_reach, .. }
                | CheckedUnitEffectOperationPlan::RegisterAccess { service_reach, .. } => {
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
                | CheckedUnitEffectOperationPlan::Complete { .. } => {}
            }
        }
    }
    for (boundary, _) in boundaries {
        // A callable keeps its owning interface even when its explicit reach
        // is empty or names unrelated capability facets. This catalog supplies
        // identities to boundary declarations, not an implicit effect row.
        if boundary.kind == semantic_vocabulary::BoundaryRequirementKind::Trait {
            let owner = checked
                .typed
                .service_reaches
                .id_for_symbol(boundary.contract_owner)
                .ok_or(LoweringError::Unsupported(
                    "boundary trait has no service identity",
                ))?;
            if !selected.contains(&owner) {
                selected.push(owner);
            }
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

pub(super) fn lower_selected_unit_services(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
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
            let definition =
                facts
                    .services
                    .definition(*service)
                    .ok_or(LoweringError::Unsupported(
                        "Unit closure references an unknown checked service",
                    ))?;
            let identity = service_declaration_identity(checked, definition.symbol)?;
            Ok((*service, definition, identity))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    selected_definitions.sort_by(|left, right| left.2.cmp(&right.2));
    if selected_definitions
        .iter()
        .any(|(_, _, identity)| identity.is_empty())
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
                identity: identities.intern(&identity),
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
                        .trait_machine_instances(owner)
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
    let lowered = crate::terminal_identities::lower_service_reach_sorted(source, service_ids)?;
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
