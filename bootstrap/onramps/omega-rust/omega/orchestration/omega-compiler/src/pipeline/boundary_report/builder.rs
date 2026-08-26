use omega_artifacts::{
    BoundaryContract, BoundaryReport, BoundaryTarget, CapabilityBlastRadius,
    CapabilityBlastRadiusFlow, CapabilityBlastRadiusRoute, UncheckedBoundaryPolicy,
};
use omega_effects::build_boundary_provider_approval_registry;
use psi_arena::HandleSpan;
use psi_checked_trees::CheckedTrees;
use psi_diagnostics::Diagnostic;
use psi_effects::CapabilityFlowKind;
use psi_symbols::SymbolHandle;
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::identifier::Identifier;
use psi_syntax_trees::item::{BoundaryMode, CapabilityContractKind, Item};

/// Adds the capability blast-radius section to a boundary report, describing the
/// theoretical authority each boundary capability can mint and the authority-flow
/// verbs it participates in (chapter 18, "Capabilities And Authority Flow").
pub(crate) fn append_capability_blast_radius(
    report: &mut BoundaryReport,
    checked: &CheckedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let ledger = checked_capability_flow_ledger(checked)?;
    let registry = build_boundary_provider_approval_registry(checked);
    let mut staged = Vec::new();

    for trait_definition in checked.traits() {
        if !trait_definition.is_boundary {
            continue;
        }

        let approved_provider = boundary_provider_is_approved(&registry, trait_definition.symbol);
        staged.push(CapabilityBlastRadius {
            capability: trait_definition.name.to_string(),
            approved_provider,
            uses: capability_verb_count(&ledger, trait_definition.symbol, CapabilityFlowKind::Uses),
            returns: capability_verb_count(
                &ledger,
                trait_definition.symbol,
                CapabilityFlowKind::Returns,
            ),
            acquires: capability_verb_count(
                &ledger,
                trait_definition.symbol,
                CapabilityFlowKind::Acquires,
            ),
            stores: capability_verb_count(
                &ledger,
                trait_definition.symbol,
                CapabilityFlowKind::Stores,
            ),
            derives: capability_verb_count(
                &ledger,
                trait_definition.symbol,
                CapabilityFlowKind::Derives,
            ),
            flows: capability_flow_rows(&ledger, trait_definition.symbol),
        });
    }

    for row in staged {
        report.capability_blast_radius.insert(row);
    }
    Ok(())
}

fn boundary_provider_is_approved(
    registry: &omega_effects::BoundaryProviderApprovalRegistry,
    trait_symbol: SymbolHandle,
) -> bool {
    registry.authorize_boundary_call(trait_symbol).is_approved()
}

fn capability_verb_count(
    ledger: &[CheckedCapabilityFlow],
    capability_symbol: SymbolHandle,
    kind: CapabilityFlowKind,
) -> usize {
    ledger
        .iter()
        .filter(|flow| flow.kind == kind && flow.capability_symbol == capability_symbol)
        .count()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CheckedCapabilityFlow {
    capability_symbol: SymbolHandle,
    kind: CapabilityFlowKind,
    state: String,
    machine_overload_identity: String,
    statement_index: usize,
    call_ordinal: usize,
    via: Option<CapabilityBlastRadiusRoute>,
}

/// Structured flow rows retain every exact checked site. Rendering belongs to
/// the artifact writer; display text is not reconstructed as an identity.
fn capability_flow_rows(
    ledger: &[CheckedCapabilityFlow],
    capability_symbol: SymbolHandle,
) -> Vec<CapabilityBlastRadiusFlow> {
    ledger
        .iter()
        .filter(|flow| flow.capability_symbol == capability_symbol)
        .map(|flow| CapabilityBlastRadiusFlow {
            state: flow.state.clone(),
            machine_overload_identity: flow.machine_overload_identity.clone(),
            authority_flow: flow.kind.as_str().to_owned(),
            statement_index: flow.statement_index,
            call_ordinal: flow.call_ordinal,
            via: flow.via.clone(),
        })
        .collect()
}

fn checked_capability_flow_ledger(
    checked: &CheckedTrees,
) -> Result<Vec<CheckedCapabilityFlow>, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    validate_capability_flow_spans(checked, &mut diagnostics);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let mut coordinates = Vec::new();
    let mut ledger = Vec::new();
    for flow in checked.facts.capabilities.flows() {
        let capability_matches = checked
            .traits()
            .iter()
            .filter(|definition| definition.symbol == flow.capability_symbol)
            .collect::<Vec<_>>();
        let capability = match capability_matches.as_slice() {
            [capability] if capability.is_boundary => *capability,
            [capability] => {
                diagnostics.push(Diagnostic::error(format!(
                    "capability blast-radius flow {:?} resolves to non-boundary trait `{}`",
                    flow.capability_symbol, capability.name,
                )));
                continue;
            }
            [] => {
                diagnostics.push(Diagnostic::error(format!(
                    "capability blast-radius flow has no exact boundary capability {:?}",
                    flow.capability_symbol,
                )));
                continue;
            }
            _ => {
                diagnostics.push(Diagnostic::error(format!(
                    "capability blast-radius flow has duplicate exact capability owners {:?}",
                    flow.capability_symbol,
                )));
                continue;
            }
        };

        let coordinate = (
            flow.capability_symbol,
            flow.kind,
            flow.machine_symbol,
            flow.state_symbol,
            flow.statement_index,
            flow.call_ordinal,
        );
        if coordinates.contains(&coordinate) {
            diagnostics.push(Diagnostic::error(format!(
                "capability blast-radius flow `{}` contains duplicate exact {} coordinate ({:?}, {:?}, statement {}, call {})",
                capability.name,
                flow.kind.as_str(),
                flow.machine_symbol,
                flow.state_symbol,
                flow.statement_index,
                flow.call_ordinal,
            )));
            continue;
        }
        coordinates.push(coordinate);

        let Some((machine, state)) = exact_owned_state(
            checked,
            flow.machine_symbol,
            flow.state_symbol,
            "capability blast-radius flow",
            &mut diagnostics,
        ) else {
            continue;
        };
        let statements = checked.statement_table.statements(state.statement_nodes);
        if flow.statement_index >= statements.len() {
            diagnostics.push(Diagnostic::error(format!(
                "capability blast-radius flow in state {:?} has out-of-range statement index {} for {} typed statements",
                flow.state_symbol,
                flow.statement_index,
                statements.len(),
            )));
            continue;
        }

        let call_matches = exact_service_call_matches(
            checked,
            flow.machine_symbol,
            flow.state_symbol,
            flow.statement_index,
            flow.call_ordinal,
        );
        let call = match call_matches.as_slice() {
            [call] => *call,
            [] => {
                diagnostics.push(Diagnostic::error(format!(
                    "capability blast-radius flow in state {:?} has no exact checked service-call coordinate (statement {}, call {})",
                    flow.state_symbol, flow.statement_index, flow.call_ordinal,
                )));
                continue;
            }
            _ => {
                diagnostics.push(Diagnostic::error(format!(
                    "capability blast-radius flow in state {:?} has duplicate exact checked service-call coordinates (statement {}, call {})",
                    flow.state_symbol, flow.statement_index, flow.call_ordinal,
                )));
                continue;
            }
        };

        let machine_overload_identity = match checked.normalized_machine_overload_identity(machine)
        {
            Some(identity) if !identity.identity().is_empty() => identity.identity(),
            _ => {
                diagnostics.push(Diagnostic::error(format!(
                    "capability blast-radius flow machine `{}` has no nonempty exact overload identity",
                    machine.name,
                )));
                continue;
            }
        };
        let via = if flow.is_propagated() {
            if call.target_state != flow.via_state_symbol {
                diagnostics.push(Diagnostic::error(format!(
                    "capability blast-radius propagated flow in state {:?} names via state {:?}, but exact call target is {:?}",
                    flow.state_symbol, flow.via_state_symbol, call.target_state,
                )));
                continue;
            }
            let Some((via_machine, via_state)) = exact_state_owner(
                checked,
                flow.via_state_symbol,
                "capability blast-radius propagated route",
                &mut diagnostics,
            ) else {
                continue;
            };
            let via_overload_identity = match checked
                .normalized_machine_overload_identity(via_machine)
            {
                Some(identity) if !identity.identity().is_empty() => identity.identity(),
                _ => {
                    diagnostics.push(Diagnostic::error(format!(
                            "capability blast-radius propagated route machine `{}` has no nonempty exact overload identity",
                            via_machine.name,
                        )));
                    continue;
                }
            };
            Some(CapabilityBlastRadiusRoute {
                state: format_state_path(via_machine.name.as_str(), via_state.name.as_str()),
                machine_overload_identity: via_overload_identity,
            })
        } else {
            None
        };

        ledger.push(CheckedCapabilityFlow {
            capability_symbol: flow.capability_symbol,
            kind: flow.kind,
            state: format_state_path(machine.name.as_str(), state.name.as_str()),
            machine_overload_identity,
            statement_index: flow.statement_index,
            call_ordinal: flow.call_ordinal,
            via,
        });
    }

    if diagnostics.is_empty() {
        Ok(ledger)
    } else {
        Err(diagnostics)
    }
}

fn validate_capability_flow_spans(checked: &CheckedTrees, diagnostics: &mut Vec<Diagnostic>) {
    if checked.traits().len() != checked.typed.roots.traits.count() as usize {
        diagnostics.push(Diagnostic::error(
            "capability blast-radius ledger has an invalid typed trait span",
        ));
    }
    let mut boundary_symbols = Vec::new();
    for definition in checked
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
    {
        if !definition.symbol.is_valid() {
            diagnostics.push(Diagnostic::error(format!(
                "capability blast-radius ledger contains boundary capability `{}` with an invalid symbol",
                definition.name,
            )));
        } else if boundary_symbols.contains(&definition.symbol) {
            diagnostics.push(Diagnostic::error(format!(
                "capability blast-radius ledger contains duplicate exact boundary capability definition {:?}",
                definition.symbol,
            )));
        } else {
            boundary_symbols.push(definition.symbol);
        }
    }

    if checked.machines().len() != checked.typed.roots.machines.count() as usize {
        diagnostics.push(Diagnostic::error(
            "capability blast-radius ledger has an invalid typed machine span",
        ));
    }
    for machine in checked.machines() {
        if checked.machine_states(machine).len() != machine.states.count() as usize {
            diagnostics.push(Diagnostic::error(format!(
                "capability blast-radius ledger has an invalid typed state span for machine {:?}",
                machine.symbol,
            )));
        }
        for state in checked.machine_states(machine) {
            if checked
                .statement_table
                .statements(state.statement_nodes)
                .len()
                != state.statement_nodes.count() as usize
            {
                diagnostics.push(Diagnostic::error(format!(
                    "capability blast-radius ledger has an invalid typed statement span for state {:?}",
                    state.symbol,
                )));
            }
        }
    }

    let reaches = &checked.facts.service_reaches;
    if reaches.machines().len() != reaches.root_machines.count() as usize {
        diagnostics.push(Diagnostic::error(
            "capability blast-radius ledger has an invalid service-reach machine span",
        ));
    }
    for machine in reaches.machines() {
        if reaches.states_for(machine).len() != machine.states.count() as usize {
            diagnostics.push(Diagnostic::error(format!(
                "capability blast-radius ledger has an invalid service-reach state span for machine {:?}",
                machine.machine,
            )));
        }
        for state in reaches.states_for(machine) {
            if reaches.calls_for(state).len() != state.calls.count() as usize {
                diagnostics.push(Diagnostic::error(format!(
                    "capability blast-radius ledger has an invalid service-reach call span for state {:?}",
                    state.state,
                )));
            }
        }
    }
}

fn exact_owned_state<'program>(
    checked: &'program CheckedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    context: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(
    &'program psi_checked_trees::machine::Machine,
    &'program psi_checked_trees::state::State,
)> {
    let machines = checked
        .machines()
        .iter()
        .filter(|machine| machine.symbol == machine_symbol)
        .collect::<Vec<_>>();
    let machine = match machines.as_slice() {
        [machine] => *machine,
        [] => {
            diagnostics.push(Diagnostic::error(format!(
                "{context} has no exact typed machine owner {machine_symbol:?}",
            )));
            return None;
        }
        _ => {
            diagnostics.push(Diagnostic::error(format!(
                "{context} has duplicate exact typed machine owners {machine_symbol:?}",
            )));
            return None;
        }
    };
    let Some((owner, state)) = exact_state_owner(checked, state_symbol, context, diagnostics)
    else {
        return None;
    };
    if owner.symbol != machine.symbol {
        diagnostics.push(Diagnostic::error(format!(
            "{context} state {state_symbol:?} belongs to typed machine {:?}, not {machine_symbol:?}",
            owner.symbol,
        )));
        return None;
    }
    Some((machine, state))
}

fn exact_state_owner<'program>(
    checked: &'program CheckedTrees,
    state_symbol: SymbolHandle,
    context: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(
    &'program psi_checked_trees::machine::Machine,
    &'program psi_checked_trees::state::State,
)> {
    let owners = checked
        .machines()
        .iter()
        .flat_map(|machine| {
            checked
                .machine_states(machine)
                .iter()
                .filter(move |state| state.symbol == state_symbol)
                .map(move |state| (machine, state))
        })
        .collect::<Vec<_>>();
    match owners.as_slice() {
        [(machine, state)] => Some((*machine, *state)),
        [] => {
            diagnostics.push(Diagnostic::error(format!(
                "{context} has no exact typed state owner {state_symbol:?}",
            )));
            None
        }
        _ => {
            diagnostics.push(Diagnostic::error(format!(
                "{context} has duplicate exact typed state owners {state_symbol:?}",
            )));
            None
        }
    }
}

fn exact_service_call_matches<'program>(
    checked: &'program CheckedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
) -> Vec<&'program psi_checked_trees::CallServiceReachRows> {
    let reaches = &checked.facts.service_reaches;
    reaches
        .machines()
        .iter()
        .filter(|machine| machine.machine == machine_symbol)
        .flat_map(|machine| reaches.states_for(machine))
        .filter(|state| state.state == state_symbol)
        .flat_map(|state| reaches.calls_for(state))
        .filter(|call| call.statement_index == statement_index && call.call_ordinal == call_ordinal)
        .collect()
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
    operator: &psi_syntax_trees::item::OperatorDefinition,
) {
    collect_declared_boundary(
        report,
        capability,
        &identifier_path_name(syntax, operator.name),
        syntax.items.capability_contracts(operator.contracts),
    );
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
        append_capability_blast_radius, boundary_provider_is_approved, build_boundary_report,
        checked_capability_flow_ledger,
    };
    use omega_artifacts::{BoundaryReport, CapabilityBlastRadius};
    use omega_effects::{BoundaryProviderApproval, BoundaryProviderApprovalRegistry};
    use psi_arena::HandleSpan;
    use psi_checked_trees::{
        CallServiceReachRows, CheckedTrees, MachineServiceReachRows, StateServiceReachRows,
    };
    use psi_effects::{CapabilityFlowFact, CapabilityFlowKind};
    use psi_source_files_to_tokens::Lexer;
    use psi_symbols::SymbolHandle;
    use psi_tokens_to_syntax_trees::parse_syntax_trees;
    use psi_typed_trees::machine::Machine;
    use psi_typed_trees::name::Identifier as TypedIdentifier;
    use psi_typed_trees::state::State;
    use psi_typed_trees::trait_definition::TraitDefinition;

    struct LedgerFixture {
        checked: CheckedTrees,
        capability: SymbolHandle,
        machine: SymbolHandle,
        state: SymbolHandle,
        helper_machine: SymbolHandle,
        helper_state: SymbolHandle,
    }

    fn symbol(index: u32) -> SymbolHandle {
        SymbolHandle::from_arena_index(index)
    }

    fn push_machine(
        checked: &mut CheckedTrees,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        machine_name: &str,
        state_name: &str,
    ) {
        let mut machine = Machine {
            symbol: machine_symbol,
            name: TypedIdentifier::generated(machine_name),
            ..Default::default()
        };
        let mut state = State {
            symbol: state_symbol,
            name: TypedIdentifier::generated(state_name),
            ..Default::default()
        };
        checked
            .typed
            .statement_table
            .push_statement(&mut state.statement_nodes, Default::default());
        checked.typed.push_machine_state(&mut machine, state);
        checked.typed.push_machine(machine);
    }

    fn ledger_fixture() -> LedgerFixture {
        let capability = symbol(1);
        let machine = symbol(2);
        let state = symbol(3);
        let helper_machine = symbol(4);
        let helper_state = symbol(5);
        let mut checked = CheckedTrees::default();

        checked.typed.push_trait_definition(TraitDefinition {
            symbol: capability,
            is_boundary: true,
            name: TypedIdentifier::generated("StorageRoot"),
            ..Default::default()
        });
        push_machine(&mut checked, machine, state, "Main::main", "main");
        push_machine(
            &mut checked,
            helper_machine,
            helper_state,
            "Vault::pick",
            "pick",
        );

        let mut calls = HandleSpan::empty();
        checked.facts.service_reaches.calls.append_to_span(
            &mut calls,
            CallServiceReachRows {
                statement_index: 0,
                call_ordinal: 0,
                target_state: helper_state,
                target_machine: helper_machine,
                ..Default::default()
            },
        );
        let mut states = HandleSpan::empty();
        checked.facts.service_reaches.states.append_to_span(
            &mut states,
            StateServiceReachRows {
                state,
                calls,
                ..Default::default()
            },
        );
        checked.facts.service_reaches.machines.append_to_span(
            &mut checked.facts.service_reaches.root_machines,
            MachineServiceReachRows {
                machine,
                states,
                ..Default::default()
            },
        );

        for (kind, via_state_symbol) in [
            (CapabilityFlowKind::Uses, SymbolHandle::invalid()),
            (CapabilityFlowKind::Acquires, helper_state),
        ] {
            checked.facts.capabilities.flows.append(CapabilityFlowFact {
                kind,
                capability_symbol: capability,
                machine_symbol: machine,
                state_symbol: state,
                statement_index: 0,
                call_ordinal: 0,
                via_state_symbol,
            });
        }

        LedgerFixture {
            checked,
            capability,
            machine,
            state,
            helper_machine,
            helper_state,
        }
    }

    #[test]
    fn checked_capability_ledger_retains_exact_direct_and_propagated_rows() {
        let fixture = ledger_fixture();
        let ledger = checked_capability_flow_ledger(&fixture.checked).expect("valid ledger");
        assert_eq!(ledger.len(), 2);
        let direct = ledger
            .iter()
            .find(|flow| flow.kind == CapabilityFlowKind::Uses)
            .expect("direct row");
        assert_eq!(direct.state, "Main::main");
        assert!(!direct.machine_overload_identity.is_empty());
        assert_eq!(direct.via, None);
        let propagated = ledger
            .iter()
            .find(|flow| flow.kind == CapabilityFlowKind::Acquires)
            .expect("propagated row");
        assert_eq!(
            propagated.via.as_ref().map(|route| route.state.as_str()),
            Some("Vault::pick")
        );
        assert!(
            !propagated
                .via
                .as_ref()
                .expect("route")
                .machine_overload_identity
                .is_empty()
        );

        let mut report = BoundaryReport::default();
        append_capability_blast_radius(&mut report, &fixture.checked).expect("valid append");
        let rows = report
            .capability_blast_radius
            .iter()
            .map(|(_, row)| row)
            .collect::<Vec<_>>();
        let [row] = rows.as_slice() else {
            panic!("one boundary capability row expected")
        };
        assert_eq!(
            (row.uses, row.acquires, row.returns, row.stores, row.derives),
            (1, 1, 0, 0, 0)
        );
        assert_eq!(row.flows.len(), 2);
    }

    #[test]
    fn zero_flow_capability_and_late_failure_preserve_atomic_append() {
        let mut fixture = ledger_fixture();
        fixture.checked.facts.capabilities.flows.clear();
        let mut report = BoundaryReport::default();
        append_capability_blast_radius(&mut report, &fixture.checked).expect("zero-flow append");
        let (_, row) = report
            .capability_blast_radius
            .iter()
            .next()
            .expect("zero-flow capability row");
        assert_eq!(row.flows, Vec::new());
        assert_eq!(
            (row.uses, row.returns, row.acquires, row.stores, row.derives),
            (0, 0, 0, 0, 0)
        );

        let mut fixture = ledger_fixture();
        fixture
            .checked
            .facts
            .capabilities
            .flows
            .for_each_mut(|_, flow| {
                if flow.kind == CapabilityFlowKind::Acquires {
                    flow.via_state_symbol = fixture.state;
                }
            });
        let mut report = BoundaryReport::default();
        report
            .capability_blast_radius
            .insert(CapabilityBlastRadius {
                capability: "sentinel".into(),
                ..Default::default()
            });
        let before = report.clone();
        assert!(append_capability_blast_radius(&mut report, &fixture.checked).is_err());
        assert_eq!(report, before, "late invalid flow must append no rows");
    }

    #[derive(Clone, Copy)]
    enum LedgerCorruption {
        InvalidTraitSpan,
        InvalidCapabilitySymbol,
        MissingCapability,
        DuplicateCapability,
        NonBoundaryCapability,
        InvalidMachineSpan,
        MissingMachine,
        DuplicateMachine,
        InvalidStateSpan,
        MissingState,
        CrossOwnerState,
        DuplicateState,
        InvalidStatementSpan,
        OutOfRangeStatement,
        InvalidServiceMachineSpan,
        InvalidServiceStateSpan,
        InvalidServiceCallSpan,
        MissingCallCoordinate,
        DuplicateCallCoordinate,
        MissingViaState,
        WrongViaTarget,
        DuplicateFlowCoordinate,
    }

    #[test]
    fn checked_capability_ledger_fails_closed_on_every_custody_drift() {
        let cases = [
            (
                LedgerCorruption::InvalidTraitSpan,
                "invalid typed trait span",
            ),
            (
                LedgerCorruption::InvalidCapabilitySymbol,
                "with an invalid symbol",
            ),
            (
                LedgerCorruption::MissingCapability,
                "no exact boundary capability",
            ),
            (
                LedgerCorruption::DuplicateCapability,
                "duplicate exact boundary capability definition",
            ),
            (
                LedgerCorruption::NonBoundaryCapability,
                "non-boundary trait",
            ),
            (
                LedgerCorruption::InvalidMachineSpan,
                "invalid typed machine span",
            ),
            (
                LedgerCorruption::MissingMachine,
                "no exact typed machine owner",
            ),
            (
                LedgerCorruption::DuplicateMachine,
                "duplicate exact typed machine owners",
            ),
            (
                LedgerCorruption::InvalidStateSpan,
                "invalid typed state span",
            ),
            (LedgerCorruption::MissingState, "no exact typed state owner"),
            (
                LedgerCorruption::CrossOwnerState,
                "belongs to typed machine",
            ),
            (
                LedgerCorruption::DuplicateState,
                "duplicate exact typed state owners",
            ),
            (
                LedgerCorruption::InvalidStatementSpan,
                "invalid typed statement span",
            ),
            (
                LedgerCorruption::OutOfRangeStatement,
                "out-of-range statement index",
            ),
            (
                LedgerCorruption::InvalidServiceMachineSpan,
                "invalid service-reach machine span",
            ),
            (
                LedgerCorruption::InvalidServiceStateSpan,
                "invalid service-reach state span",
            ),
            (
                LedgerCorruption::InvalidServiceCallSpan,
                "invalid service-reach call span",
            ),
            (
                LedgerCorruption::MissingCallCoordinate,
                "no exact checked service-call coordinate",
            ),
            (
                LedgerCorruption::DuplicateCallCoordinate,
                "duplicate exact checked service-call coordinates",
            ),
            (
                LedgerCorruption::MissingViaState,
                "no exact typed state owner",
            ),
            (LedgerCorruption::WrongViaTarget, "but exact call target is"),
            (
                LedgerCorruption::DuplicateFlowCoordinate,
                "duplicate exact uses coordinate",
            ),
        ];

        for (corruption, expected) in cases {
            let mut fixture = ledger_fixture();
            match corruption {
                LedgerCorruption::InvalidTraitSpan => fixture.checked.typed.tables.traits.clear(),
                LedgerCorruption::InvalidCapabilitySymbol => {
                    fixture
                        .checked
                        .typed
                        .tables
                        .traits
                        .for_each_mut(|_, definition| definition.symbol = SymbolHandle::invalid());
                    fixture
                        .checked
                        .facts
                        .capabilities
                        .flows
                        .for_each_mut(|_, flow| flow.capability_symbol = SymbolHandle::invalid());
                }
                LedgerCorruption::MissingCapability => fixture
                    .checked
                    .facts
                    .capabilities
                    .flows
                    .for_each_mut(|_, flow| flow.capability_symbol = symbol(90)),
                LedgerCorruption::DuplicateCapability => {
                    fixture
                        .checked
                        .typed
                        .push_trait_definition(TraitDefinition {
                            symbol: fixture.capability,
                            is_boundary: true,
                            name: TypedIdentifier::generated("DuplicateStorageRoot"),
                            ..Default::default()
                        });
                }
                LedgerCorruption::NonBoundaryCapability => fixture
                    .checked
                    .typed
                    .tables
                    .traits
                    .for_each_mut(|_, definition| definition.is_boundary = false),
                LedgerCorruption::InvalidMachineSpan => {
                    fixture.checked.typed.tables.machines.clear()
                }
                LedgerCorruption::MissingMachine => fixture
                    .checked
                    .facts
                    .capabilities
                    .flows
                    .for_each_mut(|_, flow| flow.machine_symbol = symbol(91)),
                LedgerCorruption::DuplicateMachine => fixture
                    .checked
                    .typed
                    .tables
                    .machines
                    .for_each_mut(|_, machine| {
                        if machine.symbol == fixture.helper_machine {
                            machine.symbol = fixture.machine;
                        }
                    }),
                LedgerCorruption::InvalidStateSpan => {
                    fixture.checked.typed.tables.machine_states.clear()
                }
                LedgerCorruption::MissingState => fixture
                    .checked
                    .facts
                    .capabilities
                    .flows
                    .for_each_mut(|_, flow| flow.state_symbol = symbol(93)),
                LedgerCorruption::CrossOwnerState => fixture
                    .checked
                    .facts
                    .capabilities
                    .flows
                    .for_each_mut(|_, flow| flow.machine_symbol = fixture.helper_machine),
                LedgerCorruption::DuplicateState => fixture
                    .checked
                    .typed
                    .tables
                    .machine_states
                    .for_each_mut(|_, state| {
                        if state.symbol == fixture.helper_state {
                            state.symbol = fixture.state;
                        }
                    }),
                LedgerCorruption::InvalidStatementSpan => {
                    fixture.checked.typed.statement_table = Default::default()
                }
                LedgerCorruption::OutOfRangeStatement => fixture
                    .checked
                    .facts
                    .capabilities
                    .flows
                    .for_each_mut(|_, flow| flow.statement_index = 7),
                LedgerCorruption::InvalidServiceMachineSpan => {
                    fixture.checked.facts.service_reaches.machines.clear()
                }
                LedgerCorruption::InvalidServiceStateSpan => {
                    fixture.checked.facts.service_reaches.states.clear()
                }
                LedgerCorruption::InvalidServiceCallSpan => {
                    fixture.checked.facts.service_reaches.calls.clear()
                }
                LedgerCorruption::MissingCallCoordinate => fixture
                    .checked
                    .facts
                    .capabilities
                    .flows
                    .for_each_mut(|_, flow| flow.call_ordinal = 7),
                LedgerCorruption::DuplicateCallCoordinate => {
                    let duplicate = fixture.checked.facts.service_reaches.machines()[0].clone();
                    fixture
                        .checked
                        .facts
                        .service_reaches
                        .machines
                        .append_to_span(
                            &mut fixture.checked.facts.service_reaches.root_machines,
                            duplicate,
                        );
                }
                LedgerCorruption::MissingViaState => {
                    fixture
                        .checked
                        .facts
                        .capabilities
                        .flows
                        .for_each_mut(|_, flow| {
                            if flow.kind == CapabilityFlowKind::Acquires {
                                flow.via_state_symbol = symbol(92);
                            }
                        });
                    fixture
                        .checked
                        .facts
                        .service_reaches
                        .calls
                        .for_each_mut(|_, call| call.target_state = symbol(92));
                }
                LedgerCorruption::WrongViaTarget => fixture
                    .checked
                    .facts
                    .capabilities
                    .flows
                    .for_each_mut(|_, flow| {
                        if flow.kind == CapabilityFlowKind::Acquires {
                            flow.via_state_symbol = fixture.state;
                        }
                    }),
                LedgerCorruption::DuplicateFlowCoordinate => {
                    let duplicate = fixture
                        .checked
                        .facts
                        .capabilities
                        .flows
                        .iter()
                        .map(|(_, flow)| *flow)
                        .find(|flow| flow.kind == CapabilityFlowKind::Uses)
                        .expect("direct flow");
                    fixture.checked.facts.capabilities.flows.append(duplicate);
                }
            }

            let diagnostics = checked_capability_flow_ledger(&fixture.checked)
                .expect_err("corrupted capability ledger must reject");
            let combined = diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n");
            assert!(
                combined.contains(expected),
                "corruption did not produce `{expected}`:\n{combined}"
            );
        }
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
    fn boundary_report_collects_targets_and_declared_boundary_operators() {
        let source = r#"
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
        assert_eq!(report.contracts.len(), 1);
        assert_eq!(report.unchecked_policies.len(), 1);

        let (_, target) = report.targets.iter().next().expect("target");
        assert_eq!(target.checked_boundaries, 1);
        assert_eq!(target.unchecked_boundaries, 1);
        assert_eq!(target.host_provider, "omega::host");

        assert!(report.contracts.iter().any(|(_, contract)| {
            contract.capability == "operator" && contract.state == "Slice::index"
        }));
    }
}
