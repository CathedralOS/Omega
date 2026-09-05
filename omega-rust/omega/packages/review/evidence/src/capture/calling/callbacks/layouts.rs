use super::rejected;
use crate::capture::semantics::declarations::nominal_identity;
use crate::capture::semantics::types::review_signature_type_identity_with_binders;
use crate::project_checked_conformance_policy;
use crate::record::{
    PackagePolicyCallbackInlineField, PackagePolicyCallbackLayout,
    PackagePolicyCallbackLayoutApplication,
};
use compiler::CheckedCompilation;
use diagnostics::Diagnostic;
use layout::TargetClosedPlanLaidDataLayoutIdentity;
use provider_planning::calling_policy_plans::BoundaryCallbackLayoutEntry;
use typed_trees::name::Identifier;
use typed_trees::typed_trees::PlanLaidLayout;
use typed_trees::types::TypeReferenceNode;

pub(super) fn project(
    compilation: &CheckedCompilation,
    entry: &BoundaryCallbackLayoutEntry,
    lifetime_binders: &[Identifier],
) -> Result<PackagePolicyCallbackLayout, Vec<Diagnostic>> {
    let root = plan(compilation, entry.root_layout())?;
    let inline_field = entry
        .inline_field()
        .map(|field| {
            let indices = root
                .field_symbols
                .iter()
                .enumerate()
                .filter(|(_, symbol)| **symbol == field.symbol())
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            let [index] = indices.as_slice() else {
                return Err(rejected(
                    "callback inline field is not uniquely owned by its root layout",
                ));
            };
            let source_symbol =
                root.schema_field_symbols
                    .get(*index)
                    .copied()
                    .ok_or_else(|| {
                        rejected("callback inline field lost its exact schema declaration")
                    })?;
            Ok(PackagePolicyCallbackInlineField {
                field: nominal_identity(compilation, source_symbol)?,
                offset: quantity(field.offset())?,
                extent: quantity(field.extent())?,
                alignment: quantity(field.alignment())?,
                child_layout: application(compilation, field.child_layout(), lifetime_binders)?,
            })
        })
        .transpose()?;
    let terminal = entry.terminal_slot();
    Ok(PackagePolicyCallbackLayout {
        formal_ordinal: entry.formal_ordinal(),
        native_ordinal: entry.native_ordinal(),
        root_layout: application(compilation, entry.root_layout(), lifetime_binders)?,
        inline_field,
        terminal_slot: project_checked_conformance_policy(
            compilation,
            &terminal.slot_application,
            lifetime_binders,
        )?,
        terminal_offset: quantity(terminal.offset)?,
        terminal_byte_size: quantity(terminal.byte_size)?,
        terminal_alignment: quantity(terminal.alignment)?,
        composed_offset: quantity(entry.composed_offset())?,
    })
}

fn plan<'a>(
    compilation: &'a CheckedCompilation,
    layout: &TargetClosedPlanLaidDataLayoutIdentity,
) -> Result<&'a PlanLaidLayout, Vec<Diagnostic>> {
    let matches = compilation
        .plan_laid_layouts
        .iter()
        .filter(|plan| plan.data_symbol == layout.data_symbol)
        .collect::<Vec<_>>();
    let [plan] = matches.as_slice() else {
        return Err(rejected(
            "callback layout lost its unique checked application",
        ));
    };
    if plan.size != layout.physical.size || plan.align != layout.physical.alignment {
        return Err(rejected(
            "callback layout geometry differs from its checked application",
        ));
    }
    Ok(plan)
}

fn application(
    compilation: &CheckedCompilation,
    layout: &TargetClosedPlanLaidDataLayoutIdentity,
    lifetime_binders: &[Identifier],
) -> Result<PackagePolicyCallbackLayoutApplication, Vec<Diagnostic>> {
    let plan = plan(compilation, layout)?;
    let schemas = compilation
        .data_definitions()
        .iter()
        .filter(|definition| definition.symbol == plan.schema_symbol)
        .collect::<Vec<_>>();
    let [schema] = schemas.as_slice() else {
        return Err(rejected("callback layout has no exact schema declaration"));
    };
    let mut projected = compilation.clone();
    let reference = match schema.generic_instance {
        Some(reference) => reference,
        None => projected
            .typed
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: schema.symbol,
                name: schema.name.clone(),
            }),
    };
    Ok(PackagePolicyCallbackLayoutApplication {
        policy: nominal_identity(compilation, plan.policy_symbol)?,
        schema: review_signature_type_identity_with_binders(
            &projected,
            reference,
            &[],
            lifetime_binders,
        )?,
        byte_size: quantity(layout.physical.size)?,
        alignment: quantity(layout.physical.alignment)?,
    })
}

fn quantity(value: usize) -> Result<u64, Vec<Diagnostic>> {
    u64::try_from(value).map_err(|_| rejected("callback geometry exceeds u64"))
}
