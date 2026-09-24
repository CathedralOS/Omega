//! Only a machine of one authored state retains its borrowed `self` on a
//! scalar graph; elsewhere the receiver stays ambient on the attachment
//! carrier. A multi-state machine that reads through it belongs to the Unit
//! state graph, whose callers rejoin the receiver; one that never reads it
//! keeps a receiver-free graph its Unit callers reach as a pure scalar call.
use super::checked_program_result;
use checked_trees::{CheckedControlResultPlan, CheckedTrees, CheckedUnitEffectOperationPlan};

fn machine(checked: &CheckedTrees, suffix: &str) -> symbols::SymbolHandle {
    checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with(suffix))
        .unwrap()
        .symbol
}

fn assert_state_graph_owns(checked: &CheckedTrees, symbol: symbols::SymbolHandle) {
    assert!(
        checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(symbol)
            .is_none(),
        "a receiver-reading machine has no scalar graph"
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(symbol)
        .expect("the Unit state graph owns the receiver-reading machine");
    assert!(matches!(
        plan.result,
        CheckedControlResultPlan::Scalar { .. }
    ));
    assert!(
        plan.states[0]
            .structural_parameters
            .iter()
            .any(|parameter| parameter.is_self),
        "the state graph retains the receiver its callers rejoin"
    );
}

#[test]
fn receiver_field_reads_leave_the_scalar_graph_for_the_state_graph() {
    let source = r#"
        data Filter { width: u64; }
        machine Filter::count(&self, alignment: u64) -> u64
        crashes Abort
        {
            transition alignment > 0 {
                true -> divide(alignment)
                false -> violated(alignment)
            }
            state violated(&self, alignment: u64) -> u64 {
                crash Abort;
            }
            state divide(&self, alignment: u64) -> u64 {
                transition {
                    _ -> (self.width / alignment)
                }
            }
        }
    "#;
    let checked = checked_program_result(source)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    assert_state_graph_owns(&checked, machine(&checked, "::count"));
}

#[test]
fn mixed_receiver_reads_leave_the_scalar_graph_for_the_state_graph() {
    let source = r#"
        data Alignment { bytes: u64; }
        data Filter { width: u64; }
        machine Filter::region_size(&self) -> u64 {
            self.width
        }
        machine Filter::count(&self, width: u64, alignment: Alignment) -> u64
        crashes Abort
        {
            let bytes: u64 = alignment.bytes;
            transition self.width >= width && bytes > 0 {
                true -> divide(width, bytes)
                false -> violated()
            }
            state violated(&self) -> u64 {
                crash Abort;
            }
            state divide(&self, width: u64, bytes: u64) -> u64 {
                let size: u64 = self.region_size();
                transition {
                    _ -> (size / bytes)
                }
            }
        }
    "#;
    let checked = checked_program_result(source)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    assert_state_graph_owns(&checked, machine(&checked, "::count"));
}

/// The receiver-free graph is the Unit caller's pure scalar target, so a
/// multi-state callee a `run` state calls through `self` stays callable.
#[test]
fn unread_receiver_keeps_a_receiver_free_graph_its_unit_callers_reach() {
    let source = r#"
        data Main { width: u64; }
        machine Main::proc(&self, n: u64 [0..=1000]) -> u64 {
            transition n > 0 {
                true -> work(n)
                false -> 0
            }
            state work(&self, n: u64 [1..=1000]) -> u64 {
                let a: u64 [0..=1001] = n + 1;
                transition {
                    _ -> (a)
                }
            }
        }
        machine Main::run(&mut self) {
            let r: u64 = self.proc(5);
            self.width = r;
        }
    "#;
    let checked = checked_program_result(source)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    let proc = machine(&checked, "::proc");
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(proc)
        .expect("an unread receiver keeps the scalar graph");
    assert_eq!(graph.states.len(), 2);
    assert!(
        graph
            .states
            .iter()
            .all(|state| state.structural_parameters.is_empty()),
        "the graph retains no receiver"
    );
    // Candidate pruning drops a caller whose scalar target is unavailable,
    // so the published plan is the availability evidence: the call is a
    // computation root of the caller's scalar local.
    let run = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine(&checked, "::run"))
        .expect("the Unit caller of the multi-state callee keeps its plan");
    assert!(matches!(
        run.operations.first(),
        Some(CheckedUnitEffectOperationPlan::EstablishScalarLocal {
            value: checked_trees::CheckedCallScalarArgument::Computation(_),
            ..
        })
    ));
}
