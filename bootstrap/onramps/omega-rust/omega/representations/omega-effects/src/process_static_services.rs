/// Collision and handover semantics selected by one named process-static
/// service. The component framework validates this contract but does not infer
/// a policy from the service's implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessStaticServicePolicy {
    RejectDuplicate,
    Versioned,
    AtomicTransfer { handover_contract_identity: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessStaticServiceContract {
    pub service_identity: String,
    pub policy: ProcessStaticServicePolicy,
}

/// Candidate registration owned by one component era. Establishment evidence
/// remains exact even though the process-static service outlives that era.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceRegistrationCandidate {
    pub registration_identity: u64,
    pub component_era_identity: u64,
    pub logical_key: String,
    pub version_identity: Option<String>,
    pub establishment_receipt_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveServiceRegistration {
    registration_identity: u64,
    component_era_identity: u64,
    logical_key: String,
    version_identity: Option<String>,
    establishment_receipt_identity: String,
}

impl ActiveServiceRegistration {
    pub const fn registration_identity(&self) -> u64 {
        self.registration_identity
    }

    pub const fn component_era_identity(&self) -> u64 {
        self.component_era_identity
    }

    pub const fn logical_key(&self) -> &str {
        self.logical_key.as_str()
    }

    pub fn version_identity(&self) -> Option<&str> {
        self.version_identity.as_deref()
    }

    pub const fn establishment_receipt_identity(&self) -> &str {
        self.establishment_receipt_identity.as_str()
    }
}

/// Provider evidence for one exact atomic name handover. Atomic visibility,
/// retirement of the old registration, and transfer of its obligations are
/// independent facts; none is inferred from the others.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtomicServiceHandoverReceipt {
    service_identity: String,
    handover_contract_identity: String,
    handover_receipt_identity: String,
    logical_key: String,
    previous_registration_identity: u64,
    previous_component_era_identity: u64,
    candidate_registration_identity: u64,
    candidate_component_era_identity: u64,
    candidate_establishment_receipt_identity: String,
    new_published_atomically: bool,
    previous_registration_retired: bool,
    obligations_transferred: bool,
}

impl AtomicServiceHandoverReceipt {
    #[allow(clippy::too_many_arguments)]
    pub fn from_provider(
        contract: &ProcessStaticServiceContract,
        previous: &ActiveServiceRegistration,
        candidate: &ServiceRegistrationCandidate,
        handover_receipt_identity: String,
        new_published_atomically: bool,
        previous_registration_retired: bool,
        obligations_transferred: bool,
    ) -> Self {
        let handover_contract_identity = match &contract.policy {
            ProcessStaticServicePolicy::AtomicTransfer {
                handover_contract_identity,
            } => handover_contract_identity.clone(),
            ProcessStaticServicePolicy::RejectDuplicate | ProcessStaticServicePolicy::Versioned => {
                String::new()
            }
        };
        Self {
            service_identity: contract.service_identity.clone(),
            handover_contract_identity,
            handover_receipt_identity,
            logical_key: previous.logical_key.clone(),
            previous_registration_identity: previous.registration_identity,
            previous_component_era_identity: previous.component_era_identity,
            candidate_registration_identity: candidate.registration_identity,
            candidate_component_era_identity: candidate.component_era_identity,
            candidate_establishment_receipt_identity: candidate
                .establishment_receipt_identity
                .clone(),
            new_published_atomically,
            previous_registration_retired,
            obligations_transferred,
        }
    }
}

/// Retained outcome of a successful transfer. The previous registration's
/// obligation did not disappear; it moved to the exact new registration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceHandoverCompletion {
    service_identity: String,
    logical_key: String,
    previous_registration_identity: u64,
    new_registration_identity: u64,
    handover_receipt_identity: String,
}

impl ServiceHandoverCompletion {
    pub const fn service_identity(&self) -> &str {
        self.service_identity.as_str()
    }

    pub const fn logical_key(&self) -> &str {
        self.logical_key.as_str()
    }

    pub const fn previous_registration_identity(&self) -> u64 {
        self.previous_registration_identity
    }

    pub const fn new_registration_identity(&self) -> u64 {
        self.new_registration_identity
    }

    pub const fn handover_receipt_identity(&self) -> &str {
        self.handover_receipt_identity.as_str()
    }

    pub const fn obligations_disappeared(&self) -> bool {
        false
    }
}

/// Generic active-registration carrier for one process-static service. It is
/// intentionally policy-neutral beyond enforcing the service's published
/// collision/handover contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessStaticServiceRegistry {
    contract: ProcessStaticServiceContract,
    active: Vec<ActiveServiceRegistration>,
    consumed_handover_receipts: Vec<String>,
}

impl ProcessStaticServiceRegistry {
    pub fn new(contract: ProcessStaticServiceContract) -> Result<Self, String> {
        validate_contract(&contract)?;
        Ok(Self {
            contract,
            active: Vec::new(),
            consumed_handover_receipts: Vec::new(),
        })
    }

    pub const fn contract(&self) -> &ProcessStaticServiceContract {
        &self.contract
    }

    pub fn active(&self) -> &[ActiveServiceRegistration] {
        &self.active
    }

    pub fn registration_for_key(&self, logical_key: &str) -> Option<&ActiveServiceRegistration> {
        self.active
            .iter()
            .find(|registration| registration.logical_key == logical_key)
    }

    pub fn register(
        &mut self,
        candidate: ServiceRegistrationCandidate,
    ) -> Result<(), Box<ServiceRegistrationError>> {
        if let Err(diagnostic) = validate_candidate(&self.contract, &candidate) {
            return Err(Box::new(ServiceRegistrationError {
                candidate,
                diagnostic,
            }));
        }
        if self.active.iter().any(|registration| {
            registration.registration_identity == candidate.registration_identity
        }) {
            return Err(Box::new(ServiceRegistrationError {
                candidate,
                diagnostic: "process-static service registration identity is already active".into(),
            }));
        }

        let collision = match &self.contract.policy {
            ProcessStaticServicePolicy::RejectDuplicate
            | ProcessStaticServicePolicy::AtomicTransfer { .. } => self
                .active
                .iter()
                .any(|registration| registration.logical_key == candidate.logical_key),
            ProcessStaticServicePolicy::Versioned => self.active.iter().any(|registration| {
                registration.logical_key == candidate.logical_key
                    && registration.version_identity == candidate.version_identity
            }),
        };
        if collision {
            let diagnostic = match self.contract.policy {
                ProcessStaticServicePolicy::RejectDuplicate => {
                    "process-static service rejects a duplicate logical key"
                }
                ProcessStaticServicePolicy::Versioned => {
                    "process-static service rejects a duplicate logical key/version pair"
                }
                ProcessStaticServicePolicy::AtomicTransfer { .. } => {
                    "process-static service requires an accepted atomic handover for an active logical key"
                }
            };
            return Err(Box::new(ServiceRegistrationError {
                candidate,
                diagnostic: diagnostic.into(),
            }));
        }

        self.active.push(candidate.into());
        sort_active(&mut self.active);
        Ok(())
    }

    pub fn atomic_handover(
        &mut self,
        candidate: ServiceRegistrationCandidate,
        receipt: AtomicServiceHandoverReceipt,
    ) -> Result<ServiceHandoverCompletion, Box<ServiceHandoverError>> {
        let ProcessStaticServicePolicy::AtomicTransfer {
            handover_contract_identity,
        } = &self.contract.policy
        else {
            return Err(Box::new(ServiceHandoverError {
                candidate,
                receipt,
                diagnostic: "process-static service does not publish atomic transfer".into(),
            }));
        };
        if let Err(diagnostic) = validate_candidate(&self.contract, &candidate) {
            return Err(Box::new(ServiceHandoverError {
                candidate,
                receipt,
                diagnostic,
            }));
        }
        let Some(previous_index) = self
            .active
            .iter()
            .position(|registration| registration.logical_key == candidate.logical_key)
        else {
            return Err(Box::new(ServiceHandoverError {
                candidate,
                receipt,
                diagnostic: "atomic service handover has no active previous registration".into(),
            }));
        };
        let previous = &self.active[previous_index];
        let receipt_matches = receipt.service_identity == self.contract.service_identity
            && receipt.handover_contract_identity == *handover_contract_identity
            && receipt.logical_key == candidate.logical_key
            && receipt.previous_registration_identity == previous.registration_identity
            && receipt.previous_component_era_identity == previous.component_era_identity
            && receipt.candidate_registration_identity == candidate.registration_identity
            && receipt.candidate_component_era_identity == candidate.component_era_identity
            && receipt.candidate_establishment_receipt_identity
                == candidate.establishment_receipt_identity;
        let diagnostic = if receipt.handover_receipt_identity.trim().is_empty() {
            Some("atomic service handover has no receipt identity")
        } else if self
            .consumed_handover_receipts
            .contains(&receipt.handover_receipt_identity)
        {
            Some("atomic service handover receipt is replayed")
        } else if !receipt_matches {
            Some(
                "atomic service handover receipt does not bind the exact contract, previous registration, and candidate",
            )
        } else if candidate.registration_identity == previous.registration_identity {
            Some("atomic service handover must establish a distinct registration identity")
        } else if self.active.iter().enumerate().any(|(index, registration)| {
            index != previous_index
                && registration.registration_identity == candidate.registration_identity
        }) {
            Some("atomic service handover candidate identity is already active")
        } else if !receipt.new_published_atomically {
            Some("atomic service handover does not prove atomic publication")
        } else if !receipt.previous_registration_retired {
            Some("atomic service handover does not retire the previous registration")
        } else if !receipt.obligations_transferred {
            Some("atomic service handover does not transfer previous obligations")
        } else {
            None
        };
        if let Some(diagnostic) = diagnostic {
            return Err(Box::new(ServiceHandoverError {
                candidate,
                receipt,
                diagnostic: diagnostic.into(),
            }));
        }

        let previous_registration_identity = previous.registration_identity;
        let completion = ServiceHandoverCompletion {
            service_identity: self.contract.service_identity.clone(),
            logical_key: candidate.logical_key.clone(),
            previous_registration_identity,
            new_registration_identity: candidate.registration_identity,
            handover_receipt_identity: receipt.handover_receipt_identity.clone(),
        };
        self.active[previous_index] = candidate.into();
        sort_active(&mut self.active);
        self.consumed_handover_receipts
            .push(receipt.handover_receipt_identity);
        Ok(completion)
    }
}

impl From<ServiceRegistrationCandidate> for ActiveServiceRegistration {
    fn from(candidate: ServiceRegistrationCandidate) -> Self {
        Self {
            registration_identity: candidate.registration_identity,
            component_era_identity: candidate.component_era_identity,
            logical_key: candidate.logical_key,
            version_identity: candidate.version_identity,
            establishment_receipt_identity: candidate.establishment_receipt_identity,
        }
    }
}

#[derive(Debug)]
pub struct ServiceRegistrationError {
    candidate: ServiceRegistrationCandidate,
    diagnostic: String,
}

impl ServiceRegistrationError {
    pub const fn diagnostic(&self) -> &str {
        self.diagnostic.as_str()
    }

    pub fn into_candidate(self) -> ServiceRegistrationCandidate {
        self.candidate
    }
}

#[derive(Debug)]
pub struct ServiceHandoverError {
    candidate: ServiceRegistrationCandidate,
    receipt: AtomicServiceHandoverReceipt,
    diagnostic: String,
}

impl ServiceHandoverError {
    pub const fn diagnostic(&self) -> &str {
        self.diagnostic.as_str()
    }

    pub fn into_parts(self) -> (ServiceRegistrationCandidate, AtomicServiceHandoverReceipt) {
        (self.candidate, self.receipt)
    }
}

fn validate_contract(contract: &ProcessStaticServiceContract) -> Result<(), String> {
    if contract.service_identity.trim().is_empty() {
        return Err("process-static service has no identity".into());
    }
    if let ProcessStaticServicePolicy::AtomicTransfer {
        handover_contract_identity,
    } = &contract.policy
        && handover_contract_identity.trim().is_empty()
    {
        return Err("process-static service has no atomic-handover contract identity".into());
    }
    Ok(())
}

fn validate_candidate(
    contract: &ProcessStaticServiceContract,
    candidate: &ServiceRegistrationCandidate,
) -> Result<(), String> {
    if candidate.registration_identity == 0 {
        return Err("process-static service registration has the reserved zero identity".into());
    }
    if candidate.component_era_identity == 0 {
        return Err(
            "process-static service registration has the reserved zero era identity".into(),
        );
    }
    if candidate.logical_key.trim().is_empty() {
        return Err("process-static service registration has no logical key".into());
    }
    if candidate.establishment_receipt_identity.trim().is_empty() {
        return Err("process-static service registration has no establishment receipt".into());
    }
    match (&contract.policy, &candidate.version_identity) {
        (ProcessStaticServicePolicy::Versioned, Some(version)) if !version.trim().is_empty() => {}
        (ProcessStaticServicePolicy::Versioned, _) => {
            return Err(
                "versioned process-static service registration has no version identity".into(),
            );
        }
        (
            ProcessStaticServicePolicy::RejectDuplicate
            | ProcessStaticServicePolicy::AtomicTransfer { .. },
            None,
        ) => {}
        (
            ProcessStaticServicePolicy::RejectDuplicate
            | ProcessStaticServicePolicy::AtomicTransfer { .. },
            Some(_),
        ) => {
            return Err(
                "unversioned process-static service contract received a version identity".into(),
            );
        }
    }
    Ok(())
}

fn sort_active(active: &mut [ActiveServiceRegistration]) {
    active.sort_by(|left, right| {
        left.logical_key
            .cmp(&right.logical_key)
            .then_with(|| left.version_identity.cmp(&right.version_identity))
            .then_with(|| left.registration_identity.cmp(&right.registration_identity))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract(policy: ProcessStaticServicePolicy) -> ProcessStaticServiceContract {
        ProcessStaticServiceContract {
            service_identity: "ProcessRegistry".into(),
            policy,
        }
    }

    fn candidate(
        registration_identity: u64,
        era: u64,
        key: &str,
        version: Option<&str>,
    ) -> ServiceRegistrationCandidate {
        ServiceRegistrationCandidate {
            registration_identity,
            component_era_identity: era,
            logical_key: key.into(),
            version_identity: version.map(str::to_owned),
            establishment_receipt_identity: format!("receipt:establish:{registration_identity}"),
        }
    }

    #[test]
    fn reject_duplicate_policy_preserves_the_failed_candidate() {
        let mut registry = ProcessStaticServiceRegistry::new(contract(
            ProcessStaticServicePolicy::RejectDuplicate,
        ))
        .expect("contract");
        registry
            .register(candidate(1, 10, "console", None))
            .expect("first registration");
        let error = registry
            .register(candidate(2, 20, "console", None))
            .expect_err("duplicate logical key");
        assert!(error.diagnostic().contains("duplicate logical key"));
        let candidate = (*error).into_candidate();
        assert_eq!(candidate.registration_identity, 2);
        assert_eq!(registry.active().len(), 1);
    }

    #[test]
    fn versioned_policy_allows_distinct_versions_but_not_an_exact_collision() {
        let mut registry =
            ProcessStaticServiceRegistry::new(contract(ProcessStaticServicePolicy::Versioned))
                .expect("contract");
        registry
            .register(candidate(1, 10, "codec", Some("v1")))
            .expect("v1");
        registry
            .register(candidate(2, 20, "codec", Some("v2")))
            .expect("v2 coexists");
        let error = registry
            .register(candidate(3, 30, "codec", Some("v2")))
            .expect_err("duplicate version");
        assert!(error.diagnostic().contains("key/version pair"));
        assert_eq!(registry.active().len(), 2);
    }

    #[test]
    fn atomic_handover_requires_all_three_independent_provider_facts() {
        let service_contract = contract(ProcessStaticServicePolicy::AtomicTransfer {
            handover_contract_identity: "Registry::atomic_transfer/v1".into(),
        });
        let mut registry =
            ProcessStaticServiceRegistry::new(service_contract.clone()).expect("atomic contract");
        registry
            .register(candidate(1, 10, "window-class", None))
            .expect("initial owner");
        let next = candidate(2, 20, "window-class", None);
        let incomplete = AtomicServiceHandoverReceipt::from_provider(
            &service_contract,
            registry.registration_for_key("window-class").expect("old"),
            &next,
            "receipt:handover:1-to-2".into(),
            true,
            true,
            false,
        );
        let error = registry
            .atomic_handover(next, incomplete.clone())
            .expect_err("obligations were not transferred");
        assert!(error.diagnostic().contains("does not transfer"));
        let (next, returned_receipt) = (*error).into_parts();
        assert_eq!(returned_receipt, incomplete);
        assert_eq!(registry.active()[0].registration_identity(), 1);

        let complete = AtomicServiceHandoverReceipt::from_provider(
            &service_contract,
            registry
                .registration_for_key("window-class")
                .expect("old retained after failed handover"),
            &next,
            "receipt:handover:1-to-2".into(),
            true,
            true,
            true,
        );
        let completion = registry
            .atomic_handover(next, complete)
            .expect("exact atomic transfer");
        assert_eq!(completion.previous_registration_identity(), 1);
        assert_eq!(completion.new_registration_identity(), 2);
        assert!(!completion.obligations_disappeared());
        assert_eq!(registry.active()[0].registration_identity(), 2);
        assert_eq!(registry.active()[0].component_era_identity(), 20);
    }

    #[test]
    fn atomic_handover_rejects_identity_drift_and_receipt_replay() {
        let service_contract = contract(ProcessStaticServicePolicy::AtomicTransfer {
            handover_contract_identity: "Registry::atomic_transfer/v1".into(),
        });
        let mut registry =
            ProcessStaticServiceRegistry::new(service_contract.clone()).expect("atomic contract");
        registry
            .register(candidate(1, 10, "handler", None))
            .expect("initial owner");
        let next = candidate(2, 20, "handler", None);
        let receipt = AtomicServiceHandoverReceipt::from_provider(
            &service_contract,
            registry.registration_for_key("handler").expect("old"),
            &next,
            "receipt:handover:1-to-2".into(),
            true,
            true,
            true,
        );
        let mut drifted = receipt.clone();
        drifted.candidate_component_era_identity = 21;
        let error = registry
            .atomic_handover(next, drifted)
            .expect_err("era drift");
        assert!(error.diagnostic().contains("does not bind"));
        let (next, _) = (*error).into_parts();
        registry
            .atomic_handover(next, receipt.clone())
            .expect("first exact use");

        let third = candidate(3, 30, "handler", None);
        let mut replay = AtomicServiceHandoverReceipt::from_provider(
            &service_contract,
            registry.registration_for_key("handler").expect("new old"),
            &third,
            receipt.handover_receipt_identity,
            true,
            true,
            true,
        );
        replay.previous_registration_identity = 2;
        let error = registry
            .atomic_handover(third, replay)
            .expect_err("receipt replay");
        assert!(error.diagnostic().contains("replayed"));
        assert_eq!(registry.active()[0].registration_identity(), 2);
    }
}
