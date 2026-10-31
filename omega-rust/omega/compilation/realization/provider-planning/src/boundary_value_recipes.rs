//! Preserve a checked representation roster while Psi closes carrier types.
//!
//! Source correspondence and native descriptive occurrences have the same
//! order. The recipe ordinal is local to this roster, never a new selection or
//! an admission receipt. The existing application commitments remain intact.

pub(crate) fn representation_inputs(
    selections: &[representation_planning::OpaqueRepresentationSelection],
) -> (
    Vec<checked_trees::typed_trees::boundary_values::BoundaryRepresentationType>,
    Vec<layout::BoundaryOpaqueRepresentationUse>,
) {
    selections
        .iter()
        .map(|selection| {
            let application = selection.application();
            (
                checked_trees::typed_trees::boundary_values::BoundaryRepresentationType {
                    opaque: selection.opaque(),
                    carrier: selection.carrier_type(),
                },
                layout::BoundaryOpaqueRepresentationUse {
                    opaque: selection.opaque(),
                    conformance: application.declaration,
                    carrier: selection.carrier(),
                    // Materialization replaces this with the exact occurrence.
                    shape_root: 0,
                    application_report_fingerprint: application.report_fingerprint,
                    conformance_application_commitment: application.commitment.as_bytes(),
                    representation_schema_version: selection.schema_version(),
                    origin: selection.origin(),
                    lifecycle: selection.lifecycle(),
                    copy_disposition: selection.copy_disposition(),
                    selected_application_commitment: selection.selected_application_commitment(),
                },
            )
        })
        .unzip()
}
