use checked_interpreter::InterpretOptions;
use checked_interpreter::interpret_entry;

fn checked_program(source: &str) -> checked_trees::CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("symbols");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("types");
    typed_trees_to_checked_trees::lower_typed_trees(typed).expect("checked program")
}

#[test]
fn borrowed_storage_replacement_round_trips_the_exact_place() {
    // The guide canary pattern: move the value out of borrowed storage, then
    // restore the exact place before the exclusive loan ends. Custody passes
    // out of `self.inventory` into `replacement` and back exactly once.
    let checked = checked_program(
        "data Inventory { slots: i32; }
         data Main { inventory: Inventory; }
         machine Main::replace(&mut self) {
             let replacement: Inventory = self.inventory;
             self.inventory = move replacement;
         }
         machine main() -> i32 {
             let mut owner: Main = Main { inventory: Inventory { slots: 41 } };
             owner.replace();
             transition owner.inventory.slots == 41 { true -> 7 false -> 0 }
         }",
    );
    let outcome = interpret_entry(&checked, "main", &[], InterpretOptions::default());
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 7);
}

#[test]
fn borrowed_storage_replacement_publishes_the_caller_visible_update() {
    // Extraction, a consuming transform, and repair: `taken` is consumed
    // exactly once by the `bump` receiver, and the replacement value the
    // caller observes is the transformed one.
    let checked = checked_program(
        "data Inventory { slots: i32; }
         machine Inventory::bump(self) -> Inventory {
             Inventory { slots: self.slots + 1 }
         }
         data Main { inventory: Inventory; }
         machine Main::replace(&mut self) {
             let taken: Inventory = self.inventory;
             self.inventory = move taken.bump();
         }
         machine main() -> i32 {
             let mut owner: Main = Main { inventory: Inventory { slots: 41 } };
             owner.replace();
             transition owner.inventory.slots == 42 { true -> 7 false -> 0 }
         }",
    );
    let outcome = interpret_entry(&checked, "main", &[], InterpretOptions::default());
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 7);
}

#[test]
fn quiet_checked_body_with_operational_ceiling_can_transform_detached_contents() {
    // A declared may-ceiling is not evidence that this local checked body
    // parks. The call's inferred envelope is quiet, so it may transform the
    // detached value while disjoint caller storage remains established.
    let checked = checked_program(
        "data Inventory { slots: i32; }
         machine Inventory::bump(self) -> Inventory suspends; blocks; {
             Inventory { slots: self.slots + 1 }
         }
         data Main { inventory: Inventory; count: i32; }
         machine Main::replace(&mut self) {
             let taken: Inventory = self.inventory;
             self.count = 7;
             self.inventory = move taken.bump();
         }
         machine main() -> i32 {
             let mut owner: Main = Main {
                 inventory: Inventory { slots: 41 }, count: 6
             };
             owner.replace();
             transition owner.inventory.slots == 42 && owner.count == 7 {
                 true -> 7
                 false -> 0
             }
         }",
    );
    let outcome = interpret_entry(&checked, "main", &[], InterpretOptions::default());
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 7);
}
