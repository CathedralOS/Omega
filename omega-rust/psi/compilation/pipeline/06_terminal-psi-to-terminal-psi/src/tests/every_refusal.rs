//! A compilation reports every machine the checker cannot read yet, not only
//! the first: reading records each refused declaration and body and goes on
//! (`typed_trees_to_terminal_psi::read::Program::keep`), and the verdict list
//! holds one refusal for each. Each refusal must be the one that machine
//! meets when read by itself, so going on never invents a refusal.

use super::*;

/// Three helpers whose bodies each hold a float literal, which the reader
/// refuses, and an entry that calls all three.
const THREE_HELPERS: &str = r#"
machine first() -> u32 { let x: f64 = 1.5; 1 }
machine second() -> u32 { let y: f64 = 2.5; 2 }
machine third() -> u32 { let z: f64 = 3.5; 3 }
machine entry() -> u32 {
    let a: u32 = first();
    let b: u32 = second();
    let c: u32 = third();
    a
}
"#;

/// The entry closure's verdicts, as `judge_compilation` lists them.
fn verdicts(text: &str) -> Vec<Result<RouteVerdict, crate::CompilationRefusal>> {
    let typed = boundary_calls::typed(text);
    let entry = boundary_calls::entry(&typed);
    crate::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            &typed,
            typed_trees_to_terminal_psi::Compilation {
                declaration_metadata: &[],
                entries: &[entry],
                subjects: &[],
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

/// The detail of each verdict that refuses a construct as not supported.
fn not_supported(verdicts: &[Result<RouteVerdict, crate::CompilationRefusal>]) -> Vec<String> {
    verdicts
        .iter()
        .filter_map(|verdict| match verdict {
            Ok(RouteVerdict::FallBack(refused)) => Some(refused.detail.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn every_refused_body_is_its_own_verdict() {
    let refused = not_supported(&verdicts(THREE_HELPERS));
    assert_eq!(refused.len(), 3, "{refused:#?}");
    for (detail, helper) in refused.iter().zip(["first", "second", "third"]) {
        assert!(
            detail.contains("float literal")
                && detail.ends_with(&format!("while reading the body of `{helper}`")),
            "{detail}"
        );
    }
}

/// Going on after a refusal reads each later machine exactly as it would be
/// read alone: the refusal of `second` in the program with all three is the
/// refusal of the program whose only problem is `second`.
#[test]
fn going_on_after_a_refusal_reports_what_each_machine_meets_alone() {
    let together = not_supported(&verdicts(THREE_HELPERS));
    let alone = not_supported(&verdicts(
        &THREE_HELPERS
            .replace("let x: f64 = 1.5; ", "")
            .replace("let z: f64 = 3.5; ", ""),
    ));
    assert_eq!(alone.len(), 1, "{alone:#?}");
    assert_eq!(together[1], alone[0]);
}

/// A machine whose signature is refused is read once. Each later caller is
/// refused as depending on it, by name, instead of reading and refusing the
/// same signature again.
#[test]
fn callers_of_a_refused_signature_name_the_refused_machine() {
    let refused = not_supported(&verdicts(
        r#"
machine scaled(value: f64) -> u32 requires value > 1.5 { 1 }
machine left() -> u32 { scaled(2.0) }
machine right() -> u32 { scaled(3.0) }
machine entry() -> u32 {
    let a: u32 = left();
    let b: u32 = right();
    a
}
"#,
    ));
    assert_eq!(refused.len(), 2, "{refused:#?}");
    assert!(
        refused[0].contains("while reading the signature of `scaled`")
            && refused[0].ends_with("while reading the body of `left`"),
        "{}",
        refused[0]
    );
    assert!(
        refused[1].contains("a call of a machine whose signature is refused above (`scaled`)")
            && refused[1].ends_with("while reading the body of `right`"),
        "{}",
        refused[1]
    );
}

/// A direct reading still refuses at the first machine it cannot read, so a
/// caller that asks for one closure gets the refusal it always got.
#[test]
fn a_direct_reading_still_refuses_at_the_first() {
    let typed = boundary_calls::typed(THREE_HELPERS);
    let entry = boundary_calls::entry(&typed);
    let frozen: typed_trees::FrozenTypedTrees = typed.into();
    let refused = typed_trees_to_terminal_psi::elaborate_for(
        &frozen,
        typed_trees_to_terminal_psi::Compilation {
            declaration_metadata: &[],
            entries: &[entry],
            subjects: &[],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        },
        terminal_codec::TerminalPublication::Invocation,
    )
    .unwrap_err();
    assert!(
        refused
            .detail
            .ends_with("while reading the body of `first`"),
        "{refused:?}"
    );
}

/// Lowering, too, goes on past a machine it cannot plan yet. Both machines
/// read; the storage plan that lowering makes for each (the reference ends
/// of `reference_ends::install`) refuses each as not supported. Both
/// refusals are verdicts, in machine order, one per machine.
#[test]
fn every_machine_lowering_cannot_plan_is_its_own_verdict() {
    let take = |name: &str| {
        format!(
            "machine {name}(value: Inner in Stamped, next: Inner in Stamped) -> Inner in Stamped {{\n\
             let mut owner: Inner in Stamped = value;\n\
             let local: &mut Inner in Stamped = &mut owner;\n\
             let taken: Inner in Stamped = move local;\n\
             local = next;\n\
             taken\n}}\n"
        )
    };
    let source = format!(
        "data Inner {{ value: u32; }}\n\
         domain Inner::Stamped established by stamp;\n\
         machine stamp(value: Inner) -> Inner in Stamped {{ value as +Stamped }}\n\
         {}{}",
        take("take"),
        take("take_again"),
    );
    let typed = boundary_calls::typed(&source);
    let verdicts = crate::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            &typed,
            typed_trees_to_terminal_psi::Compilation {
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
    );
    let refused: Vec<_> = verdicts
        .iter()
        .filter_map(|verdict| match verdict {
            Ok(RouteVerdict::FallBack(refused)) => Some(refused),
            _ => None,
        })
        .collect();
    assert_eq!(refused.len(), 2, "{verdicts:#?}");
    assert!(refused.iter().all(|refused| refused.detail.contains(
        "whole-referent restoration requires unqualified nonlinear owned storage without claims"
    )));
    assert!(refused[0].machine < refused[1].machine, "{refused:#?}");
}
