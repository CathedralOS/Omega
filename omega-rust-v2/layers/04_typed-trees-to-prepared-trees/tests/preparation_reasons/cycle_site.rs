#[path = "../support/typed_packages.rs"]
mod typed_packages;

use omega_preparation::{CheckedPreparationEvidence, PreparationReason, RefusalCause, prepare_typing_pass_with_cause};
use omega_typed::{TypedCall, TypedTrees};
use omega_typer::TypingProgress;

const LIBRARY: &str = "public machine a(n:u64)->u64 reaches none crashes none terminates {b(n)} machine b(n:u64)->u64 {a(n)}";
const CLIENT: &str = "const SIZE:u64=1u64 + geo\\a(4);";
const MESSAGE: &str =
    "static evaluation cannot admit this invocation: a cyclic static invocation requires a checked termination proof\nrecursive component: a -> b -> a";

fn check_closing_call(alter: Option<fn(&mut TypedCall)>) {
    check_cycle(CLIENT, "geo\\a(4)", alter);
}

fn check_cycle(
    client: &str,
    invocation: &str,
    alter: Option<fn(&mut TypedCall)>,
) {
    let trees = typed_packages::typed(LIBRARY, client);
    let (trees, package, file, range) = if let Some(alter) = alter {
        let resolved = trees.resolved().clone();
        let mut facts = trees.into_facts();
        assert!(facts.library.is_none());
        let closing = facts
            .calls
            .iter_mut()
            .find(|call| call.caller.as_ref().is_some_and(|caller| caller.path == "b") && call.machine.path == "a")
            .unwrap();
        alter(closing);
        (TypedTrees::new(resolved, facts, 0), "client", "client.omg", super::last(client, invocation))
    } else {
        (trees, "geometry", "geometry.omg", super::last(LIBRARY, "a(n)"))
    };
    let input = TypingProgress::Complete(trees);
    let refusal = prepare_typing_pass_with_cause(&input, &[], None, CheckedPreparationEvidence::default()).unwrap_err();
    spec_corpus::assert_preparation_cause!(
        refusal,
        RefusalCause::Preparation(PreparationReason::CyclicInvocation),
        package,
        file,
        range,
        MESSAGE
    );
}

#[test]
fn a_verified_cycle_closes_in_the_defining_package() {
    check_closing_call(None);
}

#[test]
fn a_callback_with_an_unvisited_cycle_uses_the_same_location_rule() {
    let client = "data Step=machine(n:u64)->u64; machine apply<const F:Step>(n:u64)->u64 {F(n)} const SIZE:u64=1u64 + apply::<geo\\a>(4);";
    check_cycle(client, "apply::<geo\\a>(4)", None);
    check_cycle(client, "apply::<geo\\a>(4)", Some(|call| call.node.node = usize::MAX));
}

#[test]
fn an_absent_closing_node_falls_back_to_the_selected_invocation() {
    check_closing_call(Some(|call| call.node.node = usize::MAX));
}

#[test]
fn an_absent_closing_file_falls_back_to_the_selected_invocation() {
    check_closing_call(Some(|call| call.node.file = usize::MAX));
}

#[test]
fn a_closing_node_with_another_source_site_is_not_a_location() {
    check_closing_call(Some(|call| call.site.package = "client".into()));
    check_closing_call(Some(|call| call.site.file = "client.omg".into()));
    check_closing_call(Some(|call| call.site.span = omega_lexer::SourceSpan::new(0, 1)));
}
