//! Content digests over review-pass inputs for the persisted compiled-review
//! cache.
//!
//! An in-session carry compares `PackageCompilationInputs` and
//! `BuildSnapshotRequest` structurally, which includes checkout-absolute
//! roots; the persisted key instead digests each package's content-authenticated
//! commitment plus every semantic attachment a compile reads. Physical roots
//! deliberately stay out: equal content under a moved checkout is the same
//! compilation input. The digest binds semantics only — custody and
//! consumption are still re-verified on every hit by the carrying pass.

use crate::review_record_encoding::{
    authority_class_tag, declaration_kind_tag, hash_framed, purpose_tag, semantic_binding_role_tag,
};
use crate::{
    AcceptedSemanticBinding, BuildDependencyOccurrence, BuildSnapshotCapture, BuildSnapshotRequest,
    BuildSourceCaptureObligation, IndependentComponentDescription, PackageCompilationInputs,
    PackageDependencyClosure, PackageGeneratedSourceBundle,
};
use build_output::{CapturedBuildSourceInput, CapturedSourceEntryKind};
use checked_interpreter::CanonicalFilesystemMetadataIndex;
use sha2::{Digest, Sha256};

const INPUTS_DIGEST_DOMAIN: &[u8] = b"OMEGA-COMPILED-REVIEW-CACHE-INPUTS\x02";
const SNAPSHOT_DIGEST_DOMAIN: &[u8] = b"OMEGA-COMPILED-REVIEW-CACHE-SNAPSHOT\x01";

/// Content digest of the complete compilation inputs a review pass reads:
/// the shared source graph's canonical names, authenticated content
/// commitments, and reconciled edge maps, plus the checked target's purpose,
/// generated-source bundles, accepted semantic bindings, and independent
/// component descriptions.
pub fn review_cache_inputs_digest(inputs: &PackageCompilationInputs) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(INPUTS_DIGEST_DOMAIN);
    digest.update(inputs.root().digest());
    digest.update([declaration_kind_tag(inputs.root_role())]);
    for (identity, _root) in inputs.packages() {
        digest.update(identity.digest());
        hash_framed(
            &mut digest,
            inputs.package_name(identity).unwrap_or_default().as_bytes(),
        );
        match inputs.canonical_source_metadata(identity) {
            Some(index) => {
                digest.update([1]);
                hash_metadata_index(&mut digest, index);
            }
            None => digest.update([0]),
        }
    }
    for (requester, alias, target) in inputs.dependencies() {
        digest.update([0]);
        digest.update(requester.digest());
        hash_framed(&mut digest, alias.as_bytes());
        digest.update(target.digest());
    }
    digest.update([1]);
    for (requester, alias, target) in inputs.build_dependencies() {
        digest.update(requester.digest());
        hash_framed(&mut digest, alias.as_bytes());
        digest.update(target.digest());
    }
    digest.update([purpose_tag(inputs.compilation_purpose())]);
    digest.update([u8::from(inputs.semantic_role_discovery())]);
    for bundle in inputs.dependency_generated_source_bundles() {
        hash_generated_bundle(&mut digest, bundle);
    }
    for binding in inputs.accepted_semantic_bindings() {
        hash_accepted_binding(&mut digest, binding);
    }
    for description in inputs.independent_component_descriptions() {
        hash_component_description(&mut digest, description);
    }
    digest.finalize().into()
}

/// Content digest of one build snapshot request: the declared required
/// outputs, the capture view, and every named input's authenticated content.
pub fn review_cache_snapshot_digest(snapshot: &BuildSnapshotRequest) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(SNAPSHOT_DIGEST_DOMAIN);
    digest.update((snapshot.required_outputs().len() as u64).to_le_bytes());
    for required in snapshot.required_outputs() {
        hash_framed(&mut digest, required);
    }
    match snapshot.capture() {
        BuildSnapshotCapture::PackageInventory => digest.update([0]),
        BuildSnapshotCapture::Scoped(request) => {
            digest.update([1]);
            for (path, obligation) in request.entries() {
                hash_framed(&mut digest, path);
                digest.update([match obligation {
                    BuildSourceCaptureObligation::Required => 0,
                    BuildSourceCaptureObligation::Optional => 1,
                }]);
            }
        }
    };
    for (name, input) in snapshot.inputs() {
        hash_framed(&mut digest, name);
        hash_captured_input(&mut digest, input);
    }
    for (occurrence, inputs) in snapshot.dependency_inputs() {
        hash_dependency_occurrence(&mut digest, occurrence);
        digest.update((inputs.len() as u64).to_le_bytes());
        for (name, input) in inputs {
            hash_framed(&mut digest, name);
            hash_captured_input(&mut digest, input);
        }
    }
    digest.finalize().into()
}

pub(crate) fn hash_metadata_index(digest: &mut Sha256, index: &CanonicalFilesystemMetadataIndex) {
    digest.update(index.policy_version().to_le_bytes());
    digest.update(index.source_content_commitment());
    digest.update((index.rows().len() as u64).to_le_bytes());
    for row in index.rows() {
        hash_framed(digest, row.relative_path());
        match row.kind() {
            checked_interpreter::CanonicalFilesystemMetadataRowKind::Directory => {
                digest.update([0]);
            }
            checked_interpreter::CanonicalFilesystemMetadataRowKind::File {
                logical_byte_length,
                executable,
            } => {
                digest.update([1]);
                digest.update(logical_byte_length.to_le_bytes());
                digest.update([executable as u8]);
            }
            checked_interpreter::CanonicalFilesystemMetadataRowKind::Symlink {
                target_spelling_logical_byte_length,
            } => {
                digest.update([2]);
                digest.update(target_spelling_logical_byte_length.to_le_bytes());
            }
        }
    }
}

fn hash_captured_input(digest: &mut Sha256, input: &CapturedBuildSourceInput) {
    hash_metadata_index(digest, input.canonical_source_metadata());
    digest.update(input.file_bytes().to_le_bytes());
    digest.update((input.entries().len() as u64).to_le_bytes());
    for entry in input.entries() {
        hash_framed(digest, entry.relative_path());
        match entry.kind() {
            CapturedSourceEntryKind::Directory => digest.update([0]),
            CapturedSourceEntryKind::File {
                byte_length,
                executable,
            } => {
                digest.update([1]);
                digest.update(byte_length.to_le_bytes());
                digest.update([executable as u8]);
                if let Some(file) = input.file(entry.relative_path()) {
                    digest.update(Sha256::digest(file.bytes()));
                }
            }
            CapturedSourceEntryKind::Symlink { target } => {
                digest.update([2]);
                digest.update(Sha256::digest(target));
            }
        };
    }
}

fn hash_dependency_occurrence(digest: &mut Sha256, occurrence: &BuildDependencyOccurrence) {
    digest.update(occurrence.requester().digest());
    digest.update([purpose_tag(occurrence.purpose())]);
    hash_framed(digest, occurrence.alias().as_bytes());
    digest.update(occurrence.target().digest());
}

fn hash_dependency_closure(digest: &mut Sha256, closure: &PackageDependencyClosure) {
    digest.update(closure.root().digest());
    digest.update([declaration_kind_tag(closure.root_role())]);
    digest.update((closure.packages().len() as u64).to_le_bytes());
    for package in closure.packages() {
        digest.update(package.digest());
    }
    digest.update((closure.dependencies().len() as u64).to_le_bytes());
    for binding in closure.dependencies() {
        digest.update(binding.requester().digest());
        hash_framed(digest, binding.alias().as_bytes());
        digest.update(binding.target().digest());
        digest.update([purpose_tag(binding.purpose())]);
    }
}

fn hash_generated_bundle(digest: &mut Sha256, bundle: &PackageGeneratedSourceBundle) {
    digest.update(bundle.package().digest());
    digest.update([purpose_tag(bundle.purpose())]);
    digest.update([crate::production_subject::target_profile_tag(
        bundle.target(),
    )]);
    match bundle.build_execution_profile() {
        Some(profile) => digest.update([1, crate::production_subject::target_profile_tag(profile)]),
        None => digest.update([0]),
    };
    hash_dependency_closure(digest, bundle.dependency_closure());
    digest.update(bundle.source_consumption_commitment().digest());
    digest.update((bundle.sources().len() as u64).to_le_bytes());
    for source in bundle.sources() {
        hash_framed(digest, source.relative_path());
        digest.update(source.digest());
    }
}

fn hash_accepted_binding(digest: &mut Sha256, binding: &AcceptedSemanticBinding) {
    digest.update([semantic_binding_role_tag(binding.role())]);
    digest.update(binding.package().digest());
    hash_framed(digest, binding.declaration_path().as_bytes());
    digest.update(binding.normalized_schema_digest().as_bytes());
    match binding.selected_provider_plan_digest() {
        Some(plan) => {
            digest.update([1]);
            digest.update(plan.as_bytes());
        }
        None => digest.update([0]),
    };
    digest.update((binding.terminal_authority_permissions().len() as u64).to_le_bytes());
    for permission in binding.terminal_authority_permissions() {
        digest.update(permission.service_schema().as_bytes());
        hash_framed(digest, permission.requirement_identity().as_bytes());
        for class in permission.permitted().classes() {
            digest.update([authority_class_tag(*class)]);
        }
    }
}

fn hash_component_description(digest: &mut Sha256, description: &IndependentComponentDescription) {
    digest.update(description.package().digest());
    let subject = description.expected_subject();
    digest.update(subject.program_fingerprint.as_bytes());
    hash_framed(digest, description.description());
}
