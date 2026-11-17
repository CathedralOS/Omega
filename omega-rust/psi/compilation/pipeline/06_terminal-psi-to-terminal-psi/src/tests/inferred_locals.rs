//! An inferred binding keeps the initializer's actual type and ownership.
use super::*;

const SOURCE: &str = r#"
data Ticket [linear] { number: u64; }
data Envelope<T> { item: T; }
machine identity<T>(value: T) -> T { move value }
machine main() -> u64 {
    let ticket = Ticket { number: 29u64 };
    let envelope: Envelope<Ticket> = Envelope { item: move ticket };
    let wrapped = identity<Envelope<Ticket>>(move envelope);
    let { item } = move wrapped;
    let { number } = move item;
    number
}
"#;

fn judged(text: &str) -> Vec<Result<RouteVerdict, crate::CompilationRefusal>> {
    let (typed, _) = typed_trees_to_typed_trees::prepare_typed_trees(
        super::boundary_calls::typed(text),
        &typed_trees_to_typed_trees::PreparationRequest::settled(),
    )
    .expect("the actual initializer must reach checked preparation");
    crate::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            &typed,
            typed_trees_to_terminal_psi::Compilation {
                calling: None,
                providers: None,
                declaration_metadata: &[],
                entries: &[],
                subjects: &[None],
                purpose: source::DependencyScope::Product,
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

    let repeated = SOURCE.replace(
        "let { item } = move wrapped;",
        "let { item } = move wrapped; let { item as again } = move wrapped; let { number as ignored } = move again;",
    );
    let rejected = judged(&repeated);
    for verdict in &rejected {
        println!("repeated inferred result: {verdict:?}");
    }
    assert!(
        rejected.iter().any(|verdict| match verdict {
            Err(crate::CompilationRefusal::Elaboration(refusal))
            | Ok(RouteVerdict::FallBack(refusal)) => refusal
                .detail
                .contains("decomposition source is not a complete live value",),
            _ => false,
        }),
        "{rejected:?}"
    );
}
