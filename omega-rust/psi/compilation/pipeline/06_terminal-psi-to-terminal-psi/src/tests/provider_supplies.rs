//! A supplied provider is judged against its exact requirement. The main
//! controls nominate a named implementation and check both the runtime
//! closure and its separate refinement witness.

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
Native satisfies Service {{
    machine go(value: u32) -> u32 {supplied} {{ {body} }}
}}
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
        .chain(
            typed
                .conformances()
                .iter()
                .map(|declared| (typed.symbols.name(declared.symbol), declared.symbol)),
        )
        .find(|(declared, _)| *declared == name)
        .unwrap()
        .1
}

/// The machines of the accepted compilation that claims `Native` as the
/// supply of `slot`, or what the first part that does not accept says.
fn supplied(text: &str, slot: &str) -> Result<Vec<String>, String> {
    supplied_with_subjects(text, slot, &[])
}

fn supplied_with_subjects(
    text: &str,
    slot: &str,
    subjects: &[Option<semantic_vocabulary::PackageKeyIdentity>],
) -> Result<Vec<String>, String> {
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
    let supplies = [typed_trees_to_terminal_psi::Supply {
        slot,
        provider: declared(&typed, "Native"),
    }];
    let verdicts = crate::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            &typed,
            typed_trees_to_terminal_psi::Compilation {
                calling: None,
                providers: None,

                declaration_metadata: &[],
                entries: &[entry(&typed)],
                subjects,
                purpose: source::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                supplies: &supplies,
            },
            terminal_codec::TerminalPublication::Portable,
        )
        .program,
    );
    let mut closure = None;
    for (part, verdict) in verdicts.into_iter().enumerate() {
        match verdict {
            Ok(RouteVerdict::Accepted(accepted)) => {
                if part == 0 {
                    closure = Some(accepted);
                }
            }
            Ok(RouteVerdict::Rejected {
                formations,
                violations,
                unproved,
                unestablished,
                conflicts,
                exceeded,
            }) => {
                let messages = formations
                    .iter()
                    .map(|entry| entry.reject_diagnostic().message)
                    .chain(violations.iter().map(crate::RuleViolation::message))
                    .chain(unproved.iter().map(crate::UnprovedObligation::message))
                    .chain(unestablished.iter().map(crate::UnestablishedUse::message))
                    .chain(conflicts.iter().map(crate::ConflictingUse::message))
                    .chain(exceeded.iter().map(crate::ExceededReach::message))
                    .collect::<Vec<_>>();
                assert!(
                    !messages.is_empty(),
                    "part {part} rejection names no failure"
                );
                return Err(format!("part {part} rejected: {}", messages.join("\n")));
            }
            Ok(RouteVerdict::Incomplete(_)) => return Err(format!("part {part} incomplete")),
            Ok(RouteVerdict::FallBack(refusal)) => {
                return Err(format!("part {part} not supported: {}", refusal.detail));
            }
            Err(refusal) => return Err(format!("part {part} refused: {refusal:?}")),
        }
    }
    let closure = closure.expect("a successful supply includes its runtime closure");
    closure
        .publish()
        .expect("an accepted supply can publish its runtime module");
    Ok(closure
        .elaboration
        .machines
        .iter()
        .map(|(_, symbol)| typed.symbols.display_path(*symbol, "::"))
        .collect())
}

const CONTRACT: &str = "requires value <= 10 ensures result <= 11 reaches none crashes none";

#[test]
fn multiple_provider_declarations_publish_in_portable_order() {
    // All three maps in the kept program are checked declarations. Selecting
    // one must not stop the other exact candidates from being published.
    let typed = typed(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../tests/fixtures/run/providers/target_selected_implementation/main.omg"
    )));
    let supplies = [typed_trees_to_terminal_psi::Supply {
        slot: declared(&typed, "TargetConstants"),
        provider: declared(&typed, "WindowsConstants"),
    }];
    let entries = [super::build_purpose::symbol(&typed, "main")];
    let subjects = [Some(super::build_purpose::package())];
    let prepared = typed_trees_to_terminal_psi::prepare_compilation(
        &typed,
        typed_trees_to_terminal_psi::Compilation {
            calling: None,
            providers: None,
            declaration_metadata: &[],
            entries: &entries,
            subjects: &subjects,
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &supplies,
        },
        terminal_codec::TerminalPublication::Portable,
    );
    let mut accepted = None;
    for (part, verdict) in crate::judge_compilation(prepared.program)
        .into_iter()
        .enumerate()
    {
        let Ok(RouteVerdict::Accepted(closure)) = verdict else {
            panic!("provider part {part} did not accept: {verdict:?}");
        };
        if part == 0 {
            accepted = Some(closure);
        }
    }
    let accepted = accepted.expect("the runtime closure");
    assert_eq!(accepted.elaboration.module.provider_candidates.len(), 6);
    let publication = accepted.publish().expect("all provider rows publish");
    let artifact = publication.artifact;
    let restored = terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
        .expect("the published provider roster decodes");
    assert_eq!(
        restored.semantic_module().provider_candidates,
        accepted.elaboration.module.provider_candidates,
    );
}

#[test]
fn a_data_owned_provider_reports_the_attachment_violation() {
    let text = format!(
        r#"
boundary trait Service {{
    machine go(value: u32) -> u32 {CONTRACT};
}}
data Native {{}}
machine Native::go(value: u32) -> u32 satisfies Service::go {CONTRACT} {{ value + 1 }}
machine entry() {{ let reply: u32 = Service::go(3); }}
"#
    );
    assert_eq!(
        supplied(&text, "Service::go").unwrap_err(),
        "part 0 rejected: a data declaration cannot own a machine; declare an ordinary static machine"
    );
}

/// An inherited-only schema must not become an empty accepted supply merely
/// because its own declaration lists no machine signatures.
#[test]
fn an_unserved_inherited_whole_trait_supply_is_not_vacuously_accepted() {
    let text = "boundary trait Parent { machine go() reaches none crashes none; }
        boundary trait Service: Parent {}
        Native satisfies Parent { machine go() reaches none crashes none {} }
        machine entry() {}";
    let error = supplied(text, "Service").unwrap_err();
    assert!(
        error.contains("whole-trait supply with parameters, parents or refinements"),
        "{error}"
    );
}

/// A provider can keep its own ceilings while exceeding the contract it
/// supplies, on either axis, service reach or callback invocation.
///
/// Where its body makes the call that exceeds the requirement, the runtime
/// body is what fails: it "is checked as a conformer of that requirement,
/// under the invocation context the requirement supplies"
/// (conformances.md, "Binding a machine to a requirement"), and "a derived
/// reach that exceeds its ceiling rejects at the call that contributes the
/// excess" (effects.md). That body is judged in the runtime module, part 0,
/// before the conformance witness.
///
/// Where the provider only declares the wider ceiling and its body makes no
/// such call, the body keeps the requirement's ceilings and the production
/// conformance witness, part 1, rejects the clause that does not refine the
/// requirement.
#[test]
fn conformance_witness_checks_reach_and_invocation_ceilings() {
    let program = |required: &str, body: &str| {
        format!(
            r#"
boundary trait Output {{}}
boundary trait Handler {{}}
boundary trait Console {{
    machine write(value: u32) reaches Output crashes none invokes Handler;
}}
boundary trait Service {{
    machine go(value: u32) -> u32 {required};
}}
Native satisfies Service {{
    machine go(value: u32) -> u32 reaches Output crashes none invokes Handler {{
        {body}
        value
    }}
}}
machine entry() {{ let reply: u32 = Service::go(3); }}
"#
        )
    };
    let calling = "Console::write(value);";
    supplied(
        &program("reaches Output crashes none invokes Handler", calling),
        "Service::go",
    )
    .unwrap();
    for (required, axis) in [
        ("reaches none crashes none invokes Handler", "reach"),
        ("reaches Output crashes none", "synchronous invocation"),
    ] {
        let rejection = supplied(&program(required, calling), "Service::go").unwrap_err();
        // The selected provider may be checked in the main closure before its
        // separate refinement witness. Pin the rejected operation, not that order.
        assert!(
            rejection.contains("rejected: machine `Native::go`"),
            "{rejection}"
        );
        assert!(rejection.contains("Console::write"), "{rejection}");
        assert!(
            rejection.contains(if axis == "reach" { "Output" } else { "Handler" }),
            "{rejection}"
        );
        assert!(rejection.contains(axis), "{rejection}");
        let declared = supplied(&program(required, ""), "Service::go").unwrap_err();
        assert!(declared.starts_with("part 1 rejected"), "{declared}");
        assert!(declared.contains(axis), "{declared}");
    }
}

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
        typed_trees_to_terminal_psi::prepare_compilation(
            &typed,
            typed_trees_to_terminal_psi::Compilation {
                calling: None,
                providers: None,

                declaration_metadata: &[],
                entries: &[entry(&typed)],
                subjects: &[],
                purpose: source::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Portable,
        )
        .program,
    );
    let [Ok(RouteVerdict::Accepted(accepted))] = verdicts.as_slice() else {
        panic!("the entry closure alone is accepted: {verdicts:?}");
    };
    assert_eq!(accepted.elaboration.machines.len(), 1);
}

fn referenced_provider(contract: &str) -> String {
    format!(
        r#"
boundary trait Service {{
    machine go(value: u32) -> u32 {CONTRACT};
}}
machine provider_go(value: u32) -> u32 satisfies Service::go {contract} {{ value + 1 }}
Native satisfies Service {{
    Service::go = provider_go;
}}
machine entry() {{ let reply: u32 = Service::go(3); }}
"#
    )
}

#[test]
fn a_provider_supply_judges_the_existing_machine_selected_by_its_row() {
    let exact = referenced_provider(CONTRACT);
    let stricter =
        referenced_provider("requires value <= 5 ensures result <= 11 reaches none crashes none");
    let subjects = [Some(super::build_purpose::package())];
    for slot in ["Service::go", "Service"] {
        let machines = supplied_with_subjects(&exact, slot, &subjects).unwrap();
        assert_eq!(machines, ["entry", "provider_go"]);
        let rejected = supplied_with_subjects(&stricter, slot, &subjects).unwrap_err();
        assert!(rejected.starts_with("part 1 rejected:"), "{rejected}");
        assert!(rejected.contains("value <= 5"), "{rejected}");
    }
}

#[test]
fn a_named_provider_selection_needs_no_edge_on_its_machine() {
    let without_edge =
        |contract| referenced_provider(contract).replacen(" satisfies Service::go ", " ", 1);
    let plain = without_edge(CONTRACT);
    let stricter =
        without_edge("requires value <= 5 ensures result <= 11 reaches none crashes none");
    for slot in ["Service::go", "Service"] {
        assert_eq!(supplied(&plain, slot).unwrap(), ["entry", "provider_go"]);
        let rejection = supplied(&stricter, slot).unwrap_err();
        assert!(rejection.starts_with("part 1 rejected:"), "{rejection}");
        assert!(rejection.contains("value <= 5"), "{rejection}");
    }

    // The selected route limits its input to 5. A direct call still sees
    // the plain machine's own contract, which accepts input 7.
    let narrowed = plain.replacen(
        CONTRACT,
        "requires value <= 5 ensures result <= 11 reaches none crashes none",
        1,
    );
    let direct = narrowed.replace("Service::go(3)", "provider_go(7)");
    assert_eq!(
        supplied(&direct, "Service").unwrap(),
        ["entry", "provider_go"]
    );
    let routed = narrowed.replace("Service::go(3)", "Service::go(7)");
    let rejection = supplied(&routed, "Service").unwrap_err();
    assert!(
        rejection
            .starts_with("part 0 rejected: cannot prove requires clause 1 of the called machine."),
        "{rejection}"
    );
}

#[test]
fn a_provider_supply_refuses_duplicate_rows_for_one_requirement() {
    let mut typed = typed(&referenced_provider(CONTRACT));
    let implementation = typed
        .tables
        .conformances
        .get_mut(typed.roots.conformances.start());
    let [row] = implementation.rows.as_slice() else {
        panic!("the source selects exactly one requirement row");
    };
    assert_eq!(
        row.source,
        typed_trees::trait_definition::ConformanceRowSource::Reference
    );
    let supply = typed_trees_to_terminal_psi::Supply {
        slot: row.requirement,
        provider: implementation.symbol,
    };
    implementation.rows.push(row.clone());
    assert_eq!(
        supply.realization(&typed, supply.slot).unwrap_err(),
        "provider does not give the supplied requirement exactly one machine"
    );
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
        uncovered.contains("provider does not give the supplied requirement exactly one machine"),
        "{uncovered}"
    );
    // A data type is not a slot.
    let no_slot = supplied(&format!("data NotSlot {{}}\n{text}"), "NotSlot").unwrap_err();
    assert!(
        no_slot.contains("slot that is not a trait or one of its requirements"),
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
        .find(|machine| typed.symbols.display_path(machine.symbol, "::") == "Native::go")
        .unwrap()
        .symbol;
    let supplies = [typed_trees_to_terminal_psi::Supply {
        slot: declared(&typed, "Service"),
        provider: declared(&typed, "Native"),
    }];
    let verdicts = crate::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            &typed,
            typed_trees_to_terminal_psi::Compilation {
                calling: None,
                providers: None,

                declaration_metadata: &[],
                entries: &[member],
                subjects: &[],
                purpose: source::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                supplies: &supplies,
            },
            terminal_codec::TerminalPublication::Portable,
        )
        .program,
    );
    assert!(
        matches!(verdicts.as_slice(), [Ok(RouteVerdict::FallBack(refusal))]
        if refusal.detail.contains("boundary conformer is callable only through its provider slot")),
        "a provider cannot be selected as an ordinary named program entry"
    );
}

#[test]
fn an_ordinary_policy_supply_checks_its_body_without_declaring_a_boundary() {
    let ordinary = |body| {
        program(CONTRACT, "", body)
            .replace("boundary trait Service", "trait Service")
            .replace("Service::go(3)", "Native::go(3)")
    };
    let accepted = supplied(&ordinary("value + 1"), "Service::go").unwrap();
    assert!(
        accepted
            .iter()
            .any(|machine| machine.ends_with("Native::go"))
    );
    let refused = supplied(&ordinary("value + 2"), "Service::go").unwrap_err();
    assert!(refused.contains("rejected"), "{refused}");
}
