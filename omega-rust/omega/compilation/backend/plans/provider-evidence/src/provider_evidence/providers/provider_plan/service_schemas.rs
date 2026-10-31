//! Provider plan and service schema digests, service schemas, methods,
//! progress premises and entry claims.

use super::DigestFeed;
use crate::provider_evidence::evidence_digest;
use crate::provider_evidence::providers::provider_plan::ProviderPlanRow;
use crate::provider_evidence::providers::provider_plan::digest_encoder::ProviderPlanDigestEncoder;
use crate::provider_evidence::providers::provider_plan::identity_table::{
    ProviderIdentityId, ProviderIdentityTable,
};
/// Exact evaluated calling-application identity. This is realization evidence,
/// not a typed-source declaration or permission to execute an application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryCallingPlanCommitment([u8; 32]);

impl BoundaryCallingPlanCommitment {
    pub const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn is_zero(self) -> bool {
        self.0 == [0; 32]
    }
}

evidence_digest!(
    /// Collision-resistant identity of one exact normalized provider plan.
    ///
    /// Construction remains crate-owned. Compact plan fingerprints are
    /// retained for existing reports and coordinates, but admission joins
    /// should retain this digest or the complete [`ProviderPlan`].
    ProviderPlanDigest,
    from: #[doc(hidden)] pub
);

evidence_digest!(
    /// Collision-resistant identity of one exact normalized service schema.
    ///
    /// This excludes provider realization rows while retaining the
    /// declaration owner, every requirement signature, authority flow, and
    /// selected calling-plan application carried by [`ServiceSchema`].
    ServiceSchemaDigest,
    from: #[doc(hidden)] pub
);

pub use language_semantics::service_surface::{
    ServiceEntryAuthorityFlow, ServiceEntryClaim, ServiceMethod, ServiceProgressEstablishmentRoute,
    ServiceProgressEstablishmentRouteKind, ServiceProgressPremise, ServiceProgressSubject,
    ServiceReceiverApplication, ServiceResultClaim, ServiceSurface,
};

/// A checked declaration plus separately selected target calling evidence.
/// Sharing a surface never shares a selection, grant or execution verdict.
#[derive(Debug, Clone, Default)]
pub struct ServiceSchema {
    pub declaration: std::sync::Arc<ServiceSurface>,
    pub calling_applications: Vec<ServiceCallingApplication>,
}

/// The two coordinates describe one complete evaluated calling application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServiceCallingPlan {
    pub report_fingerprint: u64,
    pub commitment: BoundaryCallingPlanCommitment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceCallingApplication {
    /// Coordinate in this schema's declaration identity table.
    pub requirement: ProviderIdentityId,
    pub plan: ServiceCallingPlan,
}

impl ServiceSchema {
    pub fn from_declaration(declaration: std::sync::Arc<ServiceSurface>) -> Self {
        Self {
            declaration,
            calling_applications: Vec::new(),
        }
    }

    pub fn calling_plan(&self, requirement: ProviderIdentityId) -> Option<&ServiceCallingPlan> {
        self.calling_applications
            .iter()
            .find(|application| application.requirement == requirement)
            .map(|application| &application.plan)
    }

    pub fn identity_text(&self, identity: ProviderIdentityId) -> &str {
        self.declaration.identity_text(identity)
    }

    pub fn root_application_identity(&self) -> Option<&str> {
        self.declaration.root_application_identity()
    }

    pub fn selection_slot_identity(
        &self,
    ) -> (
        Option<semantic_vocabulary::PackageKeyIdentity>,
        &str,
        Option<&str>,
    ) {
        self.declaration.selection_slot_identity()
    }

    pub(crate) fn validate_receiver_applications(&self) -> Result<(), &'static str> {
        self.declaration.validate_receiver_applications()?;
        self.validate_calling_applications()
    }

    /// Verify attachment coordinates before admission or flattened encoding.
    /// An orphan or duplicate must not disappear during serialization.
    pub fn validate_calling_applications(&self) -> Result<(), &'static str> {
        for (position, application) in self.calling_applications.iter().enumerate() {
            if !application.requirement.is_valid()
                || !self
                    .declaration
                    .methods
                    .iter()
                    .any(|method| method.requirement_identity == application.requirement)
                || self.calling_applications[..position]
                    .iter()
                    .any(|previous| previous.requirement == application.requirement)
            {
                return Err("calling evidence has a foreign or repeated requirement coordinate");
            }
            if application.plan.report_fingerprint == 0 || application.plan.commitment.is_zero() {
                return Err("calling evidence has an empty fingerprint or commitment");
            }
        }
        Ok(())
    }

    /// Domain-separated SHA-256 commitment to the complete normalized schema.
    pub fn identity_digest(&self) -> ServiceSchemaDigest {
        self.digest_with_calling(true)
    }

    /// Entry-role acceptance deliberately excludes target calling attachments.
    pub fn declaration_identity_digest(&self) -> ServiceSchemaDigest {
        self.digest_with_calling(false)
    }

    fn digest_with_calling(&self, include_calling: bool) -> ServiceSchemaDigest {
        let mut encoder = ProviderPlanDigestEncoder::for_service_schema();
        encoder.string(self.identity_text(self.declaration.trait_name));
        encoder.package_identity(self.declaration.trait_package_identity);
        encoder.receiver_applications(self);
        let mut methods = self.declaration.methods.iter().collect::<Vec<_>>();
        methods.sort_by(|left, right| {
            left.name.cmp(&right.name).then_with(|| {
                self.identity_text(left.requirement_identity)
                    .cmp(self.identity_text(right.requirement_identity))
            })
        });
        encoder.len(methods.len());
        for method in methods {
            encoder.service_method(
                method,
                &self.declaration.identities,
                if include_calling {
                    self.calling_plan(method.requirement_identity)
                } else {
                    None
                },
            );
        }
        ServiceSchemaDigest::from_digest(encoder.finish())
    }
}

impl PartialEq for ServiceSchema {
    fn eq(&self, other: &Self) -> bool {
        self.declaration == other.declaration
            && self.calling_applications.len() == other.calling_applications.len()
            && self
                .declaration
                .methods
                .iter()
                .zip(&other.declaration.methods)
                .all(|(left, right)| {
                    self.calling_plan(left.requirement_identity)
                        == other.calling_plan(right.requirement_identity)
                })
    }
}
impl Eq for ServiceSchema {}

impl ServiceSchema {
    /// Whether this whole schema owns exactly the demanded receiver application
    /// and its entire declared roster. Coverage is never a union of providers.
    pub fn supplies_receiver_schema(&self, demanded: &Self) -> bool {
        self.receiver_application_index(demanded).is_some()
    }

    /// Retain the exact receiver view beneath this whole plan. Its index is
    /// meaningful only together with the digest of the owning provider plan;
    /// consumers can use its complete original roster without rebuilding an
    /// ancestor schema or selecting another provider.
    pub fn receiver_application_index(&self, demanded: &Self) -> Option<usize> {
        let root = demanded.declaration.receiver_applications.first()?;
        self.declaration
            .receiver_applications
            .iter()
            .position(|application| self.receiver_contract_matches(application, demanded, root))
    }

    /// Distinct whole choices cannot install the same exact receiver view.
    /// Equal code bodies do not make their selecting owners interchangeable.
    pub fn shares_receiver_application(&self, other: &Self) -> bool {
        self.declaration.receiver_applications.iter().any(|left| {
            other
                .declaration
                .receiver_applications
                .iter()
                .any(|right| self.receiver_contract_matches(left, other, right))
        })
    }

    /// An inherited receiver retains the descendant's evaluated calling
    /// contract. Identical declaring requirements under distinct entry plans
    /// do not compete for one installed receiver; equal complete contracts
    /// still cannot mix their choosing owners.
    fn receiver_contract_matches(
        &self,
        application: &ServiceReceiverApplication,
        other: &Self,
        other_application: &ServiceReceiverApplication,
    ) -> bool {
        application.eq_in(
            &self.declaration.identities,
            other_application,
            &other.declaration.identities,
        ) && application.requirement_identities.iter().all(|identity| {
            let Some(method) = self
                .declaration
                .methods
                .iter()
                .find(|method| method.requirement_identity == *identity)
            else {
                return false;
            };
            other.declaration.methods.iter().any(|other_method| {
                method.eq_in(
                    &self.declaration.identities,
                    other_method,
                    &other.declaration.identities,
                ) && self.calling_plan(method.requirement_identity)
                    == other.calling_plan(other_method.requirement_identity)
            })
        })
    }

    /// Match a provider row to one exact requirement. The readable method name
    /// must agree as a drift check, but only the canonical overload identity
    /// selects the requirement. The row's identity resolves against the owning
    /// plan's `identities` table.
    pub fn row_binds_method(
        &self,
        row: &ProviderPlanRow,
        method: &ServiceMethod,
        row_identities: &ProviderIdentityTable,
    ) -> bool {
        row.requirement_identity.is_valid()
            && row.method == method.name
            && self
                .declaration
                .identities
                .id_at(row_identities.text(row.requirement_identity))
                == Some(method.requirement_identity)
    }

    pub fn method_for_row(
        &self,
        row: &ProviderPlanRow,
        row_identities: &ProviderIdentityTable,
    ) -> Option<&ServiceMethod> {
        self.declaration
            .methods
            .iter()
            .find(|method| self.row_binds_method(row, method, row_identities))
    }
}

#[cfg(test)]
mod tests;

impl compiler_storage::Record for ServiceSchemaDigest {
    fn write(
        &self,
        output: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
        compiler_storage::Record::write(self.as_bytes(), output)
    }
    fn read<Context>(
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        compiler_storage::Record::read(input).map(Self::from_digest)
    }
}
impl compiler_storage::Record for ProviderPlanDigest {
    fn write(
        &self,
        output: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
        compiler_storage::Record::write(self.as_bytes(), output)
    }
    fn read<Context>(
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        compiler_storage::Record::read(input).map(Self::from_digest)
    }
}
