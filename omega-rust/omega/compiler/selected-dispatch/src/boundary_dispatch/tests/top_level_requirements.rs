//! A public receiver-free top-level `boundary requirement` settles one direct-
//! call dispatch row keyed on its owner and entry state, execution settlement
//! redirects the journaled call to the selected checked adapter, and a called
//! requirement with no selected provider rejects at settlement.

use super::{
    Arc, CheckedTrees, ProviderBinding, ProviderPlan, settle_selected_boundary_adapter_dispatch,
};
use provider_planning::ProviderPlanDerivation;
use typed_trees::expression::ExpressionNode;

const REQUIREMENT_SOURCE: &str = r#"
    pub data CheckedMath {}
    pub boundary requirement CheckedMath::offset_zero(value: i32) -> i32;

    data CheckedMathProvider {}
    machine CheckedMathProvider::offset_zero_impl(input: i32) -> i32
    satisfies CheckedMath::offset_zero
    {
        transition { _ -> (input + 0) }
    }

    data Client {}
    machine Client::run(&mut self) -> i32 {
        let selected: i32 = CheckedMath::offset_zero(35);
        transition { _ -> (selected) }
    }
"#;

fn requirement_fixture(source: &str) -> (CheckedTrees, Vec<ProviderPlan>) {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize requirement fixture");
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse requirement fixture");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve requirement fixture");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type requirement fixture");
    let plans = provider_planning::derive_satisfies_plans(
        &typed,
        ProviderPlanDerivation::unevaluated(None),
    )
    .into_iter()
    .map(|derived| derived.plan)
    .collect::<Vec<_>>();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect("check requirement fixture");
    (checked, plans)
}

fn entry_symbol(checked: &CheckedTrees, machine_name: &str) -> symbols::SymbolHandle {
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
        .unwrap_or_else(|| panic!("missing machine `{machine_name}`"));
    checked
        .typed
        .machine_states(machine)
        .first()
        .expect("entry state")
        .symbol
}

/// The symbol of parameter `parameter_name` on `machine_name`'s entry state.
fn entry_parameter_symbol(
    checked: &CheckedTrees,
    machine_name: &str,
    parameter_name: &str,
) -> symbols::SymbolHandle {
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
        .unwrap_or_else(|| panic!("missing machine `{machine_name}`"));
    checked
        .typed
        .machine_states(machine)
        .iter()
        .flat_map(|state| checked.typed.state_parameters(state))
        .find(|parameter| parameter.name.as_str() == parameter_name)
        .unwrap_or_else(|| panic!("missing parameter `{machine_name}`::{parameter_name}"))
        .symbol
}

fn data_symbol(checked: &CheckedTrees, name: &str) -> symbols::SymbolHandle {
    checked
        .typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == name)
        .unwrap_or_else(|| panic!("missing data `{name}`"))
        .symbol
}

fn direct_call(
    checked: &CheckedTrees,
) -> (
    typed_trees::expression::ExpressionHandle,
    symbols::SymbolHandle,
) {
    checked
        .typed
        .expression_table
        .expression_entries()
        .find_map(|(handle, expression)| match expression {
            ExpressionNode::Call(call) if call.target.as_str() == "offset_zero" => {
                Some((handle, call.target_symbol))
            }
            _ => None,
        })
        .expect("direct requirement call")
}

fn selected_all(plans: &[ProviderPlan]) -> effects::SelectedProviderPlanFacts {
    let names = plans
        .iter()
        .map(|plan| plan.name.clone())
        .collect::<Vec<_>>();
    effects::SelectedProviderPlanFacts::from_selection(plans, &names).expect("select every plan")
}

#[test]
fn public_requirement_settles_one_owner_keyed_direct_call_row() {
    let (checked, plans) = requirement_fixture(REQUIREMENT_SOURCE);
    assert_eq!(plans.len(), 1, "one derived top-level requirement plan");
    let selected = selected_all(&plans);
    let requirement = entry_symbol(&checked, "CheckedMath::offset_zero");
    let realization = entry_symbol(&checked, "CheckedMathProvider::offset_zero_impl");
    let owner = data_symbol(&checked, "CheckedMath");
    let (_, call_target) = direct_call(&checked);
    assert_eq!(
        call_target, requirement,
        "the direct call targets the requirement entry"
    );

    let original = Arc::new(checked);
    let mut settled = Arc::new(original.as_ref().clone());
    settle_selected_boundary_adapter_dispatch(&mut settled, &selected).unwrap();
    assert_eq!(
        settled.typed, original.typed,
        "association settles no source rewrite"
    );
    let rows = &settled.facts.boundary_adapter_dispatch;
    assert_eq!(rows.len(), 1, "{rows:?}");
    let row = &rows[0];
    assert_eq!(row.receiver, owner);
    assert_eq!(row.requirement, requirement);
    assert_eq!(row.realization_state, realization);
    assert!(!row.forward_receiver && row.family_tuple.is_empty());
}

#[test]
fn called_requirement_without_a_selected_provider_rejects_at_settlement() {
    let (checked, _) = requirement_fixture(REQUIREMENT_SOURCE);
    let original = Arc::new(checked);
    let mut settled = Arc::new(original.as_ref().clone());
    let diagnostics = settle_selected_boundary_adapter_dispatch(
        &mut settled,
        &effects::SelectedProviderPlanFacts::default(),
    )
    .expect_err("a direct call to an unselected public requirement rejects");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
            "direct call `offset_zero` names public boundary requirement `CheckedMath::offset_zero`, which has no selected provider"
        )),
        "{diagnostics:?}"
    );
}

const SELF_REQUIREMENT_SOURCE: &str = r#"
    pub data Token {}
    pub boundary requirement Token::consume(self) -> i32;

    data TokenProvider {}
    machine TokenProvider::consume_impl(token: Token) -> i32
    satisfies Token::consume
    {
        transition { _ -> (41) }
    }

    data Client {}
    machine Client::run(&mut self, token: Token) -> i32 {
        _ = token.consume();
        transition { _ -> (7) }
    }
"#;

/// A public `self` requirement is called through a member receiver
/// (`token.consume()`). Settlement emits one dispatch row per receiver
/// place — the row keys on the place's own symbol, not the nominal owner —
/// and forwards that place as the adapter's leading argument.
#[test]
fn a_self_requirement_settles_a_receiver_place_keyed_forwarding_row() {
    let (checked, plans) = requirement_fixture(SELF_REQUIREMENT_SOURCE);
    assert_eq!(plans.len(), 1, "one derived self-requirement plan");
    let selected = selected_all(&plans);
    let requirement = entry_symbol(&checked, "Token::consume");
    let realization = entry_symbol(&checked, "TokenProvider::consume_impl");
    let token = entry_parameter_symbol(&checked, "Client::run", "token");

    let mut settled = Arc::new(checked);
    settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect("a member call on an owned receiver settles");
    let rows = &settled.facts.boundary_adapter_dispatch;
    assert_eq!(rows.len(), 1, "{rows:?}");
    let row = &rows[0];
    assert_eq!(row.receiver, token);
    assert_eq!(row.requirement, requirement);
    assert_eq!(row.realization_state, realization);
    assert!(row.forward_receiver && row.family_tuple.is_empty());
}

const BORROWED_SELF_REQUIREMENT_SOURCE: &str = r#"
    pub data Token {}
    pub boundary requirement Token::inspect(&self) -> i32;

    data TokenProvider {}
    machine TokenProvider::inspect_impl(token: &Token) -> i32
    satisfies Token::inspect
    {
        transition { _ -> (41) }
    }

    data Client {}
    machine Client::run(&mut self, token: Token) -> i32 {
        _ = token.inspect();
        transition { _ -> (7) }
    }
"#;

/// A public `&self` requirement is called through a member receiver the
/// same way an owned `self` requirement is: the call borrows the receiver
/// place for its duration, so settlement keys one forwarding row on the
/// place's own symbol and the settled call splices `&place` — a `Borrow`
/// node with shared access — as the adapter's leading `&Token` argument.
#[test]
fn a_shared_borrow_self_requirement_settles_a_forwarding_row() {
    let (checked, plans) = requirement_fixture(BORROWED_SELF_REQUIREMENT_SOURCE);
    assert_eq!(plans.len(), 1, "one derived `&self`-requirement plan");
    let selected = selected_all(&plans);
    let requirement = entry_symbol(&checked, "Token::inspect");
    let realization = entry_symbol(&checked, "TokenProvider::inspect_impl");
    let token = entry_parameter_symbol(&checked, "Client::run", "token");

    let mut settled = Arc::new(checked);
    settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect("a member call on a shared-borrow receiver settles");
    let rows = &settled.facts.boundary_adapter_dispatch;
    assert_eq!(rows.len(), 1, "{rows:?}");
    let row = &rows[0];
    assert_eq!(row.receiver, token);
    assert_eq!(row.requirement, requirement);
    assert_eq!(row.realization_state, realization);
    assert!(row.forward_receiver && row.family_tuple.is_empty());
}

const MUTABLE_SELF_REQUIREMENT_SOURCE: &str = r#"
    pub data Token {}
    pub boundary requirement Token::mutate(&mut self) -> i32;

    data TokenProvider {}
    machine TokenProvider::mutate_impl(token: &mut Token) -> i32
    satisfies Token::mutate
    {
        transition { _ -> (41) }
    }

    data Client {}
    machine Client::run(&mut self, mut token: Token) -> i32 {
        _ = token.mutate();
        transition { _ -> (7) }
    }
    machine Client::value(&mut self, mut token: Token) -> i32 {
        transition { _ -> (token.mutate()) }
    }
"#;

/// A public `&mut self` requirement is called through a member receiver the
/// same way an owned `self` or shared `&self` requirement is: the call
/// mutably borrows the receiver place for its duration, so settlement keys
/// one forwarding row on the place's own symbol. Checking already required
/// the receiver place to be writable, so a `mut` parameter reaches
/// settlement; the rewrite splices `&mut place` as the adapter's leading
/// `&mut Token` argument.
#[test]
fn a_mutating_self_requirement_settles_a_forwarding_row() {
    let (checked, plans) = requirement_fixture(MUTABLE_SELF_REQUIREMENT_SOURCE);
    assert_eq!(plans.len(), 1, "one derived `&mut self`-requirement plan");
    let selected = selected_all(&plans);
    let requirement = entry_symbol(&checked, "Token::mutate");
    let realization = entry_symbol(&checked, "TokenProvider::mutate_impl");
    let token = entry_parameter_symbol(&checked, "Client::run", "token");

    let mut settled = Arc::new(checked);
    settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect("a member call on a mutating receiver settles");
    let rows = &settled.facts.boundary_adapter_dispatch;
    let row = rows
        .iter()
        .find(|row| row.receiver == token)
        .expect("the `mut token` parameter receiver place");
    assert_eq!(row.requirement, requirement);
    assert_eq!(row.realization_state, realization);
    assert!(row.forward_receiver && row.family_tuple.is_empty());
}

/// A `&mut self` member call on a receiver place that is not writable
/// rejects at checking through the ordinary receiver-borrow conflict gate —
/// the settlement row never sees a receiver it could not mutably borrow.
#[test]
fn a_mutating_self_requirement_rejects_an_unwritable_receiver() {
    let source = MUTABLE_SELF_REQUIREMENT_SOURCE.replace("mut token: Token", "token: Token");
    assert_ne!(source, MUTABLE_SELF_REQUIREMENT_SOURCE);
    let tokens = source_files_to_tokens::Lexer::new(&source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve");
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect_err("an immutable receiver cannot serve a `&mut self` requirement call");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("requires a mutable receiver, but its source is not writable in this state")),
        "{diagnostics:?}"
    );
}

/// A projected `self.field` receiver place settles the same forwarding row:
/// the member call mutably borrows `self.token`, writable because the
/// caller's own receiver is `&mut self`, and the rewrite splices
/// `&mut self.token` as the adapter's leading argument.
#[test]
fn a_mutating_self_requirement_settles_a_projected_field_receiver() {
    let source = r#"
        pub data Token {}
        pub boundary requirement Token::mutate(&mut self) -> i32;

        data TokenProvider {}
        machine TokenProvider::mutate_impl(token: &mut Token) -> i32
        satisfies Token::mutate
        {
            transition { _ -> (41) }
        }

        data Client { token: Token }
        machine Client::run(&mut self) -> i32 {
            _ = self.token.mutate();
            transition { _ -> (7) }
        }
    "#;
    let (checked, plans) = requirement_fixture(source);
    let selected = selected_all(&plans);
    let requirement = entry_symbol(&checked, "Token::mutate");
    let realization = entry_symbol(&checked, "TokenProvider::mutate_impl");

    let mut settled = Arc::new(checked);
    settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect("a `self.field` member call on a mutating receiver settles");
    let rows = &settled.facts.boundary_adapter_dispatch;
    assert_eq!(rows.len(), 1, "{rows:?}");
    let row = &rows[0];
    assert_eq!(row.requirement, requirement);
    assert_eq!(row.realization_state, realization);
    assert!(row.forward_receiver && row.family_tuple.is_empty());
}

/// A `&mut self` requirement's adapter must take the owner as a `&mut
/// Owner` leading parameter: a shared `&Owner` leading parameter cannot
/// receive the mutable borrow the member call declared, so the `satisfies`
/// conformance check rejects it before settlement.
#[test]
fn a_mutating_self_requirement_rejects_a_shared_borrow_adapter() {
    let source = MUTABLE_SELF_REQUIREMENT_SOURCE.replace(
        "machine TokenProvider::mutate_impl(token: &mut Token) -> i32",
        "machine TokenProvider::mutate_impl(token: &Token) -> i32",
    );
    assert_ne!(source, MUTABLE_SELF_REQUIREMENT_SOURCE);
    let tokens = source_files_to_tokens::Lexer::new(&source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve");
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect_err("a shared leading parameter cannot serve a `&mut self` requirement");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(
                "parameter 0 has shape `token: &Token`, but the requirement declares `&mut self`"
            )),
        "{diagnostics:?}"
    );
}

/// A `&write self` requirement never reaches settlement: the write-only
/// non-observation slice is proved only for checked Omega bodies, so a
/// write-only receiver on a requirement declaration rejects at checking.
#[test]
fn a_write_only_self_requirement_stays_closed_at_checking() {
    let source = REQUIREMENT_SOURCE
        .replace(
            "pub boundary requirement CheckedMath::offset_zero(value: i32) -> i32;",
            "pub boundary requirement CheckedMath::offset_zero(&write self, value: i32) -> i32;",
        )
        .replace(
            "machine CheckedMathProvider::offset_zero_impl(input: i32) -> i32",
            "machine CheckedMathProvider::offset_zero_impl(math: &write CheckedMath, input: i32) -> i32",
        )
        .replace(
            "let selected: i32 = CheckedMath::offset_zero(35);\n        transition { _ -> (selected) }",
            "transition { _ -> (35) }",
        );
    assert_ne!(source, REQUIREMENT_SOURCE);
    let tokens = source_files_to_tokens::Lexer::new(&source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve");
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect_err("a `&write self` requirement is not a checked-body declaration");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("declares `&write`, but the current milestone proves non-observation only for checked Omega bodies")),
        "{diagnostics:?}"
    );
}

/// A `&self` requirement's adapter must take the owner as a `&Owner`
/// leading parameter: an owned `Owner` leading parameter forwards a custody
/// the call never declared, so the `satisfies` conformance check rejects it
/// before settlement.
#[test]
fn a_borrowed_self_requirement_rejects_an_owned_receiver_adapter() {
    let source = BORROWED_SELF_REQUIREMENT_SOURCE.replace(
        "machine TokenProvider::inspect_impl(token: &Token) -> i32",
        "machine TokenProvider::inspect_impl(token: Token) -> i32",
    );
    assert_ne!(source, BORROWED_SELF_REQUIREMENT_SOURCE);
    let tokens = source_files_to_tokens::Lexer::new(&source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve");
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect_err("an owned leading parameter cannot serve a `&self` requirement");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
            "must realize the requirement receiver as exact carrier `Token` with the same reference shape"
        )),
        "{diagnostics:?}"
    );
}

#[test]
fn a_requirement_realized_by_a_compiler_intrinsic_settles_no_adapter_row_and_is_not_rejected() {
    // The plan for an intrinsic satisfier is derived and selected; the
    // direct call executes through the named-float intrinsic bridge, so
    // association settlement neither emits an adapter row nor rejects it.
    let source = r#"
        pub data Negation [copy] {}
        pub boundary requirement Negation::flip(value: f32) -> f32;

        pub data NegationProvider {}
        machine NegationProvider::flip32(value: f32) -> f32
            satisfies Negation::flip
            via ForeignBinding::CompilerIntrinsic;

        data Client {}
        machine Client::run(&mut self) -> f32 {
            let flipped: f32 = Negation::flip(1.0f32);
            transition { _ -> (flipped) }
        }
    "#;
    let (checked, plans) = requirement_fixture(source);
    assert_eq!(
        plans.len(),
        1,
        "the intrinsic satisfier derives one plan: {plans:?}"
    );
    let selected = selected_all(&plans);
    let mut settled = Arc::new(checked);
    settle_selected_boundary_adapter_dispatch(&mut settled, &selected).expect(
        "an intrinsic-realized requirement call is the intrinsic bridge's, not an adapter row",
    );
    assert!(settled.facts.boundary_adapter_dispatch.is_empty());
}

/// A generic token-machine requirement keeps its selected plan row keyed by
/// the authored template: `Indexing::index` projects one schema whose row
/// names the generic provider, while checking mints a closed requirement and
/// a closed provider instance for every demanded application. Settlement
/// replays the selected-requirement specialization roster — one exact
/// dispatch row per closed instance, keyed on the closed requirement's own
/// entry state and naming the specialized provider's entry state — and never
/// aliases the closed identities back to the template's.
const SPECIALIZED_REQUIREMENT_SOURCE: &str = r#"
    data Buffer<Element, const Count: u64> {
        value: Element
    }
    data Position {}
    data Indexing {}

    boundary machine [] Indexing::index<Element, const Count: u64>(
        items: Buffer<Element, Count>,
        position: Position
    ) -> bool
    requires Count == Count;

    data IndexingProvider {}
    machine IndexingProvider::index<Value, const Length: u64>(
        items: Buffer<Value, Length>,
        position: Position
    ) -> bool
    satisfies Indexing::index
    {
        true
    }

    data Client {}
    machine Client::run(&mut self, items: Buffer<i32, 4>, position: Position) -> bool {
        transition { _ -> (items[position]) }
    }
"#;

/// The retained specialization instances of one machine template: the
/// template itself is identified by its surviving static binders, and each
/// demanded closed instance joins it through `machine_specializations`
/// receipts rather than a shared display name — specialized instances carry
/// a minted `$specialized$` name. Symbol handles return so callers may move
/// the checked surface after the join is computed.
fn template_and_instances(
    checked: &CheckedTrees,
    name: &str,
) -> (symbols::SymbolHandle, Vec<symbols::SymbolHandle>) {
    let templates = checked
        .typed
        .machines()
        .iter()
        .filter(|machine| {
            machine.name.as_str() == name
                && !checked.typed.machine_type_parameters(machine).is_empty()
        })
        .collect::<Vec<_>>();
    let [template] = templates.as_slice() else {
        panic!("`{name}` has no unique generic template")
    };
    let instances = checked
        .typed
        .machine_specializations
        .iter()
        .filter(|receipt| receipt.template == template.symbol)
        .filter_map(|receipt| {
            checked
                .typed
                .machines()
                .iter()
                .find(|machine| machine.symbol == receipt.instance)
        })
        .map(|machine| machine.symbol)
        .collect();
    (template.symbol, instances)
}

fn machine_identity_by_symbol(checked: &CheckedTrees, symbol: symbols::SymbolHandle) -> String {
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == symbol)
        .expect("machine symbol resolves");
    checked
        .typed
        .normalized_machine_overload_identity(machine)
        .expect("machine carries a normalized identity")
        .identity()
}

fn entry_state_symbol(
    checked: &CheckedTrees,
    symbol: symbols::SymbolHandle,
) -> symbols::SymbolHandle {
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == symbol)
        .expect("machine symbol resolves");
    checked
        .typed
        .machine_states(machine)
        .first()
        .expect("entry state")
        .symbol
}

/// Attach the specialization roster rows `bind_selected_provider_plan_facts`
/// derives in production: the fixture replays the same retained receipts so
/// settlement sees the closure it would receive after binding.
fn with_specialization_roster(
    checked: &CheckedTrees,
    selected: effects::SelectedProviderPlanFacts,
) -> effects::SelectedProviderPlanFacts {
    let mut symbol_diagnostics = Vec::new();
    let top_level_symbols =
        validation::TopLevelSymbols::build(&checked.typed, &mut symbol_diagnostics);
    assert!(
        symbol_diagnostics.is_empty(),
        "fixture top-level symbols build exactly: {symbol_diagnostics:?}"
    );
    let mut roster = Vec::new();
    for plan in selected.plans() {
        for row in &plan.rows {
            let ProviderBinding::CheckedAdapter {
                machine_identity: selected_machine_identity,
                ..
            } = &row.binding
            else {
                continue;
            };
            let Some(provider_template) = checked
                .typed
                .machine_by_normalized_overload_identity(selected_machine_identity)
            else {
                continue;
            };
            let requirement_template = checked
                .typed
                .machines()
                .iter()
                .find(|machine| {
                    machine.supply_mode
                        == language_semantics::MachineSupplyMode::TopLevelRequirement
                        && !checked.typed.machine_type_parameters(machine).is_empty()
                        && provider_planning::service_schema::from_typed_boundary_requirement(
                            &checked.typed,
                            machine,
                        )
                        .is_some_and(|schema| schema == plan.schema)
                })
                .unwrap_or_else(|| {
                    panic!("plan `{}` binds no generic requirement template", plan.name)
                });
            for receipt in checked
                .typed
                .machine_specializations
                .iter()
                .filter(|receipt| receipt.template == provider_template.symbol)
            {
                if !receipt.operator_realizations.iter().any(|application| {
                    application.requirement_symbol == requirement_template.symbol
                }) {
                    continue;
                }
                let Some(instance) = checked
                    .typed
                    .machines()
                    .iter()
                    .find(|machine| machine.symbol == receipt.instance)
                else {
                    panic!("provider specialization retains no typed instance")
                };
                let closed = validation::resolve_closed_requirement_application(
                    &checked.typed,
                    instance,
                    requirement_template,
                    &top_level_symbols,
                )
                .unwrap_or_else(|diagnostic| {
                    panic!(
                        "provider instance `{}` retains one exact closed requirement: {}",
                        instance.name, diagnostic.message,
                    )
                });
                roster.push(effects::SelectedRequirementSpecialization {
                    provider_plan_report_identity: plan.report_fingerprint(),
                    selected_requirement_identity: row.requirement_identity.clone(),
                    selected_machine_identity: selected_machine_identity.clone(),
                    requirement_identity: machine_identity_by_symbol(checked, closed.symbol),
                    machine_identity: machine_identity_by_symbol(checked, instance.symbol),
                });
            }
        }
    }
    selected
        .with_specialized_requirement_rows(roster)
        .expect("fixture roster binds to the selected plan")
}

#[test]
fn a_generic_requirement_settles_one_row_per_closed_requirement_instance() {
    let (checked, plans) = requirement_fixture(SPECIALIZED_REQUIREMENT_SOURCE);
    assert_eq!(
        plans.len(),
        1,
        "the generic requirement template derives one plan: {plans:?}"
    );
    let (requirement_template, closed_requirements) =
        template_and_instances(&checked, "Indexing::index");
    let (provider_template, provider_instances) =
        template_and_instances(&checked, "IndexingProvider::index");
    assert_eq!(
        closed_requirements.len(),
        1,
        "`Buffer<i32, 4>` demand mints one closed requirement"
    );
    assert_eq!(provider_instances.len(), 1);
    let requirement_template_entry = entry_state_symbol(&checked, requirement_template);
    let provider_template_entry = entry_state_symbol(&checked, provider_template);
    let closed_requirement_entry = entry_state_symbol(&checked, closed_requirements[0]);
    let provider_instance_entry = entry_state_symbol(&checked, provider_instances[0]);
    let closed_requirement_owner = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == closed_requirements[0])
        .expect("closed requirement resolves")
        .attached_data_symbol;
    let selected = with_specialization_roster(&checked, selected_all(&plans));

    let mut settled = Arc::new(checked);
    settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect("the roster settles each closed requirement exactly");

    let rows = &settled.facts.boundary_adapter_dispatch;
    let [row] = rows.as_slice() else {
        panic!("one roster row settles one dispatch row: {rows:?}")
    };
    assert_eq!(row.receiver, closed_requirement_owner);
    assert_eq!(row.requirement, closed_requirement_entry);
    assert_eq!(row.realization_state, provider_instance_entry);
    assert!(!row.forward_receiver && row.family_tuple.is_empty());
    assert_ne!(
        row.requirement, requirement_template_entry,
        "the closed requirement never aliases its template's entry"
    );
    assert_ne!(
        row.realization_state, provider_template_entry,
        "the specialized provider never aliases its template's entry"
    );
}

/// A demanded closed requirement the roster does not rebind can only reach
/// the unbound template, so settlement rejects rather than emitting the
/// template's entry as an executable row.
#[test]
fn a_closed_requirement_instance_missing_its_roster_row_rejects() {
    let (checked, plans) = requirement_fixture(SPECIALIZED_REQUIREMENT_SOURCE);
    let selected = selected_all(&plans);
    let mut settled = Arc::new(checked);
    let diagnostics = settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect_err("an unbound closed instance never falls back to the template");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("checking retained 1 closed requirement instances")
        }),
        "{diagnostics:?}"
    );
}
