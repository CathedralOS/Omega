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
fn projected_record_call_selects_the_admitted_target_body() {
    let checked = checked_with_service(
        r#"
pub boundary trait Host { machine write(value: u64); }
pub data Reader { value: u64; }
machine Reader::read_value(&self) -> u64 { self.value }
windows_x86_64 machine Reader::read_value(&self) -> u64 { self.value }
pub data Context { reader: Reader; host: Binding<Host>; }
machine run(context: &mut Context) reaches Host {
    let value: u64 = context.reader.read_value();
    context.host.write(value);
}
"#,
    );
    let selected = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Reader::read_value" && machine.target.is_none())
        .expect("the already-admitted body remains selected");
    let selected_entry = checked.machine_states(selected)[0].symbol;
    let caller = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "run")
        .expect("caller");
    let state = &checked.machine_states(caller)[0];
    let checked_trees::statement::StatementNode::LocalData(local) =
        &checked.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("value binding")
    };
    let checked_trees::expression::ExpressionNode::Call(call) =
        checked.expression_table.expression(local.initial_value)
    else {
        panic!("projected call")
    };
    assert_eq!(call.target_symbol, selected_entry);
}

const SUM_SOURCE: &str = r#"
pub data ReadingResult [copy] { case Empty; case Value(total: u64); }
pub boundary trait Host {
    machine read() -> ReadingResult;
    machine write(total: u64);
}
pub data Context { host: Binding<Host>; }
machine run(context: &mut Context) reaches Host {
    let result: ReadingResult = context.host.read();
    transition result {
        ReadingResult::Value { total } -> report(context, total)
        ReadingResult::Empty -> report(context, 0)
    }
    state report(context: &mut Context, total: u64) {
        context.host.write(total);
    }
}
"#;

#[test]
fn copy_sum_payload_and_explicit_context_cross_state_transition() {
    assert_sum_dispatch(SUM_SOURCE, false);
}

#[test]
fn affine_sum_payload_retains_its_disposal_receipt() {
    assert_sum_dispatch(&SUM_SOURCE.replace(" [copy]", ""), true);
}

#[test]
fn copy_sum_parameter_and_explicit_context_cross_state_transition() {
    assert_sum_dispatch(
        &SUM_SOURCE
            .replace(
                "run(context: &mut Context)",
                "run(context: &mut Context, result: ReadingResult)",
            )
            .replace("    let result: ReadingResult = context.host.read();", ""),
        false,
    );
}

fn assert_sum_dispatch(source: &str, has_affine_drop: bool) {
    let checked = checked_with_service(source);
    let machine = machine_named(&checked, "run");
    let plans = &checked.facts.flow.terminal_unit_effects;
    let plan = plans.composed_for_machine(machine).unwrap_or_else(|| {
        panic!(
            "sum lost its case dispatch: {:?}",
            plans.omission_for_machine(machine)
        )
    });
    assert!(plan.attachment_type_identity.is_none());
    let checked_trees::CheckedComposedUnitControlTerminatorPlan::ClosedSum { cases, .. } =
        &plan.states[0].terminator
    else {
        panic!("sum must retain closed-case dispatch")
    };
    assert_eq!(cases.len(), 2);
    assert_eq!(
        cases.iter().map(|case| case.payloads.len()).sum::<usize>(),
        1
    );
    assert_eq!(
        checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .any(|(_, event)| {
                event.machine_symbol == machine
                    && event.kind == language_semantics::PermissionEventKind::AffineDrop
            }),
        has_affine_drop
    );
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
