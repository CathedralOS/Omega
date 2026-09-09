//! Explicit result disposal must not turn a value-returning boundary into Unit.

use super::*;

const SOURCE: &str = r#"
data ReadResult { case Empty; case Bytes(count: u64); }
boundary trait Host {
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
    let source = SOURCE.replace("machine run(buffer: &mut [u8]) reaches Host {", "data Root { host: Host; buffer: [u8; 256]; }\nmachine Root::run(&mut self) reaches Host {")
        .replace("Host::mark(", "self.host.mark(")
        .replace("Host::read(buffer)", "self.host.read(&mut self.buffer)");
    let checked = checked(&source);
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
