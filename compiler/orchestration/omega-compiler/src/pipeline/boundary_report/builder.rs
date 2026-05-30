use omega_artifacts::{
    BoundaryContract, BoundaryReport, BoundaryTarget, CapabilityBlastRadius,
    UncheckedBoundaryPolicy,
};
use omega_checked_trees::CheckedTrees;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;
use omega_effects::{CapabilityFlowKind, build_host_authority_registry};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::identifier::Identifier;
use omega_syntax_trees::item::{
    BoundaryLevel, BoundaryMode, CapabilityContractKind, CapabilityMember, Item,
};

/// Adds the capability blast-radius section to a boundary report, describing the
/// theoretical authority each boundary capability can mint and the authority-flow
/// verbs it participates in (chapter 18, "Capabilities And Authority Flow").
pub(crate) fn append_capability_blast_radius(report: &mut BoundaryReport, checked: &CheckedTrees) {
    let registry = build_host_authority_registry(checked);

    for trait_definition in checked.traits() {
        if !trait_definition.is_boundary {
            continue;
        }

        let provider = registry.provider(trait_definition.symbol);
        let authority_effects = provider
            .map(|provider| {
                provider
                    .effects
                    .names()
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let approved_provider = provider.map(|provider| provider.whitelisted).unwrap_or(true);

        report.capability_blast_radius.insert(CapabilityBlastRadius {
            capability: trait_definition.name.to_string(),
            authority_effects,
            approved_provider,
            uses: capability_verb_count(checked, trait_definition.symbol, CapabilityFlowKind::Uses),
            returns: capability_verb_count(
                checked,
                trait_definition.symbol,
                CapabilityFlowKind::Returns,
            ),
            acquires: capability_verb_count(
                checked,
                trait_definition.symbol,
                CapabilityFlowKind::Acquires,
            ),
            stores: capability_verb_count(
                checked,
                trait_definition.symbol,
                CapabilityFlowKind::Stores,
            ),
            derives: capability_verb_count(
                checked,
                trait_definition.symbol,
                CapabilityFlowKind::Derives,
            ),
        });
    }
}

fn capability_verb_count(
    checked: &CheckedTrees,
    capability_symbol: SymbolHandle,
    kind: CapabilityFlowKind,
) -> usize {
    checked
        .facts
        .capabilities
        .flows()
        .filter(|flow| flow.kind == kind && flow.capability_symbol == capability_symbol)
        .count()
}

pub(super) fn build_boundary_report(syntax: &SyntaxTrees) -> BoundaryReport {
    let mut report = BoundaryReport::default();

    for item in syntax.root_items() {
        match item {
            Item::Capability(capability) => {
                for member in syntax.items.capability_members(capability.members) {
                    let CapabilityMember::State(state) = member else {
                        continue;
                    };
                    collect_boundary_contracts(
                        &mut report,
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
                    for boundary in syntax.items.boundary_levels(function.boundaries) {
                        report.contracts.insert(BoundaryContract {
                            capability: library_name.clone(),
                            state: function.signature.name.to_string(),
                            boundary: boundary_name(boundary),
                            requires_count: 0,
                            ensures_count: 0,
                        });
                    }
                }
            }
            Item::Operator(operator) if operator.is_boundary => {
                collect_operator_boundary(&mut report, "operator", syntax, operator);
            }
            Item::Domain(domain) => {
                for operator in syntax.items.operators(domain.operators) {
                    if operator.is_boundary {
                        collect_operator_boundary(
                            &mut report,
                            &format!("domain operator {}", domain.name.as_str()),
                            syntax,
                            operator,
                        );
                    }
                }
            }
            Item::Target(target) => {
                let policies = syntax.items.boundary_policies(target.boundary_policies);
                report.targets.insert(BoundaryTarget {
                    name: target.name.to_string(),
                    host_provider: target
                        .host
                        .as_ref()
                        .map(|host| identifier_path_name(syntax, host.provider))
                        .unwrap_or_else(|| "none".to_owned()),
                    host_settings: target.host.as_ref().map_or(0, |host| {
                        syntax.items.target_host_settings(host.settings).len()
                    }),
                    checked_boundaries: policies
                        .iter()
                        .filter(|policy| matches!(policy.mode, BoundaryMode::Checked))
                        .count(),
                    unchecked_boundaries: policies
                        .iter()
                        .filter(|policy| matches!(policy.mode, BoundaryMode::Unchecked))
                        .count(),
                });

                for policy in policies {
                    if matches!(policy.mode, BoundaryMode::Unchecked) {
                        report.unchecked_policies.insert(UncheckedBoundaryPolicy {
                            target: target.name.to_string(),
                            name: identifier_path_name(syntax, policy.path),
                        });
                    }
                }
            }
            _ => {}
        }
    }

    report
}

fn collect_operator_boundary(
    report: &mut BoundaryReport,
    capability: &str,
    syntax: &SyntaxTrees,
    operator: &omega_syntax_trees::item::OperatorDefinition,
) {
    collect_declared_boundary(
        report,
        capability,
        &identifier_path_name(syntax, operator.name),
        syntax.items.capability_contracts(operator.contracts),
    );
}

fn collect_boundary_contracts(
    report: &mut BoundaryReport,
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
        let CapabilityContractKind::Boundary(boundary) = &contract.kind else {
            continue;
        };
        report.contracts.insert(BoundaryContract {
            capability: capability.to_owned(),
            state: state.to_owned(),
            boundary: boundary_name(boundary),
            requires_count,
            ensures_count,
        });
    }
}

fn collect_declared_boundary(
    report: &mut BoundaryReport,
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

    report.contracts.insert(BoundaryContract {
        capability: capability.to_owned(),
        state: state.to_owned(),
        boundary: state.to_owned(),
        requires_count,
        ensures_count,
    });
}

fn boundary_name(boundary: &BoundaryLevel) -> String {
    match boundary {
        BoundaryLevel::Host => "host".to_owned(),
        BoundaryLevel::Named(name) => name.to_string(),
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
    use super::build_boundary_report;
    use omega_source_files_to_tokens::Lexer;
    use omega_tokens_to_syntax_trees::parse_syntax_trees;

    #[test]
    fn boundary_report_collects_targets_contracts_and_operators() {
        let source = r#"
            capability Core {
                entry index() {
                    requires true;
                    ensures true;
                    boundary compiler_slice;
                }
            }

            boundary operator Slice::index<T>(items: &[T], index: usize) -> T
            requires
                index < items.len;

            target native {
                host: omega::host {
                    os = darwin
                }
                boundary omega::host::contracts
                boundary unchecked invariant_proofs
            }
        "#;
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let report = build_boundary_report(&syntax);

        assert_eq!(report.targets.len(), 1);
        assert_eq!(report.contracts.len(), 2);
        assert_eq!(report.unchecked_policies.len(), 1);

        let (_, target) = report.targets.iter().next().expect("target");
        assert_eq!(target.checked_boundaries, 1);
        assert_eq!(target.unchecked_boundaries, 1);
        assert_eq!(target.host_provider, "omega::host");

        assert!(report.contracts.iter().any(|(_, contract)| {
            contract.capability == "Core" && contract.boundary == "compiler_slice"
        }));
        assert!(report.contracts.iter().any(|(_, contract)| {
            contract.capability == "operator" && contract.state == "Slice::index"
        }));
    }
}
