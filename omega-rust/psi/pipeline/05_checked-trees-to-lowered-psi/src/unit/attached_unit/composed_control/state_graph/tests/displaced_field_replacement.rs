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

fn entry_kinds(module: &terminal_psi::TerminalModule) -> Vec<&OperationKind> {
    module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("entry machine")
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .map(|operation| &operation.kind)
        .collect()
}

#[test]
fn displaced_call_result_field_replacement_lowers_and_verifies() {
    let checked = crate::front_end::checked_program(SOURCE);
    let lowered = crate::lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .expect("the composed graph lowers its displaced field replacement");
    let kinds = entry_kinds(&lowered.semantic_module);
    let position = |wanted: fn(&OperationKind) -> bool| {
        kinds
            .iter()
            .position(|kind| wanted(kind))
            .expect("the replacement emits every member")
    };
    let call = position(|kind| {
        matches!(
            kind,
            OperationKind::CallStructuralWithScalarArguments { .. }
        )
    });
    let moved = position(|kind| matches!(kind, OperationKind::MoveStructuralField { .. }));
    let stored = position(|kind| matches!(kind, OperationKind::StoreStructuralField { .. }));
    assert!(
        call < moved && moved < stored,
        "the call evaluates before the old value is displaced and the window closes last"
    );
    let (
        OperationKind::MoveStructuralField {
            source,
            path,
            field,
        },
        OperationKind::StoreStructuralField {
            destination,
            path: store_path,
            field: store_field,
            ..
        },
    ) = (kinds[moved], kinds[stored])
    else {
        unreachable!("positions select the window pair")
    };
    assert_eq!(
        (source, path, field),
        (destination, store_path, store_field),
        "the store reseats the exact hole the move opened"
    );
    // The displaced value dies on the call's continuation edge, as planned.
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .unwrap();
    let displaced = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::MoveStructuralField { .. }))
        .and_then(|operation| match &operation.result {
            terminal_psi::OperationResult::Structural(result) => Some(result.place),
            _ => None,
        })
        .expect("the move binds the displaced value");
    assert!(entry.blocks.iter().any(|block| matches!(&block.terminator,
        Terminator::Jump { trivial_affine_discards, .. } if trivial_affine_discards == &[displaced])));
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("independent verification reconstructs the closed window");
    let _artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Main::main"),
    )
    .produce(
        terminal_production::TerminalProductionCustody::artifact_only(
            &mut terminal_production::TerminalProductionTimings::default(),
        ),
    )
    .expect("the replacement publishes a checked Terminal artifact");
}

/// Sibling work after the replacement and a replacement in a later state
/// compose through the same operation sequence. (A copy-typed field is not a
/// variation here yet: checking omits that caller's plan at result custody
/// accounting before lowering sees it.)
#[test]
fn displaced_field_replacement_composes_with_sibling_work_and_later_states() {
    let checked = crate::front_end::checked_program(
        r#"
        data Vec2 { x: i32 in Wrapping; y: i32 in Wrapping; }
        data Main { hits: i32 in Wrapping; vec: Vec2; }
        machine Main::make(&mut self) -> Vec2 {
            transition self.hits == 0 { true -> first() _ -> again() }
            state first(&mut self) -> Vec2 { self.hits = self.hits + 1; Vec2 { x: 1, y: 2 } }
            state again(&mut self) -> Vec2 { Vec2 { x: 5, y: 6 } }
        }
        machine Main::main(&mut self) {
            transition self.hits == 0 { true -> replace() _ -> done() }
            state replace(&mut self) {
                self.vec = self.make();
                self.hits = self.hits + self.vec.y;
                transition { _ -> done() }
            }
            state done(&mut self) { }
        }
        "#,
    );
    let lowered = crate::lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .expect("a later state's replacement lowers beside its sibling store");
    assert!(
        entry_kinds(&lowered.semantic_module)
            .iter()
            .any(|kind| matches!(kind, OperationKind::StoreStructuralField { .. }))
    );
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("independent verification reconstructs the closed window");
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
