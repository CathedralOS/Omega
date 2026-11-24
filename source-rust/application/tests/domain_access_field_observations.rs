//! The field selected by an Inaccessible observation has no permitted
//! operations. Its sibling still uses the ordinary mutable loan.

#[path = "support/program.rs"]
mod program;

const SOURCE: &str = r#"use {omega}/language/core/layout.omg as layout;
use {omega}/language/core/access/access.omg as access;
use {omega}/language/core/reflect/reflect.omg as reflect;

data Pair { visible: u32; hidden: u32; }
domain Pair::Restricted access Limit;
Limit satisfies |layout|::AccessPolicy {
    machine plan(schema: |reflect|::TypeSchema) -> |layout|::AccessPlan reaches none crashes none terminates {
        let mutable plan: |layout|::AccessPlan = |access|::new_plan(@schema, |layout|::Issuance::Unrestricted);
        |access|::observe(@mutable plan, |reflect|::field(Pair::hidden), |layout|::FieldAccess::Inaccessible);
        plan
    }
}
machine select(pair: @mutable Pair in Restricted) -> u32 {
    pair.visible = 42u32;
    pair.visible
}
machine main() crashes Trap {
    let mutable pair: Pair in Restricted = Pair { visible: 1u32, hidden: 17u32 } as +Restricted;
    transition select(@mutable pair) == 42u32 { true -> done(), _ -> fail() }
    machine done() {}
    machine fail() { crash Trap; }
}
"#;

#[test]
fn an_inaccessible_field_allows_use_of_its_sibling() {
    program::assert_silent_return(SOURCE);
}

#[test]
fn a_loan_cannot_read_or_write_the_excluded_field() {
    for (original, changed) in [
        ("    pair.visible\n", "    pair.hidden\n"),
        ("pair.visible =", "pair.hidden ="),
    ] {
        let source = SOURCE.replace(original, changed);
        assert_ne!(source, SOURCE);
        let (output, report) = program::invoke(&source, &["run", "--engine", "psi"]);
        assert!(!output.status.success(), "{report}");
        assert!(
            report.contains("this field is inaccessible") && report.contains("select"),
            "{report}"
        );
    }
}
