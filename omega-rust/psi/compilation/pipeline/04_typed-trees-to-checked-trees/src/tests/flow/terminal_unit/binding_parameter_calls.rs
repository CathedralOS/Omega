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
