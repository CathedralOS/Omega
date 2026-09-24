//! Explicit result disposal must not turn a value-returning boundary into Unit.
use super::CheckedUnitEffectOperationPlan;
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::checked_with_service;
use crate::tests::flow::terminal_unit::machine_named;

const SOURCE: &str = r#"
pub data ReadResult { case Empty; case Bytes(count: u64); }
pub boundary trait Host {
    machine read(buffer: &mut [u8]) -> ReadResult;
    machine mark(value: u64);
}
machine run(buffer: &mut [u8]) reaches Host {
    Host::mark(1);
    _ = Host::read(buffer);
    Host::mark(2);
}
"#;

#[test]
fn discarded_boundary_result_retains_result_and_authored_effect_order() {
    let checked = checked(SOURCE);
    let root = machine_named(&checked, "run");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(root)
        .expect("discarded structural boundary result keeps a complete Unit caller");
    assert_eq!(plan.operations.len(), 5);
    let CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
        coordinate,
        result,
        discard_result_on_return,
        ..
    } = &plan.operations[1]
    else {
        panic!("value-returning boundary call")
    };
    assert_eq!(coordinate.statement_index, 1);
    assert_eq!(result.statement_index, 1);
    assert_eq!(result.binding_ordinal, 0);
    assert!(!discard_result_on_return);
    assert!(
        matches!(&plan.operations[2], CheckedUnitEffectOperationPlan::CallContinuationCleanup {
        coordinate: cleanup, affine_discards,
    } if cleanup == coordinate && affine_discards.len() == 1)
    );
}

#[test]
fn discarded_boundary_result_accepts_a_projected_fixed_buffer() {
    let source = SOURCE.replace("machine run(buffer: &mut [u8]) reaches Host {", "data Root { host: Binding<Host>; buffer: [u8; 256]; }\nmachine Root::run(&mut self) reaches Host {")
        .replace("Host::mark(", "self.host.mark(")
        .replace("Host::read(buffer)", "self.host.read(&mut self.buffer)");
    let checked = checked_with_service(&source);
    let root = machine_named(&checked, "run");
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(root)
            .is_some(),
        "entry-owned raw fixed storage reaches the boundary reader"
    );
}

// A boundary that publishes `blocks;` still returns through the ordinary
// call edge: the acknowledged `block` call plans exactly like the
// nonblocking one, so an entry that waits on hosted input keeps its Unit
// plan and its attachment identity for ProgramEntry establishment.
#[test]
fn discarded_blocking_boundary_result_keeps_the_entry_unit_plan() {
    let source = SOURCE
        .replace(
            "machine read(buffer: &mut [u8]) -> ReadResult;",
            "machine read(buffer: &mut [u8]) -> ReadResult blocks;",
        )
        .replace("machine run(buffer: &mut [u8]) reaches Host {", "data Root { host: Binding<Host>; buffer: [u8; 256]; }\nmachine Root::run(&mut self) reaches Host {")
        .replace("Host::mark(", "self.host.mark(")
        .replace("_ = Host::read(buffer);", "_ = block self.host.read(&mut self.buffer);");
    let checked = checked_with_service(&source);
    let root = machine_named(&checked, "run");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(root)
        .unwrap_or_else(|| {
            panic!(
                "a blocking boundary read keeps the attached Unit caller: {:?}",
                checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .omission_for_machine(root)
            )
        });
    assert_eq!(plan.operations.len(), 5);
    assert!(matches!(
        &plan.operations[1],
        CheckedUnitEffectOperationPlan::BoundaryStructuralCall { coordinate, .. }
            if coordinate.statement_index == 1
    ));
    assert!(
        plan.attachment_type_identity.is_some(),
        "the attached entry rejoins its receiver type identity"
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .omission_for_machine(root)
            .is_none()
    );
}

// Suspension stays outside the synchronous Unit call vocabulary.
#[test]
fn discarded_suspending_boundary_result_still_omits_the_unit_plan() {
    let source = SOURCE
        .replace(
            "machine read(buffer: &mut [u8]) -> ReadResult;",
            "machine read(buffer: &mut [u8]) -> ReadResult;\n    machine poll() -> ReadResult suspends;",
        )
        .replace("_ = Host::read(buffer);", "_ = suspend Host::poll();");
    let checked = checked(&source);
    let root = machine_named(&checked, "run");
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(root)
            .is_none(),
        "a suspending boundary call has no synchronous Unit plan"
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .omission_for_machine(root)
            .is_some()
    );
}
