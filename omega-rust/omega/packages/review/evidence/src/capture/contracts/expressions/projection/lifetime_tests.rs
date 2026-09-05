use super::value_forms::project_value_form;
use crate::capture::contracts::facts::{ContractProjectionContext, project_contracts};
use crate::record::{
    PackageReviewContractExpression, PackageReviewContractFact, PackageReviewContractStaticArgument,
};
use omega_compiler::{CheckedCompilation, compile_to_checked_with_packages};
use omega_package_compilation::{PackageCompilationInputs, PackageSourceBinding};
use psi_typed_trees::{expression::ExpressionNode, name::Identifier};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

struct Source(PathBuf);

impl Drop for Source {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn checked_source() -> (Source, CheckedCompilation) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let source = Source(std::env::temp_dir().join(format!(
        "omega-contract-lifetime-scope-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
    )));
    std::fs::create_dir(&source.0).unwrap();
    std::fs::write(
        source.0.join("main.omg"),
        r#"
pub data View<'slot, Value> { value: &'slot Value; }
pub machine tag<Value>() -> u64 { 0 }
pub machine generic_tag<'a, 'b>(first: &'a u64, second: &'b u64) -> u64
requires tag<View<'a, u64>>() == tag<View<'a, u64>>()
{ 0 }
"#,
    )
    .unwrap();
    std::fs::write(
        source.0.join("build.omg"),
        "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
    )
    .unwrap();
    let package = psi_core::PackageKeyIdentity::from_digest([41; 32]).unwrap();
    let inputs = PackageCompilationInputs::new_package(
        package,
        vec![PackageSourceBinding::new(
            package,
            "review-fixture",
            source.0.clone(),
        )],
        Vec::new(),
    )
    .unwrap();
    let checked = compile_to_checked_with_packages(
        &source.0.join("main.omg"),
        Some("windows_x86_64"),
        inputs,
    )
    .expect("existing top-level generic-tag contract fixture checks");
    (source, checked)
}

#[test]
fn original_contract_expressions_use_scoped_lifetime_ordinals() {
    let (_source, checked) = checked_source();
    let before = checked.clone();
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "generic_tag")
        .unwrap();
    let reference_type = checked
        .typed
        .state_parameters
        .iter()
        .find(|(_, parameter)| parameter.name.as_str() == "first")
        .map(|(_, parameter)| parameter.type_reference)
        .unwrap();
    let source_name = Identifier::generated("a");
    let project = |outer: &str, local: &str, select_local: bool| {
        let lifetimes = [Identifier::generated(outer), Identifier::generated(local)];
        let mut substitutions = vec![(source_name.clone(), lifetimes[0].clone())];
        if select_local {
            substitutions.push((source_name.clone(), lifetimes[1].clone()));
        }
        let context = ContractProjectionContext {
            subject_kind: "callable",
            subject_name: "generic_tag",
            owner: psi_checked_trees::ContractProofFactOwner::Machine { machine_symbol: machine.symbol },
            point: psi_facts::ProgramPoint::Machine { machine_symbol: machine.symbol },
            parameters: &[],
            domain_symbol: None,
            data_symbol: None,
            lifetime_binders: &lifetimes,
            lifetime_substitutions: &substitutions,
            selection_exposure: psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface,
        };
        let contracts =
            project_contracts(&checked, checked.machine_contracts(machine), &context, &[])
                .expect("original checked expressions retain exact call and proof custody");
        let zero = project_value_form(
            &checked,
            &context,
            &[],
            &ExpressionNode::ZeroValue(reference_type),
            &|_| panic!("zero value has no expression children"),
        )
        .unwrap()
        .expect("existing reference type projects in the lexical scope");
        (contracts, zero)
    };
    let local = project("a", "$local", true);
    let renamed = project("outer-renamed", "$local-renamed", true);
    let outer = project("a", "$local", false);
    assert_eq!(local, renamed, "scope alpha-renaming preserves policy");
    assert_ne!(
        local.0, outer.0,
        "nested static arguments distinguish local from outer"
    );
    assert_ne!(
        local.1, outer.1,
        "zero-value reference types distinguish local from outer"
    );
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        right,
        ..
    }) = local.0[0].fact()
    else {
        panic!("one original equality contract")
    };
    let PackageReviewContractExpression::Call {
        static_arguments, ..
    } = right.as_ref()
    else {
        panic!("original tag call")
    };
    let [
        PackageReviewContractStaticArgument::GenericType {
            lifetime_arguments, ..
        },
    ] = static_arguments.as_slice()
    else {
        panic!("one generic View argument")
    };
    assert_eq!(
        lifetime_arguments,
        &[1],
        "local shadowing must not bind ordinal zero"
    );
    assert_eq!(
        checked, before,
        "projection does not rewrite checked source or evidence"
    );
}
