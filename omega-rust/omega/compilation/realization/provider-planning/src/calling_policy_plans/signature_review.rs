// We prepare review addresses (`PreparedCallingReview` and
// `PreparedOpaqueReviewSelection`) while Source exists and attach them to
// canonical calling geometry (`CanonicalCallingSignature`) without reopening
// Source. Refusals remain deferred until an actual policy result demands the
// corresponding address, and these associations grant neither invocation,
// layout, nor selected-representation authority.

use super::callback_layout_catalog::BoundaryCallbackLayoutEntry;
use super::{
    BoundaryCallbackBinder, BoundaryNativeParameter, BoundaryOpaqueRepresentationUse,
    CanonicalCallingSignature, MaterializedBoundarySignature,
};
use checked_trees::typed_trees::SourceRecordCorrespondence;
use checked_trees::typed_trees::service_declarations::signatures::BoundarySignatureDeclaration;
use std::sync::Arc;

// One selected opaque representation as review needs it: the portable
// application that Terminal Psi also carries, which is the join key, beside the
// in-process symbols a review report names. The owner of the selection records
// projects these rows while Source exists. A selection whose portable
// application could not be formed keeps its refusal, which surfaces only if a
// canonical signature actually uses an opaque representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedOpaqueReviewSelection {
    pub application:
        Result<language_semantics::opaque_representations::SelectedOpaqueApplication, String>,
    pub opaque: symbols::SymbolHandle,
    pub carrier: symbols::SymbolHandle,
    pub conformance: symbols::SymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PreparedCallingReview {
    requirement_identity: String,
    callback_binders: Vec<BoundaryCallbackBinder>,
    direct_callbacks: Vec<super::BoundaryDirectCallbackParameter>,
    opaque_selections: Arc<[PreparedOpaqueReviewSelection]>,
}

impl PreparedCallingReview {
    pub(super) fn prepare(
        declaration: &BoundarySignatureDeclaration,
        opaque_selections: Arc<[PreparedOpaqueReviewSelection]>,
    ) -> Result<Self, String> {
        let prepared = declaration.projection.as_ref().map_err(Clone::clone)?;
        let mut callback_binders = Vec::with_capacity(prepared.telescope.binders.len());
        for (ordinal, binder) in prepared.telescope.binders.iter().enumerate() {
            let source = prepared
                .callback_sources
                .get(ordinal)
                .ok_or("calling review lost its source binder telescope")?;
            callback_binders.push(BoundaryCallbackBinder {
                binder: binder.binder,
                requirement: binder.requirement,
                static_machine_ordinal: binder.static_machine_ordinal,
                requirement_identity: binder.requirement_identity.clone(),
                invokes_synchronously: binder.invokes_synchronously,
                parameter_symbol: source.parameter_symbol,
                requirement_trait: source.requirement_trait,
                requirement_machine: source.requirement_machine,
            });
        }
        Ok(Self {
            requirement_identity: declaration.requirement_identity.clone(),
            callback_binders,
            direct_callbacks: prepared.telescope.direct_callbacks.clone(),
            opaque_selections,
        })
    }

    pub(super) fn attach(
        &self,
        canonical: CanonicalCallingSignature,
        module: &terminal_psi::TerminalModule,
        source_records: &[SourceRecordCorrespondence],
        target: target::TargetProfile,
    ) -> Result<MaterializedBoundarySignature, String> {
        if canonical.requirement_identity != self.requirement_identity {
            return Err("calling review declaration differs from its exact canonical ABI".into());
        }
        let geometry = &canonical.policy.geometry;
        let callback_binders = canonical
            .policy
            .callback_binders()?
            .iter()
            .map(|binder| {
                let original = self
                    .callback_binders
                    .iter()
                    .find(|source| source.binder == binder.binder)
                    .ok_or("calling review lost its exact original callback binder")?;
                if original.requirement_identity != binder.requirement_identity
                    || original.static_machine_ordinal != binder.static_machine_ordinal
                    || original.requirement != binder.requirement
                    || original.invokes_synchronously != binder.invokes_synchronously
                {
                    return Err(
                        "canonical callback binder differs from its source declaration".into(),
                    );
                }
                Ok(original.clone())
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
            let selection = self.opaque_review(&occurrence.application)?;
            opaque_representations.push(BoundaryOpaqueRepresentationUse {
                opaque: selection.opaque,
                carrier: selection.carrier,
                conformance: selection.conformance,
                shape_root: occurrence.shape_root,
                application_report_fingerprint: occurrence
                    .application
                    .application_report_fingerprint,
                conformance_application_commitment: occurrence
                    .application
                    .conformance_application_commitment,
                representation_schema_version: occurrence.application.schema_version,
                origin: occurrence.application.origin,
                lifecycle: occurrence.application.lifecycle,
                copy_disposition: occurrence.application.copy_disposition,
                selected_application_commitment: occurrence
                    .application
                    .selected_application_commitment,
            });
        }
        let super::CallingPolicySignature {
            target,
            geometry,
            callbacks,
        } = canonical.policy;
        Ok(MaterializedBoundarySignature {
            abi: canonical.abi,
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
            direct_callback_parameters: self.direct_callbacks.clone(),
        })
    }

    fn opaque_review(
        &self,
        application: &language_semantics::opaque_representations::SelectedOpaqueApplication,
    ) -> Result<&PreparedOpaqueReviewSelection, String> {
        let mut selected = None;
        for selection in self.opaque_selections.iter() {
            if selection.application.as_ref().map_err(Clone::clone)? == application {
                if selected.is_some() {
                    return Err(
                        "canonical opaque occurrence has competing source selections".into(),
                    );
                }
                selected = Some(selection);
            }
        }
        selected.ok_or_else(|| {
            "canonical opaque occurrence lost its exact original source selection".into()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use language_semantics::opaque_representations::{
        OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION, OpaqueRepresentationApplicationOrigin,
        OpaqueRepresentationCopyDisposition, OpaqueRepresentationLifecycleDisposition,
        SelectedOpaqueApplication, selected_application_commitment,
    };

    fn application() -> SelectedOpaqueApplication {
        let conformance = [7; 32];
        SelectedOpaqueApplication {
            opaque_identity: "Opaque".into(),
            carrier_identity: "Bytes".into(),
            conformance_identity: "Representation".into(),
            application_report_fingerprint: 7,
            conformance_application_commitment: conformance,
            identity_encoding: vec![1, 2, 3],
            schema_version: OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION,
            origin: OpaqueRepresentationApplicationOrigin::NamedConformance,
            lifecycle: OpaqueRepresentationLifecycleDisposition::Inert,
            copy_disposition: OpaqueRepresentationCopyDisposition::PlacementOnly,
            selected_application_commitment: selected_application_commitment(
                conformance,
                OpaqueRepresentationLifecycleDisposition::Inert,
                OpaqueRepresentationCopyDisposition::PlacementOnly,
                OpaqueRepresentationApplicationOrigin::NamedConformance,
            ),
        }
    }

    fn review(rows: Vec<Result<SelectedOpaqueApplication, String>>) -> PreparedCallingReview {
        PreparedCallingReview {
            requirement_identity: "Boundary::call".into(),
            callback_binders: vec![],
            direct_callbacks: vec![],
            opaque_selections: rows
                .into_iter()
                .map(|application| PreparedOpaqueReviewSelection {
                    application,
                    opaque: symbols::SymbolHandle::invalid(),
                    carrier: symbols::SymbolHandle::invalid(),
                    conformance: symbols::SymbolHandle::invalid(),
                })
                .collect::<Vec<_>>()
                .into(),
        }
    }

    #[test]
    fn opaque_review_consumes_deferred_results_in_original_order() {
        let expected = application();
        let refused = review(vec![
            Err("first original refusal".into()),
            Ok(expected.clone()),
        ]);
        assert_eq!(
            refused.opaque_review(&expected).err().as_deref(),
            Some("first original refusal")
        );
        let refused = review(vec![
            Ok(expected.clone()),
            Ok(expected.clone()),
            Err("later refusal".into()),
        ]);
        assert_eq!(
            refused.opaque_review(&expected).err().as_deref(),
            Some("canonical opaque occurrence has competing source selections")
        );
    }

    #[test]
    fn opaque_review_joins_the_complete_application_not_its_report() {
        let expected = application();
        let mut other = expected.clone();
        other.identity_encoding.push(4);
        let prepared = review(vec![Ok(other.clone()), Ok(expected.clone())]);
        let selected = prepared
            .opaque_review(&expected)
            .expect("one exact complete application");
        assert_eq!(selected.application.as_ref().unwrap(), &expected);
        assert_eq!(
            prepared
                .opaque_review(&other)
                .unwrap()
                .application
                .as_ref()
                .unwrap(),
            &other
        );
        let mut missing = expected;
        missing.carrier_identity = "OtherBytes".into();
        assert!(prepared.opaque_review(&missing).is_err());
    }
}
