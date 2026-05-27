use crate::pipeline::compile_options::CompileOptions;
use omega_artifacts::{
    ArtifactWriter, TrustReport, TrustRoot, TrustTarget, TrustedContract, UncheckedTrustPolicy,
    UnresolvedTrustReference,
};
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::identifier::Identifier;
use omega_syntax_trees::item::{
    CapabilityContractKind, CapabilityMember, Item, TrustLevel, TrustMode,
};
use std::collections::{HashMap, HashSet};

pub(super) fn write_trust_report(
    options: &CompileOptions,
    syntax: &SyntaxTrees,
) -> Result<(), Vec<Diagnostic>> {
    let writer =
        ArtifactWriter::new(&options.build_dir()).map_err(|diagnostic| vec![diagnostic])?;
    writer
        .write_trust_report(&build_trust_report(syntax))
        .map_err(|diagnostic| vec![diagnostic])
}

fn build_trust_report(syntax: &SyntaxTrees) -> TrustReport {
    let mut report = TrustReport::default();
    let mut root_declarations = Vec::new();
    let mut roots = HashSet::new();
    let mut checked_root_uses = HashMap::<String, usize>::new();
    let mut unchecked_root_uses = HashMap::<String, usize>::new();

    for item in syntax.root_items() {
        if let Item::TrustDefinition(trust) = item {
            roots.insert(trust.name.to_string());
            root_declarations.push((trust.name.to_string(), trust.token_count));
        }
    }

    for item in syntax.root_items() {
        match item {
            Item::Capability(capability) => {
                for member in syntax.items.capability_members(capability.members) {
                    let CapabilityMember::State(state) = member else {
                        continue;
                    };
                    collect_trusted_contracts(
                        &mut report,
                        &roots,
                        &mut checked_root_uses,
                        capability.name.as_str(),
                        state.signature.name.as_str(),
                        syntax.items.capability_contracts(state.contracts),
                    );
                }
            }
            Item::Library(library) => {
                let library_name = library
                    .name
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| library.path.clone());
                for function in syntax.items.library_functions(library.functions) {
                    for trust in syntax.items.trust_levels(function.trusts) {
                        let trust_level = trust_level_name(trust);
                        let resolved = trust_resolves(trust, &roots);
                        record_trust_use(trust, &roots, &mut checked_root_uses);
                        report.trusted_contracts.insert(TrustedContract {
                            capability: library_name.clone(),
                            state: function.signature.name.to_string(),
                            trust_level: trust_level.clone(),
                            resolved,
                            requires_count: 0,
                            ensures_count: 0,
                        });
                        if !resolved {
                            report.unresolved_trusts.insert(UnresolvedTrustReference {
                                capability: library_name.clone(),
                                state: function.signature.name.to_string(),
                                trust_level,
                            });
                        }
                    }
                }
            }
            Item::Target(target) => {
                let policies = syntax.items.trust_policies(target.trust_policies);
                report.targets.insert(TrustTarget {
                    name: target.name.to_string(),
                    host_provider: target
                        .host
                        .as_ref()
                        .map(|host| identifier_path_name(syntax, host.provider))
                        .unwrap_or_else(|| "none".to_owned()),
                    host_settings: target.host.as_ref().map_or(0, |host| {
                        syntax.items.target_host_settings(host.settings).len()
                    }),
                    checked_trusts: policies
                        .iter()
                        .filter(|policy| matches!(policy.mode, TrustMode::Checked))
                        .count(),
                    unchecked_trusts: policies
                        .iter()
                        .filter(|policy| matches!(policy.mode, TrustMode::Unchecked))
                        .count(),
                });

                for policy in policies {
                    let trust_name = identifier_path_name(syntax, policy.path);
                    if matches!(policy.mode, TrustMode::Unchecked) {
                        report.unchecked_policies.insert(UncheckedTrustPolicy {
                            target: target.name.to_string(),
                            name: trust_name.clone(),
                        });
                    }
                    if roots.contains(&trust_name) {
                        let root_uses = if matches!(policy.mode, TrustMode::Unchecked) {
                            &mut unchecked_root_uses
                        } else {
                            &mut checked_root_uses
                        };
                        *root_uses.entry(trust_name.clone()).or_insert(0) += 1;
                    }
                    if trust_name != "host" && !roots.contains(&trust_name) {
                        report.unresolved_trusts.insert(UnresolvedTrustReference {
                            capability: "target".to_owned(),
                            state: target.name.to_string(),
                            trust_level: trust_name,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    for (name, token_count) in root_declarations {
        report.trust_roots.insert(TrustRoot {
            checked_references: checked_root_uses.get(&name).copied().unwrap_or(0),
            unchecked_references: unchecked_root_uses.get(&name).copied().unwrap_or(0),
            name,
            token_count,
        });
    }

    report
}

fn collect_trusted_contracts(
    report: &mut TrustReport,
    roots: &HashSet<String>,
    checked_root_uses: &mut HashMap<String, usize>,
    capability: &str,
    state: &str,
    contracts: &[omega_syntax_trees::item::CapabilityContract],
) {
    let requires_count = contracts
        .iter()
        .filter(|contract| matches!(contract.kind, CapabilityContractKind::Requires))
        .count();
    let ensures_count = contracts
        .iter()
        .filter(|contract| matches!(contract.kind, CapabilityContractKind::Ensures))
        .count();

    for contract in contracts {
        let CapabilityContractKind::Trusted(trust) = &contract.kind else {
            continue;
        };
        let trust_level = trust_level_name(trust);
        let resolved = trust_resolves(trust, roots);
        record_trust_use(trust, roots, checked_root_uses);
        report.trusted_contracts.insert(TrustedContract {
            capability: capability.to_owned(),
            state: state.to_owned(),
            trust_level: trust_level.clone(),
            resolved,
            requires_count,
            ensures_count,
        });
        if !resolved {
            report.unresolved_trusts.insert(UnresolvedTrustReference {
                capability: capability.to_owned(),
                state: state.to_owned(),
                trust_level,
            });
        }
    }
}

fn record_trust_use(
    trust: &TrustLevel,
    roots: &HashSet<String>,
    root_uses: &mut HashMap<String, usize>,
) {
    let TrustLevel::Named(name) = trust else {
        return;
    };
    if roots.contains(name.as_str()) {
        *root_uses.entry(name.to_string()).or_insert(0) += 1;
    }
}

fn trust_resolves(trust: &TrustLevel, roots: &HashSet<String>) -> bool {
    match trust {
        TrustLevel::Host => true,
        TrustLevel::Named(name) => roots.contains(name.as_str()),
    }
}

fn trust_level_name(trust: &TrustLevel) -> String {
    match trust {
        TrustLevel::Host => "host".to_owned(),
        TrustLevel::Named(name) => name.to_string(),
    }
}

fn identifier_path_name(syntax: &SyntaxTrees, path: HandleSpan<Identifier>) -> String {
    syntax
        .items
        .identifier_path_members(path)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::")
}

#[cfg(test)]
mod tests {
    use super::build_trust_report;
    use omega_source_files_to_tokens::Lexer;
    use omega_tokens_to_syntax_trees::parse_syntax_trees;

    #[test]
    fn trust_report_collects_roots_targets_and_unresolved_references() {
        let source = r#"
            trust compiler_slice_index {
            }

            capability Core {
                entry index() {
                    requires true;
                    ensures true;
                    trust compiler_slice_index;
                    trust missing_contract_root;
                }
            }

            target native {
                host: omega::host {
                    os = darwin
                }
                trust compiler_slice_index
                trust unchecked missing_target_root
            }
        "#;
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let report = build_trust_report(&syntax);

        assert_eq!(report.trust_roots.len(), 1);
        assert_eq!(report.targets.len(), 1);
        assert_eq!(report.trusted_contracts.len(), 2);
        assert_eq!(report.unresolved_trusts.len(), 2);
        assert_eq!(report.unchecked_policies.len(), 1);

        let (_, target) = report.targets.iter().next().expect("target");
        assert_eq!(target.checked_trusts, 1);
        assert_eq!(target.unchecked_trusts, 1);
        assert_eq!(target.host_provider, "omega::host");

        let (_, root) = report.trust_roots.iter().next().expect("trust root");
        assert_eq!(root.name, "compiler_slice_index");
        assert_eq!(root.checked_references, 2);
        assert_eq!(root.unchecked_references, 0);

        assert!(report.trusted_contracts.iter().any(|(_, contract)| {
            contract.trust_level == "compiler_slice_index" && contract.resolved
        }));
        assert!(
            report
                .unresolved_trusts
                .iter()
                .any(|(_, trust)| { trust.trust_level == "missing_contract_root" })
        );
        assert!(
            report
                .unresolved_trusts
                .iter()
                .any(|(_, trust)| { trust.trust_level == "missing_target_root" })
        );
    }
}
