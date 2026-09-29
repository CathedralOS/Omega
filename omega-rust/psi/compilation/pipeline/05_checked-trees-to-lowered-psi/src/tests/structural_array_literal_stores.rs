//! A whole array literal of structural elements stored into a fixed-array
//! field has no whole-array establishment in Terminal. The checked plan
//! establishes every element first, then replaces `field[k]` through one
//! window pair per element. Both the single-state body and the composed state
//! graph must rejoin that roster to the authored literal element by element:
//! reordering, dropping or retargeting a member is refused at lowering.

use super::lower_machine;
use crate::TerminalMachineSelection;
use checked_trees::{
    CheckedTrees, CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralPathSegment,
};
use terminal_psi::OperationKind;

/// A single-state body: its operations live in the ordinary machine plan.
const SINGLE_STATE: &str = r#"
    data Cell { case Wall; case Floor; case Door; }
    data Board { cells: [Cell; 3]; }
    machine Board::reset(&mut self) { self.cells = [Cell::Door, Cell::Wall, Cell::Floor]; }
"#;

/// A state graph: the literal's roster sits in the entry state, and a later
/// guard observes one replaced element.
const STATE_GRAPH: &str = r#"
    data Cell { case Wall; case Floor; case Door; }
    data Board { cells: [Cell; 3]; }
    machine Board::reset(&mut self) {
        self.cells = [Cell::Door, Cell::Wall, Cell::Floor];
        transition self.cells[0] { Cell::Door -> good() _ -> bad() }
        state good(&mut self) { }
        state bad(&mut self) { }
    }
"#;

/// A nested literal: each leaf replaces the element at both of its indices.
const NESTED: &str = r#"
    data Cell { case Wall; case Floor; case Door; }
    data Board { rows: [[Cell; 2]; 2]; }
    machine Board::reset(&mut self) {
        self.rows = [[Cell::Door, Cell::Wall], [Cell::Floor, Cell::Door]];
    }
"#;

/// Affine record elements: each displaced old element is disposed on the
/// statement's continuation after its own store.
const AFFINE_RECORDS: &str = r#"
    data Point { x: i32; y: i32; }
    data Board { cells: [Point; 3]; }
    machine Board::reset(&mut self) {
        self.cells = [Point { x: 1, y: 2 }, Point { x: 3, y: 4 }, Point { x: 5, y: 6 }];
    }
"#;

fn lower(checked: &CheckedTrees) -> Result<usize, String> {
    lower_machine(checked, TerminalMachineSelection::Name("Board::reset"))
        .map(|lowered| {
            lowered
                .semantic_module
                .machines
                .iter()
                .flat_map(|machine| &machine.blocks)
                .flat_map(|block| &block.operations)
                .filter(|operation| {
                    matches!(operation.kind, OperationKind::StoreStructuralField { .. })
                })
                .count()
        })
        .map_err(|error| format!("{error:?}"))
}

/// The one operation list that plans the literal's element stores, whichever
/// plan family holds it.
fn replacing_operations(checked: &mut CheckedTrees) -> &mut Vec<CheckedUnitEffectOperationPlan> {
    let effects = &mut checked.facts.flow.terminal_unit_effects;
    let has_store = |operations: &[CheckedUnitEffectOperationPlan]| {
        operations.iter().any(|operation| {
            matches!(
                operation,
                CheckedUnitEffectOperationPlan::StoreStructuralField { .. }
            )
        })
    };
    if let Some(plan) = effects
        .machines
        .iter_mut()
        .find(|plan| has_store(&plan.operations))
    {
        return &mut plan.operations;
    }
    &mut effects
        .composed_machines
        .iter_mut()
        .flat_map(|plan| plan.states.iter_mut())
        .find(|state| has_store(&state.operations))
        .expect("the literal plans element stores")
        .operations
}

fn positions(
    operations: &[CheckedUnitEffectOperationPlan],
    selected: fn(&CheckedUnitEffectOperationPlan) -> bool,
) -> Vec<usize> {
    operations
        .iter()
        .enumerate()
        .filter(|(_, operation)| selected(operation))
        .map(|(position, _)| position)
        .collect()
}

fn is_establishment(operation: &CheckedUnitEffectOperationPlan) -> bool {
    matches!(
        operation,
        CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
    )
}

fn is_move(operation: &CheckedUnitEffectOperationPlan) -> bool {
    matches!(
        operation,
        CheckedUnitEffectOperationPlan::MoveStructuralField { .. }
    )
}

fn is_store(operation: &CheckedUnitEffectOperationPlan) -> bool {
    matches!(
        operation,
        CheckedUnitEffectOperationPlan::StoreStructuralField { .. }
    )
}

/// Point one element's window pair at another element of the same array.
fn retarget_window(operations: &mut [CheckedUnitEffectOperationPlan], element: usize, to: u64) {
    let retarget = |path: &mut Vec<CheckedUnitStructuralPathSegment>| {
        if let Some(CheckedUnitStructuralPathSegment::FixedIndex(index)) = path.last_mut() {
            *index = to;
        }
    };
    let moved = positions(operations, is_move)[element];
    let stored = positions(operations, is_store)[element];
    if let CheckedUnitEffectOperationPlan::MoveStructuralField { source, .. } =
        &mut operations[moved]
    {
        retarget(&mut source.path);
    }
    if let CheckedUnitEffectOperationPlan::StoreStructuralField { destination, .. } =
        &mut operations[stored]
    {
        retarget(&mut destination.path);
    }
}

type Mutation = (&'static str, fn(&mut Vec<CheckedUnitEffectOperationPlan>));

const MUTATIONS: [Mutation; 5] = [
    ("reordered element establishments", |operations| {
        let established = positions(operations, is_establishment);
        operations.swap(established[0], established[1]);
    }),
    ("dropped element window pair", |operations| {
        let stored = *positions(operations, is_store).last().unwrap();
        operations.remove(stored);
        operations.remove(stored - 1);
    }),
    ("swapped element destinations", |operations| {
        retarget_window(operations, 0, 1);
        retarget_window(operations, 1, 0);
    }),
    ("element stored twice", |operations| {
        retarget_window(operations, 2, 1);
    }),
    ("store of another element's value", |operations| {
        let established = positions(operations, is_establishment);
        let CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. } =
            &operations[established[1]]
        else {
            unreachable!()
        };
        let other = result.binding_ordinal;
        let stored = positions(operations, is_store)[0];
        if let CheckedUnitEffectOperationPlan::StoreStructuralField { value, .. } =
            &mut operations[stored]
        {
            value.source = CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: other,
            };
        }
    }),
];

fn assert_roster_is_exact(source: &str, leaves: usize) {
    let original = crate::front_end::checked_program(source);
    assert_eq!(
        lower(&original),
        Ok(leaves),
        "each element replaces its own window"
    );
    for (label, mutate) in MUTATIONS {
        let mut changed = original.clone();
        let operations = replacing_operations(&mut changed);
        assert_eq!(positions(operations, is_establishment).len(), leaves);
        mutate(operations);
        assert!(lower(&changed).is_err(), "{label} must not lower");
    }
}

fn is_cleanup(operation: &CheckedUnitEffectOperationPlan) -> bool {
    matches!(
        operation,
        CheckedUnitEffectOperationPlan::CallContinuationCleanup { .. }
    )
}

#[test]
fn a_single_state_structural_array_literal_rejoins_every_element() {
    assert_roster_is_exact(SINGLE_STATE, 3);
}

#[test]
fn a_state_graph_structural_array_literal_rejoins_every_element() {
    assert_roster_is_exact(STATE_GRAPH, 3);
}

#[test]
fn a_nested_structural_array_literal_rejoins_every_leaf() {
    assert_roster_is_exact(NESTED, 4);
}

#[test]
fn displaced_affine_elements_keep_their_own_disposal() {
    assert_roster_is_exact(AFFINE_RECORDS, 3);
    let original = crate::front_end::checked_program(AFFINE_RECORDS);
    let mut changed = original.clone();
    let operations = replacing_operations(&mut changed);
    let cleanups = positions(operations, is_cleanup);
    assert_eq!(
        cleanups.len(),
        3,
        "each displaced element dies on the continuation"
    );
    operations.remove(cleanups[1]);
    assert!(
        lower(&changed).is_err(),
        "a displaced element without its disposal must not lower"
    );
}
