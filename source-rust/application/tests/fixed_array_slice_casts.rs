//! The owner ruling on slices and fixed-array references: `as` converts
//! between `@[T; N]` and `@[T]` at the call site, and an expected argument
//! type alone performs no conversion. The slice covers all `N` elements of the
//! same backing, and its length comes from the array type.
//!
//! See `OWNER_QUESTIONS.md`, "Explicit casts between slices and fixed-array
//! references", and `wiki/spec/layouts/recasts.md`.

#[path = "support/observations.rs"]
mod observations;
#[path = "support/project.rs"]
mod project;

use platform_custody::temporary_directory::TemporaryDirectory;

const BUILD: &str = "use main.omg as app;
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: \"casts\" };
    Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
}
";

fn stage(name: &str, main: &str) -> TemporaryDirectory {
    project::stage(name, &[("build.omg", BUILD), ("main.omg", main)])
}

/// Each form of fixed-array reference reaches a slice formal through `as`,
/// and the slice reads the same elements.
#[test]
fn a_fixed_array_reference_casts_to_a_slice_of_its_whole_extent() {
    let main = "machine total(items: @[u8]) -> u64 requires items.length == 4 {
    (items[0] as u64) + (items[1] as u64) + (items[2] as u64) + (items[3] as u64)
}
machine via(lane: @[u8; 4]) -> u64 { total(lane as @[u8]) }
public machine main() crashes Trap {
    let row: [u8; 4] = [1u8, 2u8, 3u8, 40u8];
    let borrowed: u64 = total(@row as @[u8]);
    let held: @[u8; 4] = @row;
    let local: u64 = total(held as @[u8]);
    let formal: u64 = via(@row);
    transition borrowed == 46u64 && local == 46u64 && formal == 46u64 {
        true -> done(),
        false -> wrong()
    }
    machine done() {}
    machine wrong() { crash Trap; }
}
";
    observations::assert_silent_return(|| stage("array-to-slice", main), &["build.omg"]);
}

/// Without the cast, a fixed-array reference does not become a slice.
#[test]
fn a_bare_fixed_array_reference_is_not_a_slice_argument() {
    let main = "machine first(items: @[u8]) -> u8 requires items.length > 0 { items[0] }
machine via(lane: @[u8; 4]) -> u8 { first(lane) }
public machine main() {
    let row: [u8; 4] = [1u8, 2u8, 3u8, 4u8];
    _ = via(@row);
}
";
    let scratch = stage("bare-array-argument", main);
    let (output, report) = observations::run(&scratch, &["run", "--engine", "psi"], &["build.omg"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("a reference to a fixed array is not a byte view"),
        "{report}"
    );
}
