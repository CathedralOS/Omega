//! Process-wide service registrations and the atomic handover between two of them.

/// Collision and handover semantics selected by one named process-static
/// service. The component framework validates this contract but does not infer
/// a policy from the service's implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProcessStaticServicePolicy {
    RejectDuplicate,
    Versioned,
    AtomicTransfer { handover_contract_identity: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProcessStaticServiceContract {
    pub service_identity: String,
    pub policy: ProcessStaticServicePolicy,
}

/// Candidate registration owned by one component era. Establishment evidence
/// remains exact even though the process-static service outlives that era.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ServiceRegistrationCandidate {
    pub registration_identity: u64,
    pub component_era_identity: u64,
    pub logical_key: String,
    pub version_identity: Option<String>,
    pub establishment_receipt_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActiveServiceRegistration {
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

    pub(crate) fn version_identity(&self) -> Option<&str> {
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
pub(crate) struct AtomicServiceHandoverReceipt {
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
pub(crate) struct ServiceHandoverCompletion {
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
pub(crate) struct ProcessStaticServiceRegistry {
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


    pub fn register(
        &mut self,
        candidate: ServiceRegistrationCandidate,
    ) -> Result<(), Box<ServiceRegistrationError>> {
        if let Err(diagnostic) = validate_candidate(&self.contract, &candidate) {
            return Err(Box::new(ServiceRegistrationError {
                diagnostic,
            }));
        }
        if self.active.iter().any(|registration| {
            registration.registration_identity == candidate.registration_identity
        }) {
            return Err(Box::new(ServiceRegistrationError {
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
                diagnostic: diagnostic.into(),
            }));
        }

        self.active.push(candidate.into());
        sort_active(&mut self.active);
        Ok(())
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
pub(crate) struct ServiceRegistrationError {
    diagnostic: String,
}

impl ServiceRegistrationError {
    pub const fn diagnostic(&self) -> &str {
        self.diagnostic.as_str()
    }

}

#[derive(Debug)]
pub(crate) struct ServiceHandoverError {
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
