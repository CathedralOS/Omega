//! Substitutable-field inventory for the installed-realization custody
//! matrix driven by `tests.rs`'s
//! `installed_realization_rejects_every_one_field_substitution`.
//!
//! The declared lanes are the `RealizationSpec` record's representable
//! custody axes: every provider-visible choice behind one installed
//! realization that the occurrence digest or a lifecycle receipt binds.
//! Keeping the inventory in its own file names the boundary between what is
//! declared substitutable and the shared `run_one_field_substitution_matrix`
//! driver that exercises every leg.
//!
//! `extent_issuance` is deliberately not a lane: the provider-issuance
//! origin is not retained in placement evidence, so substituting it
//! canonicalizes to the identical realization — an informational axis the
//! test names explicitly rather than declaring here.

use super::{RealizationSpec, authentic_spec, spec_constraints};
use crate::artifacts::container::{
    ArtifactRelocationKind, DecodedArtifactRelocation, normalized_proof_payload_digest,
};
use crate::artifacts::{InstallationAudience, RetainedContainerProof};
use crate::installation::WxEnforcement;
use crate::test_support::entry_id;
use layout_plans::{PlacementPhase, RelocationTarget};
use target::Architecture;

optimization_core::custody_field_inventory! {
    /// One substitutable lane of the `RealizationSpec` record; checked by the
    /// registry replay plus lifecycle joins in
    /// `installed_realization_rejects_every_one_field_substitution`.
    pub enum InstalledRealizationCustodyFieldForTest {
        ArtifactCodeBytes,
        ArtifactArchitecture,
        ArtifactIdentity,
        ArtifactEntrySet,
        ArtifactEntryIdentity,
        ArtifactEntryCodeOffset,
        ArtifactRelocationRoster,
        AdmissionReceipt,
        ContainerProofDropped,
        ContainerProofDigest,
        ContainerProofBytes,
        InstalledIdentity,
        PlacementIdentity,
        InstallationScope,
        ConstraintInstallationScope,
        InstallationAudience,
        PermittedRangeDropped,
        PermittedRangeStart,
        PermittedRangeEnd,
        PlacementAlignment,
        PlacementPhase,
        PlacementMachineRegime,
        PlacementBase,
        PlacementLength,
        PlacementAddressSpace,
        PlacementRightsRoster,
        PlacementProvenance,
        PlacementMappingEra,
        ExtentLineage,
        RealizedFootprint,
        FinalValidationIdentity,
        WxEnforcement,
    }
}

/// The named independent-checker verdict for a substituted realization.
/// The one-shot registry authority replays the complete installed evidence
/// against the authentic record it burned on, so every representable
/// substitution fails the same first seam; the driver's `joined_replay`
/// then asserts the remaining lifecycle seams reject in both directions.
#[derive(Debug, PartialEq)]
pub enum InstalledRealizationCustodyCheck {
    /// `registry.matches` refuses to bind the substituted realization to the
    /// authentic occurrence.
    RegistryMismatch,
}

/// An authentic foreign spec: a different placement identity realizes a
/// different installed occurrence, so its retained custody differs from the
/// honest record the matrix builds. It supplies the donor placement the
/// `PlacementIdentity` leg substitutes.
pub fn foreign_realization_spec_donor() -> RealizationSpec {
    let mut spec = authentic_spec();
    spec.placement = 108;
    spec
}

/// The family's honest-recomputation hook: mutate exactly the declared spec
/// field, keeping every other provider-visible choice honest, so the
/// re-realized occurrence digest recomputes over the substitution.
/// `donor` supplies the foreign placement identity the `PlacementIdentity`
/// leg borrows; the remaining legs take fixed non-identity alternates.
pub fn substitute_installed_realization_for_test(
    spec: &mut RealizationSpec,
    field: InstalledRealizationCustodyFieldForTest,
    donor: &RealizationSpec,
) {
    use InstalledRealizationCustodyFieldForTest as Leg;
    match field {
        Leg::ArtifactCodeBytes => spec.code[0] ^= 1,
        Leg::ArtifactArchitecture => spec.architecture = Architecture::Aarch64,
        Leg::ArtifactIdentity => spec.artifact = 2,
        Leg::ArtifactEntrySet => spec.entry_set = 35,
        Leg::ArtifactEntryIdentity => spec.entry = 1002,
        Leg::ArtifactEntryCodeOffset => spec.entry_offset = 24,
        Leg::ArtifactRelocationRoster => {
            spec.relocations.push(DecodedArtifactRelocation {
                kind: ArtifactRelocationKind::Absolute64,
                destination_offset: 8,
                target: RelocationTarget::Entry(entry_id(1001)),
                addend: 0,
            });
            spec.resolve = |_| Some(0x9000);
        }
        Leg::AdmissionReceipt => spec.admission_receipt = 41,
        Leg::ContainerProofDropped => spec.container_proof = None,
        Leg::ContainerProofDigest => {
            spec.container_proof = Some(RetainedContainerProof {
                digest: normalized_proof_payload_digest(b"forged proof payload"),
                bytes: b"container proof payload".to_vec(),
            });
        }
        Leg::ContainerProofBytes => {
            spec.container_proof = Some(RetainedContainerProof {
                digest: normalized_proof_payload_digest(b"container proof payload"),
                bytes: b"forged proof payload".to_vec(),
            });
        }
        Leg::InstalledIdentity => spec.installed = 282,
        Leg::PlacementIdentity => spec.placement = donor.placement,
        // A scope the plan never recorded must restate both the authority's
        // scope identity and the artifact constraint's scope claim.
        Leg::InstallationScope => {
            spec.scope = 62;
            spec.constraints = spec_constraints(
                Some((0x1000, 0x1_0000)),
                4096,
                PlacementPhase::PostHandoff,
                None,
                Some(62),
            );
        }
        Leg::ConstraintInstallationScope => {
            spec.constraints = spec_constraints(
                Some((0x1000, 0x1_0000)),
                4096,
                PlacementPhase::PostHandoff,
                None,
                None,
            );
        }
        Leg::InstallationAudience => {
            spec.audience = InstallationAudience::DormantLocal;
        }
        Leg::PermittedRangeDropped => {
            spec.constraints =
                spec_constraints(None, 4096, PlacementPhase::PostHandoff, None, Some(61));
        }
        Leg::PermittedRangeStart => {
            spec.constraints = spec_constraints(
                Some((0x800, 0x1_0000)),
                4096,
                PlacementPhase::PostHandoff,
                None,
                Some(61),
            );
        }
        Leg::PermittedRangeEnd => {
            spec.constraints = spec_constraints(
                Some((0x1000, 0x2_0000)),
                4096,
                PlacementPhase::PostHandoff,
                None,
                Some(61),
            );
        }
        Leg::PlacementAlignment => {
            spec.constraints = spec_constraints(
                Some((0x1000, 0x1_0000)),
                2048,
                PlacementPhase::PostHandoff,
                None,
                Some(61),
            );
        }
        Leg::PlacementPhase => {
            spec.constraints = spec_constraints(
                Some((0x1000, 0x1_0000)),
                4096,
                PlacementPhase::Load,
                None,
                Some(61),
            );
        }
        Leg::PlacementMachineRegime => {
            spec.constraints = spec_constraints(
                Some((0x1000, 0x1_0000)),
                4096,
                PlacementPhase::PostHandoff,
                Some(10),
                Some(61),
            );
        }
        Leg::PlacementBase => spec.extent_base = 0x8000,
        Leg::PlacementLength => spec.extent_length = 8192,
        Leg::PlacementAddressSpace => spec.extent_space = 54,
        Leg::PlacementRightsRoster => spec.extent_rights = vec![51, 55],
        Leg::PlacementProvenance => spec.extent_provenance = 56,
        Leg::PlacementMappingEra => spec.extent_era = 57,
        Leg::ExtentLineage => spec.extent_lineage = 117,
        Leg::RealizedFootprint => spec.realized_footprint = 72,
        Leg::FinalValidationIdentity => spec.validation = 181,
        Leg::WxEnforcement => spec.wx = WxEnforcement::ConventionOnly,
    }
}

/// The exact checker verdict each declared leg must produce: every
/// representable substitution diverges the complete installed evidence, so
/// the registry authority's replay refuses to bind it.
pub fn installed_realization_custody_outcome(
    _field: InstalledRealizationCustodyFieldForTest,
) -> optimization_core::MutationOutcome<InstalledRealizationCustodyCheck> {
    optimization_core::MutationOutcome::ExactError(
        InstalledRealizationCustodyCheck::RegistryMismatch,
    )
}
