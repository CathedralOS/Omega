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

use crate::digest_encoder::DomainDigestEncoder;
use crate::review_record_encoding::{
    authority_class_tag, declaration_kind_tag, purpose_tag, semantic_binding_role_tag,
};
use crate::{
    AcceptedSemanticBinding, BuildSnapshotCapture, BuildSnapshotRequest,
    BuildSourceCaptureObligation, IndependentComponentDescription, PackageCompilationInputs,
    PackageDependencyClosure, PackageGeneratedSourceBundle,
};
use build_output::{CapturedBuildSourceInput, CapturedSourceEntryKind};
use filesystem_authority::CanonicalFilesystemMetadataIndex;

const INPUTS_DIGEST_DOMAIN: &[u8] = b"OMEGA-COMPILED-REVIEW-CACHE-INPUTS\x02";
const SNAPSHOT_DIGEST_DOMAIN: &[u8] = b"OMEGA-COMPILED-REVIEW-CACHE-SNAPSHOT\x01";

/// Content digest of the complete compilation inputs a review pass reads:
/// the shared source graph's canonical names, authenticated content
/// commitments, and reconciled edge maps, plus the checked target's purpose,
/// generated-source bundles, accepted semantic bindings, and independent
/// component descriptions.
pub fn review_cache_inputs_digest(inputs: &PackageCompilationInputs) -> [u8; 32] {
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(INPUTS_DIGEST_DOMAIN);
    digest.raw(&inputs.root().digest());
    digest.byte(declaration_kind_tag(inputs.root_role()));
    for (identity, _root) in inputs.packages() {
        digest.raw(&identity.digest());
        digest.bytes(inputs.package_name(identity).unwrap_or_default().as_bytes());
        match inputs.canonical_source_metadata(identity) {
            Some(index) => {
                digest.byte(1);
                hash_metadata_index(&mut digest, index);
            }
            None => digest.byte(0),
        }
    }
    for (requester, alias, target) in inputs.dependencies() {
        digest.byte(0);
        digest.raw(&requester.digest());
        digest.bytes(alias.as_bytes());
        digest.raw(&target.digest());
    }
    digest.byte(1);
    for (requester, alias, target) in inputs.build_dependencies() {
        digest.raw(&requester.digest());
        digest.bytes(alias.as_bytes());
        digest.raw(&target.digest());
    }
    digest.byte(purpose_tag(inputs.compilation_purpose()));
    digest.byte(u8::from(inputs.semantic_role_discovery()));
    for bundle in inputs.dependency_generated_source_bundles() {
        hash_generated_bundle(&mut digest, bundle);
    }
    for binding in inputs.accepted_semantic_bindings() {
        hash_accepted_binding(&mut digest, binding);
    }
    for description in inputs.independent_component_descriptions() {
        hash_component_description(&mut digest, description);
    }
    digest.finish()
}

/// Content digest of one build snapshot request: the declared required
/// outputs, the capture view, and every named input's authenticated content.
pub fn review_cache_snapshot_digest(snapshot: &BuildSnapshotRequest) -> [u8; 32] {
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(SNAPSHOT_DIGEST_DOMAIN);
    digest.u64(snapshot.required_outputs().len() as u64);
    for required in snapshot.required_outputs() {
        digest.bytes(required);
    }
    match snapshot.capture() {
        BuildSnapshotCapture::PackageInventory => digest.byte(0),
        BuildSnapshotCapture::Scoped(request) => {
            digest.byte(1);
            for (path, obligation) in request.entries() {
                digest.bytes(path);
                digest.byte(match obligation {
                    BuildSourceCaptureObligation::Required => 0,
                    BuildSourceCaptureObligation::Optional => 1,
                });
            }
        }
    };
    for (name, input) in snapshot.inputs() {
        digest.bytes(name);
        hash_captured_input(&mut digest, input);
    }
    digest.finish()
}

pub(crate) fn hash_metadata_index(
    digest: &mut DomainDigestEncoder,
    index: &CanonicalFilesystemMetadataIndex,
) {
    digest.u32(index.policy_version());
    digest.raw(index.source_content_commitment());
    digest.u64(index.rows().len() as u64);
    for row in index.rows() {
        digest.bytes(row.relative_path());
        match row.kind() {
            filesystem_authority::CanonicalFilesystemMetadataRowKind::Directory => {
                digest.byte(0);
            }
            filesystem_authority::CanonicalFilesystemMetadataRowKind::File {
                logical_byte_length,
                executable,
            } => {
                digest.byte(1);
                digest.u64(logical_byte_length);
                digest.byte(executable as u8);
            }
            filesystem_authority::CanonicalFilesystemMetadataRowKind::Symlink {
                target_spelling_logical_byte_length,
            } => {
                digest.byte(2);
                digest.u64(target_spelling_logical_byte_length);
            }
        }
    }
}

fn hash_captured_input(digest: &mut DomainDigestEncoder, input: &CapturedBuildSourceInput) {
    hash_metadata_index(digest, input.canonical_source_metadata());
    digest.u64(input.file_bytes());
    digest.u64(input.entries().len() as u64);
    for entry in input.entries() {
        digest.bytes(entry.relative_path());
        match entry.kind() {
            CapturedSourceEntryKind::Directory => digest.byte(0),
            CapturedSourceEntryKind::File {
                byte_length,
                executable,
            } => {
                digest.byte(1);
                digest.u64(byte_length);
                digest.byte(executable as u8);
                if let Some(file) = input.file(entry.relative_path()) {
                    digest.raw(&crate::digest_encoder::content_identity(&file.bytes()));
                }
            }
            CapturedSourceEntryKind::Symlink { target } => {
                digest.byte(2);
                digest.raw(&crate::digest_encoder::content_identity(&target));
            }
        };
    }
}

fn hash_dependency_closure(digest: &mut DomainDigestEncoder, closure: &PackageDependencyClosure) {
    digest.raw(&closure.root().digest());
    digest.byte(declaration_kind_tag(closure.root_role()));
    digest.u64(closure.packages().len() as u64);
    for package in closure.packages() {
        digest.raw(&package.digest());
    }
    digest.u64(closure.dependencies().len() as u64);
    for binding in closure.dependencies() {
        digest.raw(&binding.requester().digest());
        digest.bytes(binding.alias().as_bytes());
        digest.raw(&binding.target().digest());
        digest.byte(purpose_tag(binding.purpose()));
    }
}

fn hash_generated_bundle(digest: &mut DomainDigestEncoder, bundle: &PackageGeneratedSourceBundle) {
    digest.raw(&bundle.package().digest());
    digest.byte(purpose_tag(bundle.purpose()));
    digest.byte(crate::target_profile_tags::target_profile_tag(
        bundle.target(),
    ));
    match bundle.build_execution_profile() {
        Some(profile) => {
            digest.byte(1);
            digest.byte(crate::target_profile_tags::target_profile_tag(profile));
        }
        None => digest.byte(0),
    };
    hash_dependency_closure(digest, bundle.dependency_closure());
    digest.raw(&bundle.source_consumption_commitment().digest());
    digest.u64(bundle.sources().len() as u64);
    for source in bundle.sources() {
        digest.bytes(source.relative_path());
        digest.raw(&source.digest());
    }
}

fn hash_accepted_binding(digest: &mut DomainDigestEncoder, binding: &AcceptedSemanticBinding) {
    digest.byte(semantic_binding_role_tag(binding.role()));
    digest.raw(&binding.package().digest());
    digest.bytes(binding.declaration_path().as_bytes());
    digest.raw(binding.normalized_schema_digest().as_bytes());
    match binding.selected_provider_plan_digest() {
        Some(plan) => {
            digest.byte(1);
            digest.raw(plan.as_bytes());
        }
        None => digest.byte(0),
    };
    digest.u64(binding.terminal_authority_permissions().len() as u64);
    for permission in binding.terminal_authority_permissions() {
        digest.raw(permission.service_schema().as_bytes());
        digest.bytes(
            binding
                .terminal_authority_permission_identities()
                .text(permission.requirement_identity())
                .unwrap_or_default()
                .as_bytes(),
        );
        for class in permission.permitted().classes() {
            digest.byte(authority_class_tag(*class));
        }
    }
}

fn hash_component_description(
    digest: &mut DomainDigestEncoder,
    description: &IndependentComponentDescription,
) {
    digest.raw(&description.package().digest());
    let subject = description.expected_subject();
    digest.raw(subject.program_fingerprint.as_bytes());
    digest.bytes(description.description());
}
