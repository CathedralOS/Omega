//! Borrowed-storage invariant windows (ch11/ownership spec): a consuming move
//! out of `&mut`-reachable storage opens a repair obligation on the exact
//! absent place; an ordinary store of a same-typed owned value discharges it;
//! every exit edge, stale observation, ancestor use, and repeated extraction
//! while the place is absent rejects.

use crate::tests::front_end::checked_program_result;

#[test]
fn direct_provider_call_still_consumes_an_erased_member_argument() {
    let source = "boundary trait Probe { machine take_descriptor(value: Sealed) -> i32; }
        machine probe_binding() -> i32 { 0 }
        machine take_descriptor(value: Sealed) -> i32
            satisfies Probe::take_descriptor via probe_binding();
        data Evidence { case Only; }
        data Sealed { payload: i32; proof [erased]: Evidence; }
        data Main { sealed: Sealed; }
        machine Main::replace(context: &mut Main) reaches Probe {
            let observed: i32 = take_descriptor(context.sealed);
        }";
    let diagnostics = checked_program_result(source)
        .expect_err("a direct provider call still moves the borrowed argument");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("boundary or service call")
                && diagnostic.message.contains("sealed")
        }),
        "{diagnostics:#?}"
    );
}

#[test]
fn evaluation_order_selected_index_uses_its_collection_operand() {
    for collection_parameter in ["&Buffer", "&mut Buffer"] {
        checked_program_result(&format!(
            "data Buffer {{ value: i32; }}
             machine [] Buffer::index(buffer: {collection_parameter}, index: u64) -> i32 {{ buffer.value }}
             machine read(buffer: &mut Buffer) -> i32 {{ buffer[0] }}"
        ))
        .expect("indexing reborrows the collection using the explicit parameter contract");
    }

    checked_program_result(
        "data Buffer { value: i32; }
         machine [] Buffer::index(buffer: Buffer, index: u64) -> i32 { buffer.value }
         data Main { buffer: Buffer; }
         machine Main::replace(context: &mut Main) {
             let reading: i32 = context.buffer[0];
             context.buffer = Buffer { value: reading };
         }",
    )
    .expect("the selected collection operand detaches before invocation and is restored");

    let diagnostics = checked_program_result(
        "data Inventory { slots: i32; }
         data Buffer { inventory: Inventory; value: i32; }
         machine [] Buffer::index(buffer: &Buffer, index: u64) -> i32 { buffer.value }
         data Main { buffer: Buffer; }
         machine Main::replace(context: &mut Main) {
             let taken: Inventory = context.buffer.inventory;
             let reading: i32 = context.buffer[0];
             context.buffer.inventory = move taken;
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
    let checked = checked_program_result(
        "machine identity(value: i32) -> i32 { value }
         data Inventory { slots: i32; }
         data Main { inventory: Inventory; }
         machine Main::replace(context: &mut Main) {
             let taken: Inventory = context.inventory;
             let value: i32 = identity(7);
             context.inventory = move taken;
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
fn enclosing_owner_move_rejects_while_a_subtree_is_absent() {
    let diagnostics = match checked_program_result(
        r#"
        data Inner { tag: i32; }
        data Inventory { inner: Inner; }
        data Main { inventory: Inventory; }
        machine Main::main(context: &mut Main) {
            let inner: Inner = context.inventory.inner;
            let whole: Inventory = context.inventory;
            context.inventory = move whole;
            context.inventory.inner = move inner;
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
fn store_into_the_absent_subtree_rejects() {
    let diagnostics = match checked_program_result(
        r#"
        data Inner { tag: i32; }
        data Inventory { inner: Inner; }
        data Main { inventory: Inventory; }
        machine Main::main(context: &mut Main) {
            let inner: Inner = context.inventory.inner;
            context.inventory.inner.tag = 9;
            context.inventory.inner = move inner;
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
fn unreachable_arm_moves_do_not_block_agreement() {
    checked_program_result(
        r#"
        data Inventory { slots: i32; }
        data Main { inventory: Inventory; other: Inventory; }
        machine Main::main(context: &mut Main, flag: bool) {
            let taken: Inventory = match flag {
                true -> context.inventory
                false -> context.inventory
                _ -> context.other
            };
            context.inventory = move taken;
        }
        "#,
    )
    .expect("the post-covered arm can never run, so its move owes nothing");
}

#[test]
fn nested_disagreement_rejects_through_the_outer_join() {
    let diagnostics = match checked_program_result(
        r#"
        data Inventory { slots: i32; }
        data Main { inventory: Inventory; }
        machine Main::main(context: &mut Main, flag: bool, other_flag: bool) {
            let taken: Inventory = match flag {
                true -> match other_flag {
                    true -> context.inventory
                    _ -> Inventory { slots: 0 }
                }
                _ -> context.inventory
            };
            context.inventory = move taken;
        }
        "#,
    ) {
        Ok(_) => panic!("a nested disagreement cannot ride an agreeing outer arm"),
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
fn repeated_extraction_inside_one_arm_rejects() {
    // Two arguments move `context.inventory` inside one arm — the second meets
    // the hole the first opened, so the join never gets a consistent debt.
    let diagnostics = match checked_program_result(
        r#"
        data Inventory { slots: i32; }
        machine deliver(inventory: Inventory, other: Inventory) -> Inventory { inventory }
        data Main { inventory: Inventory; }
        machine Main::main(context: &mut Main, flag: bool) {
            let taken: Inventory = match flag {
                true -> deliver(context.inventory, context.inventory)
                _ -> context.inventory
            };
            context.inventory = move taken;
        }
        "#,
    ) {
        Ok(_) => panic!("a second move inside the arm must meet an absent place"),
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
    checked_program_result(
        r#"
        data Inventory { slots: i32; }
        data Main { inventory: Inventory; }
        machine Main::main(context: &mut Main, flag: bool)
        crashes Trap
        {
            transition flag {
                true -> busted(context)
                _ -> busted(context)
            }

            state busted(context: &mut Main) {
                let abandoned: Inventory = context.inventory;
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
         data Main {{ inventory: Inventory; }}
         machine Main::replace(context: &mut Main) reaches Waiter {body_contract} {{ {body} }}"
    )
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
        "_ = block Waiter::wait();",
        "let mut observed: i32; observed = block Waiter::wait();",
        "let result: [i32; 2] = [0, block Waiter::wait()];",
        "let result: i32 = identity(block Waiter::wait());",
    ] {
        let diagnostics = checked_program_result(&operational_window_source(
            "blocks",
            &format!(
                "let taken: Inventory = context.inventory;
                 {call}
                 context.inventory = move taken;"
            ),
        ))
        .err()
        .unwrap_or_else(|| panic!("call escaped borrowed window checking: {call}"));
        assert_window_call_fence(&diagnostics);
    }
}

#[test]
fn suspending_initializer_requires_restored_borrowed_storage() {
    let diagnostics = checked_program_result(&operational_window_source(
        "suspends",
        "let taken: Inventory = context.inventory;
         let result: i32 = suspend Waiter::wait();
         context.inventory = move taken;",
    ))
    .expect_err("a suspension must retain restoration custody or reject");
    assert_window_call_fence(&diagnostics);
}

#[test]
fn skipped_blocking_operand_does_not_cross_the_storage_window() {
    let source = operational_window_source(
        "blocks",
        "let taken: Inventory = context.inventory;
         let skipped: bool = false && block Waiter::wait();
         context.inventory = move taken;",
    )
    .replace("machine wait() -> i32", "machine wait() -> bool");
    checked_program_result(&source)
        .expect("an operand that cannot execute does not park the invocation");
}

fn assert_boundary_window_rejection(source: &str) {
    let diagnostics = match checked_program_result(source) {
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
fn recursive_quiet_calls_do_not_invent_boundary_exposure() {
    let source = "boundary machine sample() -> i32 ensures true;
         machine quiet(flag: bool) -> i32 {
             transition flag { true -> 7 _ -> quiet(true) }
         }
         data Inventory { slots: i32; }
         data Main { inventory: Inventory; }
         machine Main::replace(context: &mut Main, flag: bool) {
             let taken: Inventory = context.inventory;
             let reading: i32 = quiet(flag);
             context.inventory = move taken;
         }";
    checked_program_result(source)
        .expect("a cycle in the retained call graph is not itself a boundary");
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
             machine Main::replace(context: &mut Main) {{
                 let taken: Inventory = context.inventory;
                 let reading: i32 = {invocation};
                 context.inventory = move taken;
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
             machine Main::replace(context: &mut Main, left: Number, right: Number) {{
                 let taken: Inventory = context.inventory;
                 let reading: Number = {invocation};
                 context.inventory = move taken;
             }}"
        ));
    }
}

#[test]
fn quiet_builtin_wrapper_can_run_during_a_storage_window() {
    checked_program_result(
        "machine quiet(value: i32) -> i32 { min(value, value) }
         data Inventory { slots: i32; }
         data Main { inventory: Inventory; }
         machine Main::replace(context: &mut Main) {
             let taken: Inventory = context.inventory;
             let reading: i32 = quiet(7);
             context.inventory = move taken;
         }",
    )
    .expect("an exact pure builtin inside a checked body is not a boundary");
}
