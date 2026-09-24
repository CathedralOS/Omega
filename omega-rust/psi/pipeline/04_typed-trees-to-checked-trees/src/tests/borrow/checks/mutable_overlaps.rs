use super::check_program;
use crate::borrow::build_borrow_facts;
use crate::checks::check_unretained_borrow_fixture_facts as check_checked_facts;
use crate::flow::build_domain_facts;
use crate::flow::build_flow_facts;
use crate::proof::build_proof_facts;
use crate::semantic::facts::build_semantic_facts;
use crate::tests::front_end::typed_program;

#[test]
fn rejects_read_then_mutable_overlap_independent_of_argument_order() {
    let source = r#"
        data Main { value: i32; }

        machine Main::main(&mut self) {
            self.mix(self.value, &mut self.value);
        }

        machine Main::mix(&mut self, before: i32, current: &mut i32) {
            current = before;
        }
    "#;

    let diagnostics = check_program(source)
        .expect_err("a read followed by a mutable access to the same place must conflict");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("as both mutable and read-only"),
        "expected the order-independent access conflict, got:\n{combined}"
    );
}

#[test]
fn dependent_sibling_reads_remain_noninterfering() {
    let source = r#"
        data Ledger
        where
            count <= len,
        {
            len: u32;
            count: u32;
        }

        data Main { ledger: Ledger; }

        machine Main::main(&self) {
            self.observe(self.ledger.count, self.ledger.len);
        }

        machine Main::observe(&self, count: u32, len: u32) {
        }
    "#;

    check_program(source).expect("two reads cannot invalidate their shared dependent fact");
}

#[test]
fn dependent_sibling_mutation_refuses_noninterference_without_panicking() {
    let source = r#"
        data Ledger
        where
            count <= len,
        {
            len: u32;
            count: u32;
        }

        data Main { ledger: Ledger; }

        machine Main::main(&mut self) {
            self.observe(&mut self.ledger.count, self.ledger.len);
        }

        machine Main::observe(&mut self, count: &mut u32, len: u32) {
        }
    "#;

    let diagnostics = check_program(source)
        .expect_err("a mutable dependent sibling access must not establish non-interference");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("as both mutable and read-only"),
        "expected a normal dependent-sibling diagnostic, got:\n{combined}"
    );
}

#[test]
fn rejects_direct_mutable_borrow_while_local_alias_is_active() {
    let source = r#"
        data Main {
            value: i32;
        }

        machine Main::main(&mut self) {
            let alias: &mut i32 = &mut self.value;
            self.use_value(&mut self.value);
            self.write_alias(alias);
        }

        machine Main::use_value(&mut self, value: &mut i32) {
            value = 1;
        }

        machine Main::write_alias(&mut self, value: &mut i32) {
            value = 2;
        }
    "#;

    let typed = typed_program(source);
    let proof_plan = proof::obligations::build_proof_plan(&typed);
    let operations = validation::infer_operational_may(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let mut semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(
        &typed,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operations,
    );
    let facts = checked_trees::CheckFacts {
        semantic,
        proof,
        borrow,
        domains,
        flow,
        ..Default::default()
    };

    let diagnostics = check_checked_facts(&typed, &facts)
        .expect_err("active local alias should block direct mutable borrow");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(combined.contains("local borrow `alias` is still active"));
    assert!(combined.contains("borrowed at statement 0"));
    assert!(combined.contains("its last use is at statement 2"));
    assert!(combined.contains("released at state exit"));
}

#[test]
fn rejects_direct_mutable_borrow_while_helper_alias_is_active() {
    let source = r#"
        data Exit {
            destination: i32;
        }

        data Room {
            exits: [Exit; 1];
        }

        data Main {
            room: Room;
        }

        machine Main::main(&mut self) {
            let alias: &mut [Exit] = self.room.exits.as_mut_slice();
            self.use_exit(&mut self.room.exits[0]);
            self.write_alias(alias);
        }

        machine Main::use_exit(&mut self, exit: &mut Exit) {
            exit = Exit { destination: 1 };
        }

        machine Main::write_alias(&mut self, exits: &mut [Exit]) {
            exits[0] = Exit { destination: 2 };
        }
    "#;

    let typed = typed_program(source);
    let proof_plan = proof::obligations::build_proof_plan(&typed);
    let operations = validation::infer_operational_may(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let mut semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(
        &typed,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operations,
    );
    let facts = checked_trees::CheckFacts {
        semantic,
        proof,
        borrow,
        domains,
        flow,
        ..Default::default()
    };

    let diagnostics = check_checked_facts(&typed, &facts)
        .expect_err("helper-returned local alias should block direct mutable borrow");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(combined.contains("local borrow `alias` is still active"));
    assert!(combined.contains("borrowed at statement 0"));
    assert!(combined.contains("its last use is at statement 2"));
    assert!(combined.contains("released at state exit"));
}

#[test]
fn accepts_adjacent_mutable_windows_with_one_symbolic_half_open_boundary() {
    let source = r#"
        data Main { items: [i32; 4]; }

        machine Main::split(&mut self) -> u64 {
            let mid: u64 = 2;
            let cut: u64 = mid;
            let left: &mut [i32] = self.items[0..cut];
            let right: &mut [i32] = self.items[mid..4];
            left.len + right.len
        }
    "#;

    check_program(source)
        .expect("the exact shared symbolic boundary proves half-open window adjacency");
}

#[test]
fn changing_symbolic_window_start_to_zero_restores_overlap_rejection() {
    let source = r#"
        data Main { items: [i32; 4]; }

        machine Main::split(&mut self) -> u64 {
            let mid: u64 = 2;
            let cut: u64 = mid;
            let left: &mut [i32] = self.items[0..cut];
            let right: &mut [i32] = self.items[0..4];
            left.len + right.len
        }
    "#;

    let diagnostics = check_program(source)
        .expect_err("changing the second start from `mid` to `0` makes the windows overlap");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("creates local borrow `right` while local borrow `left` is still active"),
        "expected the exact symbolic-bound mutation conflict, got:\n{combined}"
    );
}

#[test]
fn rejects_local_borrow_creation_while_prior_alias_is_active() {
    let source = r#"
        data Main {
            value: i32;
        }

        machine Main::main(&mut self) {
            let first: &mut i32 = &mut self.value;
            let second: &mut i32 = &mut self.value;
            self.write_alias(first);
            self.write_alias(second);
        }

        machine Main::write_alias(&mut self, value: &mut i32) {
            value = 2;
        }
    "#;

    let typed = typed_program(source);
    let proof_plan = proof::obligations::build_proof_plan(&typed);
    let operations = validation::infer_operational_may(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let mut semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(
        &typed,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operations,
    );
    let facts = checked_trees::CheckFacts {
        semantic,
        proof,
        borrow,
        domains,
        flow,
        ..Default::default()
    };

    let diagnostics = check_checked_facts(&typed, &facts)
        .expect_err("second local borrow alias should be rejected while the first is live");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined
            .contains("creates local borrow `second` while local borrow `first` is still active")
    );
    assert!(combined.contains("borrowed at statement 0"));
    assert!(combined.contains("its last use is at statement 2"));
}

#[test]
fn accepts_stable_mutable_reborrow_chain_from_local_alias() {
    let source = r#"
        data Cell {
            value: i32;
        }

        data Main {
            cell: Cell;
        }

        machine write_cell(cell: &mut Cell) {
            cell.value = 2;
        }

        machine Main::main(&mut self) {
            let first: &mut Cell = &mut self.cell;
            let second: &mut Cell = &mut first;
            write_cell(second);
        }
    "#;

    check_program(source).expect(
        "a mutable reborrow may derive from its active source alias and retain the ultimate place",
    );
}

#[test]
fn accepts_stable_mutable_aliases_from_fixed_and_dynamic_indexed_places() {
    let source = r#"
        data Cell {
            value: i32;
        }

        data Main {
            cells: [Cell; 2];
        }

        machine write_cell(cell: &mut Cell) {
            cell.value = 2;
        }

        machine Main::fixed(&mut self) {
            let cell: &mut Cell = &mut self.cells[0];
            write_cell(cell);
        }

        machine Main::dynamic(&mut self, index: u64)
        requires
            index < 2
        {
            let cell: &mut Cell = &mut self.cells[index];
            write_cell(cell);
        }
    "#;

    check_program(source).expect(
        "fixed and range-checked dynamic indexed places may back stable local mutable aliases",
    );
}

#[test]
fn accepts_direct_mutable_borrow_after_local_alias_last_use() {
    let source = r#"
        data Main {
            value: i32;
        }

        machine Main::main(&mut self) {
            let alias: &mut i32 = &mut self.value;
            self.write_alias(alias);
            self.use_value(&mut self.value);
        }

        machine Main::write_alias(&mut self, value: &mut i32) {
            value = 1;
        }

        machine Main::use_value(&mut self, value: &mut i32) {
            value = 2;
        }
    "#;

    let typed = typed_program(source);
    let proof_plan = proof::obligations::build_proof_plan(&typed);
    let operations = validation::infer_operational_may(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let mut semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(
        &typed,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operations,
    );
    let facts = checked_trees::CheckFacts {
        semantic,
        proof,
        borrow,
        domains,
        flow,
        ..Default::default()
    };

    check_checked_facts(&typed, &facts).expect("loan should end after alias last use");
}

#[test]
fn rejects_direct_assignment_while_local_alias_is_active() {
    let source = r#"
        data Main {
            value: i32;
        }

        machine Main::main(&mut self) {
            let alias: &mut i32 = &mut self.value;
            self.value = 3;
            self.write_alias(alias);
        }

        machine Main::write_alias(&mut self, value: &mut i32) {
            value = 2;
        }
    "#;

    let typed = typed_program(source);
    let proof_plan = proof::obligations::build_proof_plan(&typed);
    let operations = validation::infer_operational_may(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let mut semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(
        &typed,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operations,
    );
    let facts = checked_trees::CheckFacts {
        semantic,
        proof,
        borrow,
        domains,
        flow,
        ..Default::default()
    };

    let diagnostics = check_checked_facts(&typed, &facts)
        .expect_err("direct assignment should not overlap a live local alias");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains(
            "statement 1 mutates `self.value` while local borrow `alias` is still active"
        )
    );
    assert!(combined.contains("borrowed at statement 0"));
}

#[test]
fn rejects_mutating_call_through_owner_while_view_is_active() {
    // A `&mut self` call that writes the owner field is a *call* statement, not
    // an assignment. The Vec-views / owner-mutation-through-a-call rule must
    // reject it while a borrowed view of that field is still live. This is the
    // call-statement analogue of the array/slice/string owner-write rule and the
    // mechanism behind the Vec `push`-while-borrowed rejection.
    let source = r#"
        data Entry {
            value: i32;
        }

        data Main {
            entries: [Entry; 2];
        }

        machine Main::main(&mut self) {
            let view: &[Entry] = self.entries.as_slice();
            self.clear_entries();
            self.read_alias(view);
        }

        machine Main::clear_entries(&mut self) {
            self.entries[0] = Entry { value: 0 };
        }

        machine Main::read_alias(&self, entries: &[Entry]) {
            let count: u64 = entries.len;
        }
    "#;

    let typed = typed_program(source);
    let proof_plan = proof::obligations::build_proof_plan(&typed);
    let operations = validation::infer_operational_may(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let mut semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(
        &typed,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operations,
    );
    let facts = checked_trees::CheckFacts {
        semantic,
        proof,
        borrow,
        domains,
        flow,
        ..Default::default()
    };

    let diagnostics = check_checked_facts(&typed, &facts)
        .expect_err("a mutating call through the owner must conflict with a live view");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("while local borrow `view` is still active"),
        "expected owner-mutation-through-call conflict, got:\n{combined}"
    );
}

#[test]
fn rejects_vec_push_while_slice_view_is_active() {
    let source = r#"
        data Vec<T> {
        }

        machine Vec::as_slice<T>(&self) -> &[T] {
        }

        machine Vec::push<T>(&mut self, value: T) {
        }

        data Main {
            items: Vec<u8>;
        }

        machine Main::main(&mut self) {
            let view: &[u8] = self.items.as_slice();
            self.items.push(7);
            self.read_alias(view);
        }

        machine Main::read_alias(&self, items: &[u8]) {
        }
    "#;

    let typed = typed_program(source);
    let proof_plan = proof::obligations::build_proof_plan(&typed);
    let operations = validation::infer_operational_may(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let mut semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(
        &typed,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operations,
    );
    let facts = checked_trees::CheckFacts {
        semantic,
        proof,
        borrow,
        domains,
        flow,
        ..Default::default()
    };

    let diagnostics = check_checked_facts(&typed, &facts)
        .expect_err("Vec::push through the owner must conflict with a live view");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("while local borrow `view` is still active"),
        "expected Vec push conflict, got:\n{combined}"
    );
}

#[test]
fn accepts_mutating_call_through_owner_on_disjoint_field() {
    // A mutating call through the owner that writes a DISJOINT field is accepted
    // while a borrowed view of a different field is live. The call-mutation rule
    // reuses the loan-overlap engine, so a call whose summarized writes do not
    // overlap the live view's place does not conflict.
    let source = r#"
        data Entry {
            value: i32;
        }

        data Main {
            left: [Entry; 2];
            right: [Entry; 2];
        }

        machine Main::main(&mut self) {
            let view: &[Entry] = self.left.as_slice();
            self.touch_right();
            self.read_alias(view);
        }

        machine Main::touch_right(&mut self) {
            self.right[0] = Entry { value: 1 };
        }

        machine Main::read_alias(&self, entries: &[Entry]) {
            let count: u64 = entries.len;
        }
    "#;

    let typed = typed_program(source);
    let proof_plan = proof::obligations::build_proof_plan(&typed);
    let operations = validation::infer_operational_may(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let mut semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(
        &typed,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operations,
    );
    let facts = checked_trees::CheckFacts {
        semantic,
        proof,
        borrow,
        domains,
        flow,
        ..Default::default()
    };

    check_checked_facts(&typed, &facts)
        .expect("a mutating call on a disjoint field should not conflict with the view");
}

#[test]
fn accepts_known_pure_mutable_receiver_call_while_view_is_active() {
    // `&mut self` in the signature is not by itself a write. Once the target is
    // known, an empty mutation summary means this helper is read-only for borrow
    // invalidation purposes; only unknown calls need the conservative receiver
    // fallback.
    let source = r#"
        data Entry {
            value: i32;
        }

        data Main {
            entries: [Entry; 2];
        }

        machine Main::main(&mut self) {
            let view: &[Entry] = self.entries.as_slice();
            let value: i32 = self.identity(1);
            self.read_alias(view, value);
        }

        machine Main::identity(&mut self, value: i32) -> i32 {
            transition {
                _ -> value
            }
        }

        machine Main::read_alias(&self, entries: &[Entry], value: i32) {
            let count: u64 = entries.len;
        }
    "#;

    let typed = typed_program(source);
    let proof_plan = proof::obligations::build_proof_plan(&typed);
    let operations = validation::infer_operational_may(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let mut semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(
        &typed,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operations,
    );
    let facts = checked_trees::CheckFacts {
        semantic,
        proof,
        borrow,
        domains,
        flow,
        ..Default::default()
    };

    check_checked_facts(&typed, &facts)
        .expect("known pure mutable receiver helper should not invalidate a live view");
}

#[test]
fn accepts_mutable_slice_alias_index_from_fixed_array_field() {
    let source = r#"
        data Exit {
            destination: i32;
        }

        data Room {
            exits: [Exit; 4];
        }

        data Main {
            room: Room;
        }

        machine Main::main(&mut self) {
            let exits: &mut [Exit] = self.room.exits.as_mut_slice();
            exits[0] = Exit { destination: 1 };
        }
    "#;

    let typed = typed_program(source);
    let proof_plan = proof::obligations::build_proof_plan(&typed);
    let operations = validation::infer_operational_may(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let mut semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(
        &typed,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operations,
    );
    let facts = checked_trees::CheckFacts {
        semantic,
        proof,
        borrow,
        domains,
        flow,
        ..Default::default()
    };

    check_checked_facts(&typed, &facts)
        .expect("mutable slice alias from fixed array field should keep its fixed length");
}

#[test]
fn accepts_recursive_slice_parameter_index_proof_from_guard() {
    let source = r#"
        data Entry {
            value: i32;
        }

        data Main {
            entries: [Entry; 2];
        }

        machine Main::main(&mut self) {
            let entries: &[Entry] = self.entries.as_slice();
            transition {
                _ -> self.visit(entries, 0)
            }

            state visit(&mut self, entries: &[Entry], index: u64) {
                let value: i32 = entries[index].value;
                let next_index: u64 = index + 1;
                let has_next: bool = next_index < entries.len;

                transition {
                    has_next -> self.visit(entries, next_index)
                    _ -> {}
                }
            }
        }
    "#;

    let typed = typed_program(source);
    let proof_plan = proof::obligations::build_proof_plan(&typed);
    let operations = validation::infer_operational_may(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let mut semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(
        &typed,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operations,
    );
    let facts = checked_trees::CheckFacts {
        semantic,
        proof,
        borrow,
        domains,
        flow,
        ..Default::default()
    };

    check_checked_facts(&typed, &facts)
        .expect("recursive slice parameter should keep index proof from length guard");
}

#[test]
fn accepts_direct_mutable_borrow_after_local_alias_reassignment() {
    let source = r#"
        data Main {
            value: i32;
            other: i32;
        }

        machine Main::main(&mut self) {
            let alias: &mut i32 = &mut self.value;
            alias = &mut self.other;
            self.use_value(&mut self.value);
        }

        machine Main::use_value(&mut self, value: &mut i32) {
            value = 1;
        }
    "#;

    let typed = typed_program(source);
    let proof_plan = proof::obligations::build_proof_plan(&typed);
    let operations = validation::infer_operational_may(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let mut semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(
        &typed,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operations,
    );
    let facts = checked_trees::CheckFacts {
        semantic,
        proof,
        borrow,
        domains,
        flow,
        ..Default::default()
    };

    check_checked_facts(&typed, &facts)
        .expect("reassigned local alias should no longer block later direct mutable borrow");
}

/// Lifetimes stage 1 (elision rule 1): a free machine returning a view with
/// exactly one ref input links the returned view's loan to THAT input, so
/// mutating the linked source while the view is live is rejected. Before
/// stage 1 no loan was tracked for a free-machine call result at all.
#[test]
fn rejects_linked_input_mutation_while_free_machine_view_is_active() {
    let source = r#"
        data Cell {
            value: i32;
        }

        data Bag {
            cells: [Cell; 4];
        }

        data Main {
            bag: Bag;
        }

        machine pick(bag: &mut Bag) -> &mut Cell {
            let cells: &mut [Cell] = bag.cells.as_mut_slice();
            transition {
                _ -> &mut cells[2]
            }
        }

        machine Main::main(&mut self) {
            let cell: &mut Cell = pick(&mut self.bag);
            self.bag.cells[0] = Cell { value: 1 };
            cell.value = 7;
        }
    "#;

    let typed = typed_program(source);
    let proof_plan = proof::obligations::build_proof_plan(&typed);
    let operations = validation::infer_operational_may(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let mut semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(
        &typed,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operations,
    );
    let facts = checked_trees::CheckFacts {
        semantic,
        proof,
        borrow,
        domains,
        flow,
        ..Default::default()
    };

    let diagnostics = check_checked_facts(&typed, &facts)
        .expect_err("mutating the elision-linked input while the view is live should reject");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains(
            "statement 1 mutates `self.bag.cells[0]` while local borrow `cell` is still active"
        ),
        "expected the linked-input mutation rejection, got:\n{combined}"
    );
}

/// Lifetimes stage 1 (elision rule 1, the win): the returned view borrows ONLY
/// the single ref input it was linked to -- mutating a DIFFERENT ref input of
/// the caller while the view is live compiles.
#[test]
fn accepts_unlinked_ref_input_mutation_while_free_machine_view_is_active() {
    let source = r#"
        data Cell {
            value: i32;
        }

        data Bag {
            cells: [Cell; 4];
        }

        data Main {
            first: Bag;
            second: Bag;
        }

        machine pick(bag: &mut Bag) -> &mut Cell {
            let cells: &mut [Cell] = bag.cells.as_mut_slice();
            transition {
                _ -> &mut cells[2]
            }
        }

        machine fill(a: &mut Bag, b: &mut Bag) {
            let cell: &mut Cell = pick(a);
            b.cells[0] = Cell { value: 1 };
            cell.value = 7;
        }

        machine Main::main(&mut self) {
            fill(&mut self.first, &mut self.second);
        }
    "#;

    let typed = typed_program(source);
    let proof_plan = proof::obligations::build_proof_plan(&typed);
    let operations = validation::infer_operational_may(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let mut semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(
        &typed,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operations,
    );
    let facts = checked_trees::CheckFacts {
        semantic,
        proof,
        borrow,
        domains,
        flow,
        ..Default::default()
    };

    check_checked_facts(&typed, &facts)
        .expect("mutating the unlinked ref input while the view is live should compile");
}

/// A view-returning machine with MULTIPLE non-self ref inputs and an ELIDED
/// output lifetime is ambiguous and rejected at the declaration: the checker
/// cannot infer which input the view borrows, and now points at explicit
/// lifetimes (decision 15 stage 2) as the fix. A `&self` method with extra ref
/// params stays accepted (elision rule 3 links the output to self).
#[test]
fn rejects_ambiguous_view_return_with_multiple_ref_inputs() {
    let source = r#"
        data Cell {
            value: i32;
        }

        data Bag {
            cells: [Cell; 4];
        }

        data Main {
            first: Bag;
            second: Bag;
        }

        machine pick_either(a: &mut Bag, b: &mut Bag) -> &mut Cell {
            let cells: &mut [Cell] = a.cells.as_mut_slice();
            transition {
                _ -> &mut cells[2]
            }
        }

        machine Main::main(&mut self) {
            let cell: &mut Cell = pick_either(&mut self.first, &mut self.second);
            cell.value = 7;
        }
    "#;

    let typed = typed_program(source);
    let proof_plan = proof::obligations::build_proof_plan(&typed);
    let operations = validation::infer_operational_may(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let mut semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(
        &typed,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operations,
    );
    let facts = checked_trees::CheckFacts {
        semantic,
        proof,
        borrow,
        domains,
        flow,
        ..Default::default()
    };

    let diagnostics = check_checked_facts(&typed, &facts)
        .expect_err("two non-self ref inputs with a view output should be ambiguous");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("cannot infer which input the returned view borrows"),
        "expected the elision ambiguity rejection, got:\n{combined}"
    );
}

/// Bodyless provider requirements create caller-side loans too, so declaration
/// validation must reject their ambiguous view sources before any call is
/// attributed.
#[test]
fn rejects_ambiguous_view_return_from_boundary_trait_signature() {
    let source = r#"
        boundary trait Storage {
            machine view(first: &u8, second: &u8) -> &u8;
        }

        data Main {}

        machine Main::main(&mut self) {}
    "#;

    let diagnostics = check_program(source)
        .expect_err("a bodyless view requirement with two ref inputs should be ambiguous");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("cannot infer which input the returned view borrows"),
        "expected the bodyless-signature elision rejection, got:\n{combined}"
    );
}

/// Lifetimes stage 2 (frozen decision 15): an EXPLICIT output lifetime resolves
/// the otherwise-ambiguous two-ref-input case by naming the input the view
/// borrows. `pick_either<'bag>(a: &'bag mut Bag, b: &mut Bag) -> &'bag mut Cell`
/// says the view comes from `a`, so the declaration is accepted and the loan
/// follows `a`'s argument (here `self.first`).
#[test]
fn accepts_view_return_disambiguated_by_explicit_lifetime() {
    let source = r#"
        data Cell {
            value: i32;
        }

        data Bag {
            cells: [Cell; 4];
        }

        data Main {
            first: Bag;
            second: Bag;
        }

        machine pick_either<'bag>(a: &'bag mut Bag, b: &mut Bag) -> &'bag mut Cell {
            let cells: &mut [Cell] = a.cells.as_mut_slice();
            transition {
                _ -> &mut cells[2]
            }
        }

        machine Main::main(&mut self) {
            let cell: &mut Cell = pick_either(&mut self.first, &mut self.second);
            cell.value = 7;
        }
    "#;

    let typed = typed_program(source);
    let proof_plan = proof::obligations::build_proof_plan(&typed);
    let operations = validation::infer_operational_may(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let mut semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(
        &typed,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operations,
    );
    let facts = checked_trees::CheckFacts {
        semantic,
        proof,
        borrow,
        domains,
        flow,
        ..Default::default()
    };

    check_checked_facts(&typed, &facts)
        .expect("an explicit output lifetime naming one input should compile");
}
