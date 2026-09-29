//! Fused bindings are usable through ordinary record parameters, not just self.

use super::{checked_with_service, machine_named};

const SOURCE: &str = r#"
pub boundary trait Host {
    machine ticks() -> u64;
    machine mark(value: u64);
}
pub data Clock { host: Binding<Host>; }
machine read(clock: &mut Clock) reaches Host {
    let ticks: u64 = clock.host.ticks();
    Host::mark(ticks);
}
"#;

#[test]
fn binding_field_of_explicit_parameter_keeps_boundary_calls() {
    assert_boundary_calls_retained(SOURCE);
}

#[test]
fn resultless_binding_field_call_keeps_explicit_context() {
    assert_boundary_calls_retained(&SOURCE.replace("Host::mark(ticks)", "clock.host.mark(ticks)"));
}

#[test]
fn nested_resultless_binding_field_call_keeps_explicit_context() {
    assert_boundary_calls_retained(
        &SOURCE
            .replace("Host::mark(ticks)", "clock.host.mark(ticks)")
            .replace(
                "machine read(clock: &mut Clock)",
                "pub data Context { clock: Clock; } machine read(context: &mut Context)",
            )
            .replace("clock.host", "context.clock.host"),
    );
}

#[test]
fn binding_statement_rejects_a_different_requirement_target() {
    let source = format!(
        "{} pub boundary trait Other {{ machine mark(value: u64); }}",
        SOURCE.replace("Host::mark(ticks)", "clock.host.mark(ticks)")
    );
    let mut typed = crate::tests::front_end::typed_program_with_core_service(&source);
    crate::tests::bind_fixture_fused_service_erasures(&mut typed);
    let foreign = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Other")
        .map(|definition| typed.trait_machine_signatures(definition)[0].symbol)
        .expect("foreign requirement");
    let statements = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "read")
        .map(|machine| typed.machine_states(machine)[0].statement_nodes)
        .expect("caller state");
    let call = typed
        .statement_table
        .statements_mut(statements)
        .iter_mut()
        .find_map(|statement| match statement {
            typed_trees::statement::StatementNode::Call(call) => Some(call),
            _ => None,
        })
        .expect("authored boundary call");
    call.target_symbol = foreign;
    let diagnostics = crate::lower_typed_trees(typed, &crate::CheckingRequest::settled())
        .expect_err("the field's requirement cannot be replaced by a same-named one");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("projected binding call target belongs to a different requirement")),
        "{diagnostics:?}"
    );
}

#[test]
fn qualified_static_binding_call_has_no_implicit_attachment() {
    assert_boundary_calls_retained(&SOURCE.replace("machine read(", "machine Clock::read("));
}

#[test]
fn nested_binding_field_keeps_boundary_calls() {
    assert_boundary_calls_retained(
        &SOURCE
            .replace(
                "machine read(clock: &mut Clock)",
                "pub data Context { clock: Clock; }\nmachine read(context: &mut Context)",
            )
            .replace("clock.host.ticks()", "context.clock.host.ticks()"),
    );
}

#[test]
fn binding_field_without_erasure_authorization_has_no_executable_plan() {
    let typed = crate::tests::front_end::typed_program_with_core_service(SOURCE);
    let checked = crate::lower_typed_trees(typed, &crate::CheckingRequest::settled())
        .expect("source checking is independent of selected Fused erasure");
    let machine = machine_named(&checked, "read");
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine)
            .is_none()
    );
}

fn assert_boundary_calls_retained(source: &str) {
    let checked = checked_with_service(source);
    let machine = machine_named(&checked, "read");
    let plans = &checked.facts.flow.terminal_unit_effects;
    let plan = plans.for_machine(machine).unwrap_or_else(|| {
        panic!(
            "explicit binding parameter lost its calls: {:?}",
            plans.omission_for_machine(machine)
        )
    });
    assert!(plan.attachment_type_identity.is_none());
    assert_eq!(plan.structural_parameters.len(), 1);
    assert_eq!(
        plan.operations
            .iter()
            .filter(|operation| operation.call().is_some_and(|call| call.is_boundary()))
            .count(),
        2
    );
}
