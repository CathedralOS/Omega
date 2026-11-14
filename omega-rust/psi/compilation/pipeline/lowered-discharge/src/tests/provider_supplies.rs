//! A chosen provider is judged against the slot it supplies. Each test reads
//! one program from source: a boundary trait, a provider whose machine states
//! `satisfies` for the trait's requirement, and an entry that calls the
//! requirement. The roster names the supply; the verdicts are the closure's
//! and the refinement witness's.

use super::*;

/// The program as the Product source of one package: a portable module
/// needs the declaring package of each boundary requirement.
fn typed(text: &str) -> typed_trees::TypedTrees {
    super::build_purpose::frontier_of(text, "machine build_entry() { }")
}

fn entry(typed: &typed_trees::TypedTrees) -> symbols::SymbolHandle {
    super::build_purpose::symbol(typed, "entry")
}

/// One requirement and one provider machine, each with the clauses given.
fn program(required: &str, supplied: &str, body: &str) -> String {
    format!(
        r#"
boundary trait Output {{}}
boundary trait Other {{
    machine emit(value: u32) reaches Output crashes none;
}}
boundary trait Service {{
    machine go(value: u32) -> u32 {required};
}}
data Native {{}}
machine Native::go(value: u32) -> u32 satisfies Service::go {supplied} {{ {body} }}
machine entry() {{
    let reply: u32 = Service::go(3);
}}
"#
    )
}

fn declared(typed: &typed_trees::TypedTrees, name: &str) -> symbols::SymbolHandle {
    typed
        .traits()
        .iter()
        .map(|declared| (declared.name.as_str(), declared.symbol))
        .chain(
            typed
                .data_definitions()
                .iter()
                .map(|declared| (declared.name.as_str(), declared.symbol)),
        )
        .find(|(declared, _)| *declared == name)
        .unwrap()
        .1
}

/// The machines of the accepted compilation that claims `Native` as the
/// supply of `slot`, or what the first part that does not accept says.
fn supplied(text: &str, slot: &str) -> Result<Vec<String>, String> {
    let typed = typed(text);
    let slot = match slot {
        "Service::go" => {
            typed
                .trait_machine_signatures
                .iter()
                .find(|(_, signature)| signature.name.as_str() == "go")
                .unwrap()
                .1
                .symbol
        }
        whole => declared(&typed, whole),
    };
    let supplies = [structure_elaboration::Supply {
        slot,
        provider: declared(&typed, "Native"),
    }];
    let verdicts = crate::judge_compilation(
        &typed,
        structure_elaboration::Compilation {
            entries: &[entry(&typed)],
            subjects: &[],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &supplies,
        },
    );
    let mut machines = Vec::new();
    for (part, verdict) in verdicts.into_iter().enumerate() {
        match verdict {
            Ok(RouteVerdict::Accepted(accepted)) => {
                if part == 0 {
                    machines = accepted
                        .elaboration
                        .machines
                        .iter()
                        .map(|(_, symbol)| typed.symbols.display_path(*symbol, "::"))
                        .collect();
                }
            }
            Ok(RouteVerdict::Rejected {
                unproved, exceeded, ..
            }) => {
                return Err(unproved
                    .iter()
                    .map(|entry| entry.reject_diagnostic().message)
                    .chain(
                        exceeded
                            .iter()
                            .map(|entry| entry.reject_diagnostic().message),
                    )
                    .next()
                    .map(|message| format!("part {part} rejected: {message}"))
                    .unwrap_or_else(|| format!("part {part} rejected")));
            }
            Ok(RouteVerdict::Incomplete(_)) => return Err(format!("part {part} incomplete")),
            Ok(RouteVerdict::FallBack(refusal)) => {
                return Err(format!("part {part} not supported: {}", refusal.detail));
            }
            Err(refusal) => return Err(format!("part {part} refused: {refusal:?}")),
        }
    }
    Ok(machines)
}

const CONTRACT: &str = "requires value <= 10 ensures result <= 11 reaches none crashes none";

/// The provider's machine is a root of the compilation beside the entry's
/// closure, and the compilation is accepted with its refinement proved,
/// whether the supply names the requirement or the whole trait.
#[test]
fn a_provider_that_refines_its_requirement_is_judged_as_a_root() {
    let text = program(CONTRACT, CONTRACT, "value + 1");
    for slot in ["Service::go", "Service"] {
        let machines = supplied(&text, slot).unwrap();
        assert!(
            machines
                .iter()
                .any(|machine| machine.ends_with("Native::go")),
            "{slot}: {machines:?}"
        );
    }
    // Without the supply the provider is no part of the compilation.
    let typed = typed(&text);
    let verdicts = crate::judge_compilation(
        &typed,
        structure_elaboration::Compilation {
            entries: &[entry(&typed)],
            subjects: &[],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        },
    );
    let [Ok(RouteVerdict::Accepted(accepted))] = verdicts.as_slice() else {
        panic!("the entry closure alone is accepted: {verdicts:?}");
    };
    assert_eq!(accepted.elaboration.machines.len(), 1);
}

/// Each twin changes one clause of the provider or its body and leaves the
/// requirement alone. The entry's closure is the same accepted program in
/// every one; what fails is the provider.
#[test]
fn a_provider_that_does_not_refine_its_requirement_is_not_accepted() {
    // It demands more of its caller than the requirement lets it.
    let stricter = supplied(
        &program(
            CONTRACT,
            "requires value <= 5 ensures result <= 11 reaches none crashes none",
            "value + 1",
        ),
        "Service::go",
    )
    .unwrap_err();
    assert!(stricter.starts_with("part 1 rejected"), "{stricter}");
    // It promises less than the requirement promises its caller.
    let weaker = supplied(
        &program(
            CONTRACT,
            "requires value <= 10 ensures result <= 12 reaches none crashes none",
            "value + 2",
        ),
        "Service::go",
    )
    .unwrap_err();
    // The runtime body itself now owes the requirement's advertised
    // result <= 11, so it fails before its separate refinement witness.
    assert!(weaker.starts_with("part 0 rejected"), "{weaker}");
    assert!(weaker.contains("ensures contract"), "{weaker}");
    assert!(weaker.contains("result <= 11"), "{weaker}");
    // Its own body does not keep its own contract: the provider is judged
    // as a root, not taken at its word.
    let broken = supplied(&program(CONTRACT, CONTRACT, "value + 2"), "Service::go").unwrap_err();
    assert!(broken.starts_with("part 0 rejected"), "{broken}");
    // It reaches a service the requirement's `reaches none` excludes.
    let reaching = supplied(
        &program(
            CONTRACT,
            "requires value <= 10 ensures result <= 11 reaches Output crashes none",
            "Other::emit(value); value + 1",
        ),
        "Service::go",
    )
    .unwrap_err();
    assert!(reaching.contains("reach"), "{reaching}");
    // It may crash where the requirement says `crashes none`.
    let crashing = supplied(
        &program(
            CONTRACT,
            "requires value <= 10 ensures result <= 11 reaches none crashes Trap",
            "value + 1",
        ),
        "Service::go",
    )
    .unwrap_err();
    assert!(crashing.contains("crash"), "{crashing}");
}

/// A supply the route cannot pair with exactly one machine per requirement
/// is refused by name, never accepted unjudged.
#[test]
fn a_supply_without_one_machine_per_requirement_is_refused_by_name() {
    let text = program(CONTRACT, CONTRACT, "value + 1");
    // `Other` has a requirement `Native` gives no machine.
    let uncovered = supplied(&text, "Other").unwrap_err();
    assert!(
        uncovered
            .contains("provider that does not give a supplied requirement exactly one machine"),
        "{uncovered}"
    );
    // A data type is not a slot.
    let no_slot = supplied(&text, "Native").unwrap_err();
    assert!(
        no_slot.contains("slot that is not a boundary trait or one of its requirements"),
        "{no_slot}"
    );
}

#[test]
fn a_supplied_body_has_no_cached_or_selected_named_entry() {
    let text = program(CONTRACT, CONTRACT, "value + 1");
    // Supply roots are read before entry bodies. The provider is already
    // cached when this otherwise valid call tries to use its own name.
    let named = text.replace("Service::go(3)", "Native::go(3)");
    let refusal = supplied(&named, "Service").expect_err("a supplied body has no named entry");
    assert!(
        refusal.contains("boundary conformer is callable only through its provider slot"),
        "{refusal}"
    );

    let typed = typed(&text);
    let member = typed
        .machines()
        .iter()
        .find(|machine| !machine.satisfies.is_empty())
        .unwrap()
        .symbol;
    let supplies = [structure_elaboration::Supply {
        slot: declared(&typed, "Service"),
        provider: declared(&typed, "Native"),
    }];
    let verdicts = crate::judge_compilation(
        &typed,
        structure_elaboration::Compilation {
            entries: &[member],
            subjects: &[],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &supplies,
        },
    );
    assert!(
        matches!(verdicts.as_slice(), [Ok(RouteVerdict::FallBack(refusal))]
        if refusal.detail.contains("boundary conformer is callable only through its provider slot")),
        "a provider cannot be selected as an ordinary named program entry"
    );
}
