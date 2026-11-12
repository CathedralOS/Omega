/*
We define the target-neutral `OpaqueRepresentationSelection` record and its
domain-separated commitment schema (`selected_application_commitment` and
`OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION`).
*/

/*
Our durable selection record captures one build-validated
`Name satisfies OpaqueRepresentation<Opaque, Carrier>` relationship together
with its `Inert` lifecycle and `PlacementOnly` or `CheckedSemanticCopy`
disposition. Separating this representation from build-planning admission allows
downstream checking, package review, and target layout layers to consume and
replay selected opaque applications without depending on the build planner.
*/

/*
`representation-selections` is produced by `representation-planning` during
authoritative build evaluation and consumed by `typed-trees-to-typed-trees`
(Stage 04), `validation` and `typed-trees-to-checked-trees` (Stage 05). Omega
provider planning does not read these records: the compiler and Build project
the review rows and carrier pairings it needs.
*/

#![forbid(unsafe_code)]

pub mod representation_selections {
    pub use language_semantics::opaque_representations::{
        OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION, OpaqueRepresentationApplicationOrigin,
        OpaqueRepresentationCopyDisposition, OpaqueRepresentationLifecycleDisposition,
        selected_application_commitment,
    };
    use source::SourceSpan;
    use symbols::SymbolHandle;
    use typed_trees::typed_trees::ClosedConformanceApplication;

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct OpaqueRepresentationSelection {
        opaque: SymbolHandle,
        carrier: SymbolHandle,
        carrier_type: typed_trees::types::TypeReferenceHandle,
        application: ClosedConformanceApplication,
        lifecycle: OpaqueRepresentationLifecycleDisposition,
        copy_disposition: OpaqueRepresentationCopyDisposition,
        origin: OpaqueRepresentationApplicationOrigin,
        selected_application_commitment: [u8; 32],
        selecting_machine: SymbolHandle,
        source_span: SourceSpan,
    }

    impl OpaqueRepresentationSelection {
        pub fn portable_application(
            &self,
            typed: &typed_trees::TypedTrees,
        ) -> Result<language_semantics::opaque_representations::SelectedOpaqueApplication, String>
        {
            Ok(
                language_semantics::opaque_representations::SelectedOpaqueApplication {
                    opaque_identity: typed.normalized_hermetic_symbol_identity(self.opaque())?,
                    carrier_identity: typed.normalized_hermetic_symbol_identity(self.carrier())?,
                    conformance_identity: typed
                        .normalized_hermetic_symbol_identity(self.application().declaration)?,
                    application_report_fingerprint: self.application().report_fingerprint,
                    conformance_application_commitment: self.application().commitment.as_bytes(),
                    identity_encoding: self.application().identity_encoding.to_vec(),
                    schema_version: self.schema_version(),
                    origin: self.origin(),
                    lifecycle: self.lifecycle(),
                    copy_disposition: self.copy_disposition(),
                    selected_application_commitment: self.selected_application_commitment(),
                },
            )
        }
        pub fn from_validated_application(
            opaque: SymbolHandle,
            carrier: SymbolHandle,
            carrier_type: typed_trees::types::TypeReferenceHandle,
            application: ClosedConformanceApplication,
            lifecycle: OpaqueRepresentationLifecycleDisposition,
            copy_disposition: OpaqueRepresentationCopyDisposition,
            origin: OpaqueRepresentationApplicationOrigin,
            selecting_machine: SymbolHandle,
            source_span: SourceSpan,
        ) -> Self {
            let selected_application_commitment = selected_application_commitment(
                application.commitment.as_bytes(),
                lifecycle,
                copy_disposition,
                origin,
            );
            Self {
                opaque,
                carrier,
                carrier_type,
                application,
                lifecycle,
                copy_disposition,
                origin,
                selected_application_commitment,
                selecting_machine,
                source_span,
            }
        }

        pub fn opaque(&self) -> SymbolHandle {
            self.opaque
        }

        pub fn carrier(&self) -> SymbolHandle {
            self.carrier
        }

        pub fn carrier_type(&self) -> typed_trees::types::TypeReferenceHandle {
            self.carrier_type
        }

        pub fn application(&self) -> &ClosedConformanceApplication {
            &self.application
        }

        pub fn lifecycle(&self) -> OpaqueRepresentationLifecycleDisposition {
            self.lifecycle
        }

        pub fn copy_disposition(&self) -> OpaqueRepresentationCopyDisposition {
            self.copy_disposition
        }

        pub fn origin(&self) -> OpaqueRepresentationApplicationOrigin {
            self.origin
        }

        pub const fn schema_version(&self) -> u16 {
            OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION
        }

        pub const fn selected_application_commitment(&self) -> [u8; 32] {
            self.selected_application_commitment
        }

        pub fn rederived_selected_application_commitment(&self) -> [u8; 32] {
            selected_application_commitment(
                self.application.commitment.as_bytes(),
                self.lifecycle,
                self.copy_disposition,
                self.origin,
            )
        }

        pub fn selecting_machine(&self) -> SymbolHandle {
            self.selecting_machine
        }

        pub fn source_span(&self) -> SourceSpan {
            self.source_span
        }
    }

    pub fn selection_for_opaque(
        selections: &[OpaqueRepresentationSelection],
        opaque: SymbolHandle,
    ) -> Option<&OpaqueRepresentationSelection> {
        selections
            .iter()
            .find(|selection| selection.opaque == opaque)
    }

    #[cfg(test)]
    mod tests {
        use super::{
            OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION,
            OpaqueRepresentationApplicationOrigin, OpaqueRepresentationCopyDisposition,
            OpaqueRepresentationLifecycleDisposition, OpaqueRepresentationSelection,
            selected_application_commitment,
        };
        use source::SourceSpan;
        use symbols::SymbolHandle;
        use typed_trees::typed_trees::{
            ClosedConformanceApplication, ClosedConformanceApplicationCommitment,
        };

        #[test]
        fn selected_application_commitment_binds_copy_disposition() {
            let conformance = [0x5a; 32];
            let placement = selected_application_commitment(
                conformance,
                OpaqueRepresentationLifecycleDisposition::Inert,
                OpaqueRepresentationCopyDisposition::PlacementOnly,
                OpaqueRepresentationApplicationOrigin::NamedConformance,
            );
            let copying = selected_application_commitment(
                conformance,
                OpaqueRepresentationLifecycleDisposition::Inert,
                OpaqueRepresentationCopyDisposition::CheckedSemanticCopy,
                OpaqueRepresentationApplicationOrigin::NamedConformance,
            );
            assert_ne!(placement, copying);
            assert_ne!(placement, conformance);
            assert_ne!(copying, conformance);
        }

        #[test]
        fn selected_application_commitment_binds_application_commitment() {
            let base = selected_application_commitment(
                [0x5a; 32],
                OpaqueRepresentationLifecycleDisposition::Inert,
                OpaqueRepresentationCopyDisposition::PlacementOnly,
                OpaqueRepresentationApplicationOrigin::NamedConformance,
            );
            let foreign = selected_application_commitment(
                [0xab; 32],
                OpaqueRepresentationLifecycleDisposition::Inert,
                OpaqueRepresentationCopyDisposition::PlacementOnly,
                OpaqueRepresentationApplicationOrigin::NamedConformance,
            );
            assert_ne!(base, foreign);
        }

        #[test]
        fn stored_commitment_substitution_diverges_from_honest_recomputation() {
            let application = ClosedConformanceApplication {
                commitment: ClosedConformanceApplicationCommitment::from_digest([0x5a; 32]),
                ..Default::default()
            };
            let honest = OpaqueRepresentationSelection::from_validated_application(
                SymbolHandle::invalid(),
                SymbolHandle::invalid(),
                Default::default(),
                application,
                OpaqueRepresentationLifecycleDisposition::Inert,
                OpaqueRepresentationCopyDisposition::PlacementOnly,
                OpaqueRepresentationApplicationOrigin::NamedConformance,
                SymbolHandle::invalid(),
                SourceSpan::default(),
            );
            assert_eq!(
                honest.schema_version(),
                OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION
            );
            assert_eq!(
                honest.selected_application_commitment(),
                honest.rederived_selected_application_commitment(),
            );
            let forged = OpaqueRepresentationSelection {
                selected_application_commitment: [0xab; 32],
                ..honest.clone()
            };
            assert_ne!(
                forged.selected_application_commitment(),
                forged.rederived_selected_application_commitment(),
                "a substituted stored commitment must diverge from the field recomputation",
            );
            assert_ne!(forged, honest);
        }
    }
}

pub use representation_selections::{
    OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION, OpaqueRepresentationApplicationOrigin,
    OpaqueRepresentationCopyDisposition, OpaqueRepresentationLifecycleDisposition,
    OpaqueRepresentationSelection, selected_application_commitment, selection_for_opaque,
};
