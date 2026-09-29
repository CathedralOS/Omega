//! Borrowed receiver custody is independent of whether the body reads it.
//! Single-state scalar graphs retain it; multi-state bodies use the Unit
//! state graph, whose callers and edges preserve the declared receiver.
//! Tail-fused single-state bodies share one retained receiver.
use super::checked_program_result;
use checked_trees::{CheckedTrees, CheckedUnitEffectOperationPlan};

fn machine(checked: &CheckedTrees, suffix: &str) -> symbols::SymbolHandle {
    checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with(suffix))
        .unwrap()
        .symbol
}

/// The caller and callee must agree on the receiver even when no field is read.
#[test]
fn unread_multistate_receiver_keeps_its_unit_graph_and_callers() {
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
    assert!(
        checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(proc)
            .is_none(),
        "a receiver-free scalar plan cannot shadow the retained signature"
    );
    let graph = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(proc)
        .expect("the Unit state graph retains the unread receiver");
    assert_eq!(graph.states.len(), 2);
    assert!(
        graph.states.iter().all(|state| state
            .structural_parameters
            .iter()
            .any(|parameter| parameter.is_self)),
        "every authored receiver remains in its state's signature"
    );
    // Candidate pruning drops a caller whose scalar target is unavailable,
    // so the published plan is the availability evidence: the local's direct
    // receiver call is the caller's result operation into the Unit graph,
    // lending the declared shared receiver.
    let run = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine(&checked, "::run"))
        .expect("the Unit caller of the multi-state callee keeps its plan");
    assert!(matches!(
        run.operations.first(),
        Some(CheckedUnitEffectOperationPlan::Call(call)) if !call.is_boundary()
            && call.scalar_result().is_some()
            && call.target.machine == proc
            && matches!(call.structural_arguments.as_slice(), [receiver]
                if receiver.access == checked_trees::CheckedStructuralAccess::SharedBorrow)
    ));
}

#[test]
fn single_state_unread_mutable_receiver_keeps_its_declared_signature() {
    let checked = checked_program_result(
        r#"
        data Main { count: u64; }
        machine Main::value(&mut self, input: u64) -> u64 { input }
    "#,
    )
    .unwrap();
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(machine(&checked, "::value"))
        .expect("ordinary scalar body");
    let [state] = graph.states.as_slice() else {
        panic!("one state");
    };
    let [receiver] = state.structural_parameters.as_slice() else {
        panic!("the unread receiver remains a call operand");
    };
    assert!(receiver.is_self);
    assert_eq!(
        receiver.access,
        checked_trees::CheckedStructuralAccess::MutableBorrow
    );
}

const FUSED_RECEIVER_SOURCE: &str = r#"
    data Main { bias: u64; }
    machine Main::ping(&mut self, n: u64)
    terminates by n;
    -> u64
    {
        transition n == 0 {
            true -> (self.bias)
            _ -> self.pong(n - 1)
        }
    }
    machine Main::pong(PONG_RECEIVER, n: u64)
    terminates by n;
    -> u64
    {
        transition n == 0 {
            true -> (0)
            _ -> self.ping(n - 1)
        }
    }
"#;

/// Tail-fused single-state machines keep one retained receiver: every member
/// carries the same declared borrow, so the fused scalar graph (the only
/// owner of cross-machine tails) survives receiver retention.
#[test]
fn tail_fused_members_share_one_retained_receiver() {
    let source = FUSED_RECEIVER_SOURCE.replace("PONG_RECEIVER", "&mut self");
    let checked = checked_program_result(&source)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    for name in ["::ping", "::pong"] {
        let graph = checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(machine(&checked, name))
            .unwrap_or_else(|| panic!("{name} keeps its fused scalar graph"));
        assert_eq!(graph.states.len(), 2, "{name}");
        for state in &graph.states {
            let [receiver] = state.structural_parameters.as_slice() else {
                panic!("{name}: each member retains exactly its receiver");
            };
            assert!(receiver.is_self);
            assert_eq!(
                receiver.access,
                checked_trees::CheckedStructuralAccess::MutableBorrow
            );
        }
    }
}

const TAIL_RECEIVER_SOURCE: &str = r#"
    data Main { bias: u64; }
    machine Main::ping(&mut self, n: u64) -> u64 {
        transition n == 0 {
            true -> (self.bias)
            _ -> self.pong(n)
        }
    }
    machine Main::pong(PONG_RECEIVER, n: u64) -> u64 {
        transition { _ -> (n) }
    }
"#;

/// A member whose receiver differs from the entry's cannot share the one
/// machine-scope receiver, so the fusion declines instead of retargeting it.
/// (A cycle cannot mix accesses: a shared body cannot lend `&mut self`.)
#[test]
fn tail_fused_members_with_different_receivers_decline() {
    for (receiver, fused) in [("&mut self", true), ("&self", false)] {
        let source = TAIL_RECEIVER_SOURCE.replace("PONG_RECEIVER", receiver);
        let checked = checked_program_result(&source)
            .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
        let graph = checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(machine(&checked, "::ping"));
        if fused {
            let graph = graph.expect("an identical receiver keeps the fused tail graph");
            assert_eq!(graph.states.len(), 2);
        } else {
            assert!(
                graph.is_none(),
                "a shared member cannot reuse the entry's exclusive receiver"
            );
        }
    }
}
