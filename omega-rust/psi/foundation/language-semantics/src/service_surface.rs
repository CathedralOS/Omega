/*
We define operational service surfaces, service method signatures, and stable
service surface identities (`identities`).
*/

pub mod identities {
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ProviderIdentityId(u32);

    impl ProviderIdentityId {
        pub const fn from_index(index: u32) -> Self {
            Self(index)
        }

        pub const fn index(self) -> u32 {
            self.0
        }

        pub fn is_valid(self) -> bool {
            self.0 != 0
        }

        fn slot(self) -> Option<usize> {
            (self.0 != 0).then(|| self.0 as usize - 1)
        }

        fn from_slot(slot: usize) -> Self {
            Self(slot as u32 + 1)
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct ProviderIdentityTable {
        texts: Vec<Box<str>>,
        slots: std::collections::HashMap<Box<str>, ProviderIdentityId>,
    }

    impl ProviderIdentityTable {
        pub fn intern(&mut self, text: &str) -> ProviderIdentityId {
            if let Some(identity) = self.slots.get(text) {
                return *identity;
            }
            let identity = ProviderIdentityId::from_slot(self.texts.len());
            let text: Box<str> = text.into();
            self.texts.push(text.clone());
            self.slots.insert(text, identity);
            identity
        }

        pub fn id_at(&self, text: &str) -> Option<ProviderIdentityId> {
            self.slots.get(text).copied()
        }

        pub fn text(&self, identity: ProviderIdentityId) -> &str {
            identity
                .slot()
                .and_then(|slot| self.texts.get(slot))
                .map(AsRef::as_ref)
                .unwrap_or_default()
        }

        pub fn texts(&self) -> impl Iterator<Item = &str> {
            self.texts.iter().map(AsRef::as_ref)
        }
    }
}
use identities::{ProviderIdentityId, ProviderIdentityTable};

#[derive(Debug, Clone, Default)]
pub struct ServiceSurface {
    pub trait_name: ProviderIdentityId,
    pub identities: ProviderIdentityTable,
    pub trait_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub receiver_applications: Vec<ServiceReceiverApplication>,
    pub methods: Vec<ServiceMethod>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceReceiverApplication {
    pub application: ProviderIdentityId,
    pub trait_name: ProviderIdentityId,
    pub trait_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub argument_identities: Vec<ProviderIdentityId>,
    pub lifetime_argument_identities: Vec<ProviderIdentityId>,
    pub requirement_identities: Vec<ProviderIdentityId>,
}

impl ServiceReceiverApplication {
    pub fn eq_in(
        &self,
        left: &ProviderIdentityTable,
        other: &Self,
        right: &ProviderIdentityTable,
    ) -> bool {
        left.text(self.application) == right.text(other.application)
            && self.trait_package_identity == other.trait_package_identity
            && left.text(self.trait_name) == right.text(other.trait_name)
            && identity_sequence_eq(
                &self.argument_identities,
                left,
                &other.argument_identities,
                right,
            )
            && identity_sequence_eq(
                &self.lifetime_argument_identities,
                left,
                &other.lifetime_argument_identities,
                right,
            )
            && identity_sequence_eq(
                &self.requirement_identities,
                left,
                &other.requirement_identities,
                right,
            )
    }
}

fn identity_sequence_eq(
    left: &[ProviderIdentityId],
    left_table: &ProviderIdentityTable,
    right: &[ProviderIdentityId],
    right_table: &ProviderIdentityTable,
) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left_table.text(*left) == right_table.text(*right))
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServiceMethod {
    pub completion: semantic_vocabulary::BoundaryCompletion,
    pub name: String,
    pub requirement_owner: String,
    pub requirement_owner_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub requirement_identity: ProviderIdentityId,
    pub parameter_count: usize,
    pub parameter_type_identities: Vec<ProviderIdentityId>,
    pub entry_claims: Vec<ServiceEntryClaim>,
    pub has_result: bool,
    pub result_type_identity: ProviderIdentityId,
    pub result_claims: Vec<ServiceResultClaim>,
    pub service_reach: Vec<String>,
    pub synchronous_invocations: Vec<String>,
    pub may_suspend: bool,
    pub may_block: bool,
    pub terminates_guarantee: bool,
    pub termination_premises: Vec<ServiceProgressPremise>,
}

impl ServiceMethod {
    pub fn eq_in(
        &self,
        left_table: &ProviderIdentityTable,
        other: &Self,
        right_table: &ProviderIdentityTable,
    ) -> bool {
        self.completion == other.completion
            && self.name == other.name
            && self.requirement_owner == other.requirement_owner
            && self.requirement_owner_package_identity == other.requirement_owner_package_identity
            && left_table.text(self.requirement_identity)
                == right_table.text(other.requirement_identity)
            && self.parameter_count == other.parameter_count
            && identity_sequence_eq(
                &self.parameter_type_identities,
                left_table,
                &other.parameter_type_identities,
                right_table,
            )
            && self.entry_claims.len() == other.entry_claims.len()
            && self
                .entry_claims
                .iter()
                .zip(&other.entry_claims)
                .all(|(left, right)| {
                    left.parameter_index == right.parameter_index
                        && left_table.text(left.carrier_identity)
                            == right_table.text(right.carrier_identity)
                        && left.domain == right.domain
                        && left.predicate_presence == right.predicate_presence
                        && left.effective_carry == right.effective_carry
                        && left.authority_flow == right.authority_flow
                })
            && self.has_result == other.has_result
            && left_table.text(self.result_type_identity)
                == right_table.text(other.result_type_identity)
            && self.result_claims == other.result_claims
            && self.service_reach == other.service_reach
            && self.synchronous_invocations == other.synchronous_invocations
            && self.may_suspend == other.may_suspend
            && self.may_block == other.may_block
            && self.terminates_guarantee == other.terminates_guarantee
            && self.termination_premises.len() == other.termination_premises.len()
            && self
                .termination_premises
                .iter()
                .zip(&other.termination_premises)
                .all(|(left, right)| left.eq_in(left_table, right, right_table))
    }
}

impl ServiceProgressPremise {
    pub fn eq_in(
        &self,
        left_table: &ProviderIdentityTable,
        other: &Self,
        right_table: &ProviderIdentityTable,
    ) -> bool {
        self.profile == other.profile
            && self.subject == other.subject
            && self.subject_projections == other.subject_projections
            && self.establishment_routes.len() == other.establishment_routes.len()
            && self
                .establishment_routes
                .iter()
                .zip(&other.establishment_routes)
                .all(|(left, right)| {
                    left.kind == right.kind
                        && left_table.text(left.requirement_identity)
                            == right_table.text(right.requirement_identity)
                })
    }
}

impl PartialEq for ServiceSurface {
    fn eq(&self, other: &Self) -> bool {
        self.trait_package_identity == other.trait_package_identity
            && self.receiver_applications.len() == other.receiver_applications.len()
            && self
                .receiver_applications
                .iter()
                .zip(&other.receiver_applications)
                .all(|(left, right)| left.eq_in(&self.identities, right, &other.identities))
            && self.methods.len() == other.methods.len()
            && self.identity_text(self.trait_name) == other.identity_text(other.trait_name)
            && self
                .methods
                .iter()
                .zip(&other.methods)
                .all(|(left, right)| left.eq_in(&self.identities, right, &other.identities))
    }
}

impl Eq for ServiceSurface {}

impl ServiceSurface {
    pub fn matches_receiving_application(
        &self,
        requirement: &str,
        receiving: Option<&str>,
    ) -> bool {
        let Some(method) = self
            .methods
            .iter()
            .find(|method| self.identity_text(method.requirement_identity) == requirement)
        else {
            return false;
        };
        let views = || {
            self.receiver_applications.iter().filter(|application| {
                self.identity_text(application.trait_name) == method.requirement_owner
                    && application.trait_package_identity
                        == method.requirement_owner_package_identity
                    && application
                        .requirement_identities
                        .iter()
                        .any(|identity| self.identity_text(*identity) == requirement)
            })
        };
        if !self.receiver_applications.is_empty() && views().next().is_none() {
            return false;
        }
        let Some(receiving) = receiving else {
            return views().all(|application| {
                application.argument_identities.is_empty()
                    && application.lifetime_argument_identities.is_empty()
            });
        };
        if receiving.is_empty() {
            return false;
        }
        views().any(|application| self.identity_text(application.application) == receiving)
    }

    pub fn selection_slot_identity(
        &self,
    ) -> (
        Option<semantic_vocabulary::PackageKeyIdentity>,
        &str,
        Option<&str>,
    ) {
        (
            self.trait_package_identity,
            self.identity_text(self.trait_name),
            self.root_application_identity(),
        )
    }

    pub fn root_application_identity(&self) -> Option<&str> {
        let root = self.receiver_applications.first()?;
        let identity = self.identity_text(root.application);
        (!identity.is_empty()).then_some(identity)
    }

    pub fn validate_receiver_applications(&self) -> Result<(), &'static str> {
        let Some(root) = self.receiver_applications.first() else {
            return Ok(());
        };
        if root.trait_package_identity != self.trait_package_identity
            || self.identity_text(root.trait_name) != self.identity_text(self.trait_name)
        {
            return Err("receiver roster root differs from its whole schema owner");
        }
        if root.requirement_identities.len() != self.methods.len()
            || self.methods.iter().any(|method| {
                !root
                    .requirement_identities
                    .contains(&method.requirement_identity)
            })
        {
            return Err("receiver roster root omits or adds an original schema requirement");
        }
        for (position, application) in self.receiver_applications.iter().enumerate() {
            if self.identity_text(application.trait_name).is_empty()
                || application
                    .argument_identities
                    .iter()
                    .chain(&application.lifetime_argument_identities)
                    .any(|identity| self.identity_text(*identity).is_empty())
            {
                return Err("receiver application has an absent or foreign identity");
            }
            if application.requirement_identities.iter().any(|identity| {
                self.identity_text(*identity).is_empty()
                    || !self
                        .methods
                        .iter()
                        .any(|method| method.requirement_identity == *identity)
            }) {
                return Err("receiver application names a requirement outside its whole schema");
            }
            if application
                .requirement_identities
                .windows(2)
                .any(|pair| self.identity_text(pair[0]) >= self.identity_text(pair[1]))
            {
                return Err("receiver requirement roster is duplicated or not canonical");
            }
            if self.receiver_applications[..position]
                .iter()
                .any(|previous| {
                    previous.trait_package_identity == application.trait_package_identity
                        && self.identity_text(previous.trait_name)
                            == self.identity_text(application.trait_name)
                        && identity_sequence_eq(
                            &previous.argument_identities,
                            &self.identities,
                            &application.argument_identities,
                            &self.identities,
                        )
                        && identity_sequence_eq(
                            &previous.lifetime_argument_identities,
                            &self.identities,
                            &application.lifetime_argument_identities,
                            &self.identities,
                        )
                })
            {
                return Err("receiver application is repeated within one whole schema");
            }
        }
        Ok(())
    }

    pub fn identity_text(&self, identity: ProviderIdentityId) -> &str {
        self.identities.text(identity)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceProgressPremise {
    pub profile: String,
    pub subject: ServiceProgressSubject,
    pub subject_projections: Vec<String>,
    pub establishment_routes: Vec<ServiceProgressEstablishmentRoute>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceProgressEstablishmentRoute {
    pub kind: ServiceProgressEstablishmentRouteKind,
    pub requirement_identity: ProviderIdentityId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ServiceProgressEstablishmentRouteKind {
    CheckedRequirement,
    BoundaryRequirement,
}

impl ServiceProgressEstablishmentRouteKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CheckedRequirement => "checked_requirement",
            Self::BoundaryRequirement => "boundary_requirement",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ServiceProgressSubject {
    Parameter(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceEntryAuthorityFlow {
    Accepts,
}

impl ServiceEntryAuthorityFlow {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Accepts => "accepts",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceEntryClaim {
    pub parameter_index: usize,
    pub carrier_identity: ProviderIdentityId,
    pub domain: String,
    pub predicate_presence: crate::DomainPredicatePresence,
    pub effective_carry: crate::CarryPolicy,
    pub authority_flow: ServiceEntryAuthorityFlow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceResultClaim {
    pub domain: String,
    pub effective_carry: crate::CarryPolicy,
}

// Nominal declaration coordinates only; this carries no source tree or proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderSchemaDeclaration {
    BoundaryTrait(symbols::SymbolHandle),
    BoundaryRequirement(symbols::SymbolHandle),
    BoundaryOperator(symbols::SymbolHandle),
    SealedAccessor(symbols::SymbolHandle),
}

impl ProviderSchemaDeclaration {
    pub const fn symbol(self) -> symbols::SymbolHandle {
        match self {
            Self::BoundaryTrait(symbol)
            | Self::BoundaryRequirement(symbol)
            | Self::BoundaryOperator(symbol)
            | Self::SealedAccessor(symbol) => symbol,
        }
    }
}
