use crate::ast::item::{
    CapabilityContractKind, CapabilityMember, Item, TrustLevel, TrustMode, TrustPolicy,
};
use omega_core::arena::Arena;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TrustReport {
    pub targets: Arena<TrustTarget>,
    pub trust_roots: Arena<TrustRoot>,
    pub trusted_contracts: Arena<TrustedContract>,
    pub unresolved_trusts: Arena<UnresolvedTrustReference>,
    pub unchecked_policies: Arena<UncheckedTrustPolicy>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TrustTarget {
    pub name: String,
    pub host_provider: String,
    pub host_settings: usize,
    pub checked_trusts: usize,
    pub unchecked_trusts: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TrustRoot {
    pub name: String,
    pub token_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TrustedContract {
    pub capability: String,
    pub state: String,
    pub trust_level: String,
    pub resolved: bool,
    pub requires_count: usize,
    pub ensures_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UnresolvedTrustReference {
    pub capability: String,
    pub state: String,
    pub trust_level: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UncheckedTrustPolicy {
    pub target: String,
    pub name: String,
}

pub fn build_trust_report(items: &[Item], selected_target_name: Option<&str>) -> TrustReport {
    let mut report = TrustReport::default();
    let trust_root_names = collect_trust_root_names(items, &mut report);

    for item in items {
        match item {
            Item::Capability(capability) => {
                for member in &capability.members {
                    let CapabilityMember::State(state) = member else {
                        continue;
                    };

                    let mut requires_count = 0usize;
                    let mut ensures_count = 0usize;

                    for contract in &state.contracts {
                        match &contract.kind {
                            CapabilityContractKind::Requires => requires_count += 1,
                            CapabilityContractKind::Ensures => ensures_count += 1,
                            CapabilityContractKind::Trusted(trust_level) => {
                                let trust_level_name = trust_level_name(trust_level);
                                let resolved = trust_level_resolves(trust_level, &trust_root_names);

                                report.trusted_contracts.insert(TrustedContract {
                                    capability: capability.name.clone(),
                                    state: state.signature.name.clone(),
                                    trust_level: trust_level_name.clone(),
                                    resolved,
                                    requires_count,
                                    ensures_count,
                                });

                                if !resolved {
                                    report.unresolved_trusts.insert(UnresolvedTrustReference {
                                        capability: capability.name.clone(),
                                        state: state.signature.name.clone(),
                                        trust_level: trust_level_name,
                                    });
                                }
                            }
                        }
                    }
                }
            }
            Item::Target(target) => {
                if selected_target_name.is_some_and(|selected| target.name != selected) {
                    continue;
                }

                let mut checked_trusts = 0usize;
                let mut unchecked_trusts = 0usize;

                for policy in &target.trust_policies {
                    match policy.mode {
                        TrustMode::Checked => checked_trusts += 1,
                        TrustMode::Unchecked => {
                            unchecked_trusts += 1;
                            report.unchecked_policies.insert(UncheckedTrustPolicy {
                                target: target.name.clone(),
                                name: policy_name(policy),
                            });
                        }
                    }
                }

                report.targets.insert(TrustTarget {
                    name: target.name.clone(),
                    host_provider: target
                        .host
                        .as_ref()
                        .map(|host| host.provider.join("::"))
                        .unwrap_or_else(|| "none".to_owned()),
                    host_settings: target
                        .host
                        .as_ref()
                        .map(|host| host.settings.len())
                        .unwrap_or(0),
                    checked_trusts,
                    unchecked_trusts,
                });
            }
            _ => {}
        }
    }

    report
}

fn collect_trust_root_names(items: &[Item], report: &mut TrustReport) -> Vec<String> {
    let mut names = Vec::new();

    for item in items {
        let Item::TrustDefinition(trust_definition) = item else {
            continue;
        };

        names.push(trust_definition.name.clone());
        report.trust_roots.insert(TrustRoot {
            name: trust_definition.name.clone(),
            token_count: trust_definition.token_count,
        });
    }

    names
}

fn trust_level_resolves(trust_level: &TrustLevel, trust_root_names: &[String]) -> bool {
    match trust_level {
        TrustLevel::Host => true,
        TrustLevel::Named(name) => trust_root_names.iter().any(|root_name| root_name == name),
    }
}

fn trust_level_name(trust_level: &TrustLevel) -> String {
    match trust_level {
        TrustLevel::Host => "host".to_owned(),
        TrustLevel::Named(name) => name.clone(),
    }
}

fn policy_name(policy: &TrustPolicy) -> String {
    policy.path.join("::")
}
