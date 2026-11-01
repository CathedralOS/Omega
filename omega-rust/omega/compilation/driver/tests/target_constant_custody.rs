//! An untrusted review candidate cannot replace a sealed public dependency.

use super::fixture_package_inputs;
use package_evidence::{PackageReviewInput, project_checked_package_policy};
use typed_trees::expression::ExpressionNode;

#[test]
fn public_layout_constant_review_rejoins_the_complete_original_query() {
    let target = "macos_arm64";
    let profile = target::TargetProfile::MacosArm64;
    let fixture = fixture_package_inputs::repo_root()
        .join("samples/cli/facts/target_layout_derived_constant/controls/public_alias/main.omg");
    let mut prepared =
        fixture_package_inputs::reviewed_repository_fixture_package_inputs(&fixture, Some(target))
            .expect("public layout constant package inputs");
    let checked = fixture_package_inputs::check_fixture(&mut prepared, target, Default::default())
        .expect("checked public layout aliases");
    let package = checked.sources().package_identity().unwrap();
    let original = project_checked_package_policy(&checked, profile, package)
        .expect("both public constants retain their target dependencies");
    assert_eq!(original.public_api().consts().len(), 2);
    let raw = checked
        .const_declarations()
        .iter()
        .find(|value| checked.symbols.name(value.symbol) == "RAW_SIZE")
        .unwrap();
    let expression = raw.authored_initializer;
    let original_call = checked.expression_table.call_node(expression).unwrap();
    assert!(original_call.layout_query.is_some());
    let scalar_type = raw.declared_type;
    let reject = |mutate: &dyn Fn(&mut typed_trees::expression::TableCallExpression),
                  fragment: &str| {
        let mut typed = checked.typed.clone().into_inner();
        let ExpressionNode::Call(call) = typed.expression_table.expression_mut(expression) else {
            panic!("retained initializer is a layout query")
        };
        mutate(call);
        let candidate = checked_trees::CheckedTrees::with_roots(typed, checked.facts.clone());
        let errors = project_checked_package_policy(
            PackageReviewInput::supplied(&candidate, &checked),
            profile,
            package,
        )
        .expect_err("supplied mutation cannot borrow immutable query custody");
        assert!(
            errors.iter().any(|error| error.message.contains(fragment)),
            "{errors:?}"
        );
    };
    reject(
        &|call| {
            call.layout_query.as_mut().unwrap().kind =
                language_core::data_layout_queries::DataLayoutQueryKind::Alignment;
        },
        "layout-query kind differs",
    );
    reject(
        &|call| {
            call.layout_query.as_mut().unwrap().operand.type_reference = scalar_type;
        },
        "layout-query operand differs",
    );
    reject(
        &|call| {
            call.exact_coercion_target = scalar_type;
        },
        "non-query semantic payload",
    );
    let selection = checked
        .expression_table
        .expression_entries()
        .find_map(|(handle, _)| {
            checked
                .expression_table
                .authored_selection_occurrences(handle)
                .next()
        })
        .expect("checked source retains authored declaration selections");
    reject(
        &|call| {
            call.authored_call_selection = Some(selection);
        },
        "non-query semantic payload",
    );
}
