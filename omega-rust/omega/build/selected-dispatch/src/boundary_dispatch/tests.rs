//! Boundary adapter dispatch tests.

use super::{
    AdapterRow, Arc, BoundaryField, CheckedTrees, resolve_adapter_call,
    resolve_selected_adapter_row, settle_selected_boundary_adapter_dispatch,
};
use provider_planning::ProviderPlanDerivation;
mod borrowed_parameters;
mod finite_family;
mod generic_requirements;
mod mixed_providers;
mod source_retention;
mod top_level_requirements;
use crate::boundary_dispatch::boundary_fields::exact_adapter_receiver_shape;
use effects::provider_plan::{ProviderBinding, ProviderPlan};
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;

/// The toolchain core service declaration, resident so fixture sources can
/// spell `Service<R>` against the real core declaration. These bare pipelines
/// build a `SourceMap` with no package scope, so `use
/// omega::language::core::service` cannot resolve; installing the source with
/// `SourceOrigin::Toolchain` gives the typed-trees service classifier the exact
/// identity it requires.
const CORE_SERVICE_OMG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../source/library/core/service.omg"
));

/// Type `source` with the core `Service` declaration resident as a Toolchain
/// source. Fixtures spelling `Service<R>` carriers must declare the closed-over
/// requirement trait `pub`.
fn typed_with_core_service(name: &str, source: &str) -> TypedTrees {
    let mut sources = source::SourceMap::default();
    let service_source_id = sources
        .add_with_metadata(
            std::path::PathBuf::from("source/library/core/service.omg"),
            CORE_SERVICE_OMG.to_owned(),
            std::path::PathBuf::from("source/library/core"),
            None,
            source::SourceOrigin::Toolchain,
        )
        .source_id;
    let fixture_source_id = sources
        .add(std::path::PathBuf::from(name), source.to_owned())
        .source_id;
    let service_tokens = source_files_to_tokens::Lexer::new(CORE_SERVICE_OMG)
        .tokenize()
        .expect("tokenize core service declaration");
    let mut syntax =
        tokens_to_syntax_trees::parse_syntax_trees_with_id(service_source_id, &service_tokens)
            .expect("parse core service declaration");
    let fixture_tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize dispatch fixture");
    tokens_to_syntax_trees::parse_syntax_trees_into_with_id(
        &mut syntax,
        fixture_source_id,
        &fixture_tokens,
    )
    .expect("parse dispatch fixture");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
            syntax: &syntax,
            sources: Some(Arc::new(sources)),
            top_level_bindings: Vec::new(),
        },
    )
    .expect("resolve dispatch fixture");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type dispatch fixture")
}

/// Select every derived plan, one per boundary slot — the shape settled
/// orchestration hands checking, where every declared requirement keeps a
/// selected provider.
fn selected_every_plan(plans: &[ProviderPlan]) -> effects::SelectedProviderPlanFacts {
    let selected_names = plans
        .iter()
        .map(|plan| plan.name.clone())
        .collect::<Vec<_>>();
    effects::SelectedProviderPlanFacts::from_selection(plans, &selected_names)
        .expect("select every fixture plan")
}

/// Bind the fused-service erasure each selected boundary plan authorizes, the
/// way settled orchestration does before checking: one authorization per
/// selected boundary plan carrying that plan's identity digest. A fixture's
/// `Service<R>` fields and routed `Service<R>` parameters join adapters through
/// this digest, so plans must be selected first.
fn bind_fixture_fused_service_erasures(
    typed: &mut TypedTrees,
    selected: &effects::SelectedProviderPlanFacts,
) {
    let authorizations = selected
        .plans()
        .iter()
        .filter_map(|plan| {
            typed
                .traits()
                .iter()
                .find(|definition| {
                    definition.is_boundary && definition.name.as_str() == plan.schema.trait_name
                })
                .map(
                    |definition| typed_trees::typed_trees::FusedServiceErasureAuthorization {
                        requirement: definition.symbol,
                        provider_plan_digest: *plan.identity_digest().as_bytes(),
                    },
                )
        })
        .collect();
    typed
        .bind_fused_service_erasures(authorizations)
        .expect("fixture boundary traits admit fused service authorizations");
}

const SOURCE: &str = r#"
    pub boundary trait Echo {
        machine echo(value: i32) -> i32;
        machine emit(value: i32);
    }
    pub boundary trait Other {
        machine echo(value: i32) -> i32;
        machine emit(value: i32);
    }
    pub boundary trait Stateful {
        machine touch(&mut self);
    }
    pub boundary trait Forward {
        machine send(value: i32);
        machine reflect(value: i32) -> i32;
    }

    data EchoProvider {}
    machine EchoProvider::echo_adapter(value: i32) -> i32 satisfies Echo::echo {
        transition { _ -> (value) }
    }
    machine EchoProvider::emit_adapter(value: i32) satisfies Echo::emit {}

    data OtherProvider {}
    machine OtherProvider::echo_adapter(value: i32) -> i32 satisfies Other::echo {
        transition { _ -> (value) }
    }
    machine OtherProvider::emit_adapter(value: i32) satisfies Other::emit {}

    data StatefulProvider {}
    machine StatefulProvider::touch(&mut self) satisfies Stateful::touch {}

    data ForwardProvider {}
    machine ForwardProvider::send_adapter(service: Forward, value: i32)
        satisfies Forward::send {}
    machine ForwardProvider::reflect_adapter(service: Forward, value: i32) -> i32
        satisfies Forward::reflect {
        transition { _ -> (value) }
    }

    data EchoClient { service: Service<Echo>; }
    machine EchoClient::run(&mut self) -> i32 reaches Echo {
        self.service.emit(1);
        transition { _ -> (self.service.echo(35)) }
    }

    data OtherClient { service: Service<Other>; }
    machine OtherClient::run(&mut self) -> i32 reaches Other {
        self.service.emit(2);
        transition { _ -> (self.service.echo(35)) }
    }

    data ForwardClient { service: Service<Forward>; }
    machine ForwardClient::run(&mut self) -> i32 reaches Forward {
        self.service.send(3);
        transition { _ -> (self.service.reflect(35)) }
    }
"#;

struct Fixture {
    typed: TypedTrees,
    plans: Vec<ProviderPlan>,
}

fn fixture() -> Fixture {
    let mut typed = typed_with_core_service("selected-dispatch/dispatch.omg", SOURCE);
    let plans = provider_planning::derive_satisfies_plans(
        &typed,
        ProviderPlanDerivation::unevaluated(None),
    )
    .into_iter()
    .map(|derived| derived.plan)
    .collect::<Vec<_>>();
    bind_fixture_fused_service_erasures(&mut typed, &selected_every_plan(&plans));
    Fixture { typed, plans }
}

fn checked_fixture() -> (CheckedTrees, Vec<ProviderPlan>) {
    let fixture = fixture();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        fixture.typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect("check exact adapter-dispatch fixture");
    (checked, fixture.plans)
}

fn selected_plan(plans: &[ProviderPlan], schema: &str) -> effects::SelectedProviderPlanFacts {
    let selected = plan(plans, schema);
    effects::SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(selected),
        std::slice::from_ref(&selected.name),
    )
    .expect("select exact adapter plan")
}

fn plan<'plans>(plans: &'plans [ProviderPlan], schema: &str) -> &'plans ProviderPlan {
    plans
        .iter()
        .find(|plan| plan.schema.trait_name == schema)
        .unwrap_or_else(|| panic!("missing `{schema}` provider plan"))
}

fn checked_row(plan: &ProviderPlan, method: &str) -> usize {
    plan.rows
        .iter()
        .position(|row| {
            row.method == method && matches!(&row.binding, ProviderBinding::CheckedAdapter { .. })
        })
        .unwrap_or_else(|| panic!("missing checked `{method}` row"))
}

#[derive(Clone, Copy, Debug)]
enum Drift {
    None,
    EmptyOverload,
    CrossOverload,
    AbsentSchema,
    AbsentSignature,
    AbsentMachine,
    DuplicateMachine,
    WrongOwner,
    NonCheckedMachine,
    WrongConformance,
    InvalidShape,
    NonAdapterBinding,
}

#[test]
fn selected_rows_reject_every_exact_identity_drift() {
    let cases = [
        (Drift::None, None),
        (Drift::EmptyOverload, Some("no exact overload identity")),
        (Drift::CrossOverload, Some("binds 0 exact schema methods")),
        (
            Drift::AbsentSchema,
            Some("resolves to 0 exact boundary traits"),
        ),
        (
            Drift::AbsentSignature,
            Some("resolves to 0 exact typed signatures"),
        ),
        (Drift::AbsentMachine, Some("is absent from typed machines")),
        (
            Drift::DuplicateMachine,
            Some("resolves to 2 exact typed machines"),
        ),
        (
            Drift::WrongOwner,
            Some("does not belong to nominal provider"),
        ),
        (Drift::NonCheckedMachine, Some("is not a checked body")),
        (
            Drift::WrongConformance,
            Some("through 0 checked conformances"),
        ),
        (Drift::InvalidShape, Some("entry parameters")),
        (Drift::NonAdapterBinding, None),
    ];

    for (drift, expected_error) in cases {
        let mut fixture = fixture();
        let mut selected = plan(&fixture.plans, "Echo").clone();
        let row_index = checked_row(&selected, "echo");
        let method_index = selected
            .schema
            .methods
            .iter()
            .position(|method| {
                method.requirement_identity == selected.rows[row_index].requirement_identity
            })
            .expect("exact echo schema method");
        let other = plan(&fixture.plans, "Other");
        let other_row = &other.rows[checked_row(other, "echo")];
        match drift {
            Drift::None => {}
            Drift::EmptyOverload => selected.rows[row_index].requirement_identity.clear(),
            Drift::CrossOverload => {
                selected.rows[row_index].requirement_identity =
                    other_row.requirement_identity.clone();
            }
            Drift::AbsentSchema => selected.schema.trait_name = "Missing".into(),
            Drift::AbsentSignature => {
                selected.schema.methods[method_index].requirement_owner = "Other".into();
            }
            Drift::AbsentMachine => {
                selected.rows[row_index].binding = ProviderBinding::CheckedAdapter {
                    machine_identity: "EchoProvider::missing".into(),
                    machine_package_identity: None,
                };
            }
            Drift::DuplicateMachine => {
                let machine_identity = match &selected.rows[row_index].binding {
                    ProviderBinding::CheckedAdapter {
                        machine_identity, ..
                    } => machine_identity.clone(),
                    _ => unreachable!(),
                };
                let duplicate = fixture
                    .typed
                    .machine_by_normalized_overload_identity(&machine_identity)
                    .expect("selected adapter")
                    .clone();
                fixture.typed.push_machine(duplicate);
            }
            Drift::WrongOwner => selected.provider_type = "OtherProvider".into(),
            Drift::NonCheckedMachine => {
                fixture
                    .typed
                    .machines_mut()
                    .iter_mut()
                    .find(|machine| machine.name.as_str() == "EchoProvider::echo_adapter")
                    .expect("echo adapter")
                    .supply_mode = language_semantics::MachineSupplyMode::Boundary;
            }
            Drift::WrongConformance => {
                selected.provider_type = "OtherProvider".into();
                selected.rows[row_index].binding = other_row.binding.clone();
            }
            Drift::InvalidShape => {
                selected.schema.methods[method_index].parameter_count = usize::MAX;
            }
            Drift::NonAdapterBinding => {
                selected.rows[row_index].binding = ProviderBinding::CompilerIntrinsic {
                    machine: "Echo::echo.i32".into(),
                };
            }
        }

        let result =
            resolve_selected_adapter_row(&fixture.typed, &selected, &selected.rows[row_index]);
        match expected_error {
            Some(expected) => {
                let diagnostic = result.expect_err("identity drift must fail closed");
                assert!(
                    diagnostic.message.contains(expected),
                    "{drift:?}: expected `{expected}`, got `{}`",
                    diagnostic.message,
                );
            }
            None if matches!(drift, Drift::NonAdapterBinding) => {
                assert!(
                    result
                        .expect("non-adapter rows remain delegated")
                        .is_empty()
                );
            }
            None => {
                let adapter = result
                    .expect("exact row resolves")
                    .into_iter()
                    .next()
                    .expect("checked row yields adapter")
                    .expect_adapter();
                assert_eq!(adapter.receiver_trait_name, "Echo");
                assert_eq!(adapter.adapter_target, "EchoProvider::echo_adapter");
                assert!(!adapter.forward_receiver);
            }
        }
    }
}

#[test]
fn concrete_provider_self_is_excluded_from_adapter_arity() {
    let fixture = fixture();
    let selected = plan(&fixture.plans, "Stateful");
    let row = &selected.rows[checked_row(selected, "touch")];

    let adapter = resolve_selected_adapter_row(&fixture.typed, selected, row)
        .expect("concrete provider self is an exact realization receiver")
        .into_iter()
        .next()
        .expect("checked row yields adapter")
        .expect_adapter();
    assert_eq!(adapter.adapter_target, "StatefulProvider::touch");
    assert!(!adapter.forward_receiver);
}

#[test]
fn exact_leading_non_self_requirement_receiver_is_forwarded() {
    let fixture = fixture();
    let selected = plan(&fixture.plans, "Forward");
    let row = &selected.rows[checked_row(selected, "send")];

    let adapter = resolve_selected_adapter_row(&fixture.typed, selected, row)
        .expect("exact leading boundary binding resolves")
        .into_iter()
        .next()
        .expect("checked row yields adapter")
        .expect_adapter();
    assert_eq!(adapter.adapter_target, "ForwardProvider::send_adapter");
    assert!(adapter.forward_receiver);
}

#[test]
fn wrong_nonleading_or_multiple_non_self_receivers_never_forward() {
    let fixture = fixture();
    let selected = plan(&fixture.plans, "Forward");
    let row = &selected.rows[checked_row(selected, "send")];
    let ProviderBinding::CheckedAdapter {
        machine_identity, ..
    } = &row.binding
    else {
        unreachable!()
    };
    let adapter = fixture
        .typed
        .machine_by_normalized_overload_identity(machine_identity)
        .expect("forwarding adapter");
    let entry = fixture
        .typed
        .machine_states(adapter)
        .first()
        .expect("forwarding entry");
    let parameters = fixture
        .typed
        .state_parameters(entry)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    let owner = fixture
        .typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Forward")
        .expect("Forward boundary trait")
        .symbol;
    let cases = [
        (vec![parameters[1]], 0),
        (vec![parameters[1], parameters[0]], 1),
        (parameters, 0),
    ];

    for (actual, required) in cases {
        assert_eq!(
            exact_adapter_receiver_shape(&fixture.typed, &actual, required, owner, None),
            None,
        );
    }
}

fn symbol(index: u32) -> symbols::SymbolHandle {
    symbols::SymbolHandle::from_parts(index, 0)
}

fn adapter(
    receiver_trait: symbols::SymbolHandle,
    requirement_symbol: symbols::SymbolHandle,
    requirement: &str,
    target: &str,
) -> AdapterRow {
    AdapterRow {
        receiver_trait,
        provider_plan_digest: [0; 32],
        receiver_trait_name: format!("Trait{receiver_trait:?}"),
        requirement: requirement.into(),
        requirement_identity: format!("exact::{target}"),
        requirement_symbol,
        adapter_target: target.into(),
        symbol: symbol(requirement_symbol.arena_index() + 100),
        forward_receiver: false,
        family_tuple: Box::default(),
        family_tuple_display: Box::default(),
        top_level_owner: None,
    }
}

#[test]
fn same_spelled_fields_and_readable_names_never_select_adapter_rows() {
    let first_trait = symbol(1);
    let second_trait = symbol(2);
    let first_field = symbol(3);
    let second_field = symbol(4);
    let first_requirement = symbol(5);
    let second_requirement = symbol(6);
    let adapters = vec![
        adapter(
            first_trait,
            first_requirement,
            "echo",
            "FirstProvider::echo",
        ),
        adapter(
            second_trait,
            second_requirement,
            "echo",
            "SecondProvider::echo",
        ),
    ];
    let fields = [
        (
            "service",
            BoundaryField {
                symbol: first_field,
                trait_symbol: first_trait,
            },
        ),
        (
            "service",
            BoundaryField {
                symbol: second_field,
                trait_symbol: second_trait,
            },
        ),
    ];
    assert_eq!(fields[0].0, fields[1].0, "fixture field names must collide");
    let exact_fields = fields.map(|(_, field)| field);

    let cases = [
        (
            first_field,
            first_requirement,
            "echo",
            Some("FirstProvider::echo"),
            None,
        ),
        (
            second_field,
            second_requirement,
            "echo",
            Some("SecondProvider::echo"),
            None,
        ),
        (
            first_field,
            second_requirement,
            "echo",
            None,
            Some("display name but not exact target symbol"),
        ),
        (
            first_field,
            first_requirement,
            "renamed",
            None,
            Some("readable method drifted"),
        ),
        (symbol(99), first_requirement, "echo", None, None),
    ];
    for (field, target, name, expected_adapter, expected_error) in cases {
        let result = resolve_adapter_call(
            &TypedTrees::default(),
            &adapters,
            &[],
            &exact_fields,
            field,
            target,
            name,
            &[],
        );
        match (expected_adapter, expected_error) {
            (Some(expected), None) => assert_eq!(
                result
                    .expect("exact symbols resolve")
                    .expect("adapter selected")
                    .adapter_target,
                expected,
            ),
            (None, Some(expected)) => assert!(
                result
                    .expect_err("name-only drift must reject")
                    .message
                    .contains(expected),
            ),
            (None, None) => assert_eq!(result.expect("unrelated fields are ignored"), None),
            _ => unreachable!(),
        }
    }

    let duplicate = adapters[0].clone();
    assert!(
        resolve_adapter_call(
            &TypedTrees::default(),
            &[adapters[0].clone(), duplicate],
            &[],
            &exact_fields,
            first_field,
            first_requirement,
            "echo",
            &[],
        )
        .expect_err("duplicate exact rows must reject")
        .message
        .contains("matches 2 selected checked-adapter rows")
    );
}

fn statement_call(
    checked: &CheckedTrees,
    target: &str,
) -> (
    arena::HandleSpan<typed_trees::statement::StatementNode>,
    usize,
    typed_trees::statement::TableCall,
) {
    checked
        .typed
        .machines()
        .iter()
        .flat_map(|machine| checked.typed.machine_states(machine))
        .flat_map(|state| {
            checked
                .typed
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
                .filter_map(move |(index, statement)| match statement {
                    typed_trees::statement::StatementNode::Call(call)
                        if call.target.as_str() == target =>
                    {
                        Some((state.statement_nodes, index, call.clone()))
                    }
                    _ => None,
                })
        })
        .next()
        .unwrap_or_else(|| panic!("missing statement call `{target}`"))
}

fn expression_call(
    checked: &CheckedTrees,
    target: &str,
) -> (
    typed_trees::expression::ExpressionHandle,
    typed_trees::expression::TableCallExpression,
) {
    checked
        .typed
        .expression_table
        .expression_entries()
        .find_map(|(handle, expression)| match expression {
            ExpressionNode::Call(call) if call.target.as_str() == target => {
                Some((handle, call.clone()))
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing value call `{target}`"))
}

fn adapter_entry_symbol(checked: &CheckedTrees, machine_name: &str) -> symbols::SymbolHandle {
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
        .unwrap_or_else(|| panic!("missing adapter `{machine_name}`"));
    checked
        .typed
        .machine_states(machine)
        .first()
        .expect("adapter entry")
        .symbol
}

#[test]
fn selected_execution_keeps_source_and_records_exact_and_forwarding_targets() {
    for (statement_name, expression_name, statement_adapter, expression_adapter, forwarding) in [
        (
            "emit",
            "echo",
            "EchoProvider::emit_adapter",
            "EchoProvider::echo_adapter",
            false,
        ),
        (
            "send",
            "reflect",
            "ForwardProvider::send_adapter",
            "ForwardProvider::reflect_adapter",
            true,
        ),
    ] {
        let (checked, plans) = checked_fixture();
        let selected = selected_every_plan(&plans);
        let (_, _, statement) = statement_call(&checked, statement_name);
        let (_, expression) = expression_call(&checked, expression_name);
        let original = Arc::new(checked);
        let mut settled = Arc::clone(&original);
        settle_selected_boundary_adapter_dispatch(&mut settled, &selected).unwrap();
        assert_eq!(
            settled.typed, original.typed,
            "no rewritten calls or synthetic receiver expressions"
        );
        assert!(
            settled
                .facts
                .boundary_adapter_dispatch
                .iter()
                .any(|row| row.receiver == statement.receiver_symbol
                    && row.requirement == statement.target_symbol
                    && row.realization_state == adapter_entry_symbol(&settled, statement_adapter)
                    && row.forward_receiver == forwarding)
        );
        assert!(
            settled
                .facts
                .boundary_adapter_dispatch
                .iter()
                .any(|row| row.requirement == expression.target_symbol
                    && row.realization_state == adapter_entry_symbol(&settled, expression_adapter)
                    && row.forward_receiver == forwarding)
        );
        let mut other_facts = settled.facts.clone();
        other_facts.boundary_adapter_dispatch.clear();
        assert_eq!(
            other_facts, original.facts,
            "source-derived facts remain unchanged"
        );
        let first = Arc::clone(&settled);
        settle_selected_boundary_adapter_dispatch(&mut settled, &selected).unwrap();
        assert!(
            Arc::ptr_eq(&first, &settled),
            "unchanged selections require no tree copy"
        );
    }
}

#[test]
fn late_invalid_call_prevents_statement_and_value_rewrites() {
    let (mut checked, plans) = checked_fixture();
    let selected_names = plans
        .iter()
        .map(|plan| plan.name.clone())
        .collect::<Vec<_>>();
    let selected = effects::SelectedProviderPlanFacts::from_selection(&plans, &selected_names)
        .expect("select every exact fixture plan");
    let calls = checked
        .typed
        .expression_table
        .expression_entries()
        .filter_map(|(handle, expression)| {
            matches!(expression, ExpressionNode::Call(call) if call.target.as_str() == "echo")
                .then_some(handle)
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2);
    let ExpressionNode::Call(mut invalid) =
        checked.typed.expression_table.expression(calls[1]).clone()
    else {
        unreachable!()
    };
    invalid.target_symbol = symbols::SymbolHandle::invalid();
    *checked.typed.expression_table.expression_mut(calls[1]) = ExpressionNode::Call(invalid);
    let before = checked.clone();
    let original = Arc::new(checked);
    let mut rejected = Arc::clone(&original);

    let diagnostics = settle_selected_boundary_adapter_dispatch(&mut rejected, &selected)
        .expect_err("one invalid late call rejects the complete batch");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("not exact target symbol"))
    );
    assert!(Arc::ptr_eq(&rejected, &original));
    assert_eq!(rejected.as_ref(), &before);
}

#[test]
fn empty_settlement_preserves_shared_arc_identity_and_contents() {
    let (checked, _) = checked_fixture();
    let original_contents = checked.clone();
    let original = Arc::new(checked);
    let mut settled = Arc::clone(&original);

    settle_selected_boundary_adapter_dispatch(
        &mut settled,
        &effects::SelectedProviderPlanFacts::default(),
    )
    .expect("a program without selected boundary adapters is already settled");

    assert!(Arc::ptr_eq(&settled, &original));
    assert_eq!(settled.as_ref(), &original_contents);
}
