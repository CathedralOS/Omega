//! A specialized issuer can establish only its declared domain application.

use super::program;

const ISSUER: &str = r#"
data Tag {}
data Other {}
data Cell { value: u32; }
domain<I> Cell::Marked<I> established by mark;
machine mark<I>(cell: Cell) -> Cell in Cell::Marked<I> { cell as +Cell::Marked<I> }
machine read(cell: Cell in Cell::Marked<Tag>) -> u32 { cell.value }
machine main() crashes Trap {
    let observed: u32 = read(mark::<Tag>(Cell { value: 33 }));
    transition observed == 33u32 { true -> done(), false -> wrong() }
    machine done() {}
    machine wrong() crashes Trap { crash Trap; }
}
"#;

#[test]
fn a_specialized_issuer_preserves_the_value_and_its_selected_index() {
    program::assert_silent_return(ISSUER);
}

#[test]
fn a_different_index_cannot_supply_the_consumer() {
    let wrong = ISSUER.replace("read(mark::<Tag>", "read(mark::<Other>");
    let (out, report) = program::invoke(&wrong, &["--check"]);
    assert!(!out.status.success(), "{report}");
    assert!(
        report.contains("carries `Cell::Marked` at (Other)")
            && report.contains("declares `Cell::Marked` at (Tag)"),
        "{report}"
    );
}

#[test]
fn another_generic_machine_cannot_issue_the_same_membership() {
    let source = ISSUER.replace(
        "machine read(",
        "machine forge<I>(cell: Cell) -> Cell in Cell::Marked<I> { cell as +Cell::Marked<I> }\nmachine read(",
    ).replace("read(mark::<Tag>", "read(forge::<Tag>");
    let (out, report) = program::invoke(&source, &["--check"]);
    assert!(!out.status.success(), "{report}");
    assert!(
        report.contains("not one of the domain's establishment routes")
            && report.contains("in machine `forge`"),
        "{report}"
    );
}
