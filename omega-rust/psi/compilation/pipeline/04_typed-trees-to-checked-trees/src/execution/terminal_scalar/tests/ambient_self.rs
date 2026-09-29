//! Borrowed receiver custody is independent of whether the body reads it.
//! Single-state scalar graphs retain it; multi-state bodies use the Unit
//! state graph, whose callers and edges preserve the declared receiver.
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
