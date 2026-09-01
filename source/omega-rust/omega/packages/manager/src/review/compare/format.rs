//! Fixed-vocabulary rendering and canonical conflict tags.

use super::model::{
    ReviewOnlyCapabilityConflictChange, ReviewOnlyCapabilityConflictSet,
    ReviewOnlyPackageCapabilityConflicts, ReviewSetRole,
};
use omega_package_evidence::record::{
    PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk, PackageReviewCanonicalRowSource,
    PackageReviewSourceLocationOwner, PackageReviewSourceLocationRole,
    PackageReviewSyntheticSourceKind,
};
use omega_package_source::ImmutableSourceResolution;
use sha2::{Digest, Sha256};

const CONFLICT_RENDER_SCHEMA: &str = "OMEGA_PACKAGE_CAPABILITY_CONFLICTS_V19\n";

pub(super) trait ConflictRenderOutput {
    fn push_str(&mut self, value: &str);

    fn push(&mut self, value: char) {
        let mut bytes = [0; 4];
        self.push_str(value.encode_utf8(&mut bytes));
    }
}

impl ConflictRenderOutput for String {
    fn push_str(&mut self, value: &str) {
        String::push_str(self, value);
    }
}

#[derive(Default)]
pub(super) struct RenderByteCounter {
    pub(super) bytes: usize,
}

impl ConflictRenderOutput for RenderByteCounter {
    fn push_str(&mut self, value: &str) {
        self.bytes = self.bytes.saturating_add(value.len());
    }
}

pub(super) fn render_conflict_set(
    output: &mut impl ConflictRenderOutput,
    set: &ReviewOnlyCapabilityConflictSet,
) {
    output.push_str(CONFLICT_RENDER_SCHEMA);
    output.push_str("package_count ");
    output.push_str(&set.packages.len().to_string());
    output.push('\n');
    for package in &set.packages {
        render_package(output, package);
    }
    output.push_str("end_capability_conflicts\n");
}

fn render_package(
    output: &mut impl ConflictRenderOutput,
    package: &ReviewOnlyPackageCapabilityConflicts,
) {
    output.push_str("package_begin\npackage_name ");
    output.push_str(package.key.name().as_str());
    output.push_str("\npackage_key ");
    push_hex(output, &package.key.identity().digest());
    output.push('\n');
    match &package.baseline {
        super::model::ReviewOnlyCapabilityConflictBaseline::EmptyAdmission => {
            output.push_str("baseline empty_admission\n");
        }
        super::model::ReviewOnlyCapabilityConflictBaseline::RetainedReview {
            resolution,
            source_consumption,
        } => {
            output.push_str("baseline retained_review\n");
            render_resolution(output, "baseline_resolution", resolution);
            render_digest(
                output,
                "baseline_source_consumption",
                &source_consumption.digest(),
            );
        }
    }
    render_resolution(
        output,
        "candidate_resolution",
        &package.candidate_resolution,
    );
    render_digest(
        output,
        "candidate_source_consumption",
        &package.candidate_source_consumption.digest(),
    );
    render_digest(
        output,
        "candidate_closure",
        &package.candidate_closure.digest(),
    );
    output.push_str("dependency_root ");
    push_hex(output, &package.dependency_path.root().identity().digest());
    output.push('\n');
    for step in package.dependency_path.steps() {
        output.push_str("dependency_step ");
        output.push_str(&step.dependency_index().to_string());
        output.push(' ');
        output.push_str(step.alias().as_str());
        output.push(' ');
        push_hex(output, &step.requester().identity().digest());
        output.push(' ');
        push_hex(output, &step.target().identity().digest());
        output.push('\n');
    }
    output.push_str("conflict_count ");
    output.push_str(&package.conflicts.len().to_string());
    output.push('\n');
    for conflict in &package.conflicts {
        output.push_str("conflict_begin\nfingerprint ");
        push_hex(output, &conflict.fingerprint.digest());
        output.push_str("\nchange ");
        output.push_str(change_token(conflict.change));
        output.push_str("\nkind ");
        output.push_str(row_kind_token(conflict.kind));
        output.push_str("\nrisk ");
        output.push_str(row_risk_token(conflict.risk));
        output.push_str("\nrow_key ");
        render_bytes_summary(output, &conflict.row_key);
        output.push('\n');
        render_optional_bytes_summary(output, "baseline_row", conflict.baseline_row.as_deref());
        render_optional_bytes_summary(output, "candidate_row", conflict.candidate_row.as_deref());
        render_optional_row_source(output, "baseline", conflict.baseline_source.as_ref());
        render_optional_row_source(output, "candidate", conflict.candidate_source.as_ref());
        output.push_str("conflict_end\n");
    }
    output.push_str("package_end\n");
}

fn render_resolution(
    output: &mut impl ConflictRenderOutput,
    label: &str,
    resolution: &ImmutableSourceResolution,
) {
    output.push_str(label);
    match resolution {
        ImmutableSourceResolution::Git {
            commit,
            tree,
            content,
        } => {
            output.push_str(" git ");
            output.push_str(&commit.to_hex());
            output.push(' ');
            output.push_str(&tree.to_hex());
            output.push(' ');
            output.push_str(&content.to_hex());
        }
        ImmutableSourceResolution::Workspace { content } => {
            output.push_str(" workspace ");
            output.push_str(&content.to_hex());
        }
        ImmutableSourceResolution::ExternalLocal { content } => {
            output.push_str(" external_local ");
            output.push_str(&content.to_hex());
        }
    }
    output.push('\n');
}

fn render_digest(output: &mut impl ConflictRenderOutput, label: &str, digest: &[u8; 32]) {
    output.push_str(label);
    output.push(' ');
    push_hex(output, digest);
    output.push('\n');
}

fn render_optional_bytes_summary(
    output: &mut impl ConflictRenderOutput,
    label: &str,
    bytes: Option<&[u8]>,
) {
    output.push_str(label);
    output.push(' ');
    if let Some(bytes) = bytes {
        render_bytes_summary(output, bytes);
    } else {
        output.push_str("none");
    }
    output.push('\n');
}

fn render_optional_row_source(
    output: &mut impl ConflictRenderOutput,
    label: &str,
    source: Option<&PackageReviewCanonicalRowSource>,
) {
    output.push_str(label);
    output.push_str("_source ");
    match source {
        None => output.push_str("absent_row\n"),
        Some(source) => {
            let locations = source.authored_locations().unwrap_or_default();
            output.push_str("present authored ");
            output.push_str(&locations.len().to_string());
            output.push_str(" compiler_derived ");
            output.push_str(&source.compiler_derivations().len().to_string());
            output.push('\n');
            for kind in source.compiler_derivations() {
                output.push_str(label);
                output.push_str("_derivation ");
                output.push_str(synthetic_source_kind_token(*kind));
                output.push('\n');
            }
            for location in locations {
                output.push_str(label);
                output.push_str("_location ");
                output.push_str(source_location_role_token(location.role()));
                output.push(' ');
                match location.owner() {
                    PackageReviewSourceLocationOwner::Package(package) => {
                        output.push_str("package ");
                        push_hex(output, &package.digest());
                    }
                    PackageReviewSourceLocationOwner::Toolchain(source) => {
                        output.push_str("toolchain ");
                        push_hex(output, &source.digest());
                    }
                }
                output.push(' ');
                output.push_str(&location.start_byte().to_string());
                output.push(' ');
                output.push_str(&location.end_byte().to_string());
                output.push(' ');
                push_escaped_path(output, location.relative_path().as_bytes());
                output.push('\n');
            }
        }
    }
}

pub(super) const fn synthetic_source_kind_tag(kind: PackageReviewSyntheticSourceKind) -> u8 {
    match kind {
        PackageReviewSyntheticSourceKind::ProjectionHeader => 0,
        PackageReviewSyntheticSourceKind::EmptySelectedProviderSet => 1,
        PackageReviewSyntheticSourceKind::UniqueCoveringProviderSelection => 2,
        PackageReviewSyntheticSourceKind::FreeExternalProviderType => 3,
    }
}

const fn synthetic_source_kind_token(kind: PackageReviewSyntheticSourceKind) -> &'static str {
    match kind {
        PackageReviewSyntheticSourceKind::ProjectionHeader => "projection_header",
        PackageReviewSyntheticSourceKind::EmptySelectedProviderSet => "empty_selected_provider_set",
        PackageReviewSyntheticSourceKind::UniqueCoveringProviderSelection => {
            "unique_covering_provider_selection"
        }
        PackageReviewSyntheticSourceKind::FreeExternalProviderType => "free_external_provider_type",
    }
}

fn push_escaped_path(output: &mut impl ConflictRenderOutput, path: &[u8]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    output.push('"');
    for byte in path {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'.' | b'_' | b'-' | b'/' | b'<' | b'>' => {
                output.push(char::from(*byte))
            }
            b'\\' => output.push_str("\\\\"),
            _ => {
                output.push_str("\\x");
                output.push(char::from(HEX[usize::from(byte >> 4)]));
                output.push(char::from(HEX[usize::from(byte & 0x0f)]));
            }
        }
    }
    output.push('"');
}

fn render_bytes_summary(output: &mut impl ConflictRenderOutput, bytes: &[u8]) {
    output.push_str("length ");
    output.push_str(&bytes.len().to_string());
    output.push_str(" sha256 ");
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    push_hex(output, &digest);
}

fn push_hex(output: &mut impl ConflictRenderOutput, bytes: &[u8]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        output.push(HEX[usize::from(byte >> 4)] as char);
        output.push(HEX[usize::from(byte & 0x0f)] as char);
    }
}

pub(super) const fn review_role_token(role: ReviewSetRole) -> &'static str {
    match role {
        ReviewSetRole::Baseline => "baseline",
        ReviewSetRole::Candidate => "candidate",
    }
}

const fn change_token(change: ReviewOnlyCapabilityConflictChange) -> &'static str {
    match change {
        ReviewOnlyCapabilityConflictChange::Added => "added",
        ReviewOnlyCapabilityConflictChange::Removed => "removed",
        ReviewOnlyCapabilityConflictChange::Changed => "changed",
    }
}

pub(super) const fn row_kind_token(kind: PackageReviewCanonicalRowKind) -> &'static str {
    match kind {
        PackageReviewCanonicalRowKind::ProjectionHeader => "projection_header",
        PackageReviewCanonicalRowKind::PublicTrait => "public_trait",
        PackageReviewCanonicalRowKind::PublicDomain => "public_domain",
        PackageReviewCanonicalRowKind::PublicData => "public_data",
        PackageReviewCanonicalRowKind::PublicProposition => "public_proposition",
        PackageReviewCanonicalRowKind::PublicConst => "public_const",
        PackageReviewCanonicalRowKind::PublicOperator => "public_operator",
        PackageReviewCanonicalRowKind::PublicConformance => "public_conformance",
        PackageReviewCanonicalRowKind::RepresentationTcb => "representation_tcb",
        PackageReviewCanonicalRowKind::Callable => "callable",
        PackageReviewCanonicalRowKind::DangerousAuthority => "dangerous_authority",
        PackageReviewCanonicalRowKind::SelectedProviderSet => "selected_provider_set",
        PackageReviewCanonicalRowKind::AcceptedClaim => "accepted_claim",
        PackageReviewCanonicalRowKind::DangerousAuthoritySlack => "dangerous_authority_slack",
        PackageReviewCanonicalRowKind::SemanticDependency => "semantic_dependency",
        PackageReviewCanonicalRowKind::ExternalExecutableSupply => "external_executable_supply",
        PackageReviewCanonicalRowKind::BoundaryApplicationRealization => {
            "boundary_application_realization"
        }
        PackageReviewCanonicalRowKind::NonExecutableQuotientCorrespondence => {
            "non_executable_quotient_correspondence"
        }
        PackageReviewCanonicalRowKind::ContractEntailmentOpenObligation => {
            "contract_entailment_open_obligation"
        }
    }
}

pub(super) const fn row_kind_tag(kind: PackageReviewCanonicalRowKind) -> u8 {
    match kind {
        PackageReviewCanonicalRowKind::ProjectionHeader => 0,
        PackageReviewCanonicalRowKind::PublicTrait => 1,
        PackageReviewCanonicalRowKind::PublicDomain => 2,
        PackageReviewCanonicalRowKind::PublicData => 3,
        PackageReviewCanonicalRowKind::RepresentationTcb => 4,
        PackageReviewCanonicalRowKind::Callable => 5,
        PackageReviewCanonicalRowKind::DangerousAuthority => 6,
        PackageReviewCanonicalRowKind::SelectedProviderSet => 7,
        PackageReviewCanonicalRowKind::AcceptedClaim => 8,
        PackageReviewCanonicalRowKind::DangerousAuthoritySlack => 9,
        PackageReviewCanonicalRowKind::SemanticDependency => 10,
        PackageReviewCanonicalRowKind::PublicProposition => 11,
        PackageReviewCanonicalRowKind::PublicConst => 12,
        PackageReviewCanonicalRowKind::PublicOperator => 13,
        PackageReviewCanonicalRowKind::PublicConformance => 14,
        PackageReviewCanonicalRowKind::ExternalExecutableSupply => 15,
        PackageReviewCanonicalRowKind::BoundaryApplicationRealization => 16,
        PackageReviewCanonicalRowKind::NonExecutableQuotientCorrespondence => 17,
        PackageReviewCanonicalRowKind::ContractEntailmentOpenObligation => 18,
    }
}

const fn row_risk_token(risk: PackageReviewCanonicalRowRisk) -> &'static str {
    match risk {
        PackageReviewCanonicalRowRisk::Blocking => "blocking",
        PackageReviewCanonicalRowRisk::AuditRecommended => "audit_recommended",
        PackageReviewCanonicalRowRisk::OpaqueBlocking => "opaque_blocking",
    }
}

pub(super) const fn row_risk_tag(risk: PackageReviewCanonicalRowRisk) -> u8 {
    match risk {
        PackageReviewCanonicalRowRisk::Blocking => 0,
        PackageReviewCanonicalRowRisk::AuditRecommended => 1,
        PackageReviewCanonicalRowRisk::OpaqueBlocking => 2,
    }
}

pub(super) const fn change_tag(change: ReviewOnlyCapabilityConflictChange) -> u8 {
    match change {
        ReviewOnlyCapabilityConflictChange::Added => 0,
        ReviewOnlyCapabilityConflictChange::Removed => 1,
        ReviewOnlyCapabilityConflictChange::Changed => 2,
    }
}

pub(super) const fn source_location_role_tag(role: PackageReviewSourceLocationRole) -> u8 {
    match role {
        PackageReviewSourceLocationRole::Declaration => 0,
        PackageReviewSourceLocationRole::DerivationOrigin => 1,
        PackageReviewSourceLocationRole::AuthorityDeclaration => 2,
        PackageReviewSourceLocationRole::AuthorityExposure => 3,
        PackageReviewSourceLocationRole::ProviderSelection => 4,
        PackageReviewSourceLocationRole::ProviderGrant => 25,
        PackageReviewSourceLocationRole::ProviderSchemaDeclaration => 5,
        PackageReviewSourceLocationRole::ProviderTypeDeclaration => 6,
        PackageReviewSourceLocationRole::ProviderRealization => 7,
        PackageReviewSourceLocationRole::SemanticDependencyConsumer => 8,
        PackageReviewSourceLocationRole::SemanticDependencyDeclaration => 9,
        PackageReviewSourceLocationRole::ProviderRequirementDeclaration => 10,
        PackageReviewSourceLocationRole::TraitParent => 11,
        PackageReviewSourceLocationRole::ContractClause => 12,
        PackageReviewSourceLocationRole::BodyCall => 13,
        PackageReviewSourceLocationRole::SynchronousInvocation => 14,
        PackageReviewSourceLocationRole::ServiceReach => 15,
        PackageReviewSourceLocationRole::Suspension => 16,
        PackageReviewSourceLocationRole::Blocking => 17,
        PackageReviewSourceLocationRole::ExternalBinding => 18,
        PackageReviewSourceLocationRole::ConstInitializer => 19,
        PackageReviewSourceLocationRole::PropositionFormula => 20,
        PackageReviewSourceLocationRole::ProofFact => 21,
        PackageReviewSourceLocationRole::TraitRequirement => 22,
        PackageReviewSourceLocationRole::DataMember => 23,
        PackageReviewSourceLocationRole::CallableParameter => 24,
        PackageReviewSourceLocationRole::BoundaryApplicationUse => 26,
        PackageReviewSourceLocationRole::QuotientOperationDeclaration => 27,
        PackageReviewSourceLocationRole::RepresentationSelection => 28,
    }
}

pub(super) const fn source_location_role_token(
    role: PackageReviewSourceLocationRole,
) -> &'static str {
    match role {
        PackageReviewSourceLocationRole::Declaration => "declaration",
        PackageReviewSourceLocationRole::DerivationOrigin => "derivation_origin",
        PackageReviewSourceLocationRole::AuthorityDeclaration => "authority_declaration",
        PackageReviewSourceLocationRole::AuthorityExposure => "authority_exposure",
        PackageReviewSourceLocationRole::ProviderSelection => "provider_selection",
        PackageReviewSourceLocationRole::ProviderGrant => "provider_grant",
        PackageReviewSourceLocationRole::ProviderSchemaDeclaration => "provider_schema_declaration",
        PackageReviewSourceLocationRole::ProviderTypeDeclaration => "provider_type_declaration",
        PackageReviewSourceLocationRole::ProviderRequirementDeclaration => {
            "provider_requirement_declaration"
        }
        PackageReviewSourceLocationRole::ProviderRealization => "provider_realization",
        PackageReviewSourceLocationRole::SemanticDependencyConsumer => {
            "semantic_dependency_consumer"
        }
        PackageReviewSourceLocationRole::SemanticDependencyDeclaration => {
            "semantic_dependency_declaration"
        }
        PackageReviewSourceLocationRole::TraitParent => "trait_parent",
        PackageReviewSourceLocationRole::ContractClause => "contract_clause",
        PackageReviewSourceLocationRole::BodyCall => "body_call",
        PackageReviewSourceLocationRole::SynchronousInvocation => "synchronous_invocation",
        PackageReviewSourceLocationRole::ServiceReach => "service_reach",
        PackageReviewSourceLocationRole::Suspension => "suspension",
        PackageReviewSourceLocationRole::Blocking => "blocking",
        PackageReviewSourceLocationRole::ExternalBinding => "external_binding",
        PackageReviewSourceLocationRole::ConstInitializer => "const_initializer",
        PackageReviewSourceLocationRole::PropositionFormula => "proposition_formula",
        PackageReviewSourceLocationRole::ProofFact => "proof_fact",
        PackageReviewSourceLocationRole::TraitRequirement => "trait_requirement",
        PackageReviewSourceLocationRole::DataMember => "data_member",
        PackageReviewSourceLocationRole::CallableParameter => "callable_parameter",
        PackageReviewSourceLocationRole::BoundaryApplicationUse => "boundary_application_use",
        PackageReviewSourceLocationRole::QuotientOperationDeclaration => {
            "quotient_operation_declaration"
        }
        PackageReviewSourceLocationRole::RepresentationSelection => "representation_selection",
    }
}
