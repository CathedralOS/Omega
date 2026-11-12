/*
We pair a target-neutral `ServiceSurface` declaration with target calling-plan
applications (`ServiceSchema`, `ServiceCallingApplication`,
`BoundaryCallingPlanCommitment`) and compute domain-separated SHA-256
commitments (`ServiceSchemaDigest`, `ProviderPlanDigest`). Requirement
coordinates are stored as `ProviderIdentityId` handles into the declaration's
identity table.
*/

use super::DigestFeed;
use crate::provider_evidence::evidence_digest;
use crate::provider_evidence::providers::provider_plan::digest_encoder::ProviderPlanDigestEncoder;
use crate::provider_evidence::providers::provider_plan::{
    ProviderIdentityId, ProviderIdentityTable, ProviderPlanRow,
};

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

// The promise an evaluated calling plan makes at the boundary: its convention
// placements and effective machine-state contract
// (`ValidatedBoundaryEntryPlan::contract_commitment_digest`). Unlike
// `BoundaryCallingPlanCommitment` it does not name the selecting owner or the
// policy that produced it, so it is what two owners must agree on to supply the
// same receiver application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryCallingPlanPromise([u8; 32]);

impl BoundaryCallingPlanPromise {
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
    ProviderPlanDigest,
    from: #[doc(hidden)] pub
);

evidence_digest!(
    ServiceSchemaDigest,
    from: #[doc(hidden)] pub
);

pub use language_semantics::service_surface::{
    ServiceEntryAuthorityFlow, ServiceEntryClaim, ServiceMethod, ServiceProgressEstablishmentRoute,
    ServiceProgressEstablishmentRouteKind, ServiceProgressPremise, ServiceProgressSubject,
    ServiceReceiverApplication, ServiceResultClaim, ServiceSurface,
};

#[derive(Debug, Clone, Default)]
pub struct ServiceSchema {
    pub declaration: std::sync::Arc<ServiceSurface>,
    pub calling_applications: Vec<ServiceCallingApplication>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServiceCallingPlan {
    pub report_fingerprint: u64,
    pub commitment: BoundaryCallingPlanCommitment,
    pub promise: BoundaryCallingPlanPromise,
}

// Which part of the calling evidence two schemas must agree on. Supplying a
// demanded receiver needs the exact owner-bound application; detecting that two
// owners supply the same receiver compares only what the plan promises, so
// distinct policies that publish one promise still conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CallingAgreement {
    ExactApplication,
    PlanPromise,
}

impl ServiceCallingPlan {
    fn agrees_with(&self, other: &Self, agreement: CallingAgreement) -> bool {
        match agreement {
            CallingAgreement::ExactApplication => self == other,
            CallingAgreement::PlanPromise => self.promise == other.promise,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceCallingApplication {
    pub requirement: ProviderIdentityId,
    pub abi: semantic_vocabulary::CallingAbiIdentity,
    pub plan: ServiceCallingPlan,
}

impl ServiceSchema {
    pub fn from_declaration(declaration: std::sync::Arc<ServiceSurface>) -> Self {
        Self {
            declaration,
            calling_applications: Vec::new(),
        }
    }

    pub fn calling_application(
        &self,
        abi: &semantic_vocabulary::CallingAbiIdentity,
    ) -> Option<&ServiceCallingPlan> {
        self.calling_applications
            .iter()
            .find(|application| &application.abi == abi)
            .map(|application| &application.plan)
    }

    pub fn calling_for_requirement(
        &self,
        requirement: ProviderIdentityId,
    ) -> Vec<&ServiceCallingApplication> {
        let mut applications = self
            .calling_applications
            .iter()
            .filter(|application| application.requirement == requirement)
            .collect::<Vec<_>>();
        applications.sort_by(|left, right| left.abi.cmp(&right.abi));
        applications
    }

    fn calling_matches(
        &self,
        requirement: ProviderIdentityId,
        other: &Self,
        other_requirement: ProviderIdentityId,
        agreement: CallingAgreement,
    ) -> bool {
        let left = self.calling_for_requirement(requirement);
        let right = other.calling_for_requirement(other_requirement);
        left.len() == right.len()
            && left.iter().zip(&right).all(|(left, right)| {
                left.abi == right.abi && left.plan.agrees_with(&right.plan, agreement)
            })
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
                    .any(|previous| previous.abi == application.abi)
            {
                return Err("calling evidence has a foreign or repeated requirement coordinate");
            }
            application.abi.validate()?;
            if application.abi.requirement_identity != self.identity_text(application.requirement) {
                return Err("calling evidence ABI key differs from its requirement coordinate");
            }
            if application.plan.report_fingerprint == 0
                || application.plan.commitment.is_zero()
                || application.plan.promise.is_zero()
            {
                return Err("calling evidence has an empty fingerprint, commitment or promise");
            }
        }
        Ok(())
    }

    pub fn identity_digest(&self) -> ServiceSchemaDigest {
        self.digest_with_calling(true)
    }

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
                    self.calling_for_requirement(method.requirement_identity)
                } else {
                    Vec::new()
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
                    self.calling_matches(
                        left.requirement_identity,
                        other,
                        right.requirement_identity,
                        CallingAgreement::ExactApplication,
                    )
                })
    }
}
impl Eq for ServiceSchema {}

impl ServiceSchema {
    pub fn supplies_receiver_schema(&self, demanded: &Self) -> bool {
        self.receiver_application_index(demanded).is_some()
    }

    pub fn receiver_application_index(&self, demanded: &Self) -> Option<usize> {
        let root = demanded.declaration.receiver_applications.first()?;
        self.declaration
            .receiver_applications
            .iter()
            .position(|application| {
                self.receiver_contract_matches(
                    application,
                    demanded,
                    root,
                    CallingAgreement::ExactApplication,
                )
            })
    }

    // Two selected owners conflict when any receiver they supply agrees on the
    // declaration and on every calling plan's promise. The owner-bound
    // commitment names the selecting owner and its policy, so comparing it here
    // would let two policies that publish the same plan escape the conflict.
    pub fn shares_receiver_application(&self, other: &Self) -> bool {
        self.declaration.receiver_applications.iter().any(|left| {
            other.declaration.receiver_applications.iter().any(|right| {
                self.receiver_contract_matches(left, other, right, CallingAgreement::PlanPromise)
            })
        })
    }

    fn receiver_contract_matches(
        &self,
        application: &ServiceReceiverApplication,
        other: &Self,
        other_application: &ServiceReceiverApplication,
        agreement: CallingAgreement,
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
                ) && self.calling_matches(
                    method.requirement_identity,
                    other,
                    other_method.requirement_identity,
                    agreement,
                )
            })
        })
    }

    pub fn row_binds_method(
        &self,
        row: &ProviderPlanRow,
        method: &ServiceMethod,
        row_identities: &ProviderIdentityTable,
    ) -> bool {
        row.declared_requirement_identity.is_valid()
            && row.method == method.name
            && self
                .declaration
                .identities
                .id_at(row_identities.text(row.declared_requirement_identity))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinct_phantom_applications_share_a_requirement_without_collapsing() {
        let mut schema = schema("Parent", 1, false);
        let first = schema.calling_applications[0].clone();
        let mut second = first.clone();
        second
            .abi
            .operands
            .push(semantic_vocabulary::CallingStaticOperand {
                ordinal: 0,
                kind: semantic_vocabulary::CallingOperandKind::Type,
                identity: "u64".into(),
            });
        schema.calling_applications.push(second.clone());
        schema.validate_calling_applications().unwrap();
        assert_eq!(schema.calling_application(&first.abi), Some(&first.plan));
        assert_eq!(schema.calling_application(&second.abi), Some(&second.plan));
        let digest = schema.identity_digest();
        schema.calling_applications.reverse();
        assert_eq!(digest, schema.identity_digest());
        let mut changed = schema.clone();
        changed.calling_applications[0].abi.operands[0].identity = "u32".into();
        assert_ne!(digest, changed.identity_digest());
        assert_ne!(schema, changed);
        schema.calling_applications.push(first);
        assert!(schema.validate_calling_applications().is_err());
    }

    fn schema(root: &str, calling: u8, shift_identity_slots: bool) -> ServiceSchema {
        let mut identities = ProviderIdentityTable::default();
        if shift_identity_slots {
            identities.intern("unrelated mint order");
        }
        let root = identities.intern(root);
        let parent = identities.intern("Parent");
        let requirement = identities.intern("Parent::enter(u64)");
        let parameter = identities.intern("u64");
        let receiver = |name| ServiceReceiverApplication {
            application: Default::default(),
            trait_name: name,
            trait_package_identity: None,
            argument_identities: vec![],
            lifetime_argument_identities: vec![],
            requirement_identities: vec![requirement],
        };
        let declaration = std::sync::Arc::new(ServiceSurface {
            trait_name: root,
            identities,
            trait_package_identity: None,
            receiver_applications: vec![receiver(root), receiver(parent)],
            methods: vec![ServiceMethod {
                name: "enter".into(),
                requirement_owner: "Parent".into(),
                requirement_identity: requirement,
                parameter_count: 1,
                parameter_type_identities: vec![parameter],
                ..Default::default()
            }],
        });
        ServiceSchema {
            declaration,
            calling_applications: vec![ServiceCallingApplication {
                requirement,
                abi: semantic_vocabulary::CallingAbiIdentity {
                    requirement_identity: "Parent::enter(u64)".into(),
                    declaring_application: "Parent".into(),
                    operands: vec![],
                },
                plan: ServiceCallingPlan {
                    report_fingerprint: u64::from(calling),
                    commitment: BoundaryCallingPlanCommitment::from_digest([calling; 32]),
                    promise: BoundaryCallingPlanPromise::from_digest([calling; 32]),
                },
            }],
        }
    }

    #[test]
    fn inherited_views_require_their_complete_effective_calling_contract() {
        let first = schema("FirstRoot", 1, false);
        let second = schema("SecondRoot", 2, false);
        assert!(!first.shares_receiver_application(&second));
        assert!(!second.shares_receiver_application(&first));
        let demanded = schema("Parent", 1, true);
        assert_eq!(first.receiver_application_index(&demanded), Some(1));
        assert!(!second.supplies_receiver_schema(&demanded));
    }

    #[test]
    fn equal_shared_parent_contract_still_competes_across_choosing_owners() {
        let first = schema("FirstRoot", 1, false);
        let second = schema("SecondRoot", 1, true);
        assert!(first.shares_receiver_application(&second));
        assert!(second.shares_receiver_application(&first));
    }

    // Two policies that publish one plan promise still commit to different
    // owner-bound applications; the promise alone decides the conflict, while
    // supplying a demanded receiver still needs the exact application.
    #[test]
    fn same_plan_promise_from_different_policies_still_competes() {
        let first = schema("FirstRoot", 1, false);
        let mut second = schema("SecondRoot", 1, true);
        second.calling_applications[0].plan.commitment =
            BoundaryCallingPlanCommitment::from_digest([2; 32]);
        second.calling_applications[0].plan.report_fingerprint = 2;
        assert!(first.shares_receiver_application(&second));
        assert!(second.shares_receiver_application(&first));
        let mut demanded = schema("Parent", 1, true);
        demanded.calling_applications[0].plan = second.calling_applications[0].plan;
        assert!(!first.supplies_receiver_schema(&demanded));
        assert!(second.supplies_receiver_schema(&demanded));
    }

    #[test]
    fn compact_calling_coordinate_cannot_replace_its_exact_commitment() {
        let first = schema("FirstRoot", 1, false);
        let mut changed = schema("Parent", 1, true);
        changed.calling_applications[0].plan.commitment =
            BoundaryCallingPlanCommitment::from_digest([2; 32]);
        assert!(!first.supplies_receiver_schema(&changed));
    }

    #[test]
    fn incomplete_or_weakened_method_roster_cannot_supply_a_receiver() {
        let first = schema("FirstRoot", 1, false);
        let mut changed = schema("Parent", 1, true);
        let declaration = std::sync::Arc::make_mut(&mut changed.declaration);
        declaration.methods[0].parameter_type_identities[0] = declaration.identities.intern("addr");
        assert!(!first.supplies_receiver_schema(&changed));
        std::sync::Arc::make_mut(&mut changed.declaration)
            .methods
            .clear();
        assert!(!first.supplies_receiver_schema(&changed));
    }

    #[test]
    fn target_attachments_do_not_copy_or_change_the_neutral_declaration() {
        let first = schema("Root", 1, false);
        let mut second = first.clone();
        second.calling_applications[0].plan.commitment =
            BoundaryCallingPlanCommitment::from_digest([2; 32]);
        assert!(std::sync::Arc::ptr_eq(
            &first.declaration,
            &second.declaration
        ));
        assert_ne!(first.identity_digest(), second.identity_digest());
        assert_eq!(
            first.declaration_identity_digest(),
            second.declaration_identity_digest()
        );
        let unselected = ServiceSchema::from_declaration(first.declaration.clone());
        assert_eq!(
            first.declaration_identity_digest(),
            unselected.identity_digest()
        );
    }

    #[test]
    fn attachment_roster_refuses_duplicate_and_foreign_coordinates() {
        let first = schema("Root", 1, false);
        assert!(first.validate_calling_applications().is_ok());
        let mut duplicate = first.clone();
        duplicate
            .calling_applications
            .push(duplicate.calling_applications[0].clone());
        assert!(duplicate.validate_calling_applications().is_err());
        let mut foreign = first;
        foreign.calling_applications[0].requirement = ProviderIdentityId::from_index(u32::MAX);
        assert!(foreign.validate_calling_applications().is_err());
    }
}
