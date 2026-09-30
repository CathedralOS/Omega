//! `self.field = self.call(..)` inside a composed state graph: the checked
//! plan spells the displaced old value's move-out, the store of the call's
//! whole result into the opened window, and the displaced value's cleanup at
//! the call's one authored statement. The graph must rejoin that roster to the
//! assignment, emit the window through `BorrowedWindowLedger`, and leave the
//! window reconstruction to independent Terminal verification.
use super::CheckedTrees;
use crate::TerminalMachineSelection;
use checked_trees::{
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralPathSegment,
};
use terminal_psi::{OperationKind, Terminator};

#[test]
fn bound_field_replacement_rejects_another_equal_typed_local() {
    // Both bodies store the same declared local. Copy multiplicity makes the
    // other local legally available too, so rejection must prove identity,
    // not merely notice an extra affine consumption.
    for ending in [
        "",
        "transition context.cell.value == 19 { true -> done(context) _ -> done(context) }
         state done(context: &mut Context) {}",
    ] {
        let source = format!(
            "data Cell [copy] {{ value: u64; }}
             data Context {{ cell: Cell; }}
             machine replace(context: &mut Context) {{
                 let first: Cell = Cell {{ value: 17 }};
                 let second: Cell = Cell {{ value: 19 }};
                 context.cell = second;
                 {ending}
             }}"
        );
        let original = crate::front_end::checked_program(&source);
        let machine = original
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "replace")
            .expect("replacement machine");
        let authored = &original.machine_states(machine)[0];
        // Exercise the independent source replay in both consumers. The
        // native corpus separately executes affine mixed and closed sums;
        // this control uses two simultaneously live copy records so it tests
        // source identity without depending on affine-use rejection.
        let replay = |checked: &CheckedTrees| {
            let plans = &checked.facts.flow.terminal_unit_effects;
            if let Some(plan) = plans.for_machine(machine.symbol) {
                crate::unit::attached_unit::call_closure::validate_unit_operation_sequence(
                    checked, plan,
                )
            } else {
                let plan = plans
                    .composed_machines
                    .iter()
                    .find(|plan| plan.machine == machine.symbol)
                    .expect("composed replacement");
                super::super::body::validate(checked, machine.symbol, authored, &plan.states[0])
                    .map(|_| ())
            }
        };
        replay(&original).expect("both bodies rejoin their actual bound source");
        let lowered = crate::lower_machine(&original, TerminalMachineSelection::Name("replace"))
            .expect("bound copy storage remains executable after a construction join");
        terminal_verifier::validate_module(&lowered.semantic_module).expect("valid owned copy");
        let first = original
            .statement_table
            .statements(authored.statement_nodes)
            .iter()
            .find_map(|statement| match statement {
                checked_trees::statement::StatementNode::LocalData(local) => Some(local.symbol),
                _ => None,
            })
            .expect("first local");
        for substitute in [
            CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal: 0 },
            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol: first },
        ] {
            let mut forged = original.clone();
            let plans = &mut forged.facts.flow.terminal_unit_effects;
            let operations = if let Some(plan) = plans
                .machines
                .iter_mut()
                .find(|plan| plan.machine == machine.symbol)
            {
                &mut plan.operations
            } else {
                &mut plans
                    .composed_machines
                    .iter_mut()
                    .find(|plan| plan.machine == machine.symbol)
                    .expect("composed replacement")
                    .states[0]
                    .operations
            };
            let value = operations
                .iter_mut()
                .find_map(|operation| match operation {
                    CheckedUnitEffectOperationPlan::StoreStructuralField { value, .. } => {
                        Some(value)
                    }
                    _ => None,
                })
                .expect("one whole-value store");
            value.source = substitute;
            let error = replay(&forged)
                .expect_err("same-carrier storage cannot replace the authored local");
            let diagnostic = error.to_string();
            assert!(
                diagnostic.contains("bound field replacement substituted its local or producer")
                    || diagnostic.contains(
                        "Unit graph field replacement stores another value than its call"
                    ),
                "the bound local or its freshly copied snapshot must retain its source: {error}"
            );
        }
    }
}

/// The `calls/value_call_effectful_arm_rejected` customer's shape: an
/// effectful multi-state callee returns a record that replaces a record field
/// of the caller's exclusive receiver, and a later guard reads it back.
const SOURCE: &str = r#"
    data Vec2 { x: i32 in Wrapping; y: i32 in Wrapping; }
    data Main { d: i32; hits: i32 in Wrapping; vec: Vec2; spare: Vec2; }
    machine Main::delta(&mut self, d: i32) -> Vec2 {
        transition d == 1 { true -> pos() _ -> neg() }
        state pos(&mut self) -> Vec2 { self.hits = self.hits + 1; Vec2 { x: 3, y: 4 } }
        state neg(&mut self) -> Vec2 { self.hits = self.hits + 10; Vec2 { x: 7, y: 9 } }
    }
    machine Main::main(&mut self) {
        self.d = 1;
        self.vec = self.delta(self.d);
        transition self.vec.x == 3 { true -> good() _ -> bad(71) }
        state good(&mut self) { }
        state bad(&mut self, code: i32) { }
    }
"#;

fn replacing_state(checked: &mut CheckedTrees) -> &mut Vec<CheckedUnitEffectOperationPlan> {
    &mut checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter_mut()
        .flat_map(|plan| plan.states.iter_mut())
        .find(|state| {
            state.operations.iter().any(|operation| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::StoreStructuralField { .. }
                )
            })
        })
        .expect("the entry state plans the displaced-field window")
        .operations
}

/// Each member of the roster is load-bearing: dropping, reordering or
/// retargeting one is refused at lowering rather than emitted.
#[test]
fn displaced_field_replacement_rejects_plan_drift() {
    let original = crate::front_end::checked_program(SOURCE);
    crate::lower_machine(&original, TerminalMachineSelection::Name("Main::main"))
        .expect("unmodified control");
    let is_move = |operation: &CheckedUnitEffectOperationPlan| {
        matches!(
            operation,
            CheckedUnitEffectOperationPlan::MoveStructuralField { .. }
        )
    };
    let is_store = |operation: &CheckedUnitEffectOperationPlan| {
        matches!(
            operation,
            CheckedUnitEffectOperationPlan::StoreStructuralField { .. }
        )
    };
    let is_cleanup = |operation: &CheckedUnitEffectOperationPlan| {
        matches!(
            operation,
            CheckedUnitEffectOperationPlan::CallContinuationCleanup { .. }
        )
    };
    let mutations: [(&str, fn(&mut Vec<CheckedUnitEffectOperationPlan>)); 7] = [
        ("missing store", |operations| {
            operations.retain(|operation| {
                !matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::StoreStructuralField { .. }
                )
            })
        }),
        ("missing move", |operations| {
            operations.retain(|operation| {
                !matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::MoveStructuralField { .. }
                )
            })
        }),
        ("missing displaced disposal", |operations| {
            operations.retain(|operation| {
                !matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::CallContinuationCleanup { .. }
                )
            })
        }),
        ("store before move", |operations| {
            let moved = operations
                .iter()
                .position(|operation| {
                    matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::MoveStructuralField { .. }
                    )
                })
                .unwrap();
            operations.swap(moved, moved + 1);
        }),
        ("store at another place", |operations| {
            for operation in operations.iter_mut() {
                if let CheckedUnitEffectOperationPlan::StoreStructuralField {
                    destination, ..
                } = operation
                {
                    destination.path =
                        vec![CheckedUnitStructuralPathSegment::Field("spare".to_owned())];
                }
            }
        }),
        ("window at another place", |operations| {
            for operation in operations.iter_mut() {
                match operation {
                    CheckedUnitEffectOperationPlan::StoreStructuralField {
                        destination, ..
                    } => {
                        destination.path =
                            vec![CheckedUnitStructuralPathSegment::Field("spare".to_owned())];
                    }
                    CheckedUnitEffectOperationPlan::MoveStructuralField { source, .. } => {
                        source.path =
                            vec![CheckedUnitStructuralPathSegment::Field("spare".to_owned())];
                    }
                    _ => {}
                }
            }
        }),
        ("store of the displaced value", |operations| {
            for operation in operations.iter_mut() {
                if let CheckedUnitEffectOperationPlan::StoreStructuralField { value, .. } =
                    operation
                {
                    value.source = CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal: 1,
                    };
                }
            }
        }),
    ];
    for (label, mutate) in mutations {
        let mut changed = original.clone();
        let operations = replacing_state(&mut changed);
        assert!(operations.iter().any(is_move) && operations.iter().any(is_store));
        assert!(operations.iter().any(is_cleanup));
        mutate(operations);
        assert!(
            crate::lower_machine(&changed, TerminalMachineSelection::Name("Main::main")).is_err(),
            "{label} must not lower"
        );
    }
}

/// Terminal verification reconstructs the window from the emitted operations
/// alone: a module whose store was dropped, or whose store moved across the
/// continuation edge behind the guard that reads the replaced field, is
/// refused without consulting the producer's ledger.
#[test]
fn terminal_verification_refuses_an_unclosed_or_stale_read_window() {
    let checked = crate::front_end::checked_program(SOURCE);
    let lowered = crate::lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .expect("unmodified control");
    terminal_verifier::validate_module(&lowered.semantic_module).expect("unmodified control");
    let entry = lowered.semantic_module.entry;
    let take_store = |module: &mut terminal_psi::TerminalModule| {
        let machine = module
            .machines
            .iter_mut()
            .find(|machine| machine.id == entry)
            .unwrap();
        machine
            .blocks
            .iter_mut()
            .find_map(|block| {
                let index = block.operations.iter().position(|operation| {
                    matches!(operation.kind, OperationKind::StoreStructuralField { .. })
                })?;
                let target = match &block.terminator {
                    Terminator::Jump { target, .. } => *target,
                    _ => return None,
                };
                Some((block.operations.remove(index), target))
            })
            .expect("the store sits before the continuation jump")
    };
    let mut dropped = lowered.semantic_module.clone();
    take_store(&mut dropped);
    assert!(
        terminal_verifier::validate_module(&dropped).is_err(),
        "a move with no restoring store leaves the hole open at the exit"
    );
    let mut crossing = lowered.semantic_module.clone();
    let (store, successor) = take_store(&mut crossing);
    crossing
        .machines
        .iter_mut()
        .find(|machine| machine.id == entry)
        .unwrap()
        .blocks
        .iter_mut()
        .find(|block| block.id == successor)
        .unwrap()
        .operations
        .push(store);
    assert!(
        terminal_verifier::validate_module(&crossing).is_err(),
        "the guard may not read the replaced field while its hole is open"
    );
}
