//! Keep the complete declared service contract independent of target calling plans.
use super::*;
use optimization::digest_encoder::DomainDigestEncoder;

/// A destination for the stable, target-neutral bytes of a service contract.
/// Hashing and retained byte controls share the field encoding, not storage.
pub trait ServiceContractSink {
    fn service_contract_bytes(&mut self, bytes: &[u8]);
}

impl ServiceContractSink for DomainDigestEncoder {
    fn service_contract_bytes(&mut self, bytes: &[u8]) {
        self.raw(bytes);
    }
}
impl ServiceContractSink for Vec<u8> {
    fn service_contract_bytes(&mut self, bytes: &[u8]) {
        self.extend_from_slice(bytes);
    }
}

impl ServiceSurface {
    pub fn declaration_commitment(&self) -> [u8; 32] {
        let mut digest = DomainDigestEncoder::new(b"omega.service-schema.sha256.v6\0");
        self.encode_contract(&mut digest, |_, output| output.len(0));
        digest.finish()
    }

    /// Write the declaration in v6 order. The caller writes its method suffix
    /// at the exact boundary between that method's contract and the next one.
    /// Target-neutral declarations write an empty calling-application count.
    pub fn encode_contract<Output: ServiceContractSink>(
        &self,
        output: &mut Output,
        mut method_suffix: impl FnMut(&ServiceMethod, &mut Output),
    ) {
        let mut encoder = ContractEncoder { output };
        encoder.string(self.identity_text(self.trait_name));
        encoder.package_identity(self.trait_package_identity);
        encoder.len(self.receiver_applications.len());
        for application in &self.receiver_applications {
            encoder.string(self.identity_text(application.application));
            encoder.string(self.identity_text(application.trait_name));
            encoder.package_identity(application.trait_package_identity);
            for identities in [
                &application.argument_identities,
                &application.lifetime_argument_identities,
                &application.requirement_identities,
            ] {
                encoder.len(identities.len());
                for identity in identities {
                    encoder.string(self.identity_text(*identity));
                }
            }
        }
        let mut methods = self.methods.iter().collect::<Vec<_>>();
        methods.sort_by(|left, right| {
            left.name.cmp(&right.name).then_with(|| {
                self.identity_text(left.requirement_identity)
                    .cmp(self.identity_text(right.requirement_identity))
            })
        });
        encoder.len(methods.len());
        for method in methods {
            encoder.service_method(method, &self.identities);
            method_suffix(method, encoder.output);
        }
    }
}

struct ContractEncoder<'a, Output> {
    output: &'a mut Output,
}
impl<Output: ServiceContractSink> ContractEncoder<'_, Output> {
    fn byte(&mut self, value: u8) {
        self.output.service_contract_bytes(&[value]);
    }
    fn bool(&mut self, value: bool) {
        self.byte(u8::from(value));
    }
    fn len(&mut self, value: usize) {
        self.output
            .service_contract_bytes(&(value as u64).to_le_bytes());
    }
    fn string(&mut self, value: &str) {
        self.len(value.len());
        self.output.service_contract_bytes(value.as_bytes());
    }
    fn strings(&mut self, values: &[String]) {
        self.len(values.len());
        for value in values {
            self.string(value);
        }
    }
    fn package_identity(&mut self, identity: Option<semantic_vocabulary::PackageKeyIdentity>) {
        match identity {
            Some(identity) => {
                self.byte(1);
                self.output.service_contract_bytes(&identity.digest());
            }
            None => self.byte(0),
        }
    }
    fn carry_policy(&mut self, policy: crate::CarryPolicy) {
        self.byte(match policy.suspension {
            crate::CarrySuspension::Forbidden => 0,
            crate::CarrySuspension::Allowed => 1,
        });
        self.byte(match policy.cpu {
            crate::CarryCpu::Origin => 0,
            crate::CarryCpu::Any => 1,
        });
        self.byte(match policy.host_thread {
            crate::CarryHostThread::Origin => 0,
            crate::CarryHostThread::Any => 1,
        });
        self.byte(match policy.address {
            crate::CarryAddress::Stable => 0,
            crate::CarryAddress::Movable => 1,
        });
    }

    fn service_method(&mut self, method: &ServiceMethod, identities: &ProviderIdentityTable) {
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
                crate::DomainPredicatePresence::Absent => 0,
                crate::DomainPredicatePresence::Present => 1,
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
    }
}
