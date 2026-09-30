//! The declaration predicate roster crosses into Terminal.
//!
//! `Grid` states its cells with a collection invariant. Every machine whose
//! parameters reach a `Grid` publishes each cell's checked consequence,
//! `0 <= cell <= 1`, as an entry requirement, and an exclusive borrow also
//! publishes it as a normal-return guarantee. Terminal verification then
//! re-proves the rows independently: a callee's stores must restore them
//! before it returns, a caller must hold them before the call, and the Exact
//! sum reads its operand bounds from them. The fact-only `cell_is_bit`
//! citations leave no call or machine behind.

use super::lower_machine;
use crate::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::IntegerValue;
use terminal_psi::OperationKind;

const PREDICATE: &str = r#"
machine cells_valid(cells: &[i32; 4]) -> bool {
    transition cells[0] == 0 || cells[0] == 1 {
        true -> at_1(cells)
        _ -> (false)
    }
    state at_1(cells: &[i32; 4]) -> bool {
        transition cells[1] == 0 || cells[1] == 1 {
            true -> at_2(cells)
            _ -> (false)
        }
    }
    state at_2(cells: &[i32; 4]) -> bool {
        transition cells[2] == 0 || cells[2] == 1 {
            true -> at_3(cells)
            _ -> (false)
        }
    }
    state at_3(cells: &[i32; 4]) -> bool {
        transition cells[3] == 0 || cells[3] == 1 {
            true -> (true)
            _ -> (false)
        }
    }
}

machine cell_is_bit(cells: &[i32; 4], index: u64)
requires cells_valid(cells) == true
requires index < 4
ensures cells[index] >= 0 && cells[index] <= 1
{
}

data Grid [copy] where cells_valid(&board) == true, cells_valid(&next) == true {
    board: [i32; 4];
    next: [i32; 4];
    offset: i32 in Wrapping;
}

data Main {
    g: Grid;
    total: i32 in Wrapping;
}

machine Main::seed(context: &mut Main) {
    context.g.board[1] = 1;
    context.g.board[2] = 1;
    context.g.offset = 68;
}

// One generation of cell 0: cite its neighbours, then store through states.
machine Main::advance(context: &mut Main) {
    cell_is_bit(&context.g.board, 1);
    cell_is_bit(&context.g.board, 2);
    let neighbours: i32 in Wrapping = (context.g.board[1] + context.g.board[2]) as i32 in Wrapping;
    transition neighbours == 2 {
        true -> live(context)
        _ -> die(context)
    }
    state live(context: &mut Main) { context.g.next[0] = 1; }
    state die(context: &mut Main) { context.g.next[0] = 0; }
}

machine Main::sum(context: &mut Main) -> i32 {
    cell_is_bit(&context.g.next, 0);
    cell_is_bit(&context.g.next, 1);
    cell_is_bit(&context.g.next, 2);
    cell_is_bit(&context.g.next, 3);
    transition {
        _ -> (context.g.next[0] + context.g.next[1] + context.g.next[2] + context.g.next[3])
    }
}
"#;

fn source(main_body: &str) -> String {
    format!(
        "{PREDICATE}
         machine Main::main(context: &mut Main) {{
             {main_body}
             Main::seed(context);
             Main::advance(context);
             let cells: i32 in Wrapping = Main::sum(context) as i32 in Wrapping;
             context.total = cells + context.g.offset;
         }}"
    )
}

fn lowered(main_body: &str) -> lowered_psi::LoweredPsi {
    let checked = crate::front_end::checked_program(&source(main_body));
    lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .expect("a collection invariant lowers through its declaration predicate rows")
}

fn verify(module: &terminal_psi::TerminalModule, lowered: &lowered_psi::LoweredPsi) -> bool {
    terminal_verifier::verify_module(module, &lowered.proof_bundle, &AdmissionProfile::default())
        .is_ok()
}

fn has_operation(machine: &terminal_psi::TerminalMachine, kind: &OperationKind) -> bool {
    machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .any(|operation| operation.kind == *kind)
}

fn integer(value: i128) -> OperationKind {
    OperationKind::IntegerConstant {
        value: IntegerValue::Signed(value),
    }
}

/// Replace every integer constant `from` with `to` in the machines `select`
/// admits, returning how many operations changed.
fn substitute_constant(
    module: &mut terminal_psi::TerminalModule,
    select: impl Fn(&terminal_psi::TerminalMachine) -> bool,
    from: i128,
    to: i128,
) -> usize {
    let mut changed = 0;
    for machine in module.machines.iter_mut().filter(|machine| select(machine)) {
        for operation in machine
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
        {
            if operation.kind == integer(from) {
                operation.kind = integer(to);
                changed += 1;
            }
        }
    }
    changed
}

#[test]
fn roster_rows_carry_the_collection_invariant_through_calls_stores_and_sums() {
    let lowered = lowered("");
    assert!(verify(&lowered.semantic_module, &lowered));
    // Main, seed, advance and sum; the citations leave neither a call nor a
    // machine.
    assert_eq!(lowered.semantic_module.machines.len(), 4);
    for machine in &lowered.semantic_module.machines {
        // Each machine reaches `Grid` through its exclusive `context`: each
        // cell's two bounds are separate entry requirements, and the normal
        // return restores all eight cell intervals in one guarantee.
        assert_eq!(machine.contract.requires.len(), 16, "{:?}", machine.id);
        assert_eq!(machine.contract.ensures.len(), 1, "{:?}", machine.id);
    }
}

#[test]
fn a_callee_store_that_breaks_the_invariant_cannot_restore_it_at_return() {
    let lowered = lowered("");
    let mut broken = lowered.semantic_module.clone();
    // `seed` alone stores the offset 68; its two cell stores write 1.
    let seed = |machine: &terminal_psi::TerminalMachine| has_operation(machine, &integer(68));
    assert_eq!(substitute_constant(&mut broken, seed, 1, 7), 2);
    assert!(
        !verify(&broken, &lowered),
        "a cell of 7 cannot establish the returned `0 <= cell <= 1` row"
    );
}

#[test]
fn a_caller_store_that_breaks_the_invariant_cannot_discharge_the_call() {
    let lowered = lowered("context.g.board[3] = 0;");
    assert!(verify(&lowered.semantic_module, &lowered));
    let mut broken = lowered.semantic_module.clone();
    // `main` alone calls; its one store writes 0 before calling `seed`.
    let main = |machine: &terminal_psi::TerminalMachine| {
        machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(operation.kind, OperationKind::CallUnit { .. }))
    };
    assert_eq!(substitute_constant(&mut broken, main, 0, 5), 1);
    assert!(
        !verify(&broken, &lowered),
        "a cell of 5 cannot discharge the callee's `0 <= cell <= 1` requirement"
    );
}

/// An IEEE range fact rides the same roster with its exact format: each
/// machine reaching `Gauge` requires the range at entry and restores it at
/// return. `count` never writes the level, so its guarantee is the entry row
/// itself, and `main`'s second call is discharged by the first call's
/// imported guarantee.
const GAUGE: &str = "
    data Gauge where level >= 0.0 && level <= 1.0 { level: f32; }
    data Main { gauge: Gauge; count: u8 in Wrapping; }
    machine Main::count(context: &mut Main) {
        context.count = context.count + 1;
    }
    machine Main::main(context: &mut Main) {
        Main::count(context);
        Main::count(context);
    }
";

#[test]
fn an_ieee_range_row_keeps_its_format_through_calls() {
    let checked = crate::front_end::checked_program(GAUGE);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .expect("an IEEE range fact lowers through its declaration predicate row");
    assert!(verify(&lowered.semantic_module, &lowered));
    for machine in &lowered.semantic_module.machines {
        assert!(
            machine.contract.requires.iter().all(|requirement| matches!(
                requirement,
                semantic_vocabulary::Proposition::ScalarIeeeFloatComparison {
                    format: semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ..
                }
            )),
            "{:?}",
            machine.contract.requires
        );
        assert_eq!(machine.contract.requires.len(), 2);
        assert_eq!(machine.contract.ensures.len(), 1);
    }
}
