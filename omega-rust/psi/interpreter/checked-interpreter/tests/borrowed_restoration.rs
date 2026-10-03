use checked_interpreter::BuildMachineEntry;
use checked_interpreter::InterpretOptions;
use checked_interpreter::interpret_entry;

#[test]
fn borrowed_storage_replacement_round_trips_the_exact_place() {
    // The guide canary pattern: move the value out of borrowed storage, then
    // restore the exact place before the exclusive loan ends. Custody passes
    // out of `owner.inventory` into `replacement` and back exactly once.
    let checked = crate::front_end::checked_program(
        "data Inventory { slots: i32; }
         data Main { inventory: Inventory; }
         machine Main::replace(owner: &mut Main) {
             let replacement: Inventory = owner.inventory;
             owner.inventory = move replacement;
         }
         machine main() -> i32 {
             let mut owner: Main = Main { inventory: Inventory { slots: 41 } };
             Main::replace(&mut owner);
             transition owner.inventory.slots == 41 { true -> 7 false -> 0 }
         }",
    );
    let outcome = interpret_entry(
        &checked,
        BuildMachineEntry::Name("main"),
        &[],
        InterpretOptions::default(),
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 7);
}

#[test]
fn borrowed_storage_replacement_publishes_the_caller_visible_update() {
    // Extraction, a consuming transform, and repair: `taken` is consumed
    // exactly once by `Inventory::bump`, and the replacement value the
    // caller observes is the transformed one.
    let checked = crate::front_end::checked_program(
        "data Inventory { slots: i32; }
         machine Inventory::bump(inventory: Inventory) -> Inventory {
             Inventory { slots: inventory.slots + 1 }
         }
         data Main { inventory: Inventory; }
         machine Main::replace(owner: &mut Main) {
             let taken: Inventory = owner.inventory;
             owner.inventory = move Inventory::bump(taken);
         }
         machine main() -> i32 {
             let mut owner: Main = Main { inventory: Inventory { slots: 41 } };
             Main::replace(&mut owner);
             transition owner.inventory.slots == 42 { true -> 7 false -> 0 }
         }",
    );
    let outcome = interpret_entry(
        &checked,
        BuildMachineEntry::Name("main"),
        &[],
        InterpretOptions::default(),
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 7);
}
