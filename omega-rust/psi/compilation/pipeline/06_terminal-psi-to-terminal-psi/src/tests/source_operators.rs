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
