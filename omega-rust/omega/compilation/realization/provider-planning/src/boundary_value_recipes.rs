//! Source attribution beside native boundary storage, never inside layout.
//!
//! Psi receives only source type correspondence. Native layout reports each
//! exact recipe ordinal and shape root; this owner rejoins that occurrence to
//! the same explicit Build selection roster for package review and publication.
//! A geometry report grants no representation or boundary authority.

use crate::calling_policy_plans::BoundaryOpaqueRepresentationUse;
use representation_planning::OpaqueRepresentationSelection;

pub(crate) fn representation_types(
    selections: &[OpaqueRepresentationSelection],
) -> Vec<checked_trees::typed_trees::boundary_values::BoundaryRepresentationType> {
    selections
        .iter()
        .map(
            |selection| checked_trees::typed_trees::boundary_values::BoundaryRepresentationType {
                opaque: selection.opaque(),
                carrier: selection.carrier_type(),
            },
        )
        .collect()
}

pub(crate) fn representation_uses(
    selections: &[OpaqueRepresentationSelection],
    occurrences: &[layout::BoundaryOpaqueOccurrence],
) -> Result<Vec<BoundaryOpaqueRepresentationUse>, String> {
    occurrences
        .iter()
        .map(|occurrence| {
            let selection = selections
                .get(occurrence.selection_ordinal)
                .ok_or("boundary opaque recipe has no corresponding selected representation")?;
            let application = selection.application();
            Ok(BoundaryOpaqueRepresentationUse {
                opaque: selection.opaque(),
                conformance: application.declaration,
                carrier: selection.carrier(),
                shape_root: occurrence.shape_root,
                application_report_fingerprint: application.report_fingerprint,
                conformance_application_commitment: application.commitment.as_bytes(),
                representation_schema_version: selection.schema_version(),
                origin: selection.origin(),
                lifecycle: selection.lifecycle(),
                copy_disposition: selection.copy_disposition(),
                selected_application_commitment: selection.selected_application_commitment(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use representation_planning::{
        OpaqueRepresentationApplicationOrigin, OpaqueRepresentationCopyDisposition,
        OpaqueRepresentationLifecycleDisposition, OpaqueRepresentationSelection,
    };

    #[test]
    fn occurrence_join_keeps_exact_selection_and_distinct_native_roots() {
        // These are descriptive join inputs, not source-admission evidence.
        let selection = |report, digest, copy| {
            OpaqueRepresentationSelection::from_validated_application(
                symbols::SymbolHandle::invalid(),
                symbols::SymbolHandle::invalid(),
                checked_trees::typed_trees::types::TypeReferenceHandle::invalid(),
                checked_trees::typed_trees::typed_trees::ClosedConformanceApplication {
                    report_fingerprint: report,
                    commitment: checked_trees::typed_trees::typed_trees::ClosedConformanceApplicationCommitment::from_digest([digest; 32]),
                    ..Default::default()
                },
                OpaqueRepresentationLifecycleDisposition::Inert, copy,
                OpaqueRepresentationApplicationOrigin::NamedConformance,
                symbols::SymbolHandle::invalid(), source::SourceSpan::default(),
            )
        };
        let selections = [
            selection(11, 1, OpaqueRepresentationCopyDisposition::PlacementOnly),
            selection(
                22,
                2,
                OpaqueRepresentationCopyDisposition::CheckedSemanticCopy,
            ),
        ];
        let occurrences = [
            layout::BoundaryOpaqueOccurrence {
                selection_ordinal: 1,
                shape_root: 7,
            },
            layout::BoundaryOpaqueOccurrence {
                selection_ordinal: 0,
                shape_root: 2,
            },
            layout::BoundaryOpaqueOccurrence {
                selection_ordinal: 1,
                shape_root: 9,
            },
        ];
        let uses =
            super::representation_uses(&selections, &occurrences).expect("exact roster join");
        for (use_, occurrence) in uses.iter().zip(&occurrences) {
            let selected = &selections[occurrence.selection_ordinal];
            assert_eq!(use_.shape_root(), occurrence.shape_root);
            assert_eq!(
                use_.application_report_fingerprint(),
                selected.application().report_fingerprint
            );
            assert_eq!(
                use_.conformance_application_commitment(),
                selected.application().commitment.as_bytes()
            );
            assert_eq!(
                use_.selected_application_commitment(),
                selected.selected_application_commitment()
            );
            assert_eq!(use_.copy_disposition(), selected.copy_disposition());
            assert_eq!(
                use_.selected_application_commitment(),
                use_.rederived_selected_application_commitment()
            );
        }
        assert_ne!(uses[0].shape_root(), uses[2].shape_root());
    }

    #[test]
    fn native_occurrence_without_its_source_selection_cannot_publish_custody() {
        let error = super::representation_uses(
            &[],
            &[layout::BoundaryOpaqueOccurrence {
                selection_ordinal: 41,
                shape_root: 3,
            }],
        )
        .expect_err("geometry is not a selected representation");
        assert_eq!(
            error,
            "boundary opaque recipe has no corresponding selected representation"
        );
    }
}
