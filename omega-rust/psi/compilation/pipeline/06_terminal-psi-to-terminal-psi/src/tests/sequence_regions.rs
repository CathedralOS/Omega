//! Erased sequence storage can be shared across lifetimes, but each value
//! still carries the outer view region and every region of its elements.

use super::build_purpose::judge_build;
use super::*;

const SOURCE: &str = r#"
data Cell<'storage> { bytes: &'storage [u8]; }
machine echo<'storage>(items: &'storage [Cell<'storage>]) -> &'storage [Cell<'storage>] { items }
machine consume<'storage>(items: &'storage [Cell<'storage>]) { }
machine owned<'storage>(items: [Cell<'storage>]) -> [Cell<'storage>] { items }
machine build_entry<'left, 'right>(
    left: &'left [Cell<'left>], right: &'right [Cell<'right>], items: [Cell<'left>]
) {
    consume(echo(left));
    consume(echo(right));
    let moved: [Cell<'left>] = owned(items);
}
"#;

#[test]
fn sequence_calls_keep_outer_and_element_regions_on_one_erased_row() {
    let result = judge_build(SOURCE).unwrap();
    let RouteVerdict::Accepted(accepted) = result else {
        panic!("{result:?}")
    };
    let views = accepted
        .elaboration
        .module
        .structural_types
        .iter()
        .filter(|row| {
            matches!(
                row.shape,
                terminal_psi::StructuralTypeShape::ElementView {
                    access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                    ..
                }
            )
        })
        .count();
    assert_eq!(views, 1, "lifetimes do not select separate storage rows");
    accepted.publish_invocation().unwrap();

    // The second helper call cannot give its one lifetime both 'left and
    // 'right. Dropping either the outer or element tuple loses this refusal.
    let changed = SOURCE.replace(
        "right: &'right [Cell<'right>]",
        "right: &'left [Cell<'right>]",
    );
    let refusal = match judge_build(&changed) {
        Err(crate::CompilationRefusal::Elaboration(refusal))
        | Ok(RouteVerdict::FallBack(refusal)) => refusal,
        other => panic!("{other:?}"),
    };
    assert_eq!(refusal.reason, RefusalReason::Unsupported);
    assert!(
        refusal
            .detail
            .contains("call that gives one lifetime two regions"),
        "{refusal:?}"
    );
}
