use omega_artifacts::{
    BoundaryContract, BoundaryProviderEntry, BoundaryReport, BoundaryTarget, CapabilityBlastRadius,
    CapabilityBlastRadiusFlow, CapabilityBlastRadiusRoute, UncheckedBoundaryPolicy,
};
use omega_effects::build_boundary_provider_approval_registry;
use psi_arena::HandleSpan;
use psi_checked_trees::CheckedTrees;
use psi_effects::CapabilityFlowKind;
use psi_symbols::SymbolHandle;
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::identifier::Identifier;
use psi_syntax_trees::item::{
    BoundaryLevel, BoundaryMode, CapabilityContractKind, CapabilityMember, Item,
};

/// Adds the capability blast-radius section to a boundary report, describing the
/// theoretical authority each boundary capability can mint and the authority-flow
/// verbs it participates in (chapter 18, "Capabilities And Authority Flow").
pub(crate) fn append_capability_blast_radius(report: &mut BoundaryReport, checked: &CheckedTrees) {
    let registry = build_boundary_provider_approval_registry(checked);

    for trait_definition in checked.traits() {
        if !trait_definition.is_boundary {
            continue;
        }

        let approved_provider = boundary_provider_is_approved(&registry, trait_definition.symbol);

        report
            .capability_blast_radius
            .insert(CapabilityBlastRadius {
                capability: trait_definition.name.to_string(),
                approved_provider,
                uses: capability_verb_count(
                    checked,
                    trait_definition.symbol,
                    CapabilityFlowKind::Uses,
                ),
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
                flows: capability_flow_rows(checked, trait_definition.symbol),
            });
    }
}

fn boundary_provider_is_approved(
    registry: &omega_effects::BoundaryProviderApprovalRegistry,
    trait_symbol: SymbolHandle,
) -> bool {
    registry.authorize_boundary_call(trait_symbol).is_approved()
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

/// Structured flow rows retain every exact checked site. Rendering belongs to
/// the artifact writer; display text is not reconstructed as an identity.
fn capability_flow_rows(
    checked: &CheckedTrees,
    capability_symbol: SymbolHandle,
) -> Vec<CapabilityBlastRadiusFlow> {
    checked
        .facts
        .capabilities
        .flows()
        .filter(|flow| flow.capability_symbol == capability_symbol)
        .map(|flow| CapabilityBlastRadiusFlow {
            state: state_path_for_machine(checked, flow.machine_symbol, flow.state_symbol),
            machine_overload_identity: machine_overload_identity(checked, flow.machine_symbol),
            authority_flow: flow.kind.as_str().to_owned(),
            statement_index: flow.statement_index,
            call_ordinal: flow.call_ordinal,
            via: flow.is_propagated().then(|| CapabilityBlastRadiusRoute {
                state: propagated_state_path(checked, flow.via_state_symbol),
                machine_overload_identity: state_owner_overload_identity(
                    checked,
                    flow.via_state_symbol,
                ),
            }),
        })
        .collect()
}

fn machine_overload_identity(checked: &CheckedTrees, machine_symbol: SymbolHandle) -> String {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .expect("checked capability flow must name an owning machine");
    checked
        .normalized_machine_overload_identity(machine)
        .expect("checked capability-flow machine must have an entry state")
        .identity()
}

fn state_owner_overload_identity(checked: &CheckedTrees, state_symbol: SymbolHandle) -> String {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| {
            checked
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == state_symbol)
        })
        .expect("checked propagated capability flow must name a helper state");
    checked
        .normalized_machine_overload_identity(machine)
        .expect("checked propagated capability-flow machine must have an entry state")
        .identity()
}

/// Renders the exact `Machine::state` pair retained on a primary flow row.
fn state_path_for_machine(
    checked: &CheckedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> String {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .expect("checked capability flow must name an owning machine");
    let state = checked
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)
        .expect("checked capability flow state must belong to its exact owning machine");
    format_state_path(machine.name.as_str(), state.name.as_str())
}

/// A propagated row carries only the helper state coordinate; resolve its
/// owner from checked topology and reject rather than rendering a placeholder.
fn propagated_state_path(checked: &CheckedTrees, state_symbol: SymbolHandle) -> String {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| {
            checked
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == state_symbol)
        })
        .expect("checked propagated capability flow must name a helper state");
    state_path_for_machine(checked, machine.symbol, state_symbol)
}

fn format_state_path(machine_name: &str, state_name: &str) -> String {
    if machine_name == state_name || machine_name.ends_with(&format!("::{state_name}")) {
        machine_name.to_owned()
    } else {
        format!("{machine_name}::{state_name}")
    }
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

    append_provider_registry(&mut report, syntax);

    report
}

/// Adds the boundary primitive provider registry rows to the report: per
/// provider, the governing contract, categorical host-authority requirement, and target
/// applicability resolved from the boundary operator(s) bound to it. Registry
/// diagnostics are discarded here; the compile pipeline reports them through
/// its dedicated provider-validation step.
fn append_provider_registry(report: &mut BoundaryReport, syntax: &SyntaxTrees) {
    let mut discarded_diagnostics = Vec::new();
    let registry = omega_effects::build_provider_registry(syntax, &mut discarded_diagnostics);

    for provider in registry.providers() {
        report.providers.insert(BoundaryProviderEntry {
            name: provider.name.clone(),
            category: provider.category.name().to_owned(),
            contract_ref: provider.contract_ref.clone(),
            requires_host_authority: provider.requires_host_authority,
            target_applicability: provider.target_applicability.clone(),
            origin_package: provider.origin_package.clone(),
        });
    }
}

fn collect_operator_boundary(
    report: &mut BoundaryReport,
    capability: &str,
    syntax: &SyntaxTrees,
    operator: &psi_syntax_trees::item::OperatorDefinition,
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
    contracts: &[psi_syntax_trees::item::CapabilityContract],
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
    contracts: &[psi_syntax_trees::item::CapabilityContract],
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
    use super::{
        boundary_provider_is_approved, build_boundary_report, propagated_state_path,
        state_path_for_machine,
    };
    use omega_effects::{BoundaryProviderApproval, BoundaryProviderApprovalRegistry};
    use psi_checked_trees::CheckedTrees;
    use psi_source_files_to_tokens::Lexer;
    use psi_symbols::SymbolHandle;
    use psi_tokens_to_syntax_trees::parse_syntax_trees;
    use psi_typed_trees::machine::Machine;
    use psi_typed_trees::name::Identifier as TypedIdentifier;
    use psi_typed_trees::state::State;

    fn checked_state_fixture() -> (CheckedTrees, SymbolHandle, SymbolHandle, SymbolHandle) {
        let owner = SymbolHandle::from_arena_index(1);
        let state = SymbolHandle::from_arena_index(2);
        let other = SymbolHandle::from_arena_index(3);
        let other_state = SymbolHandle::from_arena_index(4);
        let mut checked = CheckedTrees::default();
        let mut machine = Machine {
            symbol: owner,
            name: TypedIdentifier::generated("Vault::expose"),
            ..Default::default()
        };
        checked.typed.push_machine_state(
            &mut machine,
            State {
                symbol: state,
                name: TypedIdentifier::generated("expose"),
                ..Default::default()
            },
        );
        checked.typed.push_machine(machine);
        let mut other_machine = Machine {
            symbol: other,
            name: TypedIdentifier::generated("Other::run"),
            ..Default::default()
        };
        checked.typed.push_machine_state(
            &mut other_machine,
            State {
                symbol: other_state,
                name: TypedIdentifier::generated("run"),
                ..Default::default()
            },
        );
        checked.typed.push_machine(other_machine);
        (checked, owner, state, other)
    }

    #[test]
    fn capability_flow_state_path_retains_exact_owned_pair() {
        let (checked, owner, state, _) = checked_state_fixture();
        assert_eq!(
            state_path_for_machine(&checked, owner, state),
            "Vault::expose"
        );
    }

    #[test]
    #[should_panic(expected = "state must belong to its exact owning machine")]
    fn capability_flow_state_path_rejects_cross_machine_pair() {
        let (checked, _, state, other) = checked_state_fixture();
        state_path_for_machine(&checked, other, state);
    }

    #[test]
    #[should_panic(expected = "must name a helper state")]
    fn propagated_capability_flow_rejects_missing_state() {
        propagated_state_path(&CheckedTrees::default(), SymbolHandle::from_arena_index(1));
    }

    #[test]
    fn blast_radius_approval_uses_exact_registry_authorization_and_fails_closed() {
        let approved = SymbolHandle::from_arena_index(1);
        let unapproved = SymbolHandle::from_arena_index(2);
        let absent = SymbolHandle::from_arena_index(3);
        let registry = BoundaryProviderApprovalRegistry::with_providers(vec![
            BoundaryProviderApproval::new(approved, true),
            BoundaryProviderApproval::new(unapproved, false),
        ]);

        assert!(boundary_provider_is_approved(&registry, approved));
        assert!(!boundary_provider_is_approved(&registry, unapproved));
        assert!(
            !boundary_provider_is_approved(&registry, absent),
            "an unrelated approved symbol must not authorize an absent exact capability"
        );
    }

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

            provider omega::language::core::Slice : SliceIndexing;

            boundary operator Slice::index<T>(items: &[T], index: usize) -> T
            provider omega::language::core::Slice
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

        assert_eq!(report.providers.len(), 1);
        let (_, provider) = report.providers.iter().next().expect("provider row");
        assert_eq!(provider.name, "omega::language::core::Slice");
        assert_eq!(provider.category, "SliceIndexing");
        assert_eq!(provider.contract_ref.as_deref(), Some("Slice::index"));
        assert!(
            !provider.requires_host_authority,
            "slice indexing provider should carry no host authority"
        );
        assert!(
            provider.target_applicability.is_empty(),
            "an operator without a named boundary applies to all targets"
        );
        assert_eq!(provider.origin_package, "omega::language::core");
    }
}
