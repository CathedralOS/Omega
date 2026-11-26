//! Identity of a prepared dependency and the products its Build supplies.
//! Staged Build receipts retain these commitments when a parent resumes.

use crate::digest_encoder::DomainDigestEncoder;
use crate::{
    AcceptedSemanticBinding, IndependentComponentDescription, PackageBuildHandoff,
    PackageCompilationInputs, PackageDependencyClosure,
};
use filesystem_authority::CanonicalFilesystemMetadataIndex;

// Previously issued Build receipts bind this domain.
const INPUTS_DIGEST_DOMAIN: &[u8] = b"OMEGA-COMPILED-REVIEW-CACHE-INPUTS\x07";

impl PackageBuildHandoff {
    pub fn identity_digest(&self) -> [u8; 32] {
        let mut digest = DomainDigestEncoder::new(b"omega-package-build-products-v5\0");
        hash_generated_bundle(&mut digest, self);
        digest.finish()
    }
}

impl PackageCompilationInputs {
    pub fn identity_digest(&self) -> [u8; 32] {
        let mut digest = DomainDigestEncoder::new(INPUTS_DIGEST_DOMAIN);
        digest.raw(&self.root().digest());
        digest.byte(self.root_role().canonical_tag());
        for (identity, _root) in self.packages() {
            digest.raw(&identity.digest());
            self.hash_build_entries(identity, &mut digest);
            digest.bytes(self.package_name(identity).unwrap_or_default().as_bytes());
            match self.resolution(identity) {
                Some(resolution) => {
                    digest.byte(1);
                    digest.raw(&resolution);
                }
                None => digest.byte(0),
            }
            match self.canonical_source_metadata(identity) {
                Some(index) => {
                    digest.byte(1);
                    hash_metadata_index(&mut digest, index);
                }
                None => digest.byte(0),
            }
        }
        for purpose in build_declarations::DependencyPurpose::ALL {
            digest.byte(purpose.canonical_tag());
            let uses = self.package_uses(purpose).collect::<Vec<_>>();
            digest.u64(uses.len() as u64);
            for edge in uses {
                hash_edge(&mut digest, &edge);
            }
        }
        digest.byte(self.compilation_purpose().canonical_tag());
        for bundle in self.dependency_build_handoffs() {
            hash_generated_bundle(&mut digest, bundle);
        }
        for binding in self.accepted_semantic_bindings() {
            hash_accepted_binding(&mut digest, binding);
        }
        for description in self.independent_component_descriptions().values() {
            hash_component_description(&mut digest, description);
        }
        digest.finish()
    }
}

fn hash_metadata_index(digest: &mut DomainDigestEncoder, index: &CanonicalFilesystemMetadataIndex) {
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

fn hash_edge(digest: &mut DomainDigestEncoder, edge: &crate::PackageDependencyBinding) {
    digest.raw(&edge.requester().digest());
    digest.bytes(edge.alias().as_bytes());
    digest.raw(&edge.target().digest());
    digest.byte(edge.purpose().canonical_tag());
    digest.u64(edge.max_reach().services().count() as u64);
    for service in edge.max_reach().services() {
        digest.bytes(service.as_bytes());
    }
}

fn hash_dependency_closure(digest: &mut DomainDigestEncoder, closure: &PackageDependencyClosure) {
    digest.raw(&closure.root().digest());
    digest.byte(closure.root_role().canonical_tag());
    digest.u64(closure.packages().len() as u64);
    for package in closure.packages() {
        digest.raw(&package.digest());
    }
    digest.u64(closure.dependencies().len() as u64);
    for binding in closure.dependencies() {
        hash_edge(digest, binding);
    }
}

fn hash_generated_bundle(digest: &mut DomainDigestEncoder, bundle: &PackageBuildHandoff) {
    digest.raw(&bundle.package().digest());
    digest.byte(bundle.purpose().canonical_tag());
    digest.byte(target_profile_tag(bundle.target()));
    match bundle.build_execution_profile() {
        Some(profile) => {
            digest.byte(1);
            digest.byte(target_profile_tag(profile));
        }
        None => digest.byte(0),
    };
    hash_dependency_closure(digest, bundle.dependency_closure());
    digest.raw(&bundle.source_consumption_commitment().digest());
    digest.raw(
        &super::provider_identity::package_provider_selections_digest(
            bundle.provider_selections(),
            bundle.selection_observation(),
        ),
    );
    digest.raw(&super::native_output_identity::digest(bundle));
    digest.u64(bundle.sources().len() as u64);
    for source in bundle.sources() {
        digest.bytes(source.relative_path());
        digest.raw(&source.digest());
    }
}

fn hash_accepted_binding(digest: &mut DomainDigestEncoder, binding: &AcceptedSemanticBinding) {
    digest.byte(binding.role().canonical_tag());
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
            digest.byte(class.canonical_tag());
        }
    }
}

fn hash_component_description(
    digest: &mut DomainDigestEncoder,
    description: &IndependentComponentDescription,
) {
    digest.raw(&description.package().digest());
    let subject = description.expected_subject();
    digest.u64(u64::from(subject.vocabulary_marker.get()));
    digest.raw(subject.program_fingerprint.as_bytes());
    digest.u64(description.independent_suppliers().len() as u64);
    for supplier in description.independent_suppliers() {
        digest.raw(&supplier.digest());
    }
    digest.bytes(description.description());
}

fn target_profile_tag(profile: target::TargetProfile) -> u8 {
    profile.identity_tag()
}
