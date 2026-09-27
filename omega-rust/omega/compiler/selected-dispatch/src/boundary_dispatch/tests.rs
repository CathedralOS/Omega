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
use crate::core_service_fixture::typed_with_core_service;
use effects::provider_plan::{ProviderBinding, ProviderPlan};
use typed_trees::TypedTrees;

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
/// `Binding<R>` fields and routed `Binding<R>` parameters join adapters through
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

    data EchoClient { service: Binding<Echo>; }
    machine EchoClient::run(&mut self) -> i32 reaches Echo {
        self.service.emit(1);
        transition { _ -> (self.service.echo(35)) }
    }

    data OtherClient { service: Binding<Other>; }
    machine OtherClient::run(&mut self) -> i32 reaches Other {
        self.service.emit(2);
        transition { _ -> (self.service.echo(35)) }
    }

    data ForwardClient { service: Binding<Forward>; }
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
            resolve_selected_adapter_row(&fixture.typed, &selected, &selected.rows[row_index], &[]);
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
