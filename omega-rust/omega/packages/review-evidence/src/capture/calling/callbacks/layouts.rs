use crate::capture::PackageReviewInput;
use crate::capture::calling::rejected;
use crate::capture::project_checked_conformance_policy;
use crate::capture::semantics::declarations::nominal_identity;
use crate::capture::semantics::types::signature_type_identity;
use crate::record::{
    PackagePolicyCallbackInlineField, PackagePolicyCallbackLayout,
    PackagePolicyCallbackLayoutApplication,
};
use diagnostics::Diagnostic;
use layout::TargetClosedDataLayoutIdentity;
use provider_planning::calling_policy_plans::BoundaryCallbackLayoutEntry;
use typed_trees::name::Identifier;
use typed_trees::typed_trees::PlanLaidLayout;
use typed_trees::types::TypeReferenceNode;
use validation::CheckingProjection;

pub(super) fn project(
    compilation: &PackageReviewInput<'_>,
    entry: &BoundaryCallbackLayoutEntry,
    lifetime_binders: &[Identifier],
) -> Result<PackagePolicyCallbackLayout, Vec<Diagnostic>> {
    let mut parent = entry.root_layout();
    let mut inline_fields = Vec::new();
    for field in entry.inline_fields().map_err(|reason| rejected(&reason))? {
        let source_symbol = if let Some(root) = plan(compilation, parent)? {
            let indices = root
                .field_symbols
                .iter()
                .enumerate()
                .filter(|(_, symbol)| **symbol == field.symbol())
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            let [index] = indices.as_slice() else {
                return Err(rejected(
                    "callback inline field is not uniquely owned by its parent layout",
                ));
            };
            root.schema_field_symbols
                .get(*index)
                .copied()
                .ok_or_else(|| {
                    rejected("callback inline field lost its exact schema declaration")
                })?
        } else {
            // The layout producer retained this exact natural record field;
            // its nominal declaration remains the package-review identity.
            field.symbol()
        };
        inline_fields.push(PackagePolicyCallbackInlineField {
            field: nominal_identity(compilation, source_symbol)?,
            offset: quantity(field.offset())?,
            extent: quantity(field.extent())?,
            alignment: quantity(field.alignment())?,
            child_layout: application(compilation, field.child_layout(), lifetime_binders)?,
        });
        parent = field.child_layout();
    }
    let terminal = entry.terminal_slot();
    Ok(PackagePolicyCallbackLayout {
        formal_ordinal: entry.formal_ordinal(),
        native_ordinal: entry.native_ordinal(),
        root_layout: application(compilation, entry.root_layout(), lifetime_binders)?,
        inline_fields,
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
    compilation: &'a PackageReviewInput<'a>,
    layout: &TargetClosedDataLayoutIdentity,
) -> Result<Option<&'a PlanLaidLayout>, Vec<Diagnostic>> {
    if layout.layout_subject_identity.is_none() {
        return Ok(None);
    }
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
    let geometry = plan.invariant_geometry().ok_or_else(|| {
        rejected("callback layout requires an exact target-family geometry closure")
    })?;
    if geometry.size != layout.physical.size || geometry.align != layout.physical.alignment {
        return Err(rejected(
            "callback layout geometry differs from its checked application",
        ));
    }
    Ok(Some(plan))
}

fn application(
    compilation: &PackageReviewInput<'_>,
    layout: &TargetClosedDataLayoutIdentity,
    lifetime_binders: &[Identifier],
) -> Result<PackagePolicyCallbackLayoutApplication, Vec<Diagnostic>> {
    let plan = plan(compilation, layout)?;
    let schemas = compilation
        .data_definitions()
        .iter()
        .filter(|definition| {
            definition.symbol == plan.map_or(layout.data_symbol, |plan| plan.schema_symbol)
        })
        .collect::<Vec<_>>();
    let [schema] = schemas.as_slice() else {
        return Err(rejected("callback layout has no exact schema declaration"));
    };
    let mut projected = CheckingProjection::new(compilation.checking_input);
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
        policy: plan.map(|plan| nominal_identity(compilation, plan.policy_symbol)).transpose()?,
        schema: signature_type_identity(
            compilation,
            &projected,
            compilation.custody.sources().exact_toolchain_sources(),
            reference,
            &[],
            lifetime_binders,
            &[],
            &[],
            false,
            language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface,
        )?,
        byte_size: quantity(layout.physical.size)?,
        alignment: quantity(layout.physical.alignment)?,
    })
}

fn quantity(value: usize) -> Result<u64, Vec<Diagnostic>> {
    u64::try_from(value).map_err(|_| rejected("callback geometry exceeds u64"))
}
