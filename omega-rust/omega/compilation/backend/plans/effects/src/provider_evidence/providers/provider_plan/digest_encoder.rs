/*
We define the domain-separated SHA-256 encoder (`ProviderPlanDigestEncoder`)
and the shared framed-feed combinator trait (`DigestFeed`) for `ProviderPlan`
and `ServiceSchema` commitments. Interned identities always feed their
canonical resolved text rather than process-local table ordinals.
*/

use crate::provider_evidence::providers::provider_plan::{
    ProviderBinding, ProviderIdentityTable, ServiceEntryAuthorityFlow, ServiceMethod,
    ServiceProgressEstablishmentRouteKind, ServiceProgressSubject,
};
use sha2::Digest;
use sha2::Sha256;

pub(crate) trait DigestFeed {
    fn raw(&mut self, bytes: &[u8]);

    fn byte(&mut self, value: u8) {
        self.raw(&[value]);
    }

    fn bool(&mut self, value: bool) {
        self.byte(u8::from(value));
    }

    fn len(&mut self, value: usize) {
        self.raw(&(value as u64).to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.raw(&value.to_le_bytes());
    }

    fn i64(&mut self, value: i64) {
        self.raw(&value.to_le_bytes());
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.len(bytes.len());
        self.raw(bytes);
    }

    fn string(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }

    fn strings(&mut self, values: &[String]) {
        self.len(values.len());
        for value in values {
            self.string(value);
        }
    }
}

pub(super) const PROVIDER_PLAN_DOMAIN: &[u8] = b"omega.provider-plan.sha256.v8\0";

pub(crate) struct ProviderPlanDigestEncoder(Sha256, Option<Vec<u8>>);

impl DigestFeed for ProviderPlanDigestEncoder {
    fn raw(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
        if let Some(preimage) = &mut self.1 {
            preimage.extend_from_slice(bytes);
        }
    }
}

impl ProviderPlanDigestEncoder {
    pub(crate) fn for_provider_plan() -> Self {
        let mut digest = Sha256::new();
        digest.update(PROVIDER_PLAN_DOMAIN);
        Self(digest, None)
    }

    pub(crate) fn for_provider_plan_preimage() -> Self {
        let mut encoder = Self(Sha256::new(), Some(Vec::new()));
        encoder.raw(PROVIDER_PLAN_DOMAIN);
        encoder
    }

    pub(crate) fn finish_preimage(self) -> Vec<u8> {
        self.1.expect("preimage encoder")
    }

    pub(crate) fn for_service_schema() -> Self {
        let mut digest = Sha256::new();
        digest.update(b"omega.service-schema.sha256.v6\0");
        Self(digest, None)
    }

    pub(crate) fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }

    pub(crate) fn receiver_applications(&mut self, schema: &super::ServiceSchema) {
        self.len(schema.declaration.receiver_applications.len());
        for application in &schema.declaration.receiver_applications {
            self.string(schema.identity_text(application.application));
            self.string(schema.identity_text(application.trait_name));
            self.package_identity(application.trait_package_identity);
            self.len(application.argument_identities.len());
            for argument in &application.argument_identities {
                self.string(schema.identity_text(*argument));
            }
            self.len(application.lifetime_argument_identities.len());
            for argument in &application.lifetime_argument_identities {
                self.string(schema.identity_text(*argument));
            }
            self.len(application.requirement_identities.len());
            for requirement in &application.requirement_identities {
                self.string(schema.identity_text(*requirement));
            }
        }
    }

    pub(crate) fn package_identity(
        &mut self,
        identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    ) {
        match identity {
            Some(identity) => {
                self.byte(1);
                self.raw(&identity.digest());
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

    pub(crate) fn service_method(
        &mut self,
        method: &ServiceMethod,
        identities: &ProviderIdentityTable,
        calling: Vec<&super::ServiceCallingApplication>,
    ) {
        self.string(&method.name);
        self.string(&method.requirement_owner);
        self.package_identity(method.requirement_owner_package_identity);
        self.string(identities.text(method.requirement_identity));
        self.len(method.parameter_count);
        for identity in &method.parameter_type_identities {
            self.string(identities.text(*identity));
        }

        let mut entry_claims = method.entry_claims.iter().collect::<Vec<_>>();
        entry_claims.sort_by_cached_key(|claim| {
            (
                claim.parameter_index,
                identities.text(claim.carrier_identity).to_owned(),
                claim.domain.clone(),
            )
        });
        self.len(entry_claims.len());
        for claim in entry_claims {
            self.len(claim.parameter_index);
            self.string(identities.text(claim.carrier_identity));
            self.string(&claim.domain);
            self.byte(match claim.predicate_presence {
                language_semantics::DomainPredicatePresence::Absent => 0,
                language_semantics::DomainPredicatePresence::Present => 1,
            });
            self.carry_policy(claim.effective_carry);
            self.byte(match claim.authority_flow {
                ServiceEntryAuthorityFlow::Accepts => 0,
            });
        }

        self.byte(match method.completion {
            semantic_vocabulary::BoundaryCompletion::NormalReturn => 0,
            semantic_vocabulary::BoundaryCompletion::External(
                semantic_vocabulary::ExternalCompletionKind::ProcessExit,
            ) => 1,
        });
        self.bool(method.has_result);
        if !method.result_type_identity.is_valid() {
            self.byte(0);
        } else {
            self.byte(1);
            self.string(identities.text(method.result_type_identity));
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
                ServiceProgressSubject::Parameter(index) => {
                    self.byte(1);
                    self.len(index);
                }
            }
            self.strings(&premise.subject_projections);
            let mut routes = premise.establishment_routes.iter().collect::<Vec<_>>();
            routes.sort_by(|left, right| {
                left.kind.cmp(&right.kind).then_with(|| {
                    identities
                        .text(left.requirement_identity)
                        .cmp(identities.text(right.requirement_identity))
                })
            });
            self.len(routes.len());
            for route in routes {
                self.byte(match route.kind {
                    ServiceProgressEstablishmentRouteKind::CheckedRequirement => 0,
                    ServiceProgressEstablishmentRouteKind::BoundaryRequirement => 1,
                });
                self.string(identities.text(route.requirement_identity));
            }
        }

        self.len(calling.len());
        for application in calling {
            self.string(&application.abi.requirement_identity);
            self.string(&application.abi.declaring_application);
            self.len(application.abi.operands.len());
            for operand in &application.abi.operands {
                self.u64(u64::from(operand.ordinal));
                self.byte(operand.kind as u8);
                self.string(&operand.identity);
            }
            self.u64(application.plan.report_fingerprint);
            self.bytes(&application.plan.commitment.as_bytes());
        }
    }

    pub(crate) fn provider_binding(
        &mut self,
        binding: &ProviderBinding,
        identities: &ProviderIdentityTable,
    ) {
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
                        self.raw(&ordinal.to_le_bytes());
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
            ProviderBinding::VtableField {
                table,
                field,
                storage,
            } => {
                self.byte(5);
                self.string(table);
                self.string(field);
                self.table_field_storage(storage);
            }
            ProviderBinding::TableFunction {
                table,
                field,
                storage,
            } => {
                self.byte(6);
                self.string(table);
                self.string(field);
                self.table_field_storage(storage);
            }
            ProviderBinding::CheckedAdapter {
                machine_identity,
                machine_package_identity,
            } => {
                self.byte(7);
                self.string(identities.text(*machine_identity));
                self.package_identity(*machine_package_identity);
            }
        }
    }

    fn table_field_storage(&mut self, storage: &Result<super::TableFieldStorage, String>) {
        self.bool(storage.is_ok());
        match storage {
            Ok(storage) => {
                self.string(&storage.table_application);
                self.string(&storage.field_declaration);
            }
            Err(reason) => self.string(reason),
        }
    }
}
