//! A guard returns the original lock through its ordinary owned cleanup.

#[path = "support/program.rs"]
mod program;

const GUARD: &str =
    include_str!("../../../tests/fixtures/synchronization/run/owned_guard/main.omg");

fn assert_deadlock(source: &str) {
    let (result, diagnostic) = program::invoke(source, &["--check"]);
    assert!(!result.status.success(), "{diagnostic}");
    assert!(
        diagnostic.contains("synchronization deadlock"),
        "{diagnostic}"
    );
}

#[test]
fn returning_from_the_guard_owner_releases_its_lock() {
    program::assert_silent_return(GUARD);
}

#[test]
fn leaving_the_flag_set_in_cleanup_blocks_the_next_acquisition() {
    let source = GUARD.replace(
        "flag.held.store(0, Publish);",
        "flag.held.store(1, Publish);",
    );
    assert_deadlock(&source);
}

#[test]
fn reacquisition_inside_the_guard_scope_precedes_cleanup() {
    let source = GUARD
        .replace(
            "= lock(flag);",
            "= lock(flag);\n    acquire(guard.flag);\n    unlock(move guard);",
        )
        .replace("use_once(@flag);\n    acquire(@flag);", "use_once(@flag);");
    assert_deadlock(&source);
}

#[test]
fn cleaning_up_another_lock_does_not_release_the_held_flag() {
    let declarations = GUARD.split_once("machine main()").unwrap().0;
    let source = format!(
        "{declarations}
        machine protect_other<'storage>(held: @'storage Flag, other: @'storage Flag) {{
            acquire(held);
            let guard: Guard<'storage> = Guard {{ flag: other }};
        }}
        machine main() {{
            let mutable held: Flag;
            let mutable other: Flag;
            protect_other(@held, @other);
            acquire(@held);
        }}"
    );
    assert_deadlock(&source);
}
