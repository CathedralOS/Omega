use super::{Case, check};

// With no assembly catalog, a mnemonic would already be refused by the resolver.
// Empty assembly isolates the typer's target requirement from instruction lookup.
#[test]
fn assembly_machine_requires_an_admitted_target() {
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[("main.omg", "machine m() {}")],
        bad: &[("main.omg", "asm machine m() {}")],
        file: "main.omg",
        occurrence: "asm machine m() {}",
        reason: "assembly typing requires an admitted target",
        rule: "Mathematical terms and assembly",
    });
}

#[test]
fn inline_assembly_requires_an_admitted_target() {
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[("main.omg", "machine m() {}")],
        bad: &[("main.omg", "machine m() { asm {} }")],
        file: "main.omg",
        occurrence: "asm {}",
        reason: "assembly typing requires an admitted target",
        rule: "Mathematical terms and assembly",
    });
}
