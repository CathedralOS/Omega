//! Native proof-carrying product admission and the bounded producer sidecar.
//!
//! A native `.proof` sidecar carries the placed-image evidence section
//! (`native_evidence`): the declared executable-text and initialized-data
//! extents inside the published container, the complete placed region
//! inventories over both, and the closed-form realization of every claimed
//! import thunk. Checking replays those legs against the exact artifact
//! bytes — region and gap digests, addresses, fingerprints and the inventory
//! seals — so the section's claims about coverage are verified, not trusted,
//! and each thunk's realized instruction sequence (and on Mach-O the exact
//! binding slot its decoded pointer loads) is verified, not trusted either.
//! Producer custody digests, even when self-consistent, still prove nothing
//! about the behavior of compiler-function regions, so the behavioral legs
//! (instruction rows against the closed target semantics, entries, incoming
//! edges, premise availability and lowering correspondence) keep the verdict
//! at `Incomplete` until their standalone checking exists.

mod native_evidence;

pub use native_evidence::{NativeEvidenceError, NativePlacedImageEvidence};

use terminal_codec::{
    PccDependency, PccGuarantee, PccIncompleteness, PccProductKind, PccProofSidecar,
    PccReceiverPolicy, PccRejection, PccVerificationOutcome, admission_profile_identity,
    pcc_artifact_commitment, terminal_assumption_closure, verify_pcc_claim_fields,
};

/// The guarantee a bounded native sidecar offers: the published bytes carry
/// an executable text and initialized data that are exactly and completely
/// covered by sealed placed-region inventories at declared addresses for the
/// declared target, with every claimed import thunk realized by the target's
/// closed thunk sequence. This is the certificate's byte-coverage and
/// thunk-realization legs, which the evidence honestly establishes — it is
/// deliberately not the behavioral guarantee
/// (`omega.native-verified-executable`), which a sidecar may not claim until
/// standalone native semantics and correspondence checking exists. A receiver
/// policy requiring a guarantee beyond this one rejects at the claim fields,
/// exactly as the contract prescribes.
pub const NATIVE_PLACED_IMAGE_COVERAGE_GUARANTEE: &str = "omega.native-placed-image-coverage.v1";

/// The semantic-profile identity a native sidecar is checked under: the fixed
/// executable semantics of the declared target.
fn native_semantic_profile_identity(target: target::NativeTarget) -> String {
    let architecture = match target.architecture {
        target::Architecture::Aarch64 => "aarch64",
        target::Architecture::X86_64 => "x86_64",
    };
    let format = match target.object_format {
        target::ObjectFormat::Elf => "elf",
        target::ObjectFormat::MachO => "macho",
        target::ObjectFormat::Coff => "coff",
    };
    format!(
        "native-executable.v1:{architecture}-{format}-{}",
        target.pointer_size
    )
}

/// One published artifact/`.proof` companion pair with the exact separate
/// byte sizes the contract requires producer and receiver to report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PccPublicationReceipt {
    pub product: PccProductKind,
    pub artifact_path: std::path::PathBuf,
    pub artifact_byte_len: u64,
    pub sidecar_path: std::path::PathBuf,
    pub sidecar_byte_len: u64,
}

impl std::fmt::Display for PccPublicationReceipt {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let product = match self.product {
            PccProductKind::Psi => "psi",
            PccProductKind::Native => "native",
        };
        write!(
            formatter,
            "{product} proof-carrying pair: {} ({} bytes) + {} ({} bytes)",
            self.artifact_path.display(),
            self.artifact_byte_len,
            self.sidecar_path.display(),
            self.sidecar_byte_len
        )
    }
}

/// Build the bounded native `.proof` sidecar for one retained artifact and
/// the exact bytes about to be published. The bytes are the post-finalization
/// output; any later byte change invalidates the sidecar's artifact
/// commitment and the evidence's declared extents alike. The offered
/// guarantee names only what the placed-image evidence establishes — verified
/// byte coverage and placement plus the closed import-thunk forms — so the
/// sidecar never asserts the behavioral claim it cannot yet discharge;
/// checking still reports that remainder `Incomplete`.
pub fn build_native_proof_sidecar(
    artifact: &crate::RetainedNativeArtifact,
    profile: &proof_admission::AdmissionProfile,
    executable_bytes: &[u8],
) -> Result<PccProofSidecar, String> {
    let evidence = NativePlacedImageEvidence::from_artifact(artifact, executable_bytes)?;
    // The omitted-dependency inventory is the retained module's own
    // installation-reach enumeration, exactly identified — a receiver must
    // independently possess the same material under either product.
    let module = terminal_codec::decode_module(artifact.psi_artifact().semantic_bytes())
        .map_err(|error| format!("cannot decode the retained terminal module: {error}"))?;
    let dependencies = module
        .root_service_reach
        .installation_dependencies
        .iter()
        .map(|dependency| PccDependency::from_installation_reach(dependency, &module.services))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("cannot enumerate the omitted dependencies: {error}"))?;
    PccProofSidecar::new(
        PccProductKind::Native,
        pcc_artifact_commitment(executable_bytes),
        native_semantic_profile_identity(artifact.target()),
        admission_profile_identity(profile),
        vec![PccGuarantee {
            identity: NATIVE_PLACED_IMAGE_COVERAGE_GUARANTEE.to_owned(),
            premises: Vec::new(),
        }],
        evidence.to_bytes(),
        terminal_assumption_closure(),
        dependencies,
    )
    .map_err(|error| format!("cannot encode the native proof sidecar: {error}"))
}

/// Check the envelope and evidence of a native artifact/proof pair without
/// granting native assurance beyond what the evidence establishes.
///
/// Claim-field validation still rejects invalid bindings or receiver-policy
/// violations. A recognized placed-image section is then replayed against the
/// exact artifact bytes — a section that lies about those bytes rejects by
/// name — while an absent or unrecognized section stays `Incomplete`. Even a
/// fully replayed section only establishes exact byte coverage of both
/// declared extents plus the closed realization of its import thunks (and on
/// aarch64 Mach-O the binding slots their decoded pointers load): the
/// behavioral remainder (instruction rows inside compiler regions, entries,
/// incoming edges, premise availability, lowering correspondence) stays
/// `Incomplete` until standalone native semantics and preservation checking
/// exists.
pub fn verify_native_proof_sidecar(
    executable_bytes: &[u8],
    sidecar_bytes: &[u8],
    policy: &PccReceiverPolicy,
) -> PccVerificationOutcome {
    let sidecar = match verify_pcc_claim_fields(executable_bytes, sidecar_bytes, policy) {
        Ok(sidecar) => sidecar,
        Err(outcome) => return outcome,
    };
    if sidecar.product() != PccProductKind::Native {
        return PccVerificationOutcome::Reject(PccRejection::new(
            "product kind",
            "sidecar does not certify a native product",
        ));
    }
    let evidence = match NativePlacedImageEvidence::from_bytes(sidecar.evidence()) {
        Ok(evidence) => evidence,
        Err(NativeEvidenceError::Unsupported) => {
            return PccVerificationOutcome::Incomplete(PccIncompleteness::UnsupportedEvidence {
                product: PccProductKind::Native,
            });
        }
        Err(NativeEvidenceError::Malformed(reason)) => {
            return PccVerificationOutcome::Reject(PccRejection::new("native evidence", reason));
        }
    };
    // The offered semantic profile must be the one the evidence's declared
    // target realizes: a sidecar cannot present aarch64-ELF semantics over
    // x86-64 bytes by relabeling the envelope.
    if native_semantic_profile_identity(evidence.target()) != sidecar.semantic_profile() {
        return PccVerificationOutcome::Reject(PccRejection::new(
            "semantic profile",
            "the evidence's declared target does not realize the offered semantic profile",
        ));
    }
    if let Err(reason) = evidence.replay_against(executable_bytes) {
        return PccVerificationOutcome::Reject(PccRejection::new(
            "native executable inventory",
            reason,
        ));
    }
    PccVerificationOutcome::Incomplete(PccIncompleteness::UnsupportedEvidence {
        product: PccProductKind::Native,
    })
}

/// Standalone receiver checking for one published artifact/`.proof` pair.
///
/// A receiver holds only the artifact bytes, the companion bytes and its own
/// pinned policy — never the source, producer memory, or producer hints about
/// which product the pair certifies. The envelope's declared product kind is
/// the only routing input: a Psi companion replays the bounded terminal
/// verification, while a native companion replays the checkable placed-image
/// leg and stays fail-closed `Incomplete` on the behavioral remainder. A
/// filename or the artifact's own shape never selects the leg, so a Psi
/// sidecar beside native bytes still rejects inside the Psi leg and a
/// relabeled envelope gains nothing.
///
/// This decode exists only to read the declared kind. Each product leg
/// re-decodes the envelope and re-checks every claim field itself, so the
/// early decode trusts nothing the product checker would not independently
/// establish — and a malformed envelope rejects here before any product leg
/// runs.
pub fn verify_published_proof_pair(
    artifact_bytes: &[u8],
    sidecar_bytes: &[u8],
    policy: &PccReceiverPolicy,
) -> PccVerificationOutcome {
    let product = match PccProofSidecar::from_bytes(sidecar_bytes) {
        Ok(sidecar) => sidecar.product(),
        Err(error) => {
            return PccVerificationOutcome::Reject(PccRejection::new(
                "sidecar envelope",
                format!("invalid encoding: {error}"),
            ));
        }
    };
    match product {
        PccProductKind::Psi => {
            terminal_codec::verify_psi_proof_sidecar(artifact_bytes, sidecar_bytes, policy)
        }
        PccProductKind::Native => {
            verify_native_proof_sidecar(artifact_bytes, sidecar_bytes, policy)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        NativePlacedImageEvidence, native_semantic_profile_identity, verify_native_proof_sidecar,
        verify_published_proof_pair,
    };
    use proof_admission::AdmissionProfile;
    use terminal_codec::{
        PccGuarantee, PccIncompleteness, PccProductKind, PccProofSidecar, PccReceiverPolicy,
        PccVerificationOutcome, pcc_artifact_commitment,
    };

    const EXECUTABLE: &[u8] = b"the published executable bytes";

    fn sidecar(product: PccProductKind, artifact: &[u8], evidence: Vec<u8>) -> PccProofSidecar {
        sidecar_with_profile(product, artifact, "semantic-profile".to_owned(), evidence)
    }

    fn sidecar_with_profile(
        product: PccProductKind,
        artifact: &[u8],
        semantic_profile: String,
        evidence: Vec<u8>,
    ) -> PccProofSidecar {
        PccProofSidecar::new(
            product,
            pcc_artifact_commitment(artifact),
            semantic_profile,
            "checker-profile".to_owned(),
            vec![PccGuarantee {
                identity: "guarantee".to_owned(),
                premises: Vec::new(),
            }],
            evidence,
            Vec::new(),
            Vec::new(),
        )
        .expect("a canonical sidecar")
    }

    fn offered_policy(sidecar: &PccProofSidecar) -> PccReceiverPolicy {
        PccReceiverPolicy::for_offered_claim(sidecar, AdmissionProfile::default())
    }

    #[test]
    fn pair_checking_rejects_a_malformed_sidecar_before_any_product_leg() {
        let artifact = b"artifact";
        let psi = sidecar(PccProductKind::Psi, artifact, Vec::new());
        let policy = offered_policy(&psi);
        assert!(matches!(
            verify_published_proof_pair(artifact, b"not a sidecar", &policy),
            PccVerificationOutcome::Reject(ref rejection) if rejection.subject == "sidecar envelope"
        ));
    }

    #[test]
    fn pair_checking_routes_on_the_declared_product_kind() {
        // A Psi companion beside non-artifact bytes rejects inside the Psi
        // leg; a Native companion carrying no recognizable evidence reaches
        // the fail-closed native leg.
        let artifact = b"not a canonical artifact";
        let psi = sidecar(PccProductKind::Psi, artifact, Vec::new());
        let psi_policy = offered_policy(&psi);
        assert!(matches!(
            verify_published_proof_pair(artifact, &psi.to_bytes(), &psi_policy),
            PccVerificationOutcome::Reject(ref rejection) if rejection.subject == "psi artifact"
        ));

        let native = sidecar(PccProductKind::Native, EXECUTABLE, vec![1, 2, 3]);
        let native_policy = offered_policy(&native);
        assert_eq!(
            verify_published_proof_pair(EXECUTABLE, &native.to_bytes(), &native_policy),
            PccVerificationOutcome::Incomplete(PccIncompleteness::UnsupportedEvidence {
                product: PccProductKind::Native,
            })
        );
    }

    #[test]
    fn pair_checking_rejects_bytes_the_sidecar_does_not_commit_to() {
        let native = sidecar(PccProductKind::Native, EXECUTABLE, Vec::new());
        let policy = offered_policy(&native);
        assert!(matches!(
            verify_published_proof_pair(b"different bytes", &native.to_bytes(), &policy),
            PccVerificationOutcome::Reject(ref rejection) if rejection.subject == "artifact bytes"
        ));
    }

    #[test]
    fn native_checking_rejects_a_psi_companion_and_stays_incomplete() {
        let psi = sidecar(PccProductKind::Psi, EXECUTABLE, Vec::new());
        let policy = offered_policy(&psi);
        assert!(matches!(
            verify_native_proof_sidecar(EXECUTABLE, &psi.to_bytes(), &policy),
            PccVerificationOutcome::Reject(ref rejection) if rejection.subject == "product kind"
        ));

        let native = sidecar(PccProductKind::Native, EXECUTABLE, Vec::new());
        let policy = offered_policy(&native);
        assert_eq!(
            verify_native_proof_sidecar(EXECUTABLE, &native.to_bytes(), &policy),
            PccVerificationOutcome::Incomplete(PccIncompleteness::UnsupportedEvidence {
                product: PccProductKind::Native,
            })
        );
    }

    /// A small real inventory over known text, built through the production
    /// placement path so the evidence rows carry honest digests.
    fn placed_inventory(text: &[u8]) -> image::PlacedExecutableRegionInventory {
        let mut image = image::FinalImage::with_capacity(
            target::NativeTarget::host(),
            image::FinalImageMemory {
                text: text.to_vec(),
                ..image::FinalImageMemory::default()
            },
            Default::default(),
            0,
            0,
            0,
        );
        image.executable_regions.push(image::FinalExecutableRegion {
            origin: image::FinalExecutableRegionOrigin::CompilerFunction,
            section_offset: 0,
            byte_count: text.len(),
            symbol: "entry".into(),
            footprint: None,
        });
        image::place_executable_regions(&image, image::FinalImageLayout::default())
            .expect("the fixture region places")
    }

    /// The honest empty data inventory for a text-only fixture image.
    fn empty_data_inventory() -> image::PlacedDataRegionInventory {
        image::place_data_regions(
            &image::FinalImage::with_capacity(
                target::NativeTarget::host(),
                image::FinalImageMemory::default(),
                Default::default(),
                0,
                0,
                0,
            ),
            image::FinalImageLayout::default(),
        )
        .expect("the empty data inventory places")
    }

    #[test]
    fn native_checking_replays_recognized_evidence_before_staying_incomplete() {
        let text = [0xabu8; 16];
        let mut executable = b"container header ".to_vec();
        let text_file_offset = executable.len() as u64;
        executable.extend_from_slice(&text);
        executable.extend_from_slice(b" trailer");

        let evidence = NativePlacedImageEvidence::from_parts(
            target::NativeTarget::host(),
            text_file_offset,
            placed_inventory(&text),
            0,
            empty_data_inventory(),
        );
        let native = sidecar_with_profile(
            PccProductKind::Native,
            &executable,
            native_semantic_profile_identity(target::NativeTarget::host()),
            evidence.to_bytes(),
        );
        let policy = offered_policy(&native);
        // The honest section replays its coverage leg and still reports the
        // behavioral legs unsupported — coverage is not certification.
        assert_eq!(
            verify_native_proof_sidecar(&executable, &native.to_bytes(), &policy),
            PccVerificationOutcome::Incomplete(PccIncompleteness::UnsupportedEvidence {
                product: PccProductKind::Native,
            })
        );

        // A section whose declared extent misses the committed bytes rejects
        // instead of reaching the unsupported-evidence outcome.
        let shifted = NativePlacedImageEvidence::from_parts(
            target::NativeTarget::host(),
            text_file_offset - 1,
            placed_inventory(&text),
            0,
            empty_data_inventory(),
        );
        let forged = sidecar_with_profile(
            PccProductKind::Native,
            &executable,
            native_semantic_profile_identity(target::NativeTarget::host()),
            shifted.to_bytes(),
        );
        let forged_policy = offered_policy(&forged);
        assert!(matches!(
            verify_native_proof_sidecar(&executable, &forged.to_bytes(), &forged_policy),
            PccVerificationOutcome::Reject(ref rejection)
                if rejection.subject == "native executable inventory"
        ));

        // An envelope offering a different target's profile over this
        // evidence rejects: the claim must be the one the evidence realizes.
        let relabeled = sidecar_with_profile(
            PccProductKind::Native,
            &executable,
            "native-executable.v1:aarch64-elf-8".to_owned(),
            evidence.to_bytes(),
        );
        let relabeled_policy = offered_policy(&relabeled);
        assert!(matches!(
            verify_native_proof_sidecar(&executable, &relabeled.to_bytes(), &relabeled_policy),
            PccVerificationOutcome::Reject(ref rejection)
                if rejection.subject == "semantic profile"
        ));

        // A recognized-but-malformed section rejects as invalid evidence.
        let malformed = sidecar(
            PccProductKind::Native,
            &executable,
            b"NPLCIMG1\x02\x00garbage".to_vec(),
        );
        let malformed_policy = offered_policy(&malformed);
        assert!(matches!(
            verify_native_proof_sidecar(&executable, &malformed.to_bytes(), &malformed_policy),
            PccVerificationOutcome::Reject(ref rejection)
                if rejection.subject == "native evidence"
        ));
    }

    #[test]
    fn pair_checking_reports_named_resource_limits_as_incomplete() {
        let artifact = b"artifact bytes beyond the limit";
        let native = sidecar(PccProductKind::Native, artifact, Vec::new());
        let mut policy = offered_policy(&native);
        policy.max_artifact_bytes = 4;
        assert_eq!(
            verify_published_proof_pair(artifact, &native.to_bytes(), &policy),
            PccVerificationOutcome::Incomplete(PccIncompleteness::ArtifactBytes {
                actual: artifact.len() as u64,
                limit: 4,
            })
        );

        let evidenced = sidecar(PccProductKind::Native, artifact, vec![1, 2, 3]);
        let mut policy = offered_policy(&evidenced);
        policy.max_evidence_bytes = 2;
        assert_eq!(
            verify_published_proof_pair(artifact, &evidenced.to_bytes(), &policy),
            PccVerificationOutcome::Incomplete(PccIncompleteness::EvidenceBytes {
                actual: 3,
                limit: 2,
            })
        );
    }
}
