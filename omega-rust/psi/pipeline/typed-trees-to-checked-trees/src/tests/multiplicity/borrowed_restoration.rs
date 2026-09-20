//! Borrowed-storage invariant windows (ch11/ownership spec): a consuming move
//! out of `&mut`-reachable storage opens a repair obligation on the exact
//! absent place; an ordinary store of a same-typed owned value discharges it;
//! every exit edge, stale observation, ancestor use, and repeated extraction
//! while the place is absent rejects.

use super::{Lexer, ResolutionRequest, lower_symbol_resolved_trees, parse_syntax_trees, resolve};
use crate::lower_typed_trees;

fn check_source(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = resolve(ResolutionRequest::new(&syntax)).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    lower_typed_trees(typed)
}

#[test]
fn evaluation_order_observes_a_field_before_its_owner_is_extracted() {
    let source = "data Inventory { slots: i32; }
        data Saved { observed: i32; inventory: Inventory; }
        data Main { inventory: Inventory; }
        machine Main::replace(&mut self) {
            let saved: Saved = Saved {
                observed: self.inventory.slots,
                inventory: self.inventory,
            };
            self.inventory = move saved.inventory;
        }";
    check_source(source).expect("the scalar read precedes the extraction");
    let reversed = source.replace(
        "observed: self.inventory.slots,\n                inventory: self.inventory,",
        "inventory: self.inventory,\n                observed: self.inventory.slots,",
    );
    let diagnostics = check_source(&reversed).expect_err("reading the absent subtree must reject");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("absent")),
        "{diagnostics:#?}"
    );
}

#[test]
fn evaluation_order_fences_only_boundary_calls_after_extraction() {
    let source = "boundary machine sample() -> i32 ensures true;
        data Inventory { slots: i32; }
        data Saved { observed: i32; inventory: Inventory; }
        data Main { inventory: Inventory; }
        machine Main::replace(&mut self) {
            let saved: Saved = Saved {
                observed: sample(),
                inventory: self.inventory,
            };
            self.inventory = move saved.inventory;
        }";
    check_source(source).expect("the boundary completes before storage becomes absent");
    let reversed = source.replace(
        "observed: sample(),\n                inventory: self.inventory,",
        "inventory: self.inventory,\n                observed: sample(),",
    );
    assert_boundary_window_rejection(&reversed);
}

#[test]
fn evaluation_order_detaches_call_operands_before_later_arguments() {
    let source = "data Inventory { slots: i32; }
        machine preserve(observed: i32, inventory: Inventory) -> Inventory { inventory }
        data Main { inventory: Inventory; }
        machine Main::replace(&mut self) {
            self.inventory = preserve(self.inventory.slots, self.inventory);
        }";
    check_source(source).expect("the first argument finishes before the second moves");
    let reversed = source
        .replace(
            "observed: i32, inventory: Inventory",
            "inventory: Inventory, observed: i32",
        )
        .replace(
            "preserve(self.inventory.slots, self.inventory)",
            "preserve(self.inventory, self.inventory.slots)",
        );
    let diagnostics = check_source(&reversed).expect_err("the later argument reads absent content");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("absent")),
        "{diagnostics:#?}"
    );
}

#[test]
fn evaluation_order_borrowed_call_observes_only_established_storage() {
    let source = "data Inventory { slots: i32; }
        machine observe(inventory: &Inventory) -> i32 { inventory.slots }
        data Saved { observed: i32; inventory: Inventory; }
        data Main { inventory: Inventory; }
        machine Main::replace(&mut self) {
            let saved: Saved = Saved {
                observed: observe(&self.inventory),
                inventory: self.inventory,
            };
            self.inventory = move saved.inventory;
        }";
    check_source(source).expect("a completed observation precedes the move");
    let reversed = source.replace(
        "observed: observe(&self.inventory),\n                inventory: self.inventory,",
        "inventory: self.inventory,\n                observed: observe(&self.inventory),",
    );
    assert!(
        check_source(&reversed).is_err(),
        "a later borrow cannot observe detached storage"
    );
}

#[test]
fn evaluation_order_retains_effects_inside_a_computed_projection() {
    let source = "pub data Inventory { slots: i32; }
        pub data Saved { inventory: Inventory; }
        machine wrap(inventory: Inventory) -> Saved { Saved { inventory: inventory } }
        data Main { inventory: Inventory; }
        machine Main::replace(&mut self) {
            self.inventory = wrap(self.inventory).inventory;
        }";
    check_source(source).expect("a quiet wrapper returns the detached content");
    let exposed = source.replace(
        "machine wrap(inventory: Inventory) -> Saved { Saved { inventory: inventory } }",
        "boundary machine wrap(inventory: Inventory) -> Saved ensures true;",
    );
    assert_boundary_window_rejection(&exposed);
}

#[test]
fn evaluation_order_selected_index_uses_its_collection_operand() {
    check_source(
        "data Buffer { value: i32; }
         machine [] Buffer::index(self, index: u64) -> i32 { self.value }
         data Main { buffer: Buffer; }
         machine Main::replace(&mut self) {
             let reading: i32 = self.buffer[0];
             self.buffer = Buffer { value: reading };
         }",
    )
    .expect("the selected collection operand detaches before invocation and is restored");

    let diagnostics = check_source(
        "data Inventory { slots: i32; }
         data Buffer { inventory: Inventory; value: i32; }
         machine [] Buffer::index(&self, index: u64) -> i32 { self.value }
         data Main { buffer: Buffer; }
         machine Main::replace(&mut self) {
             let taken: Inventory = self.buffer.inventory;
             let reading: i32 = self.buffer[0];
             self.buffer.inventory = move taken;
         }",
    )
    .expect_err("selected indexing cannot observe an incomplete whole collection");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("absent")),
        "{diagnostics:#?}"
    );
}

#[test]
fn evaluation_order_replay_rejects_a_substituted_invocation_occurrence() {
    let checked = check_source(
        "machine identity(value: i32) -> i32 { value }
         data Inventory { slots: i32; }
         data Main { inventory: Inventory; }
         machine Main::replace(&mut self) {
             let taken: Inventory = self.inventory;
             let value: i32 = identity(7);
             self.inventory = move taken;
         }",
    )
    .expect("quiet calls can run between extraction and repair");
    let mut facts = checked.facts.clone();
    facts.flow.control.calls.for_each_mut(|_, call| {
        call.authored_expression = Default::default();
        call.service_reach = Default::default();
    });
    crate::checks::check_checked_facts(&checked.typed, &facts)
        .expect_err("moving the call row must not hide its unknown access during restoration");
}

#[test]
fn move_out_and_exact_restore_compiles() {
    check_source(
        r#"
        data Inventory { slots: i32; }
        data Main { inventory: Inventory; }
        machine Main::main(&mut self) {
            let replacement: Inventory = self.inventory;
            self.inventory = move replacement;
        }
        "#,
    )
    .expect("extract-then-restore on the exact place must compile");
}

#[test]
fn disjoint_sibling_work_between_move_and_repair() {
    check_source(
        r#"
        data Inventory { slots: i32; }
        data Main { inventory: Inventory; count: i32; }
        machine Main::main(&mut self) {
            let replacement: Inventory = self.inventory;
            let n: i32 = self.count;
            self.count = n;
            self.inventory = move replacement;
        }
        "#,
    )
    .expect("disjoint sibling work is allowed while the window is open");
}

#[test]
fn fresh_replacement_value_discharges_the_window() {
    check_source(
        r#"
        data Inventory { slots: i32; }
        data Main { inventory: Inventory; }
        machine Main::main(&mut self) {
            let replacement: Inventory = self.inventory;
            self.inventory = Inventory { slots: 9 };
        }
        "#,
    )
    .expect("the replacement need not be the removed value");
}

#[test]
fn nested_field_window_repairs_on_the_exact_place() {
    check_source(
        r#"
        data Inner { tag: i32; }
        data Inventory { inner: Inner; }
        data Main { inventory: Inventory; }
        machine Main::main(&mut self) {
            let inner: Inner = self.inventory.inner;
            self.inventory.inner = move inner;
        }
        "#,
    )
    .expect("a nested absent subtree repairs at its exact place");
}

#[test]
fn mutable_local_route_rebases_to_the_same_storage() {
    check_source(
        r#"
        data Inner { tag: i32; }
        data Inventory { inner: Inner; }
        data Main { inventory: Inventory; }
        machine Main::main(&mut self) {
            let r: &mut Inventory = &mut self.inventory;
            let inner: Inner = r.inner;
            self.inventory.inner = move inner;
        }
        "#,
    )
    .expect("a hole opened through a `&mut` local repairs through the owner path");
}

#[test]
fn call_result_value_can_repair_the_window() {
    check_source(
        r#"
        data Inventory { slots: i32; }
        machine bump(inventory: Inventory) -> Inventory { inventory }
        data Main { inventory: Inventory; }
        machine Main::main(&mut self) {
            let replacement: Inventory = self.inventory;
            let bumped: Inventory = bump(replacement);
            self.inventory = move bumped;
        }
        "#,
    )
    .expect("the detached value may travel through a call before repair");
}

#[test]
fn repair_before_transition_serves_every_arm() {
    check_source(
        r#"
        data Inventory { slots: i32; }
        data Main { inventory: Inventory; flag: bool; }
        machine Main::main(&mut self) {
            let replacement: Inventory = self.inventory;
            self.inventory = move replacement;
            transition self.flag {
                true -> done()
                _ -> done()
            }

            state done(&mut self) {
            }
        }
        "#,
    )
    .expect("a discharged window is closed on every exit edge");
}

#[test]
fn missing_repair_rejects_at_exit() {
    let diagnostics = match check_source(
        r#"
        data Inventory { slots: i32; }
        data Main { inventory: Inventory; }
        machine Main::main(&mut self) {
            let replacement: Inventory = self.inventory;
        }
        "#,
    ) {
        Ok(_) => panic!("an unrestored window must not reach the return edge"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot transfer a non-copy value out of borrowed storage")),
        "{diagnostics:#?}"
    );
}

#[test]
fn early_return_leaving_the_window_open_rejects() {
    let diagnostics = match check_source(
        r#"
        data Inventory { slots: i32; }
        data Main { inventory: Inventory; }
        machine Main::take(&mut self) -> Inventory {
            let replacement: Inventory = self.inventory;
            replacement
        }
        "#,
    ) {
        Ok(_) => panic!("the return edge must see a discharged window"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("without replacing its owner")),
        "{diagnostics:#?}"
    );
}

#[test]
fn transition_edge_rejects_a_window_left_open() {
    let diagnostics = match check_source(
        r#"
        data Inventory { slots: i32; }
        data Main { inventory: Inventory; flag: bool; }
        machine Main::main(&mut self) {
            let replacement: Inventory = self.inventory;
            transition self.flag {
                true -> done()
                _ -> repair(replacement)
            }

            state repair(&mut self, replacement: Inventory) {
                self.inventory = move replacement;
                transition { _ -> done() }
            }

            state done(&mut self) {
            }
        }
        "#,
    ) {
        Ok(_) => panic!("a window cannot cross a transition edge"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("without replacing its owner")),
        "{diagnostics:#?}"
    );
}

#[test]
fn wrong_place_replacement_leaves_the_debt_open() {
    let diagnostics = match check_source(
        r#"
        data Inventory { slots: i32; }
        data Main { inventory: Inventory; other: Inventory; }
        machine Main::main(&mut self) {
            let replacement: Inventory = self.inventory;
            self.other = move replacement;
        }
        "#,
    ) {
        Ok(_) => panic!("storing a sibling place must not discharge the hole"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("without replacing its owner")),
        "{diagnostics:#?}"
    );
}

#[test]
fn wrong_type_replacement_does_not_discharge_the_window() {
    // Assignment typing pins the stored value's type to the place's declared
    // type; a differently-typed "repair" is refused before it can pretend to
    // close the window, and the window still demands the exact moved type.
    let diagnostics = match check_source(
        r#"
        data Inventory { slots: i32; }
        data Other { tag: i32; }
        data Main { inventory: Inventory; }
        machine Main::main(&mut self) {
            let replacement: Inventory = self.inventory;
            self.inventory = move Other { tag: 1 };
        }
        "#,
    ) {
        Ok(_) => panic!("a differently-typed value cannot close the hole"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("incompatible data types")),
        "{diagnostics:#?}"
    );
}

#[test]
fn repeated_extraction_while_absent_rejects() {
    let diagnostics = match check_source(
        r#"
        data Inventory { slots: i32; }
        data Main { inventory: Inventory; }
        machine Main::main(&mut self) {
            let replacement: Inventory = self.inventory;
            let stolen: Inventory = self.inventory;
            self.inventory = move replacement;
        }
        "#,
    ) {
        Ok(_) => panic!("the absent place cannot be moved again"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("is absent")),
        "{diagnostics:#?}"
    );
}

#[test]
fn stale_read_while_absent_rejects() {
    let diagnostics = match check_source(
        r#"
        data Inventory { slots: i32; }
        data Main { inventory: Inventory; }
        machine Main::main(&mut self) {
            let replacement: Inventory = self.inventory;
            let view: &Inventory = &self.inventory;
            self.inventory = move replacement;
        }
        "#,
    ) {
        Ok(_) => panic!("the absent place cannot be observed or re-borrowed"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("is absent")),
        "{diagnostics:#?}"
    );
}

#[test]
fn enclosing_owner_move_rejects_while_a_subtree_is_absent() {
    let diagnostics = match check_source(
        r#"
        data Inner { tag: i32; }
        data Inventory { inner: Inner; }
        data Main { inventory: Inventory; }
        machine Main::main(&mut self) {
            let inner: Inner = self.inventory.inner;
            let whole: Inventory = self.inventory;
            self.inventory = move whole;
            self.inventory.inner = move inner;
        }
        "#,
    ) {
        Ok(_) => panic!("an ancestor carrying an absent subtree cannot move"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("is absent")),
        "{diagnostics:#?}"
    );
}

#[test]
fn call_on_the_incomplete_owner_rejects() {
    let diagnostics = match check_source(
        r#"
        data Inventory { slots: i32; }
        data Main { inventory: Inventory; }
        machine Main::observe(&self) -> i32 { self.inventory.slots }
        machine Main::main(&mut self) {
            let replacement: Inventory = self.inventory;
            let n: i32 = self.observe();
            self.inventory = move replacement;
        }
        "#,
    ) {
        Ok(_) => panic!("a call may not observe an incomplete owner"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("is absent")),
        "{diagnostics:#?}"
    );
}

#[test]
fn store_into_the_absent_subtree_rejects() {
    let diagnostics = match check_source(
        r#"
        data Inner { tag: i32; }
        data Inventory { inner: Inner; }
        data Main { inventory: Inventory; }
        machine Main::main(&mut self) {
            let inner: Inner = self.inventory.inner;
            self.inventory.inner.tag = 9;
            self.inventory.inner = move inner;
        }
        "#,
    ) {
        Ok(_) => panic!("absent storage cannot take a deeper partial store"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("is absent")),
        "{diagnostics:#?}"
    );
}

#[test]
fn shared_borrow_extraction_still_rejects() {
    let diagnostics = match check_source(
        r#"
        data Inventory { slots: i32; }
        data Main { inventory: Inventory; }
        machine Main::main(&self) {
            let replacement: Inventory = self.inventory;
        }
        "#,
    ) {
        Ok(_) => panic!("a shared borrow cannot open a restoration window"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot transfer a non-copy value out of borrowed storage")),
        "{diagnostics:#?}"
    );
}

#[test]
fn shared_route_local_cannot_extract() {
    let diagnostics = match check_source(
        r#"
        data Inner { tag: i32; }
        data Inventory { inner: Inner; }
        data Main { inventory: Inventory; }
        machine Main::main(&mut self) {
            let x: &Inventory = &self.inventory;
            let inner: Inner = x.inner;
            self.inventory.inner = move inner;
        }
        "#,
    ) {
        Ok(_) => panic!("a shared route local cannot open a restoration window"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot transfer a non-copy value out of borrowed storage")),
        "{diagnostics:#?}"
    );
}

#[test]
fn crash_exit_abandons_the_window() {
    check_source(
        r#"
        data Inventory { slots: i32; }
        data Main { inventory: Inventory; flag: bool; }
        machine Main::main(&mut self)
        crashes Trap
        {
            transition self.flag {
                true -> busted()
                _ -> busted()
            }

            state busted(&mut self) {
                let abandoned: Inventory = self.inventory;
                crash Trap;
            }
        }
        "#,
    )
    .expect("a crash edge abandons the window under the survivor contract");
}

fn operational_window_source(contract: &str, body: &str) -> String {
    let body_contract = if contract.is_empty() {
        String::new()
    } else {
        format!("{contract};")
    };
    format!(
        "boundary trait Waiter {{ machine wait() -> i32 {contract}; }}
         machine identity(value: i32) -> i32 {{ value }}
         data Inventory {{ slots: i32; }}
         data Main {{ inventory: Inventory; waiter: Waiter; observed: i32; }}
         machine Main::replace(&mut self) reaches Waiter {body_contract} {{ {body} }}"
    )
}

#[test]
fn blocking_initializer_cannot_park_an_open_storage_window() {
    let diagnostics = check_source(&operational_window_source(
        "blocks",
        "let taken: Inventory = self.inventory;
         let result: i32 = block self.waiter.wait();
         self.inventory = move taken;",
    ))
    .expect_err("initializer calls must not park an incomplete borrowed owner");
    assert_window_call_fence(&diagnostics);
}

fn assert_window_call_fence(diagnostics: &[diagnostics::Diagnostic]) {
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("cannot suspend or block")
                && diagnostic.message.contains("inventory")
        }),
        "{diagnostics:#?}"
    );
}

#[test]
fn blocking_call_fence_is_independent_of_expression_position() {
    for call in [
        "_ = block self.waiter.wait();",
        "self.observed = block self.waiter.wait();",
        "let result: [i32; 2] = [0, block self.waiter.wait()];",
        "let result: i32 = identity(block self.waiter.wait());",
    ] {
        let diagnostics = check_source(&operational_window_source(
            "blocks",
            &format!(
                "let taken: Inventory = self.inventory;
                 {call}
                 self.inventory = move taken;"
            ),
        ))
        .err()
        .unwrap_or_else(|| panic!("call escaped borrowed window checking: {call}"));
        assert_window_call_fence(&diagnostics);
    }
}

#[test]
fn suspending_initializer_requires_restored_borrowed_storage() {
    let diagnostics = check_source(&operational_window_source(
        "suspends",
        "let taken: Inventory = self.inventory;
         let result: i32 = suspend self.waiter.wait();
         self.inventory = move taken;",
    ))
    .expect_err("a suspension must retain restoration custody or reject");
    assert_window_call_fence(&diagnostics);
}

#[test]
fn blocking_calls_outside_the_storage_window_remain_valid() {
    check_source(&operational_window_source(
        "blocks",
        "let before: i32 = block self.waiter.wait();
         let taken: Inventory = self.inventory;
         self.inventory = move taken;
         self.observed = block self.waiter.wait();",
    ))
    .expect("only calls crossing the open window owe restoration custody");
}

#[test]
fn blocking_replacement_value_is_checked_before_the_repair_store() {
    let source = operational_window_source(
        "blocks",
        "let taken: Inventory = self.inventory;
         self.inventory = block self.waiter.wait();",
    )
    .replace("machine wait() -> i32", "machine wait() -> Inventory");
    let diagnostics = check_source(&source)
        .expect_err("the repair value is evaluated while its destination is absent");
    assert_window_call_fence(&diagnostics);
}

#[test]
fn skipped_blocking_operand_does_not_cross_the_storage_window() {
    let source = operational_window_source(
        "blocks",
        "let taken: Inventory = self.inventory;
         let skipped: bool = false && block self.waiter.wait();
         self.inventory = move taken;",
    )
    .replace("machine wait() -> i32", "machine wait() -> bool");
    check_source(&source).expect("an operand that cannot execute does not park the invocation");
}

fn assert_boundary_window_rejection(source: &str) {
    let diagnostics = match check_source(source) {
        Ok(_) => panic!("a boundary must not observe an incomplete borrowed owner"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("boundary or service call")
                && diagnostic.message.contains("inventory")
        }),
        "{diagnostics:#?}"
    );
}

#[test]
fn nonblocking_service_call_requires_restored_borrowed_storage() {
    assert_boundary_window_rejection(&operational_window_source(
        "",
        "let taken: Inventory = self.inventory;
         let reading: i32 = self.waiter.wait();
         self.inventory = move taken;",
    ));
}

#[test]
fn empty_reach_boundary_and_its_wrapper_require_restored_borrowed_storage() {
    for invocation in ["sample()", "wrapped_sample()", "outer_sample()"] {
        assert_boundary_window_rejection(&format!(
            "boundary machine sample() -> i32 ensures true;
             machine wrapped_sample() -> i32 {{ sample() }}
             machine outer_sample() -> i32 {{ wrapped_sample() }}
             data Inventory {{ slots: i32; }}
             data Main {{ inventory: Inventory; }}
             machine Main::replace(&mut self) {{
                 let taken: Inventory = self.inventory;
                 let reading: i32 = {invocation};
                 self.inventory = move taken;
             }}"
        ));
    }
}

#[test]
fn empty_reach_boundary_on_a_later_callee_state_still_fences_the_call() {
    assert_boundary_window_rejection(
        "boundary machine sample() -> i32 ensures true;
         machine wrapped_sample(flag: bool) -> i32 {
             transition flag { true -> later() _ -> 0 }
             state later() -> i32 { sample() }
         }
         data Inventory { slots: i32; }
         data Main { inventory: Inventory; }
         machine Main::replace(&mut self, flag: bool) {
             let taken: Inventory = self.inventory;
             let reading: i32 = wrapped_sample(flag);
             self.inventory = move taken;
         }",
    );
}

#[test]
fn ordinary_wrapper_retains_its_conservative_service_reach() {
    for body in ["observer.wait()", "7"] {
        assert_boundary_window_rejection(&format!(
            "boundary trait Observer {{ machine wait() -> i32; }}
             machine observe(observer: &Observer) -> i32 reaches Observer {{ {body} }}
             data Inventory {{ slots: i32; }}
             data Main {{ inventory: Inventory; observer: Observer; }}
             machine Main::replace(&mut self) reaches Observer {{
                 let taken: Inventory = self.inventory;
                 let reading: i32 = observe(&self.observer);
                 self.inventory = move taken;
             }}"
        ));
    }
}

#[test]
fn nonblocking_boundary_replacement_is_checked_before_its_store() {
    assert_boundary_window_rejection(
        "pub data Inventory { slots: i32; }
         boundary machine replacement() -> Inventory ensures true;
         data Main { inventory: Inventory; }
         machine Main::replace(&mut self) {
             let taken: Inventory = self.inventory;
             self.inventory = replacement();
         }",
    );
}

#[test]
fn boundary_calls_outside_the_storage_window_remain_valid() {
    check_source(&operational_window_source(
        "",
        "let before: i32 = self.waiter.wait();
         let taken: Inventory = self.inventory;
         self.inventory = move taken;
         self.observed = self.waiter.wait();",
    ))
    .expect("boundary calls before extraction and after repair remain valid");
}

#[test]
fn recursive_quiet_calls_do_not_invent_boundary_exposure() {
    let source = "boundary machine sample() -> i32 ensures true;
         machine quiet(flag: bool) -> i32 {
             transition flag { true -> 7 _ -> quiet(true) }
         }
         data Inventory { slots: i32; }
         data Main { inventory: Inventory; }
         machine Main::replace(&mut self, flag: bool) {
             let taken: Inventory = self.inventory;
             let reading: i32 = quiet(flag);
             self.inventory = move taken;
         }";
    check_source(source).expect("a cycle in the retained call graph is not itself a boundary");
    assert_boundary_window_rejection(&source.replace("true -> 7", "true -> sample()"));
}

#[test]
fn named_boundary_operator_requires_restored_borrowed_storage() {
    for invocation in ["CheckedMath::read(7)", "wrapped_read(7)"] {
        assert_boundary_window_rejection(&format!(
            "data CheckedMath {{}}
             boundary operator CheckedMath::read(value: i32) -> i32;
             machine wrapped_read(value: i32) -> i32 {{ CheckedMath::read(value) }}
             data Inventory {{ slots: i32; }}
             data Main {{ inventory: Inventory; }}
             machine Main::replace(&mut self) {{
                 let taken: Inventory = self.inventory;
                 let reading: i32 = {invocation};
                 self.inventory = move taken;
             }}"
        ));
    }
}

#[test]
fn spelled_boundary_operator_and_wrapper_require_restored_storage() {
    for invocation in ["left + right", "combine(left, right)"] {
        assert_boundary_window_rejection(&format!(
            "data Number [copy] {{ value: i32; }}
             boundary operator + Number::add(left: Number, right: Number) -> Number;
             machine combine(left: Number, right: Number) -> Number {{ left + right }}
             data Inventory {{ slots: i32; }}
             data Main {{ inventory: Inventory; }}
             machine Main::replace(&mut self, left: Number, right: Number) {{
                 let taken: Inventory = self.inventory;
                 let reading: Number = {invocation};
                 self.inventory = move taken;
             }}"
        ));
    }
}

#[test]
fn skipped_boundary_operator_does_not_cross_the_window() {
    check_source(
        "data CheckedMath {}
         boundary operator CheckedMath::read(value: i32) -> bool;
         data Inventory { slots: i32; }
         data Main { inventory: Inventory; }
         machine Main::replace(&mut self) {
             let taken: Inventory = self.inventory;
             let skipped: bool = false && CheckedMath::read(7);
             self.inventory = move taken;
         }",
    )
    .expect("an operator invocation in a skipped operand cannot expose storage");
}

#[test]
fn quiet_builtin_wrapper_can_run_during_a_storage_window() {
    check_source(
        "machine quiet(value: i32) -> i32 { min(value, value) }
         data Inventory { slots: i32; }
         data Main { inventory: Inventory; }
         machine Main::replace(&mut self) {
             let taken: Inventory = self.inventory;
             let reading: i32 = quiet(7);
             self.inventory = move taken;
         }",
    )
    .expect("an exact pure builtin inside a checked body is not a boundary");
}

#[test]
fn unknown_call_reach_cannot_authorize_an_open_storage_window() {
    let checked = check_source(
        "machine identity(value: i32) -> i32 { value }
         data Inventory { slots: i32; }
         data Main { inventory: Inventory; }
         machine Main::replace(&mut self) {
             let taken: Inventory = self.inventory;
             let value: i32 = identity(7);
             self.inventory = move taken;
         }",
    )
    .expect("an ordinary pure call can execute during the window");
    crate::checks::check_checked_facts(&checked.typed, &checked.facts)
        .expect("unmodified facts replay");
    let mut facts = checked.facts.clone();
    facts.flow.control.calls.for_each_mut(|_, call| {
        call.service_reach = Default::default();
    });
    let diagnostics = crate::checks::check_checked_facts(&checked.typed, &facts)
        .expect_err("missing reach is not a proven empty row");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("boundary or service call")),
        "{diagnostics:#?}"
    );
}
