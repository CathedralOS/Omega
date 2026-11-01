//! Attach geometry-free review correspondence to a canonical calling input.
//! This operation does not prepare value recipes, classify Source types, build
//! layouts or select private destinations. Those belong to the canonical owner.

use super::callback_layout_catalog::BoundaryCallbackLayoutEntry;
use super::{
    BoundaryCallbackBinder, BoundaryNativeParameter, BoundaryOpaqueRepresentationUse,
    CanonicalCallingSignature, MaterializedBoundarySignature,
};
use checked_trees::typed_trees::SourceRecordCorrespondence;
use checked_trees::typed_trees::service_declarations::signatures::BoundarySignatureDeclaration;
use representation_planning::OpaqueRepresentationSelection;

pub(crate) fn attach(
    canonical: CanonicalCallingSignature,
    module: &terminal_psi::TerminalModule,
    declaration: &BoundarySignatureDeclaration,
    typed: &checked_trees::typed_trees::TypedTrees,
    source_records: &[SourceRecordCorrespondence],
    selections: &[OpaqueRepresentationSelection],
    target: target::TargetProfile,
) -> Result<MaterializedBoundarySignature, String> {
    if canonical.requirement_identity != declaration.requirement_identity {
        return Err("calling review declaration differs from its exact canonical ABI".into());
    }
    let prepared = declaration.projection.as_ref().map_err(Clone::clone)?;
    let geometry = &canonical.policy.geometry;
    let callback_binders = canonical
        .policy
        .callback_binders()?
        .iter()
        .map(|binder| {
            let ordinal = prepared
                .telescope
                .binders
                .iter()
                .position(|source| source.binder == binder.binder)
                .ok_or("calling review lost its exact original callback binder")?;
            let original = &prepared.telescope.binders[ordinal];
            let source = prepared
                .callback_sources
                .get(ordinal)
                .ok_or("calling review lost its source binder telescope")?;
            if original.requirement_identity != binder.requirement_identity
                || original.static_machine_ordinal != binder.static_machine_ordinal
                || original.requirement != binder.requirement
                || original.invokes_synchronously != binder.invokes_synchronously
            {
                return Err("canonical callback binder differs from its source declaration".into());
            }
            Ok(BoundaryCallbackBinder {
                binder: binder.binder,
                requirement: binder.requirement,
                static_machine_ordinal: binder.static_machine_ordinal,
                requirement_identity: binder.requirement_identity.clone(),
                invokes_synchronously: binder.invokes_synchronously,
                parameter_symbol: source.parameter_symbol,
                requirement_trait: source.requirement_trait,
                requirement_machine: source.requirement_machine,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let native_parameters = canonical
        .policy
        .native_parameters()?
        .iter()
        .map(|parameter| BoundaryNativeParameter {
            identity: parameter.identity,
            native_ordinal: parameter.native_ordinal,
            shape: parameter.shape,
            origin: parameter.origin,
        })
        .collect();
    let mut callback_layout_catalog = Vec::new();
    for geometry in geometry
        .callback_layouts
        .span(geometry.callback_layout_rows)
        .ok_or("canonical callback layout storage is stale")?
    {
        let coordinates = canonical
            .callback_paths
            .iter()
            .find(|row| row.destination == geometry.destination)
            .ok_or("canonical callback layout lost its producer coordinates")?;
        callback_layout_catalog.push(BoundaryCallbackLayoutEntry::from_canonical(
            module,
            geometry.clone(),
            coordinates,
            source_records,
            target,
        )?);
    }
    let mut opaque_representations = Vec::with_capacity(canonical.opaque_uses.len());
    for occurrence in &canonical.opaque_uses {
        let mut selected = None;
        for selection in selections {
            if selection.portable_application(typed)? == occurrence.application {
                if selected.is_some() {
                    return Err(
                        "canonical opaque occurrence has competing source selections".into(),
                    );
                }
                selected = Some(selection);
            }
        }
        let selection = selected
            .ok_or("canonical opaque occurrence lost its exact original source selection")?;
        opaque_representations.push(BoundaryOpaqueRepresentationUse {
            opaque: selection.opaque(),
            carrier: selection.carrier(),
            conformance: selection.application().declaration,
            shape_root: occurrence.shape_root,
            application_report_fingerprint: occurrence.application.application_report_fingerprint,
            conformance_application_commitment: occurrence
                .application
                .conformance_application_commitment,
            representation_schema_version: occurrence.application.schema_version,
            origin: occurrence.application.origin,
            lifecycle: occurrence.application.lifecycle,
            copy_disposition: occurrence.application.copy_disposition,
            selected_application_commitment: occurrence.application.selected_application_commitment,
        });
    }
    let super::CallingPolicySignature {
        target,
        geometry,
        callbacks,
    } = canonical.policy;
    Ok(MaterializedBoundarySignature {
        owner_requirement_identity: canonical.requirement_identity,
        native_target: target,
        opaque_representations,
        shapes: geometry.shapes,
        fields: geometry.fields,
        parameters: geometry.parameters,
        result: geometry.result,
        callback_binders,
        callback_demands: callbacks.demands,
        callback_layout_catalog,
        native_parameters,
        direct_callback_parameters: prepared.telescope.direct_callbacks.clone(),
    })
}
