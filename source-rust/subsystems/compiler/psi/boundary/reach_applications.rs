use crate::digest_encoder::DomainDigestEncoder;
use crate::terminal_module::TerminalIdentityIndex;
use psi_effect_contracts::{ConditionalCarryRequirement, ContractDependency};
use semantic_vocabulary::{MachineId, ServiceId};

/// The template publishes each binder's admissible reach, independently of
/// the callable chosen for this application. Use the service identities
/// rather than their module-local coordinates in the commitment.
pub fn closed_reach_template_commitment<'parameter, 'identity>(
    base: [u8; 32],
    parameters: impl IntoIterator<Item = &'parameter ClosedReachParameter>,
    suspension: &ContractDependency<bool, Vec<u32>>,
    blocking: &ContractDependency<bool, Vec<u32>>,
    carry: &[ConditionalCarryRequirement],
    service_identity: impl Fn(ServiceId) -> Option<&'identity str>,
) -> Result<[u8; 32], &'static str> {
    let mut bounds = None;
    let mut parameter_count = 0;
    for parameter in parameters {
        let position = parameter_count;
        parameter_count += 1;
        let ClosedReachParameter::Machine(binding) = parameter else {
            continue;
        };
        let digest = bounds.get_or_insert_with(|| {
            let mut digest =
                DomainDigestEncoder::new(b"omega.machine-template-binder-bounds.v1\0");
            digest.raw(&base);
            digest
        });
        digest.len(position);
        digest.byte(u8::from(binding.upper_bound_may_suspend));
        digest.byte(u8::from(binding.upper_bound_may_block));
        digest.byte(u8::from(binding.upper_bound_crashes_none));
        digest.byte(u8::from(binding.upper_bound.is_some()));
        if let Some(services) = &binding.upper_bound {
            let mut identities = services
                .iter()
                .map(|&service| {
                    service_identity(service)
                        .filter(|identity| !identity.is_empty())
                        .ok_or("closed reach bound has no exact service identity")
                })
                .collect::<Result<Vec<_>, _>>()?;
            identities.sort_unstable();
            digest.len(identities.len());
            for identity in identities {
                digest.bytes(identity.as_bytes());
            }
        }
    }
    psi_effect_contracts::template_contract_commitment(
        bounds.map_or(base, DomainDigestEncoder::finish),
        parameter_count,
        suspension,
        blocking,
        carry,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedReachApplication {
    pub template_identity: TerminalIdentityIndex,
    pub template_base_commitment: [u8; 32],
    pub template_commitment: [u8; 32],
    pub suspension_dependency: psi_effect_contracts::ContractDependency<bool, Vec<u32>>,
    pub blocking_dependency: psi_effect_contracts::ContractDependency<bool, Vec<u32>>,
    pub conditional_carry: Vec<psi_effect_contracts::ConditionalCarryRequirement>,
    pub specialization_commitment: [u8; 32],
    pub parameter_kinds: Vec<u8>,
    pub telescope: Vec<ClosedReachParameter>,
    pub capture: Option<ClosedReachCapture>,
    pub fixed: Vec<ServiceId>,
    pub dependencies: Vec<u32>,
    pub calls: Vec<ClosedReachCall>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedReachCapture {
    pub template_identity: TerminalIdentityIndex,
    pub template_base_commitment: [u8; 32],
    pub template_commitment: [u8; 32],
    pub suspension_dependency: psi_effect_contracts::ContractDependency<bool, Vec<u32>>,
    pub blocking_dependency: psi_effect_contracts::ContractDependency<bool, Vec<u32>>,
    pub conditional_carry: Vec<psi_effect_contracts::ConditionalCarryRequirement>,
    pub specialization_commitment: [u8; 32],
    pub parameter_kinds: Vec<u8>,
    pub telescope: Vec<ClosedReachParameter>,
}

impl ClosedReachApplication {
    pub fn parameters(&self) -> impl Iterator<Item = &ClosedReachParameter> {
        self.capture
            .iter()
            .flat_map(|capture| &capture.telescope)
            .chain(&self.telescope)
    }

    pub fn parameter(&self, position: u32) -> Option<&ClosedReachParameter> {
        self.parameters().nth(position as usize)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClosedReachParameter {
    Type {
        argument: String,
    },
    Const {
        argument: application_identity::const_value::CanonicalConstValue,
    },
    Proposition {
        argument: String,
    },
    Machine(ClosedReachMachineBinding),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedReachMachineBinding {
    pub nominal_requirement: Option<String>,
    pub requirement_parameter_kinds: Vec<u8>,
    pub requirement_arguments: Vec<TerminalIdentityIndex>,
    /// Omission defers to the selected callable. An empty row is an explicit bound.
    pub upper_bound: Option<Vec<ServiceId>>,
    pub upper_bound_may_suspend: bool,
    pub upper_bound_may_block: bool,
    /// The binder wrote `crashes none`, which admits only crash-free
    /// selections. Without it the binder defers to the selected callable.
    pub upper_bound_crashes_none: bool,
    pub selected_may_suspend: bool,
    pub selected_may_block: bool,
    /// The selected callee publishes at least one crash route, authored
    /// or inferred.
    pub selected_may_crash: bool,
    pub selected_identity: TerminalIdentityIndex,
    pub selected_contract_commitment: [u8; 32],
    pub selected_reach: Vec<ServiceId>,
    pub callee: Option<MachineId>,
    pub schema: Option<ClosedReachSchema>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedReachSchema {
    pub template_identity: TerminalIdentityIndex,
    pub template_commitment: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedReachCall {
    pub site: semantic_vocabulary::InvocationSite,
    pub binder: u32,
    pub application: Option<ClosedReachCallApplication>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedReachCallApplication {
    pub callee: MachineId,
    pub specialization_commitment: [u8; 32],
    pub arguments: Vec<ClosedReachArgument>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClosedReachArgument {
    Type(TerminalIdentityIndex),
    Const(TerminalIdentityIndex),
    Proposition(TerminalIdentityIndex),
    Machine {
        identity: TerminalIdentityIndex,
        contract_commitment: [u8; 32],
    },
}

impl ClosedReachArgument {
    pub fn same_identity_as(
        &self,
        identities: &crate::TerminalIdentityTable,
        other: &Self,
        other_identities: &crate::TerminalIdentityTable,
    ) -> bool {
        let (left, right) = match (self, other) {
            (Self::Type(left), Self::Type(right))
            | (Self::Const(left), Self::Const(right))
            | (Self::Proposition(left), Self::Proposition(right)) => (*left, *right),
            (
                Self::Machine {
                    identity: left,
                    contract_commitment: left_contract,
                },
                Self::Machine {
                    identity: right,
                    contract_commitment: right_contract,
                },
            ) if left_contract == right_contract => (*left, *right),
            _ => return false,
        };
        identities
            .text(left)
            .zip(other_identities.text(right))
            .is_some_and(|(left, right)| !left.is_empty() && left == right)
    }
}
