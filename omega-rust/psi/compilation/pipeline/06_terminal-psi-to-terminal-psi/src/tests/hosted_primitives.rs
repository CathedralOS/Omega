//! Compiler primitive custody is distinct from an ordinary provider body.

use super::*;

fn program(product: &str) -> typed_trees::TypedTrees {
    let sources = language_core::sealed_intrinsics::hosted_process_exit_sources();
    super::build_purpose::frontier_with_core(
        product,
        "machine build_entry() {}",
        &[
            (
                sources[0].0,
                "ProcessExitPrimitive",
                std::str::from_utf8(sources[0].2).unwrap(),
            ),
            (
                sources[1].0,
                "ProcessExit",
                std::str::from_utf8(sources[1].2).unwrap(),
            ),
            (
                sources[2].0,
                "Never",
                std::str::from_utf8(sources[2].2).unwrap(),
            ),
        ],
    )
}

fn judge_program(
    typed: &typed_trees::TypedTrees,
) -> Vec<Result<RouteVerdict, crate::CompilationRefusal>> {
    let entry = super::build_purpose::symbol(typed, "main");
    crate::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            typed,
            super::build_purpose::roster(
                &[entry],
                &[Some(super::build_purpose::package())],
                source::DependencyScope::Product,
            ),
            terminal_codec::TerminalPublication::Invocation,
        )
        .program,
    )
}

#[test]
fn hosted_primitive_retains_distinct_source_and_public_completion() {
    let typed = program(
        "use omega::language::core::Never; use omega::language::core::ProcessExit; use omega::language::core::ProcessExitPrimitive; machine main() reaches ProcessExit crashes none terminates { ProcessExitPrimitive::exit_process(7); }",
    );
    let mut verdicts = judge_program(&typed);
    let accepted = match verdicts.remove(0) {
        Ok(RouteVerdict::Accepted(accepted)) => accepted,
        other => panic!("{other:?}"),
    };
    assert!(
        verdicts
            .iter()
            .all(|v| matches!(v, Ok(RouteVerdict::Accepted(_)))),
        "{verdicts:?}"
    );
    let module = &accepted.elaboration.module;
    let primitive = module
        .boundary_machines
        .iter()
        .find(|boundary| {
            matches!(
                boundary.declaration.as_ref().map(|d| d.execution),
                Some(terminal_psi::CheckedBoundaryExecution::HostedPrimitive { .. })
            )
        })
        .unwrap();
    let requirement =
        terminal_semantics::closed_conformances::hosted_primitive_requirement(module, primitive)
            .unwrap()
            .unwrap();
    assert_ne!(primitive.id, requirement);
    let canonical = module.boundary_machine(requirement).unwrap();
    assert_ne!(primitive.identity, canonical.identity);
    assert_eq!(
        primitive.kind,
        terminal_psi::BoundaryRequirementKind::Machine
    );
    assert_eq!(primitive.external_completion_requirement(), requirement);
    assert!(module.operations().all(|operation| !matches!(
        operation.kind,
        terminal_psi::OperationKind::BoundaryCall { .. }
    )));
    accepted
        .publish_invocation()
        .expect("actual source publication");
    let closed = terminal_codec::ClosedTerminalModule::new(module.clone()).unwrap();
    let received =
        terminal_codec::ClosedTerminalModule::from_semantic_bytes(closed.semantic_bytes())
            .expect("independent primitive receiving");
    assert_eq!(received.semantic_bytes(), closed.semantic_bytes());

    for mutation in 0..5 {
        let mut changed = module.clone();
        let boundary = changed
            .boundary_machines
            .iter_mut()
            .find(|row| row.id == primitive.id)
            .unwrap();
        match mutation {
            0 => {
                boundary.declaration.as_mut().unwrap().execution =
                    terminal_psi::CheckedBoundaryExecution::HostedPrimitive {
                        operation: language_core::sealed_intrinsics::HostedOperation::ProcessExit,
                        requirement: primitive.id,
                    }
            }
            1 => boundary.owner = canonical.owner.clone(),
            2 => boundary.unconditional_termination = false,
            3 => boundary.result = terminal_psi::BoundaryMachineResult::Unit,
            4 => boundary.scalar_parameters.clear(),
            _ => unreachable!(),
        }
        let encoded = terminal_codec::ClosedTerminalModule::new(changed).unwrap();
        assert!(
            terminal_codec::ClosedTerminalModule::from_semantic_bytes(encoded.semantic_bytes())
                .is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn a_copied_primitive_name_does_not_receive_compiler_custody() {
    let typed = program(
        "use omega::language::core::Never; use omega::language::core::ProcessExit; public data CopiedPrimitive {} public boundary machine CopiedPrimitive::exit_process(return_code: i32) -> Never satisfies ProcessExit::exit_process reaches ProcessExit crashes none terminates; machine main() reaches ProcessExit crashes none terminates { CopiedPrimitive::exit_process(7); }",
    );
    let verdicts = judge_program(&typed);
    let diagnostic = format!("{verdicts:?}");
    assert!(verdicts.iter().any(Result::is_err), "{diagnostic}");
    assert!(
        diagnostic.contains("boundary conformer is callable only through its provider slot"),
        "{diagnostic}"
    );
}
