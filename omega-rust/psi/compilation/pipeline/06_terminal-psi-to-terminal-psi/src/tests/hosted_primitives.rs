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
        "use omega::language::core::Never; use omega::language::core::ProcessExit; use omega::language::core::ProcessExitPrimitive; machine main() reaches ProcessExit crashes none terminates { ProcessExitPrimitive(7); }",
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

    let primitive_semantic = module.identities.text(primitive.identity).unwrap();
    let counterfeit_semantic =
        primitive_semantic.replacen("ProcessExitPrimitive", "ProcessExitPrimitivf", 1);
    assert_ne!(primitive_semantic, counterfeit_semantic);

    // Equal-length substitutions change only the identity text, preserving its
    // coordinates and every copied proof header. Rejection must authenticate
    // the exact catalog declaration, rather than notice a detached header.
    for (original, replacement) in [
        ("ProcessExit::exit_process", "ProcessExit::fake_process"),
        (primitive_semantic, counterfeit_semantic.as_str()),
        (
            r"path\(ProcessExitPrimitive\)",
            r"path\(ProcessExitPrimitivf\)",
        ),
        ("ProcessExitPrimitive::entry", "ProcessExitPrimitive::other"),
    ] {
        let mut relabelled = closed.semantic_bytes().to_vec();
        assert_eq!(original.len(), replacement.len());
        let mut substitutions = 0;
        for offset in 0..=relabelled.len() - original.len() {
            if &relabelled[offset..offset + original.len()] == original.as_bytes() {
                relabelled[offset..offset + original.len()].copy_from_slice(replacement.as_bytes());
                substitutions += 1;
            }
        }
        assert!(substitutions > 0, "the actual artifact retains {original}");
        let error = match terminal_codec::ClosedTerminalModule::from_semantic_bytes(&relabelled) {
            Err(error) => error,
            Ok(_) => panic!("a relabelled catalog declaration was accepted: {original}"),
        };
        assert!(
            format!("{error:?}").contains(
                "hosted primitive differs from its exact Core declaration and requirement"
            ),
            "{original}: {error:?}"
        );
    }
    let mut legacy = closed.semantic_bytes().to_vec();
    legacy[8..10].copy_from_slice(&270u16.to_le_bytes());
    assert!(matches!(
        terminal_codec::ClosedTerminalModule::from_semantic_bytes(&legacy),
        Err(terminal_codec::CodecError::UnsupportedFormatMarker(270))
    ));

    // Local boundary numbers are not declaration identity across modules.
    // These comparison controls move only the catalog coordinate; they do not
    // claim that this partial relocation is an executable module rewrite.
    let mut relocated = module.clone();
    let moved_id = semantic_vocabulary::BoundaryMachineId::new(99).unwrap();
    relocated
        .boundary_machines
        .iter_mut()
        .find(|row| row.id == requirement)
        .unwrap()
        .id = moved_id;
    relocated.boundary_machines.sort_by_key(|row| row.id);
    let moved = relocated
        .boundary_machines
        .iter_mut()
        .find(|row| row.id == primitive.id)
        .unwrap();
    moved.declaration.as_mut().unwrap().execution =
        terminal_psi::CheckedBoundaryExecution::HostedPrimitive {
            operation: language_core::sealed_intrinsics::HostedOperation::ProcessExit,
            requirement: moved_id,
        };
    let actual = primitive.declaration.as_ref().unwrap();
    let moved = relocated
        .boundary_machine(primitive.id)
        .unwrap()
        .declaration
        .as_ref()
        .unwrap();
    assert!(actual.same_identity_as(module, moved, &relocated));
    let mut unrelated = relocated.clone();
    unrelated
        .boundary_machines
        .iter_mut()
        .find(|row| row.id == moved_id)
        .unwrap()
        .declaration
        .as_mut()
        .unwrap()
        .source
        .contract = [42; 32];
    let moved = unrelated
        .boundary_machine(primitive.id)
        .unwrap()
        .declaration
        .as_ref()
        .unwrap();
    assert!(!actual.same_identity_as(module, moved, &unrelated));
    let mut absent = relocated.clone();
    absent.boundary_machines.retain(|row| row.id != moved_id);
    let moved = absent
        .boundary_machine(primitive.id)
        .unwrap()
        .declaration
        .as_ref()
        .unwrap();
    assert!(!actual.same_identity_as(module, moved, &absent));

    for mutation in 0..7 {
        let mut changed = module.clone();
        if mutation == 5 {
            changed
                .boundary_machines
                .iter_mut()
                .find(|row| row.id == requirement)
                .unwrap()
                .scalar_requires
                .push(Proposition::Falsehood);
        }
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
            5 => {}
            6 => {
                boundary.result_lifetimes = match canonical.result_lifetimes {
                    None => Some(Vec::new()),
                    Some(_) => None,
                };
            }
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
        "use omega::language::core::Never; use omega::language::core::ProcessExit; public boundary machine ProcessExitPrimitive(return_code: i32) -> Never satisfies ProcessExit::exit_process reaches ProcessExit crashes none terminates; machine main() reaches ProcessExit crashes none terminates { ProcessExitPrimitive(7); }",
    );
    let verdicts = judge_program(&typed);
    // The declaration is refused, and so is `main`, which calls it.
    let [
        Ok(RouteVerdict::FallBack(refusal)),
        Ok(RouteVerdict::FallBack(caller)),
    ] = verdicts.as_slice()
    else {
        panic!("the ordinary copied declaration must have no compiler supply: {verdicts:?}");
    };
    assert_eq!(refusal.reason, RefusalReason::Unsupported);
    assert!(
        refusal
            .detail
            .contains("supply mode other than a checked body"),
        "{refusal:?}"
    );
    assert!(
        caller.detail.contains(
            "a call of a machine whose signature is refused above (`ProcessExitPrimitive`)"
        ),
        "{caller:?}"
    );
}
