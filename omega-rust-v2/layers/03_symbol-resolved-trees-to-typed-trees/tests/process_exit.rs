//! L3 hands forward the exact canonical process-exit requirement, never a spelling match.
use omega_resolver::{ParsedSource, StageSource};
use omega_typer::{ExternalCompletionKind, TypedStage, resolve_source_package, type_source_package};

fn check(source: &str) -> Result<TypedStage, String> {
    let tokens = omega_lexer::lex(source.as_bytes()).map_err(|e| e.to_string())?;
    let tree = omega_parser::parse_file(source.as_bytes(), &tokens).map_err(|e| e.to_string())?;
    let inputs = [StageSource {
        package: "app",
        source: ParsedSource {
            logical_path: "app.omg",
            text: source,
            tokens: &tokens,
            tree: &tree,
        },
    }];
    let resolved = resolve_source_package(&inputs).map_err(|e| e.message)?;
    type_source_package(&resolved)
        .map(|trees| trees.into_facts())
        .map_err(|e| format!("{}: {}", e.file, e.message))
}

#[test]
fn the_canonical_requirement_records_one_process_exit_at_its_exact_call() {
    let source = "use omega\\language\\core;
        machine quit(code:i32, other:i32) -> core\\Never reaches core\\ProcessExit { core\\ProcessExit::exit_process(code) }";
    let stage = check(source).unwrap();
    let (owner, requirement) = omega_toolchain::core_interface().process_exit_requirement();
    let [completion] = stage.external_completions.as_slice() else {
        panic!("expected one recognized process exit, found {:?}", stage.external_completions)
    };
    assert_eq!(completion.kind, ExternalCompletionKind::ProcessExit);
    assert_eq!(&completion.owner, owner);
    assert_eq!(&completion.requirement, requirement);
    assert_eq!(completion.site.span.slice(source.as_bytes()), Some(b"core\\ProcessExit::exit_process(code)".as_slice()));
    let call = stage.calls.iter().find(|call| call.node == completion.node).expect("recognized call keeps its typed selection");
    assert_eq!(&call.machine, requirement);
    let [operand] = call.operands.as_slice() else { panic!("exit keeps exactly its one authored operand") };
    assert_eq!(operand.site.span.slice(source.as_bytes()), Some(b"code".as_slice()));
}

#[test]
fn a_same_spelled_local_boundary_requirement_stays_an_ordinary_call() {
    let source = "data Never requires false {}
        boundary trait ProcessExit {
            machine exit_process(return_code: i32) -> Never
            reaches ProcessExit
            crashes none
            terminates;
        }
        machine quit(code:i32) -> Never reaches ProcessExit { ProcessExit::exit_process(code) }";
    let stage = check(source).unwrap();
    assert!(stage.external_completions.is_empty(), "{:?}", stage.external_completions);
    let (_, canonical) = omega_toolchain::core_interface().process_exit_requirement();
    let call = stage
        .calls
        .iter()
        .find(|call| call.machine.path == "ProcessExit::exit_process")
        .expect("the local requirement is still selected");
    assert_eq!(call.machine.package, "app");
    assert_ne!(&call.machine, canonical);
}
