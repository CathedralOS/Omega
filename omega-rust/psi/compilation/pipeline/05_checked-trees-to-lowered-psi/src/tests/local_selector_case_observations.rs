//! A computed case-dispatch subject (`self.grid[self.r * 3 + self.c]`) is
//! hoisted into an immutable scalar local before the subject borrow, so each
//! arm's case observation selects its element with that local's `Local`
//! position. Lowering reads the local's established value and rejoins the
//! position to the authored selector name: a selector naming another slot is
//! refused rather than observing a different element.

use super::lower_machine;
use crate::TerminalMachineSelection;
use checked_trees::{
    CheckedRuntimeIndex, CheckedScalarComputationKind, CheckedScalarComputationStructuralArgument,
    CheckedTrees, CheckedUnitStructuralPathSegment,
};

const SOURCE: &str = r#"
    data Cell { case Wall; case Floor; case Door; }
    data Board { grid: [Cell; 6]; r: u32 in Wrapping; c: u32 in Wrapping; seen: u8; }
    machine Board::scan(&mut self) {
        transition self.r * 3 + self.c < 6 { true -> classify() _ -> done() }
        state classify(&mut self) {
            transition self.grid[self.r * 3 + self.c] {
                Cell::Wall -> mark(1)
                Cell::Floor -> mark(2)
                Cell::Door -> mark(3)
            }
        }
        state mark(&mut self, seen: u8) { self.seen = seen; }
        state done(&mut self) { }
    }
"#;

/// Apply `edit` to every local runtime-element selector the case
/// observations retained, returning how many there were.
fn edit_local_selectors(checked: &mut CheckedTrees, mut edit: impl FnMut(&mut u32)) -> usize {
    let mut count = 0;
    checked
        .facts
        .values
        .scalar_computations
        .nodes
        .for_each_mut(|_, node| {
            let CheckedScalarComputationKind::CaseMembership {
                subject: CheckedScalarComputationStructuralArgument::Place(place),
                ..
            } = &mut node.kind
            else {
                return;
            };
            for segment in &mut place.path {
                if let CheckedUnitStructuralPathSegment::RuntimeIndex(
                    CheckedRuntimeIndex::Local { position },
                ) = segment
                {
                    edit(position);
                    count += 1;
                }
            }
        });
    count
}

#[test]
fn a_computed_case_subject_selects_with_its_hoisted_local() {
    let mut original = crate::front_end::checked_program(SOURCE);
    assert_eq!(
        edit_local_selectors(&mut original, |_| {}),
        3,
        "each arm observes the element its hoisted local selects"
    );
    lower_machine(&original, TerminalMachineSelection::Name("Board::scan"))
        .expect("the local-selected observation lowers");
    let mut shifted = original.clone();
    edit_local_selectors(&mut shifted, |position| *position += 1);
    assert!(
        lower_machine(&shifted, TerminalMachineSelection::Name("Board::scan")).is_err(),
        "a selector naming another scalar slot must not lower"
    );
}
