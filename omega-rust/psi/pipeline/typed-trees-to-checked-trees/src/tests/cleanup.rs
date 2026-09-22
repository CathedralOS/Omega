use crate::tests::front_end::{checked_program, checked_program_result};

fn diagnostics(source: &str) -> Vec<diagnostics::Diagnostic> {
    checked_program_result(source).expect_err("invalid cleanup declaration must reject")
}

fn rejects(source: &str, expected: &str) {
    let diagnostics = diagnostics(source);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(expected)),
        "expected diagnostic containing `{expected}`, got {diagnostics:?}"
    );
}

fn machine_symbol(checked: &checked_trees::CheckedTrees, name: &str) -> symbols::SymbolHandle {
    checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with(name))
        .unwrap_or_else(|| panic!("missing machine `{name}`"))
        .symbol
}

#[test]
fn accepts_reserved_cleanup_shape() {
    let source = r#"
        data Wrapper { value: i32; }
        machine Wrapper::drop(&mut self) {}
    "#;
    checked_program(source);
}

#[test]
fn accepts_exact_one_call_executable_cleanup_shape() {
    let source = r#"
        data Helper {}
        machine Helper::touch() {}
        data Wrapper { value: i32; }
        machine Wrapper::drop(&mut self) { Helper::touch(); }
    "#;
    checked_program(source);
}

#[test]
fn accepts_exact_two_call_executable_cleanup_shape() {
    let source = r#"
        data First {}
        machine First::touch() {}
        data Second {}
        machine Second::touch() {}
        data Wrapper { value: i32; }
        machine Wrapper::drop(&mut self) {
            First::touch();
            Second::touch();
        }
    "#;
    checked_program(source);
}

#[test]
fn accepts_exact_five_call_executable_cleanup_shape() {
    let source = r#"
        data First {}
        machine First::touch() {}
        data Second {}
        machine Second::touch() {}
        data Third {}
        machine Third::touch() {}
        data Fourth {}
        machine Fourth::touch() {}
        data Fifth {}
        machine Fifth::touch() {}
        data Wrapper { value: i32; }
        machine Wrapper::drop(&mut self) {
            First::touch();
            Second::touch();
            Third::touch();
            Fourth::touch();
            Fifth::touch();
        }
    "#;
    checked_program(source);
}

#[test]
fn rejects_authored_method_selection_of_reserved_cleanup() {
    rejects(
        r#"
            data Resource { value: i32; }
            machine Resource::drop(&mut self) {}
            machine misuse(resource: &mut Resource) {
                resource.drop();
            }
        "#,
        "reserved cleanup machine `Resource::drop` is compiler-selected",
    );
}

#[test]
fn rejects_authored_qualified_selection_of_reserved_cleanup() {
    rejects(
        r#"
            data Resource { value: i32; }
            machine Resource::drop(&mut self) {}
            machine misuse(resource: &mut Resource) {
                Resource::drop(resource);
            }
        "#,
        "reserved cleanup machine `Resource::drop` is compiler-selected",
    );
}

#[test]
fn rejects_reserved_cleanup_as_a_static_machine_argument() {
    rejects(
        r#"
            data Resource { value: i32; }
            machine Resource::drop(&mut self) {}
            machine accept<machine Selected>()
            where machine Selected(value: &mut Resource)
            { }
            machine misuse() {
                accept<Resource::drop>();
            }
        "#,
        "machine argument `Resource::drop` for `accept` does not refine `Selected`",
    );
}

#[test]
fn rejects_reserved_cleanup_selected_then_forwarded_through_a_machine_binder() {
    rejects(
        r#"
            data Resource { value: i32; }
            machine Resource::drop(&mut self) {}
            machine sink<machine Selected>()
            where machine Selected(value: &mut Resource)
            { }
            machine forward<machine Selected>()
            where machine Selected(value: &mut Resource)
            {
                sink<Selected>();
            }
            machine misuse() {
                forward<Resource::drop>();
            }
        "#,
        "machine argument `Resource::drop` for `forward` does not refine `Selected`",
    );
}

#[test]
fn ordinary_drop_spelling_remains_callable() {
    let source = r#"
        data Resource { value: i32; }
        machine Resource::drop_counter(&mut self) {}
        machine drop(resource: &mut Resource) {}
        machine accept<machine Selected>()
        where machine Selected(resource: &mut Resource)
        { }
        machine use_drop_names(resource: &mut Resource) {
            resource.drop_counter();
            drop(resource);
            accept<drop>();
        }
    "#;
    checked_program(source);
}

#[test]
fn accepts_repeated_nonempty_or_argumented_hook_bodies() {
    for source in [
        r#"
            data Helper {}
            machine Helper::touch() {}
            data Wrapper { value: i32; }
            machine Wrapper::drop(&mut self) {
                Helper::touch();
                Helper::touch();
            }
        "#,
        r#"
            data Leaf {}
            machine Leaf::finish() {}
            data First {}
            machine First::touch() {}
            data Second {}
            machine Second::touch() { Leaf::finish(); }
            data Wrapper { value: i32; }
            machine Wrapper::drop(&mut self) {
                First::touch();
                Second::touch();
            }
        "#,
        r#"
            data Helper {}
            machine Helper::touch(value: u8) {}
            data Wrapper { value: i32; }
            machine Wrapper::drop(&mut self) { Helper::touch(1u8); }
        "#,
    ] {
        checked_program(source);
    }
}

#[test]
fn accepts_cleanup_for_generic_attached_data() {
    let source = r#"
        data Wrapper<T> { value: T; }
        machine Wrapper::drop(&mut self) {}
    "#;
    checked_program(source);
}

#[test]
fn rejects_non_mutable_cleanup_receiver() {
    for receiver in ["self", "&self", "&write self"] {
        rejects(
            &format!("data Wrapper {{ value: i32; }} machine Wrapper::drop({receiver}) {{}}"),
            "must have exactly the receiver `&mut self`",
        );
    }
}

#[test]
fn rejects_cleanup_positional_parameters() {
    rejects(
        "data Wrapper { value: i32; } machine Wrapper::drop(&mut self, extra: i32) {}",
        "must have exactly the receiver `&mut self`",
    );
}

#[test]
fn rejects_cleanup_without_receiver() {
    rejects(
        "data Wrapper { value: i32; } machine Wrapper::drop() {}",
        "must have exactly the receiver `&mut self`",
    );
}

#[test]
fn rejects_method_local_cleanup_generic() {
    rejects(
        "data Wrapper { value: i32; } machine Wrapper::drop<T>(&mut self) {}",
        "may not declare method-local lifetime or type parameters",
    );
}

#[test]
fn rejects_cleanup_result() {
    rejects(
        "data Wrapper { value: i32; } machine Wrapper::drop(&mut self) -> i32 { 0 }",
        "must return Unit",
    );
}

#[test]
fn bodyless_cleanup_requires_published_termination() {
    rejects(
        "data Wrapper { value: i32; } boundary machine Wrapper::drop(&mut self) ensures true;",
        "must publish `terminates;`",
    );

    let source = r#"
        pub data Wrapper { value: i32; }
        boundary machine Wrapper::drop(&mut self) terminates; ensures true;
    "#;
    checked_program(source);
}

#[test]
fn rejects_suspending_or_blocking_cleanup() {
    for behavior in ["suspends;", "blocks;"] {
        rejects(
            &format!(
                "data Wrapper {{ value: i32; }} machine Wrapper::drop(&mut self) {behavior} {{}}"
            ),
            "must be non-suspending and nonblocking",
        );
    }
}

#[test]
fn rejects_cleanup_crash_contract() {
    for cause in ["Trap", "Abort"] {
        rejects(
            &format!(
                "data Wrapper {{ value: i32; }} machine Wrapper::drop(&mut self) crashes {cause} {{}}"
            ),
            "may not declare a crash outcome",
        );
    }
}

#[test]
fn erased_nominal_cleanup_member_leaves_the_owner_trivially_affine() {
    // A `proof [erased]` member owns no runtime representation, so an owner
    // whose only nominal-cleanup member is erased stays an ordinary affine
    // record: `enter` admits a Unit body and its `c` parameter dies as a
    // plain discard on the return edge.
    let checked = checked_program(
        r#"
        data Evidence { tag: i32; }
        machine Evidence::drop(&mut self) {}
        data Carrier { token: i32; proof [erased]: Evidence; }
        machine touch() {}
        data Root {}
        machine Root::enter(c: Carrier) { touch(); }
        "#,
    );
    let enter = machine_symbol(&checked, "Root::enter");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(enter)
        .expect("an erased member never produces runtime cleanup, so `enter` admits");
    let Some(checked_trees::CheckedUnitEffectOperationPlan::Complete {
        trivial_affine_discards,
        ..
    }) = plan.operations.last()
    else {
        panic!("`enter` must complete with an ordinary Unit return");
    };
    assert_eq!(
        trivial_affine_discards.len(),
        1,
        "the `c` parameter discards trivially"
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_nominal_affine_unit_cleanups
            .for_machine(enter)
            .is_none(),
        "no nominal cleanup plan may attach to an erased-only owner"
    );
}

#[test]
fn relevant_nominal_cleanup_member_without_the_hook_admits_no_body() {
    // The same owner with a relevant `proof` member requires nominal drop and
    // has no `Carrier::drop`, so `enter` can enter no Unit lane.
    let checked = checked_program(
        r#"
        data Evidence { tag: i32; }
        machine Evidence::drop(&mut self) {}
        data Carrier { token: i32; proof: Evidence; }
        machine touch() {}
        data Root {}
        machine Root::enter(c: Carrier) { touch(); }
        "#,
    );
    let enter = machine_symbol(&checked, "Root::enter");
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(enter)
            .is_none(),
        "a nominal-drop owner without its hook admits no ordinary body"
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_partial_affine_unit_cleanups
            .for_machine(enter)
            .is_none()
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_nominal_affine_unit_cleanups
            .for_machine(enter)
            .is_none()
    );
}
