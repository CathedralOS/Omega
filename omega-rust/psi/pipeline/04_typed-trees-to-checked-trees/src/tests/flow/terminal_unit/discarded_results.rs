//! Explicit result disposal must not turn a value-returning boundary into Unit.
use super::CheckedUnitEffectOperationPlan;
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
