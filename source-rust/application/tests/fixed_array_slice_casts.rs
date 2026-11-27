//! The owner ruling on slices and fixed-array references: `as` converts
//! between `@[T; N]` and `@[T]` at the call site, and an expected argument
//! type alone performs no conversion. The slice covers all `N` elements of the
//! same backing, and its length comes from the array type. The other way,
//! `view as @[T; N]` needs a proof that the view holds exactly `N` elements,
//! and `@view[a..b] as @[T; N]` selects that window first; both borrow the
//! view's own storage.
//!
//! See `wiki/spec/layouts/recasts.md`, "Slices and fixed-array references".

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

const VIEW_TO_ARRAY: &str =
    "machine total(lane: @[u8; 4]) -> u64 { (lane[0] as u64) + (lane[3] as u64) }
machine window(row: @[u8]) -> u64 requires row.length >= 6 {
    let lane: @[u8; 4] = @row[2..6] as @[u8; 4];
    total(lane)
}
machine whole(row: @[u8]) -> u64 requires row.length == 4 {
    let lane: @[u8; 4] = row as @[u8; 4];
    total(lane)
}
public machine main() crashes Trap {
    let data: [u8; 6] = [1u8, 2u8, 3u8, 40u8, 5u8, 60u8];
    let w: u64 = window(@data as @[u8]);
    let a: [u8; 4] = [1u8, 2u8, 3u8, 40u8];
    let h: u64 = whole(@a as @[u8]);
    transition w == 63u64 && h == 41u64 { true -> done(), false -> wrong() }
    machine done() {}
    machine wrong() { crash Trap; }
}
";

/// A view, or a window of one, casts to a fixed-array reference that reads
/// the view's own elements.
#[test]
fn a_view_casts_to_a_fixed_array_reference_of_its_proven_length() {
    let scratch = stage("view-to-array", VIEW_TO_ARRAY);
    let (output, report) = observations::run(&scratch, &["run", "--engine", "psi"], &["build.omg"]);
    assert!(output.status.success(), "{report}");
}

/// Native code does not yet form a reference into a view's payload, and says
/// so instead of compiling something else.
#[test]
fn native_code_refuses_a_fixed_array_reference_into_a_view() {
    let scratch = stage("view-to-array-native", VIEW_TO_ARRAY);
    let (output, report) = observations::run(&scratch, &["run"], &["build.omg"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("UnsupportedFixedArrayViewReference"),
        "{report}"
    );
}

/// The cast needs a proof that the view holds exactly `N` elements.
#[test]
fn a_view_without_a_length_proof_does_not_cast_to_a_fixed_array() {
    for (name, body) in [
        (
            "unproven-view",
            "let lane: @[u8; 4] = row as @[u8; 4]; total(lane)",
        ),
        (
            "short-window",
            "let lane: @[u8; 4] = @row[0..3] as @[u8; 4]; total(lane)",
        ),
    ] {
        let main = format!(
            "machine total(lane: @[u8; 4]) -> u64 {{ (lane[0] as u64) + (lane[3] as u64) }}
machine whole(row: @[u8]) -> u64 requires row.length >= 3 {{ {body} }}
public machine main() {{
    let a: [u8; 4] = [1u8, 2u8, 3u8, 40u8];
    _ = whole(@a as @[u8]);
}}
"
        );
        let scratch = stage(name, &main);
        let (output, report) =
            observations::run(&scratch, &["run", "--engine", "psi"], &["build.omg"]);
        assert!(!output.status.success(), "{name}: {report}");
        assert!(
            report.contains("cannot prove operation safety"),
            "{name}: {report}"
        );
    }
}
