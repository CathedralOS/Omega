use std::path::PathBuf;

use source::{SourceMap, SourceOrigin};
use typed_trees::TypedTrees;

fn typed(text: &str, relative: &str, origin: SourceOrigin) -> TypedTrees {
    let root = PathBuf::from("fixture");
    let mut sources = SourceMap::default();
    let source_id = sources
        .add_with_metadata(root.join(relative), text.to_owned(), root, None, origin)
        .source_id;
    crate::front_end::typed_program_from_source_map(sources, &[(source_id, text)])
}

const CALLING: &str = r#"
data BoundarySignature {}
data BoundaryPlanResult {}
trait CallingPolicy {
    machine plan(signature: BoundarySignature) -> BoundaryPlanResult;
}
trait Calling<C> where C satisfies CallingPolicy {}
data Policy {}
PolicyCalling: Policy satisfies CallingPolicy;
machine Policy::plan(signature: BoundarySignature) -> BoundaryPlanResult
    satisfies CallingPolicy::plan
{
    let result: BoundaryPlanResult;
    result
}
boundary trait Native: Calling<Policy> {
    machine send(value: &[u8]);
}
machine send_binding() -> i32 {
    0
}
machine send(value: &[u8]) satisfies Native::send via send_binding();
"#;

#[test]
fn exact_standard_calling_declarations_retain_the_custom_policy_route() {
    let program = typed(CALLING, "calling.omg", SourceOrigin::Toolchain);
    let (calling, policy) =
        validation::standard_calling_traits(&program).expect("exact declarations");
    assert_ne!(calling.symbol, policy.symbol);
    validation::validate_program(&program).expect("custom calling route remains available");
}

const VECTOR: &str = r#"
data Vec<T> { value: u64; }
boundary trait Native {
    machine send(value: &Vec<u8>);
}
machine send_binding() -> i32 {
    0
}
machine send(value: &Vec<u8>) satisfies Native::send via send_binding();
"#;

#[test]
fn public_calling_vocabulary_uses_its_contract_not_package_filename() {
    let vocabulary = include_str!("../../../../../source/library/std/calling.omg");
    let program = typed(vocabulary, "public_abi.omg", SourceOrigin::User);
    assert!(validation::standard_calling_traits(&program).is_some());
    let changed = vocabulary.replace("trait Calling<C>", "trait Calling<C, Extra>");
    assert_ne!(changed, vocabulary);
    let program = typed(&changed, "calling.omg", SourceOrigin::User);
    assert!(validation::standard_calling_traits(&program).is_none());
}

#[test]
fn empty_calling_lookalikes_do_not_enable_private_native_carriers() {
    let text = CALLING
        .replace(
            "machine plan(signature: BoundarySignature) -> BoundaryPlanResult;",
            "",
        )
        .replace("    satisfies CallingPolicy::plan", "");
    let program = typed(&text, "calling.omg", SourceOrigin::Toolchain);
    assert!(validation::standard_calling_traits(&program).is_none());
    let errors = validation::validate_program(&program)
        .expect_err("source origin cannot admit a private native carrier");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("cannot use safe slice")),
        "{errors:?}"
    );
}

#[test]
fn calling_interface_requires_its_resolved_bound_and_owned_signature() {
    let wrong_bound = CALLING.replace(
        "where C satisfies CallingPolicy",
        "where Policy satisfies CallingPolicy",
    );
    let program = typed(&wrong_bound, "calling.omg", SourceOrigin::Toolchain);
    assert!(validation::standard_calling_traits(&program).is_none());

    let borrowed_signature = CALLING.replace(
        "signature: BoundarySignature",
        "signature: &BoundarySignature",
    );
    let program = typed(&borrowed_signature, "calling.omg", SourceOrigin::Toolchain);
    assert!(validation::standard_calling_traits(&program).is_none());
}

#[test]
fn calling_vocabulary_identity_survives_a_source_comment() {
    let vocabulary = include_str!("../../../../../source/library/std/calling.omg");
    let commented = format!("// The public calling contract is unchanged.\n{vocabulary}");
    let program = typed(&commented, "public_abi.omg", SourceOrigin::User);
    assert!(
        validation::standard_calling_traits(&program).is_some(),
        "source formatting cannot remove the resolved calling contract"
    );
}

#[test]
fn calling_plan_discovery_ignores_unrelated_policy_requirements() {
    let text = CALLING.replace(
        "trait CallingPolicy {",
        "trait CallingPolicy { machine version() -> u64;",
    );
    let text = format!(
        "{text}\nmachine Policy::version() -> u64 satisfies CallingPolicy::version {{ 1 }}"
    );
    let program = typed(&text, "public_abi.omg", SourceOrigin::User);
    assert!(validation::standard_calling_traits(&program).is_some());
    validation::validate_program(&program).expect("extra requirement is independently satisfied");
}

#[test]
fn calling_plan_types_must_belong_to_the_vocabulary_owner() {
    let foreign = "pub data BoundarySignature {}\npub data BoundaryPlanResult {}";
    let text = CALLING
        .replace("data BoundarySignature {}", "use foreign;")
        .replace("data BoundaryPlanResult {}", "");
    let root = PathBuf::from("fixture");
    let mut sources = SourceMap::default();
    let foreign_id = sources
        .add_with_metadata(
            root.join("foreign.omg"),
            foreign.to_owned(),
            root.clone(),
            None,
            SourceOrigin::User,
        )
        .source_id;
    let calling_id = sources
        .add_with_metadata(
            root.join("calling.omg"),
            text.clone(),
            root,
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let program = crate::front_end::typed_program_from_source_map(
        sources,
        &[(foreign_id, foreign), (calling_id, &text)],
    );
    assert!(validation::standard_calling_traits(&program).is_none());
}

#[test]
fn exact_core_vector_still_requires_an_explicit_native_adapter() {
    let errors = validation::validate_program(&typed(VECTOR, "vec.omg", SourceOrigin::Toolchain))
        .expect_err("core descriptor cannot become a default foreign ABI record");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("cannot use vector carrier")),
        "{errors:?}"
    );
}

#[test]
fn user_vector_name_does_not_hide_a_nested_private_carrier() {
    let text = VECTOR.replace("value: u64;", "value: &[u8];");
    let errors = validation::validate_program(&typed(&text, "main.omg", SourceOrigin::User))
        .expect_err("ordinary record fields are still checked");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("cannot use safe slice")),
        "{errors:?}"
    );
}

#[test]
fn process_exit_completion_requires_the_exact_core_requirement() {
    use semantic_vocabulary::{BoundaryCompletion, ExternalCompletionKind};

    let canonical = include_str!("../../../../../source/library/core/process_exit.omg");
    let changed = format!("{canonical}\n// Different admitted source bytes.\n");
    for (text, relative, origin, expected) in [
        (
            canonical,
            "process_exit.omg",
            SourceOrigin::Toolchain,
            BoundaryCompletion::External(ExternalCompletionKind::ProcessExit),
        ),
        (
            canonical,
            "process_exit.omg",
            SourceOrigin::User,
            BoundaryCompletion::NormalReturn,
        ),
        (
            canonical,
            "other/process_exit.omg",
            SourceOrigin::Toolchain,
            BoundaryCompletion::NormalReturn,
        ),
        (
            changed.as_str(),
            "process_exit.omg",
            SourceOrigin::Toolchain,
            BoundaryCompletion::NormalReturn,
        ),
    ] {
        let program = typed(text, relative, origin);
        let definition = program
            .traits()
            .iter()
            .find(|definition| definition.name.as_str() == "ProcessExit")
            .expect("fixture trait");
        let [requirement] = program.trait_machine_signatures(definition) else {
            panic!("one process-exit requirement")
        };
        assert_eq!(
            validation::boundary_requirement_completion(&program, requirement.symbol),
            expected,
        );
        assert_eq!(
            validation::boundary_requirement_completion(&program, definition.symbol),
            BoundaryCompletion::NormalReturn,
            "the trait owner cannot substitute for its exact requirement",
        );
    }
}

#[test]
fn package_completion_receipts_rejoin_exact_owner_and_requirement() {
    use semantic_vocabulary::{BoundaryCompletion, ExternalCompletionKind, PackageKeyIdentity};

    let text = "pub boundary trait Console { machine exit_process(code: i32); machine write_byte(code: i32); }";
    let package = PackageKeyIdentity::from_digest([11; 32]).expect("package identity");
    let root = PathBuf::from("fixture");
    let mut sources = SourceMap::default();
    let source_id = sources
        .add_with_metadata(
            root.join("console.omg"),
            text.to_owned(),
            root,
            Some(package),
            SourceOrigin::User,
        )
        .source_id;
    let mut program =
        crate::front_end::typed_program_from_source_map(sources, &[(source_id, text)]);
    let definition = program.traits().iter().next().expect("Console");
    let [exit, write] = program.trait_machine_signatures(definition) else {
        panic!("two exact requirements")
    };
    let exit_symbol = exit.symbol;
    let write_symbol = write.symbol;
    let declaration = typed_trees::BoundaryCompletionDeclaration {
        package: Some(package),
        owner: definition.symbol,
        owner_path: program.symbols.display_path(definition.symbol, "::"),
        requirement: exit_symbol,
        requirement_path: program.symbols.display_path(exit_symbol, "::"),
        completion: BoundaryCompletion::External(ExternalCompletionKind::ProcessExit),
    };
    assert_eq!(
        validation::boundary_requirement_completion(&program, exit_symbol),
        BoundaryCompletion::NormalReturn
    );
    let before = program.structural_fingerprint();
    let handle = program
        .boundary_completion_declarations
        .append(declaration.clone());
    assert_ne!(
        before,
        program.structural_fingerprint(),
        "role admission invalidates source-semantic memos"
    );
    assert_eq!(
        validation::boundary_requirement_completion(&program, exit_symbol),
        declaration.completion
    );
    validation::validate_boundary_completion_declarations(&program).expect("exact role input");
    assert_eq!(
        validation::boundary_requirement_completion(&program, write_symbol),
        BoundaryCompletion::NormalReturn
    );
    for corruption in 0..4 {
        let mut changed = declaration.clone();
        match corruption {
            0 => changed.package = PackageKeyIdentity::from_digest([12; 32]),
            1 => changed.owner = write_symbol,
            2 => changed.owner_path.push_str("::Other"),
            3 => changed.requirement = write_symbol,
            _ => unreachable!(),
        }
        assert!(!validation::valid_boundary_completion_declaration(
            &program, &changed
        ));
    }
    let before = program.structural_fingerprint();
    program
        .boundary_completion_declarations
        .get_mut(handle)
        .requirement = write_symbol;
    assert_ne!(
        before,
        program.structural_fingerprint(),
        "same-length role substitutions invalidate memos"
    );
    assert_eq!(
        validation::boundary_requirement_completion(&program, write_symbol),
        BoundaryCompletion::NormalReturn
    );
    assert!(validation::validate_boundary_completion_declarations(&program).is_err());
    program
        .boundary_completion_declarations
        .get_mut(handle)
        .clone_from(&declaration);
    let _ = program.boundary_completion_declarations.append(declaration);
    assert!(
        validation::validate_boundary_completion_declarations(&program).is_err(),
        "repeated semantic-role rows are rejected"
    );
}
