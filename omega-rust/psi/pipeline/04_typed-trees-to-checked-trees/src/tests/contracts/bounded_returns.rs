use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;

fn proof_rejects(program: &typed_trees::TypedTrees) {
    let plan = proof::obligations::build_proof_plan(program);
    let diagnostics = proof::checker::check_proof_plan(&plan)
        .expect_err("a missing or invalidated arrival premise cannot prove the return");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove return value")),
        "{diagnostics:#?}"
    );
}

#[test]
fn absent_arrival_premise_keeps_the_declared_range() {
    let source = r#"
        machine increment(value: u32 [0..=4]) -> u32 [0..=4] { value + 1 }
    "#;
    proof_rejects(&parse_typed_trees(source));
    assert!(lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled()).is_err());
}
