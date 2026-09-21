//! Qualified case membership inside a declared domain's proof facts keeps its
//! selected case identity end to end: `domain T::D requires self in T::C`
//! leaves the domain provable, and a `value in T::C` obligation discharges
//! against the place's assigned case or a matching caller premise.

use super::{Sources, compile, compile_to_checked, root_inputs};
use compiler::CheckedCompileRequest;
use typed_trees::expression::ExpressionNode;

const CHOICE: &str = "module choice; pub data Choice [copy] { case Empty; case Some(value: u32); }
     pub domain Choice::NonEmpty requires self in Choice::Some;";

const REQUIRES_NONEMPTY: &str =
    "machine probe(value: choice::Choice) -> u32 requires value in choice::Choice::NonEmpty { 7 }";

fn choice_root() -> (Sources, std::path::PathBuf) {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(root.join("choice.omg"), CHOICE);
    (tree, root)
}

#[test]
fn domain_case_membership_fact_selects_the_exact_variant() {
    let (_tree, root) = choice_root();
    Sources::write(
        root.join("main.omg"),
        "use choice;
         machine unit() -> u32 { 0 }",
    );
    let checked = compile(&root, root_inputs(&root));
    let domain = checked
        .typed
        .domain_definitions()
        .iter()
        .find(|domain| {
            checked.symbols.display_path(domain.symbol, "::") == "choice::Choice::NonEmpty"
        })
        .expect("the declared domain remains");
    let [typed_trees::domain::ProofFact::Expression(expression)] =
        checked.typed.proof_facts(domain)
    else {
        panic!("the domain keeps exactly its authored fact");
    };
    // `self in T::C` lowers to the case-equality `self == T::C`: the selected
    // variant rides the right operand's resolved symbol.
    let ExpressionNode::Binary(binary) = checked.typed.expression_table.expression(*expression)
    else {
        panic!("a `Type::Case` membership lowers to an equality");
    };
    assert_eq!(
        binary.operator,
        typed_trees::expression::BinaryOperator::Equal
    );
    let ExpressionNode::Name(subject) = checked.typed.expression_table.expression(binary.left)
    else {
        panic!("the equality subject is the domain's self");
    };
    assert_eq!(
        checked
            .typed
            .expression_table
            .name_path_members(subject.members)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["self"]
    );
    let ExpressionNode::Name(case) = checked.typed.expression_table.expression(binary.right) else {
        panic!("the equality target is the selected case");
    };
    assert_eq!(
        checked.symbols.display_path(case.symbol, "::"),
        "choice::Choice::Some"
    );
}

#[test]
fn case_domain_fact_leaves_its_domain_provable_by_premise() {
    let (_tree, root) = choice_root();
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use choice;
             {REQUIRES_NONEMPTY}
             machine forward(value: choice::Choice) -> u32
                 requires value in choice::Choice::NonEmpty {{ probe(value) }}"
        ),
    );
    compile(&root, root_inputs(&root));
}

#[test]
fn case_domain_fact_discharges_through_the_place_assigned_case() {
    let (_tree, root) = choice_root();
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use choice;
             {REQUIRES_NONEMPTY}
             machine run() -> u32 {{
                 let value: choice::Choice = choice::Choice::Some {{ value: 3 }};
                 probe(value)
             }}"
        ),
    );
    compile(&root, root_inputs(&root));
}

#[test]
fn bare_case_membership_obligation_discharges_through_the_assigned_case() {
    let (_tree, root) = choice_root();
    Sources::write(
        root.join("main.omg"),
        "use choice;
         machine peek(value: choice::Choice) -> u32 requires value in choice::Choice::Some { 7 }
         machine run() -> u32 {
             let value: choice::Choice = choice::Choice::Some { value: 3 };
             peek(value)
         }",
    );
    compile(&root, root_inputs(&root));
}

#[test]
fn case_membership_obligation_discharges_through_a_case_guard() {
    let (_tree, root) = choice_root();
    Sources::write(
        root.join("main.omg"),
        "use choice;
         machine peek(value: choice::Choice) -> u32 requires value in choice::Choice::Some { 7 }
         machine run(value: choice::Choice) -> u32 {
             transition value in choice::Choice::Some { true -> peek(value) _ -> 0 }
         }",
    );
    compile(&root, root_inputs(&root));
}

#[test]
fn case_domain_fact_rejects_a_place_assigned_a_different_case() {
    let (_tree, root) = choice_root();
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use choice;
             {REQUIRES_NONEMPTY}
             machine run() -> u32 {{
                 let value: choice::Choice = choice::Choice::Empty;
                 probe(value)
             }}"
        ),
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(root_inputs(&root)),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("an Empty value does not satisfy Choice::NonEmpty");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires")),
        "{diagnostics:?}"
    );
}
