use checked_interpreter::BuildMachineEntry;
use checked_interpreter::InterpretOptions;
use checked_interpreter::{InterpretOutcome, interpret_entry};

fn checked(source: &str) -> checked_trees::CheckedTrees {
    crate::front_end::checked_program_result(source)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

fn execute(source: &str) -> InterpretOutcome {
    interpret_entry(
        &checked(source),
        BuildMachineEntry::Name("main"),
        &[],
        InterpretOptions::default(),
    )
}

fn assert_seven(source: &str) {
    let outcome = execute(source);
    assert_eq!(outcome.error, None, "{source}");
    assert_eq!(outcome.exit_code, 7, "{source}");
}

// The `&mut [i32]` view produced by `.as_mut_slice()` shares the array's
// element cells: a write through the view mutates the collection in place
// and is observed through the owning record afterward.
#[test]
fn mutable_slice_view_writethrough_parameter_root() {
    assert_seven(
        "data Bag { cells: [i32; 4]; }
        machine fill(bag: &mut Bag) {
            let cells: &mut [i32] = bag.cells.as_mut_slice();
            cells[2] = 7;
        }
        machine main() -> i32 {
            let mut bag: Bag = Bag { cells: [0, 0, 0, 0] };
            fill(&mut bag);
            transition bag.cells[2] == 7 { true -> 7 false -> 0 }
        }",
    );
}

#[test]
fn mutable_slice_view_writethrough_nested_local_record() {
    assert_seven(
        "data Main { bag: Bag; }
        data Bag { cells: [i32; 4]; }
        machine Main::sow(context: &mut Main) {
            let cells: &mut [i32] = context.bag.cells.as_mut_slice();
            cells[2] = 7;
        }
        machine main() -> i32 {
            let mut context: Main = Main { bag: Bag { cells: [0, 0, 0, 0] } };
            Main::sow(&mut context);
            transition context.bag.cells[2] == 7 { true -> 7 false -> 0 }
        }",
    );
}
