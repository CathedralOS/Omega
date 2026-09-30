//! Fact-only citations leave Terminal untouched.
//!
//! A statement call citing a theorem-only machine emits no runtime call or
//! work: its checked premises and conclusions stay with checking, and the
//! theorem enters no unit closure. The citing body lowers exactly as it would
//! without the statement, in an ordinary sequence, a scalar completion and a
//! state graph's entry state alike.

use super::lower_machine;
use crate::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use terminal_psi::OperationKind;

const SOURCE: &str = "
    machine small(value: u64)
    requires value < 8
    ensures value < 9
    {
    }

    data Counter { count: u64 in Wrapping; }

    machine Counter::reset(&mut self) {
        small(3);
        self.count = 1;
    }

    machine Counter::read(&mut self) -> u64 in Wrapping {
        small(4);
        transition { _ -> (self.count) }
    }

    machine Counter::choose(&mut self) {
        small(5);
        transition self.count == 1 {
            true -> one()
            _ -> other()
        }
        state one(&mut self) { self.count = 2; }
        state other(&mut self) { self.count = 3; }
    }

    machine Counter::run(&mut self) {
        self.reset();
        let seen: u64 in Wrapping = self.read();
        self.choose();
        self.count = seen;
    }
";

#[test]
fn fact_only_citations_emit_no_call_or_machine() {
    let checked = crate::front_end::checked_program(SOURCE);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Counter::run"))
        .expect("citing bodies lower without the cited theorem");
    // run, reset, read and choose; `small` is never a Terminal machine.
    assert_eq!(lowered.semantic_module.machines.len(), 4);
    let calls = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                OperationKind::CallUnit { .. }
                    | OperationKind::CallStructuralScalar { .. }
                    | OperationKind::Call { .. }
            )
        })
        .count();
    assert_eq!(calls, 3, "only run's three authored calls remain");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .unwrap();
}
