//! The provider plan digest encoder.

use crate::effects::capabilities::provider_plan::{
    ProviderBinding, ServiceEntryAuthorityFlow, ServiceMethod,
    ServiceProgressEstablishmentRouteKind, ServiceProgressSubject,
};
use sha2::Digest;
use sha2::Sha256;

pub(crate) struct ProviderPlanDigestEncoder(Sha256);

impl ProviderPlanDigestEncoder {
    pub(crate) fn for_provider_plan() -> Self {
        let mut digest = Sha256::new();
        digest.update(b"omega.provider-plan.sha256.v1\0");
        Self(digest)
    }

    pub(crate) fn for_service_schema() -> Self {
        let mut digest = Sha256::new();
        digest.update(b"omega.service-schema.sha256.v1\0");
        Self(digest)
    }

    pub(crate) fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }

    fn byte(&mut self, value: u8) {
        self.0.update([value]);
    }

    fn bool(&mut self, value: bool) {
        self.byte(u8::from(value));
    }

    pub(crate) fn len(&mut self, value: usize) {
        self.0.update((value as u64).to_le_bytes());
    }

    pub(crate) fn u64(&mut self, value: u64) {
        self.0.update(value.to_le_bytes());
    }

    fn i64(&mut self, value: i64) {
        self.0.update(value.to_le_bytes());
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.len(bytes.len());
        self.0.update(bytes);
    }

    pub(crate) fn string(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }

    fn strings(&mut self, values: &[String]) {
        self.len(values.len());
        for value in values {
            self.string(value);
        }
    }

    pub(crate) fn package_identity(
        &mut self,
        identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    ) {
        match identity {
            Some(identity) => {
                self.byte(1);
                self.0.update(identity.digest());
            }
            None => self.byte(0),
        }
    }

    fn carry_policy(&mut self, policy: language_semantics::CarryPolicy) {
        self.byte(match policy.suspension {
            language_semantics::CarrySuspension::Forbidden => 0,
            language_semantics::CarrySuspension::Allowed => 1,
        });
        self.byte(match policy.cpu {
            language_semantics::CarryCpu::Origin => 0,
            language_semantics::CarryCpu::Any => 1,
        });
        self.byte(match policy.host_thread {
            language_semantics::CarryHostThread::Origin => 0,
            language_semantics::CarryHostThread::Any => 1,
        });
        self.byte(match policy.address {
            language_semantics::CarryAddress::Stable => 0,
            language_semantics::CarryAddress::Movable => 1,
        });
    }

    pub(crate) fn service_method(&mut self, method: &ServiceMethod) {
        self.string(&method.name);
        self.string(&method.requirement_owner);
        self.package_identity(method.requirement_owner_package_identity);
        self.string(&method.requirement_identity);
        self.len(method.parameter_count);
        self.strings(&method.parameter_type_identities);

        let mut entry_claims = method.entry_claims.iter().collect::<Vec<_>>();
        entry_claims.sort_by(|left, right| {
            left.parameter_index
                .cmp(&right.parameter_index)
                .then_with(|| left.carrier_identity.cmp(&right.carrier_identity))
                .then_with(|| left.domain.cmp(&right.domain))
        });
        self.len(entry_claims.len());
        for claim in entry_claims {
            self.len(claim.parameter_index);
            self.string(&claim.carrier_identity);
            self.string(&claim.domain);
            self.byte(match claim.predicate_body {
                language_semantics::DomainPredicateBody::Bodyless => 0,
                language_semantics::DomainPredicateBody::Present => 1,
            });
            self.carry_policy(claim.effective_carry);
            self.byte(match claim.authority_flow {
                ServiceEntryAuthorityFlow::Accepts => 0,
            });
        }

        self.bool(method.has_result);
        match &method.result_type_identity {
            Some(identity) => {
                self.byte(1);
                self.string(identity);
            }
            None => self.byte(0),
        }
        let mut result_claims = method.result_claims.iter().collect::<Vec<_>>();
        result_claims.sort_by(|left, right| left.domain.cmp(&right.domain));
        self.len(result_claims.len());
        for claim in result_claims {
            self.string(&claim.domain);
            self.carry_policy(claim.effective_carry);
        }

        self.strings(&method.service_reach);
        self.strings(&method.synchronous_invocations);
        self.bool(method.may_suspend);
        self.bool(method.may_block);
        self.bool(method.terminates_guarantee);

        let mut premises = method.termination_premises.iter().collect::<Vec<_>>();
        premises.sort_by(|left, right| {
            left.profile
                .cmp(&right.profile)
                .then_with(|| left.subject.cmp(&right.subject))
                .then_with(|| left.subject_projections.cmp(&right.subject_projections))
        });
        self.len(premises.len());
        for premise in premises {
            self.string(&premise.profile);
            match premise.subject {
                ServiceProgressSubject::ProviderReceiver => self.byte(0),
                ServiceProgressSubject::Parameter(index) => {
                    self.byte(1);
                    self.len(index);
                }
            }
            self.strings(&premise.subject_projections);
            let mut routes = premise.establishment_routes.iter().collect::<Vec<_>>();
            routes.sort();
            self.len(routes.len());
            for route in routes {
                self.byte(match route.kind {
                    ServiceProgressEstablishmentRouteKind::CheckedRequirement => 0,
                    ServiceProgressEstablishmentRouteKind::BoundaryRequirement => 1,
                });
                self.string(&route.requirement_identity);
            }
        }

        match method.calling_plan_report_fingerprint {
            Some(fingerprint) => {
                self.byte(1);
                self.u64(fingerprint);
            }
            None => self.byte(0),
        }
        match method.calling_plan_commitment {
            Some(commitment) => {
                self.byte(1);
                self.bytes(&commitment.as_bytes());
            }
            None => self.byte(0),
        }
    }

    pub(crate) fn provider_binding(&mut self, binding: &ProviderBinding) {
        match binding {
            ProviderBinding::Import { evaluated } => {
                let locator = evaluated.locator();
                self.byte(0);
                self.string(locator.target().target_name());
                match locator.locator() {
                    target::ForeignLocatorCandidate::PeByName { library, export } => {
                        self.byte(0);
                        self.bytes(library);
                        self.bytes(export);
                    }
                    target::ForeignLocatorCandidate::PeByOrdinal { library, ordinal } => {
                        self.byte(1);
                        self.bytes(library);
                        self.0.update(ordinal.to_le_bytes());
                    }
                    target::ForeignLocatorCandidate::ElfVersioned {
                        object,
                        symbol,
                        version,
                    } => {
                        self.byte(2);
                        self.bytes(object);
                        self.bytes(symbol);
                        self.bytes(version);
                    }
                    target::ForeignLocatorCandidate::MachODylibSymbol {
                        install_name,
                        symbol,
                    } => {
                        self.byte(3);
                        self.bytes(install_name);
                        self.bytes(symbol);
                    }
                }
                self.bytes(&evaluated.receipt().identity_digest());
            }
            // Tag byte 1 belonged to the retired string-backed import
            // bootstrap. It stays unassigned so no later mechanism can
            // collide with digests of plans that once carried it.
            ProviderBinding::Syscall { number } => {
                self.byte(2);
                self.i64(*number);
            }
            ProviderBinding::CompilerIntrinsic { machine } => {
                self.byte(3);
                self.string(machine);
            }
            ProviderBinding::VtableSlot { index } => {
                self.byte(4);
                self.i64(*index);
            }
            ProviderBinding::VtableField { table, field } => {
                self.byte(5);
                self.string(table);
                self.string(field);
            }
            ProviderBinding::TableFunction { table, field } => {
                self.byte(6);
                self.string(table);
                self.string(field);
            }
            ProviderBinding::CheckedAdapter {
                machine_identity,
                machine_package_identity,
            } => {
                self.byte(7);
                self.string(machine_identity);
                self.package_identity(*machine_package_identity);
            }
        }
    }
}
