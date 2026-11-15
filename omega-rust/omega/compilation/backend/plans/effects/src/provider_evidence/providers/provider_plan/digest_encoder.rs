/*
We define the domain-separated SHA-256 encoder (`ProviderPlanDigestEncoder`)
and the shared framed-feed combinator trait (`DigestFeed`) for `ProviderPlan`
and `ServiceSchema` commitments. Interned identities always feed their
canonical resolved text rather than process-local table ordinals.
*/

use crate::provider_evidence::providers::provider_plan::{ProviderBinding, ProviderIdentityTable};
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

impl language_semantics::service_surface::ServiceContractSink for ProviderPlanDigestEncoder {
    fn service_contract_bytes(&mut self, bytes: &[u8]) {
        DigestFeed::raw(self, bytes);
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

    pub(crate) fn calling_applications(&mut self, calling: Vec<&super::ServiceCallingApplication>) {
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
