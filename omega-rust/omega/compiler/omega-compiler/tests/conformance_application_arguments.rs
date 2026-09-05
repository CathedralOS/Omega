//! Exact static arguments retained by a checked contract conformance use.

use omega_compiler::compile_to_checked_with_packages;
use omega_package_compilation::{PackageCompilationInputs, PackageSourceBinding};
use psi_core::PackageKeyIdentity;
use psi_typed_trees::expression::ExpressionNode;
use psi_typed_trees_to_checked_trees::close_conformance_application;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct FixtureDirectory(PathBuf);

impl FixtureDirectory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "omega-conformance-application-arguments-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&path).expect("create conformance argument fixture");
        Self(path)
    }
}

impl Drop for FixtureDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn checked_contract_conformance_retains_ordered_nested_static_arguments() {
    let directory = FixtureDirectory::new();
    let source = directory.0.join("main.omg");
    fs::write(
        &source,
        r#"
pub trait Ranked {}
pub data Card {}
pub data First {}
pub data Second {}
pub data Wrapper<Value> { value: Value; }
pub FieldOrder<Element, Left, const Rank: u64, Right>: Element satisfies Ranked {}
pub machine tag<Element, Order: Element satisfies Ranked>() -> u64 { 0 }
boundary machine trusted() -> u64
ensures result == tag<Card, FieldOrder<Card, Wrapper<First>, 7, Wrapper<Second>>>();
"#,
    )
    .expect("write checked conformance argument source");
    fs::write(
        directory.0.join("build.omg"),
        "machine build(builder: &mut Build) { builder.package(\"conformance-arguments\"); }\n",
    )
    .expect("write conformance argument build declaration");
    let package = PackageKeyIdentity::from_digest([0x69; 32]).unwrap();
    let inputs = PackageCompilationInputs::new_package(
        package,
        vec![PackageSourceBinding::new(
            package,
            "conformance-arguments",
            directory.0.clone(),
        )],
        Vec::new(),
    )
    .unwrap();
    let checked = compile_to_checked_with_packages(&source, Some("windows_x86_64"), inputs)
        .expect("mixed and nested generic conformance arguments should check");
    let occurrences = &checked
        .facts
        .proof
        .contract_expression_static_conformance_applications;
    let [occurrence] = occurrences.as_slice() else {
        panic!("one actual checked contract conformance occurrence")
    };
    assert_eq!(occurrence.static_argument_position, 1);
    let ExpressionNode::Call(call) = checked.expression_table.expression(occurrence.expression)
    else {
        panic!("retained occurrence must rejoin its typed contract call")
    };
    let selected = &call.machine_arguments[1];
    let supplied = &selected.application.as_ref().unwrap().arguments;
    let closed = &occurrence.application;
    assert_eq!(closed.declaration, selected.symbol);
    assert_eq!(closed.arguments.as_ref(), supplied.as_ref());
    assert_eq!(
        close_conformance_application(&checked.typed, selected).unwrap(),
        *closed
    );
    let [card, left, rank, right] = closed.arguments.as_ref() else {
        panic!("type, nested type, const, nested type stay in declaration order")
    };
    let declaration = |name: &str| {
        checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == name)
            .unwrap()
            .symbol
    };
    assert_eq!(card.symbol, declaration("Card"));
    assert_eq!(left.symbol, declaration("Wrapper"));
    assert_eq!(right.symbol, left.symbol);
    assert_eq!(rank.const_literal.as_ref().unwrap().text(), "7");
    let [left_child] = left.application.as_ref().unwrap().arguments.as_ref() else {
        panic!("left wrapper retains one exact child argument")
    };
    let [right_child] = right.application.as_ref().unwrap().arguments.as_ref() else {
        panic!("right wrapper retains one exact child argument")
    };
    assert_eq!(left_child.symbol, declaration("First"));
    assert_eq!(right_child.symbol, declaration("Second"));
    assert_ne!(left_child.symbol, right_child.symbol);
    assert_eq!(closed.type_arguments.len(), 3);
    assert_eq!(closed.const_arguments.len(), 1);
    assert!(closed.machine_arguments.is_empty());

    // These are deliberately mutated diagnostic copies, not checked program
    // replacements. Legacy receipts do not hash the newly retained typed
    // tree; equal receipts must not authorize substituting that tree.
    let mut substituted = selected.clone();
    substituted.application.as_mut().unwrap().arguments[1]
        .application
        .as_mut()
        .unwrap()
        .arguments[0]
        .symbol = right_child.symbol;
    let substituted = close_conformance_application(&checked.typed, &substituted).unwrap();
    assert_eq!(substituted.type_arguments, closed.type_arguments);
    assert_eq!(substituted.report_fingerprint, closed.report_fingerprint);
    assert_eq!(substituted.commitment, closed.commitment);
    assert_eq!(substituted.arguments[1].display_name(), left.display_name());
    assert_ne!(substituted.arguments[1], *left);
    assert_eq!(
        substituted.arguments[1]
            .application
            .as_ref()
            .unwrap()
            .arguments[0]
            .symbol,
        right_child.symbol,
    );

    let mut reordered = closed.clone();
    reordered.arguments.swap(1, 3);
    assert_ne!(reordered.arguments, closed.arguments);
    assert_eq!(reordered.report_fingerprint, closed.report_fingerprint);
    assert_eq!(reordered.commitment, closed.commitment);
    assert_eq!(closed.arguments.as_ref(), supplied.as_ref());
}
