//! A primitive operation keeps its actual operand types during source admission.

use super::*;

#[test]
fn unrelated_operator_declarations_do_not_leave_scalar_operations_unresolved() {
    for (contract, accepted) in [("requires value <= 10", true), ("", false)] {
        let typed = super::boundary_calls::typed(&format!(
            r#"
data Pair [copy] {{ first: u32; second: u32; }}
machine + combine(left: Pair, right: Pair) -> Pair {{ left }}
machine <= ordered(left: Pair, right: Pair) -> bool {{ left.first <= right.first }}
machine entry(value: u32) -> u32 {contract} {{ value + 1u32 }}
"#
        ));
        let entry = super::boundary_calls::entry(&typed);
        let elaboration = typed_trees_to_terminal_psi::elaborate_for(
            &typed,
            typed_trees_to_terminal_psi::Compilation {
                calling: None,
                providers: None,
                declaration_metadata: &[],
                entries: &[entry],
                subjects: &[],
                purpose: source::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        )
        .expect("scalar operands exclude the unrelated Pair operator families");
        let verdict = judge(Ok(elaboration)).unwrap();
        if accepted {
            assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
        } else {
            assert!(
                matches!(verdict, RouteVerdict::Rejected { .. }),
                "{verdict:?}"
            );
        }
    }
}

#[test]
fn contract_arithmetic_keeps_its_selected_maximum_u64_operands() {
    for (contract, accepted) in [
        ("requires size <= 18446744073709551615u64 - offset", true),
        ("", false),
    ] {
        let typed = super::boundary_calls::typed(&format!(
            r#"
            data Pair [copy] {{ first: u64; second: u64; }}
            machine - difference(left: Pair, right: Pair) -> Pair {{ left }}
            machine <= ordered(left: Pair, right: Pair) -> bool {{ left.first <= right.first }}
            machine entry(offset: u64, size: u64) -> u64 {contract} {{ offset + size }}
        "#
        ));
        let entry = super::boundary_calls::entry(&typed);
        let elaboration = typed_trees_to_terminal_psi::elaborate_for(
            &typed,
            typed_trees_to_terminal_psi::Compilation {
                calling: None,
                providers: None,
                declaration_metadata: &[],
                entries: &[entry],
                subjects: &[],
                purpose: source::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        )
        .expect(
            "the contract keeps its selected u64 operands despite unrelated operator declarations",
        );
        let verdict = judge(Ok(elaboration)).unwrap();
        if accepted {
            assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
        } else {
            assert!(
                matches!(verdict, RouteVerdict::Rejected { .. }),
                "{verdict:?}"
            );
        }
    }
}
