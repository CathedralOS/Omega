//! An inferred binding keeps the initializer's actual type and ownership.
use super::*;

const SOURCE: &str = include_str!(
    "../../../../../../../../../../tests/fixtures/run/ownership/inferred_generic_record/main.omg"
);
const REPEATED: &str = include_str!(
    "../../../../../../../../../../tests/fixtures/fail/ownership/inferred_generic_record_reuse/main.omg"
);

fn judged(text: &str) -> Vec<Result<RouteVerdict, crate::checking::CompilationRefusal>> {
    let (typed, _) = crate::preparation::prepare_typed_trees(
        super::boundary_calls::typed(text),
        &crate::preparation::PreparationRequest::settled(),
    )
    .expect("the actual initializer must reach checked preparation");
    crate::checking::judge_compilation(
        crate::lowering::prepare_compilation(
            &typed,
            crate::lowering::Compilation {
                calling: None,
                providers: None,
                declaration_metadata: &[],
                entries: &[],
                subjects: &[None],
                purpose: declaration_contracts::source_identity::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        )
        .program,
    )
}

#[test]
fn inferred_locals_keep_actual_generic_results_and_linear_ownership() {
    let accepted = judged(SOURCE);
    for verdict in &accepted {
        println!("inferred generic result: {verdict:?}");
    }
    assert!(!accepted.is_empty());
    assert!(
        accepted
            .iter()
            .all(|verdict| matches!(verdict, Ok(RouteVerdict::Accepted(_)))),
        "{accepted:?}"
    );

    let rejected = judged(REPEATED);
    for verdict in &rejected {
        println!("repeated inferred result: {verdict:?}");
    }
    assert!(
        rejected.iter().any(|verdict| match verdict {
            Err(crate::checking::CompilationRefusal::Elaboration(refusal))
            | Ok(RouteVerdict::FallBack(refusal)) => refusal
                .detail
                .contains("decomposition source is not a complete live value"),
            _ => false,
        }),
        "{rejected:?}"
    );
}
