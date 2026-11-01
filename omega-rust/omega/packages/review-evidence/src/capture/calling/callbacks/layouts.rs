//! Rejoin producer-retained review addresses to independently supplied Source.
//! Physical values come from the canonical application, never a rebuilt layout.

use crate::capture::PackageReviewInput;
use crate::capture::calling::rejected;
use crate::capture::project_checked_conformance_policy;
use crate::capture::semantics::declarations::nominal_identity;
use crate::capture::semantics::types::signature_type_identity;
use crate::record::{
    PackagePolicyCallbackInlineField, PackagePolicyCallbackLayout,
    PackagePolicyCallbackLayoutApplication,
};
use calling_conventions::CallbackDataGeometry;
use diagnostics::Diagnostic;
use provider_planning::calling_policy_plans::{
    BoundaryCallbackLayoutEntry, CallbackRecordReviewSource,
};
use typed_trees::name::Identifier;
use typed_trees::types::TypeReferenceNode;
use typed_trees::{PlanLaidGeometry, PlanLaidLayout, PlanLaidRealizations};
use validation::CheckingProjection;

pub(super) fn project(
    compilation: &PackageReviewInput<'_>,
    entry: &BoundaryCallbackLayoutEntry,
    lifetime_binders: &[Identifier],
) -> Result<PackagePolicyCallbackLayout, Vec<Diagnostic>> {
    let review = entry.review_source();
    let geometry_fields = entry.inline_fields().map_err(|reason| rejected(&reason))?;
    if geometry_fields.len() != review.fields.len() {
        return Err(rejected(
            "callback path lost its exact source field correspondence",
        ));
    }
    let mut parent = &review.root;
    let mut projected = CheckingProjection::new(compilation.checking_input);
    let mut inline_fields = Vec::with_capacity(geometry_fields.len());
    for (field, source) in geometry_fields.iter().zip(&review.fields) {
        let parent_reference = source_reference(&mut projected, parent)?;
        let parent_reference = validation::structural_storage_type(
            &projected,
            parent_reference,
            &parent.application.substitutions,
        );
        let data = compilation
            .data_definitions()
            .iter()
            .find(|data| data.symbol == parent.declaration)
            .ok_or_else(|| rejected("callback inline parent lost its source declaration"))?;
        let source_field = compilation
            .data_field(data, source.declaration)
            .ok_or_else(|| rejected("callback inline field lost its source declaration"))?;
        let references = validation::observed_storage_type_references(
            &projected,
            parent_reference,
            &parent.application.substitutions,
            &[],
            source.declaration,
            &source_field.member_identity(),
            false,
        )
        .map_err(|error| rejected(&format!("callback inline field type: {error:?}")))?;
        let (reference, substitutions) = references
            .entries()
            .last()
            .ok_or_else(|| rejected("callback inline field lost its receiving type"))?;
        let identity =
            validation::structural_type_identity(&projected, reference, &[], substitutions);
        if projected.type_identity_text(identity).as_deref()
            != Some(source.child.storage_identity.as_str())
        {
            return Err(rejected(
                "callback inline field selects another exact child application",
            ));
        }
        if let Some(plan) = plan(compilation, parent)? {
            let mut positions = plan
                .field_symbols
                .iter()
                .enumerate()
                .filter(|(_, symbol)| **symbol == source.declaration);
            let (ordinal, _) = positions.next().ok_or_else(|| {
                rejected("callback inline field is not owned by its exact parent layout")
            })?;
            if positions.next().is_some()
                || plan.schema_field_symbols.get(ordinal) != Some(&source.schema)
            {
                return Err(rejected(
                    "callback inline field lost its exact schema declaration",
                ));
            }
            let selected = selected_geometry(compilation, parent, plan)?;
            if selected
                .offsets
                .get(ordinal)
                .and_then(|offset| u64::try_from(*offset).ok())
                != Some(field.offset)
            {
                return Err(rejected(
                    "callback inline offset differs from its source layout application",
                ));
            }
            if selected.authored_field_storage(ordinal, false)
                != Some(semantic_vocabulary::AuthoredFieldStorage::Direct {
                    byte_offset: field.offset,
                })
            {
                return Err(rejected(
                    "callback inline path changed its exact field representation",
                ));
            }
        } else {
            let data = compilation
                .data_definitions()
                .iter()
                .find(|data| data.symbol == parent.declaration)
                .ok_or_else(|| rejected("callback natural parent has no source declaration"))?;
            if source.schema != source.declaration
                || !compilation
                    .live_data_fields(compilation.data_members(data))
                    .any(|field| field.symbol == source.declaration)
            {
                return Err(rejected(
                    "callback natural field is not owned by its parent declaration",
                ));
            }
        }
        inline_fields.push(PackagePolicyCallbackInlineField {
            field: nominal_identity(compilation, source.schema)?,
            offset: field.offset,
            extent: field.child.size,
            alignment: field.child.alignment,
            child_layout: application(compilation, &source.child, &field.child, lifetime_binders)?,
        });
        parent = &source.child;
    }
    let source_plan = plan(compilation, parent)?
        .ok_or_else(|| rejected("callback terminal slot has no exact source layout owner"))?;
    let coordinate = &review.terminal.realization;
    if parent.realization.as_ref() != Some(coordinate) {
        return Err(rejected(
            "callback slot names another containing realization",
        ));
    }
    let geometry = selected_geometry(compilation, parent, source_plan)?;
    let terminal = entry.terminal_slot();
    let source_slot = geometry
        .private_callback_demands
        .get(review.terminal.ordinal as usize)
        .ok_or_else(|| rejected("callback source lost its exact slot ordinal"))?;
    if source_slot.slot_application.selection != review.terminal.selection
        || source_slot
            .slot_application
            .selection
            .application_commitment
            != terminal.conformance_commitment
        || source_slot.slot_identity != terminal.identity
        || source_slot.layout_subject_identity != terminal.policy
        || source_slot.callback_requirement_identity != terminal.requirement_identity
        || source_slot.offset != terminal.offset
    {
        return Err(rejected(
            "callback slot differs from its exact source-owned review application",
        ));
    }
    check_geometry(
        geometry,
        geometry_fields
            .last()
            .map_or(entry.root_layout(), |field| &field.child),
    )?;
    Ok(PackagePolicyCallbackLayout {
        formal_ordinal: entry.formal_ordinal(),
        native_ordinal: entry.native_ordinal(),
        root_layout: application(
            compilation,
            &review.root,
            entry.root_layout(),
            lifetime_binders,
        )?,
        inline_fields,
        terminal_slot: project_checked_conformance_policy(
            compilation,
            &source_slot.slot_application.application,
            lifetime_binders,
        )?,
        terminal_offset: terminal.offset,
        terminal_byte_size: terminal.size,
        terminal_alignment: terminal.alignment,
        composed_offset: entry
            .composed_offset()
            .map_err(|reason| rejected(&reason))?,
    })
}

fn plan<'a>(
    compilation: &'a PackageReviewInput<'_>,
    source: &CallbackRecordReviewSource,
) -> Result<Option<&'a PlanLaidLayout>, Vec<Diagnostic>> {
    let Some(coordinate) = &source.layout else {
        if source.policy.is_valid() || source.schema != source.declaration {
            return Err(rejected(
                "callback natural source has a substituted policy or schema",
            ));
        }
        return Ok(None);
    };
    let plan = compilation
        .plan_laid_layouts
        .get(coordinate.ordinal as usize)
        .ok_or_else(|| rejected("callback source layout has a stale owner coordinate"))?;
    if plan.application != coordinate.application
        || plan.data_symbol != source.declaration
        || plan.schema_symbol != source.schema
        || plan.policy_symbol != source.policy
    {
        return Err(rejected(
            "callback source layout differs from its original checked application",
        ));
    }
    Ok(Some(plan))
}

fn check_geometry(
    source: &PlanLaidGeometry,
    geometry: &CallbackDataGeometry,
) -> Result<(), Vec<Diagnostic>> {
    if u64::try_from(source.size).ok() != Some(geometry.size)
        || u64::try_from(source.align).ok() != Some(geometry.alignment)
    {
        return Err(rejected(
            "callback layout geometry differs from its checked source application",
        ));
    }
    Ok(())
}

fn application(
    compilation: &PackageReviewInput<'_>,
    source: &CallbackRecordReviewSource,
    geometry: &CallbackDataGeometry,
    lifetime_binders: &[Identifier],
) -> Result<PackagePolicyCallbackLayoutApplication, Vec<Diagnostic>> {
    let mut projected = CheckingProjection::new(compilation.checking_input);
    let source_reference = source_reference(&mut projected, source)?;
    let identity = validation::structural_type_identity(
        &projected,
        source_reference,
        &[],
        &source.application.substitutions,
    );
    if projected.type_identity_text(identity).as_deref() != Some(source.storage_identity.as_str()) {
        return Err(rejected(
            "callback source record changed its exact storage application",
        ));
    }
    if let Some(plan) = plan(compilation, source)? {
        let selected = selected_geometry(compilation, source, plan)?;
        check_geometry(selected, geometry)?;
    }
    let mut schemas = compilation
        .data_definitions()
        .iter()
        .filter(|definition| definition.symbol == source.schema);
    let schema = schemas
        .next()
        .ok_or_else(|| rejected("callback layout has no exact source schema declaration"))?;
    if schemas.next().is_some() {
        return Err(rejected("callback layout has an ambiguous source schema"));
    }
    let reference = match schema.generic_instance {
        Some(reference) => reference,
        None => projected
            .program_mut()
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: schema.symbol,
                name: schema.name.clone(),
            }),
    };
    Ok(PackagePolicyCallbackLayoutApplication {
        policy: source.policy.is_valid().then(|| nominal_identity(compilation, source.policy)).transpose()?,
        schema: signature_type_identity(compilation, &projected, compilation.custody.sources().exact_toolchain_sources(), reference, &[], lifetime_binders, &[], &[], false, language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface)?,
        byte_size: geometry.size, alignment: geometry.alignment,
    })
}

/// Authenticate the supplied checked report against the original canonical
/// report. This reads existing evidence; it never executes a layout policy.
fn selected_geometry<'a>(
    compilation: &'a PackageReviewInput<'_>,
    source: &CallbackRecordReviewSource,
    plan: &'a PlanLaidLayout,
) -> Result<&'a PlanLaidGeometry, Vec<Diagnostic>> {
    let coordinate = source
        .realization
        .as_ref()
        .ok_or_else(|| rejected("callback source layout lost its exact realization"))?;
    let target = compilation
        .custody
        .execution()
        .selected_target_profile
        .ok_or_else(|| rejected("callback source layout has no selected target"))?;
    let geometry = match &plan.realizations {
        PlanLaidRealizations::Invariant(geometry)
            if !coordinate.member.is_valid() && coordinate.target.is_empty() =>
        {
            geometry
        }
        PlanLaidRealizations::TargetInputs(span) if coordinate.member.is_valid() => {
            let alternatives = compilation
                .plan_laid_target_layouts
                .span(*span)
                .ok_or_else(|| rejected("callback source target family has stale storage"))?;
            if !span.handles().any(|member| member == coordinate.member)
                || alternatives
                    .iter()
                    .filter(|row| row.target.as_str() == target.target_name())
                    .count()
                    != 1
                || !compilation
                    .plan_laid_target_layouts
                    .is_valid(coordinate.member)
            {
                return Err(rejected(
                    "callback source target family lost its unique exact member",
                ));
            }
            let member = compilation.plan_laid_target_layouts.get(coordinate.member);
            if member.target.as_str() != coordinate.target
                || coordinate.target != target.target_name()
                || Some(&member.target_facts) != target.layout_target_facts().as_ref()
            {
                return Err(rejected(
                    "callback source realization changed its exact target facts",
                ));
            }
            &member.geometry
        }
        _ => {
            return Err(rejected(
                "callback source realization changed its invariant or target custody",
            ));
        }
    };
    let program = compilation
        .custody
        .terminal_publication()
        .published()
        .ok_or_else(|| rejected("callback review has no original canonical publication"))?;
    let module = program.artifact().semantic_module();
    let record = module
        .native_layouts
        .record(source.record)
        .ok_or_else(|| rejected("callback review lost its original canonical record"))?;
    let original = module
        .native_layouts
        .for_target(record, target.target_name())
        .map_err(|reason| rejected(&reason))?
        .ok_or_else(|| rejected("callback review lost its original canonical report"))?;
    if geometry.native_layout_report() != original.report {
        return Err(rejected(
            "callback source layout changed its complete selected report",
        ));
    }
    Ok(geometry)
}

fn source_reference(
    projected: &mut CheckingProjection<'_>,
    source: &CallbackRecordReviewSource,
) -> Result<typed_trees::types::TypeReferenceHandle, Vec<Diagnostic>> {
    if source.application.reference.is_valid() {
        return Ok(source.application.reference);
    }
    let data = projected
        .data_definition_by_symbol(source.declaration)
        .ok_or_else(|| rejected("callback source record lost its nominal declaration"))?;
    if !projected.data_type_parameters(data).is_empty() {
        return Err(rejected(
            "callback source record lost its closed application coordinate",
        ));
    }
    let name = data.name.clone();
    Ok(projected
        .program_mut()
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: source.declaration,
            name,
        }))
}
