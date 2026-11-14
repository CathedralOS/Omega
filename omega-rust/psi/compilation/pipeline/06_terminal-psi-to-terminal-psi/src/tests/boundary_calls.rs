//! Interface contracts must survive ordinary source preparation and the
//! actual call, including its borrowed inputs. These tests do not install a
//! provider or claim native execution.

use super::*;
use std::sync::Arc;

pub(super) fn typed(text: &str) -> typed_trees::TypedTrees {
    let mut sources = source::SourceMap::default();
    let source = sources
        .add("boundary_calls/source.omg".into(), text.to_owned())
        .source_id;
    let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source, &tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
            syntax,
            sources: Some(Arc::new(sources)),
            top_level_bindings: Vec::new(),
            retained_base: None,
        },
    )
    .unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

pub(super) fn entry(typed: &typed_trees::TypedTrees) -> symbols::SymbolHandle {
    typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "entry")
        .unwrap()
        .symbol
}

fn invocation(typed: typed_trees::TypedTrees) -> Checked<typed_trees_to_terminal_psi::Elaboration> {
    // Anonymous source belongs to this activation. Do not give its interface
    // a made-up package owner just to obtain a portable declaration identity.
    let entry = entry(&typed);
    let frozen: typed_trees::FrozenTypedTrees = typed.into();
    typed_trees_to_terminal_psi::elaborate_for(
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
}

fn boundary_source(reach: &str, actual: u32) -> String {
    format!(
        r#"
boundary trait Probe {{
    machine read(value: u32) -> u32
    {reach}
    requires value <= 10
    ensures result <= 10
    crashes none;
}}
machine entry() -> u32 crashes none {{ Probe::read({actual}) }}
"#
    )
}

fn boundary(reach: &str, actual: u32) -> Checked<typed_trees_to_terminal_psi::Elaboration> {
    let typed = typed(&boundary_source(reach, actual));
    invocation(typed)
}

#[test]
fn a_source_boundary_requires_an_explicit_reach_ceiling() {
    let text = boundary_source("", 7);
    let mut sources = source::SourceMap::default();
    let source = sources
        .add("boundary_calls/omitted.omg".into(), text.clone())
        .source_id;
    let tokens = source_files_to_tokens::Lexer::new(&text)
        .tokenize()
        .unwrap();
    let refusal = match tokens_to_syntax_trees::parse_syntax_trees_with_id(source, &tokens) {
        Ok(_) => panic!("an open boundary reach contract must refuse"),
        Err(refusal) => refusal,
    };
    assert_eq!(
        refusal.message,
        "boundary-trait machine requirement `Probe::read` must declare an explicit `reaches` ceiling; write `reaches none` for empty reach"
    );
    let elaboration = boundary("reaches none", 7).unwrap();
    assert_eq!(elaboration.module.boundary_machines.len(), 1);
    let boundary = &elaboration.module.boundary_machines[0];
    assert!(boundary.owning_service.is_some());
    assert!(boundary.fixed_service_reach.is_empty());
    assert!(boundary.published_service_ceiling.is_empty());
}

#[test]
fn source_boundary_calls_keep_their_actual_input_obligations() {
    assert!(matches!(
        judge(boundary("reaches none", 7)).unwrap(),
        RouteVerdict::Accepted(_)
    ));
    let verdict = judge(boundary("reaches none", 11)).unwrap();
    assert!(
        matches!(verdict, RouteVerdict::Rejected { ref unproved, .. }
        if unproved.iter().any(|row| matches!(row.kind, ObligationKind::CallRequirement { .. }))),
        "{verdict:?}"
    );
}

/// A boundary requirement may take a type parameter of its own. It has no
/// body, so a call that applies it, `Probe::take<Reading>(..)`, is held to
/// the requirement's signature and contract with `Reading` where it named
/// `Item` (generics.md, "Closed applications and canonical identity"). Typed
/// preparation closes the requirement once per application, as it generates
/// one machine per application of a generic machine, so the program is read
/// after that stage.
fn applied_boundary(limit: u32) -> Checked<typed_trees_to_terminal_psi::Elaboration> {
    let typed = typed(&format!(
        r#"
data Reading {{ level: u32; }}
boundary trait Probe {{
    machine take<Item>(item: Item, limit: u32) -> u32
    reaches none
    requires limit <= 10
    ensures result <= 10
    crashes none;
}}
machine entry() -> u32 crashes none {{
    let first: u32 = Probe::take<u32>(7, {limit});
    let room: u32 = 10u32 - first;
    let second: u32 = Probe::take<Reading>(Reading {{ level: 3 }}, 4);
    let third: u32 = Probe::take<u32>(9, 2);
    room
}}
"#
    ));
    let entry = entry(&typed);
    let (frozen, _) = typed_trees_to_typed_trees::prepare_typed_trees(
        typed,
        &typed_trees_to_typed_trees::PreparationRequest::settled(),
    )
    .unwrap();
    typed_trees_to_terminal_psi::elaborate_for(
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
}

#[test]
fn a_boundary_requirement_is_judged_at_the_type_a_call_applies_it_at() {
    let elaboration = applied_boundary(10).unwrap();
    // `take` at `u32` and at `Reading` are two declarations of one
    // requirement; the two calls at `u32` name one of them.
    let boundaries = &elaboration.module.boundary_machines;
    assert_eq!(boundaries.len(), 2);
    let declared: Vec<_> = boundaries
        .iter()
        .map(|boundary| boundary.declaration.as_ref().unwrap())
        .collect();
    assert!(declared.iter().all(|declared| {
        declared.method_arguments.len() == 1 && declared.source.specialization.is_some()
    }));
    assert_ne!(
        declared[0]
            .source
            .specialization
            .as_ref()
            .unwrap()
            .commitment,
        declared[1]
            .source
            .specialization
            .as_ref()
            .unwrap()
            .commitment
    );
    // What the requirement guarantees at `u32` keeps `10 - first` exact.
    assert!(matches!(
        judge(Ok(elaboration)).unwrap(),
        RouteVerdict::Accepted(_)
    ));
    // The twin differs only in one argument: `limit` of 11 breaks
    // `limit <= 10`.
    let verdict = judge(applied_boundary(11)).unwrap();
    assert!(
        matches!(verdict, RouteVerdict::Rejected { ref unproved, .. }
        if unproved.iter().any(|row| matches!(row.kind, ObligationKind::CallRequirement { .. }))),
        "{verdict:?}"
    );
}

#[test]
fn an_unserved_operational_contract_cannot_be_added_to_a_lowered_boundary() {
    for blocks in [false, true] {
        let mut elaboration = boundary("reaches none", 7).unwrap();
        let declaration = &mut elaboration.module.boundary_machines[0]
            .declaration
            .as_mut()
            .unwrap()
            .source;
        declaration.published_may_block = blocks;
        declaration.published_may_suspend = !blocks;
        let refusal = discharge(&mut elaboration).unwrap_err();
        assert_eq!(refusal.reason, RefusalReason::Unsupported);
        assert_eq!(
            refusal.detail,
            "boundary call needs its suspension and blocking proof"
        );
    }
}

#[test]
fn source_boundary_effects_still_require_their_invocation_proof() {
    for effect in ["suspends", "blocks"] {
        // The source declares the effect; no mutation of the published
        // metadata supplies it. This ordinary call has no operational proof.
        let mut elaboration = boundary(&format!("reaches none {effect}"), 7)
            .expect("the declared boundary contract reaches invocation checking");
        let published = &elaboration.module.boundary_machines[0]
            .declaration
            .as_ref()
            .unwrap()
            .source;
        assert_eq!(published.published_may_suspend, effect == "suspends");
        assert_eq!(published.published_may_block, effect == "blocks");
        let refusal = discharge(&mut elaboration).unwrap_err();
        assert_eq!(refusal.reason, RefusalReason::Unsupported);
        assert_eq!(
            refusal.detail,
            "boundary call needs its suspension and blocking proof"
        );
    }
}

#[test]
fn source_boundary_crashes_are_not_erased_by_the_call_operand_shape() {
    let call = |actual| {
        let typed = typed(&format!(
            r#"
boundary trait Probe {{
    machine read(value: u32) -> u32
    reaches none
    crashes Trap value == 0;
}}
machine entry() -> u32 crashes none {{ Probe::read({actual}) }}
"#
        ));
        invocation(typed)
    };
    assert!(matches!(judge(call(1)).unwrap(), RouteVerdict::Accepted(_)));
    let verdict = judge(call(0)).unwrap();
    assert!(
        matches!(verdict, RouteVerdict::Rejected { ref unproved, .. }
        if unproved.iter().any(|row| matches!(row.kind, ObligationKind::CrashCoverage { .. }))),
        "{verdict:?}"
    );
}

/// Both total carrier arithmetic and mathematical arithmetic are restated at
/// the call, rather than rejected by a narrower crash-only term reader.
#[test]
fn source_crash_guards_substitute_total_operations_at_the_call() {
    for guard in ["value +% 1 == 0", "embed(value) + 1 == 4294967296"] {
        let call = |actual| {
            invocation(typed(&format!(
                r#"
boundary trait Probe {{
    machine read(value: u32) -> u32
    reaches none
    crashes Trap {guard};
}}
machine entry() -> u32 crashes none {{ Probe::read({actual}) }}
"#
            )))
        };
        let accepted = judge(call(1)).unwrap();
        let RouteVerdict::Accepted(accepted) = accepted else {
            panic!("{guard}: {accepted:?}");
        };
        accepted.publish_invocation().unwrap();
        let rejected = judge(call(u32::MAX)).unwrap();
        assert!(
            matches!(rejected, RouteVerdict::Rejected { ref unproved, .. }
            if unproved.iter().any(|row| matches!(row.kind, ObligationKind::CrashCoverage { .. }))),
            "{guard}: {rejected:?}"
        );
    }
}

/// A boundary requirement has no body to infer a crash ceiling from and no
/// static selection to defer to, so one that writes no `crashes` clause is
/// refused where it is called, and the same call with `crashes none` written
/// is read.
#[test]
fn a_boundary_requirement_without_a_crash_ceiling_is_refused_by_name() {
    let call = |crashes: &str| {
        let typed = typed(&format!(
            r#"
boundary trait Probe {{
    machine read(value: u32) -> u32
    reaches none{crashes};
}}
machine entry() -> u32 {{ Probe::read(1) }}
"#
        ));
        invocation(typed)
    };
    let refusal = call("").unwrap_err();
    assert_eq!(refusal.reason, RefusalReason::Unsupported);
    assert_eq!(
        refusal.detail,
        "call boundary requirement must declare its crash ceiling"
    );
    assert!(call("\n    crashes none").is_ok());
}

fn borrowed_requirement(
    second: &str,
    caller_bound: &str,
) -> typed_trees_to_terminal_psi::Elaboration {
    let typed = typed(&format!(
        r#"
trait Touch {{
    machine write(left: &mut u32, right: &mut u32)
    requires left < 10
    crashes none;
}}
machine entry<Selected satisfies Touch>(left: &mut u32, right: &mut u32)
requires left {caller_bound}
crashes none
{{
    Selected::write(left, {second});
}}
"#
    ));
    typed_trees_to_terminal_psi::elaborate_generic(&typed, entry(&typed)).unwrap()
}

#[test]
fn borrowed_generic_requirements_preserve_input_proofs_and_exclusive_loans() {
    let mut valid = borrowed_requirement("right", "< 10");
    let discharged = discharge(&mut valid).unwrap();
    let entry = discharged.machines.get(id(1)).unwrap();
    assert!(entry.conflicts.is_empty());
    assert!(entry.unestablished().next().is_none());
    assert!(
        entry.verdicts.values().all(Result::is_ok),
        "{:?}",
        entry.verdicts
    );

    let mut missing_input = borrowed_requirement("right", "<= 10");
    let discharged = discharge(&mut missing_input).unwrap();
    assert!(
        discharged
            .machines
            .get(id(1))
            .unwrap()
            .verdicts
            .values()
            .any(Result::is_err)
    );

    let mut alias = borrowed_requirement("left", "< 10");
    let discharged = discharge(&mut alias).unwrap();
    assert!(!discharged.machines.get(id(1)).unwrap().conflicts.is_empty());
}

fn conformer_contracts(member: &str, plain: &str) -> Vec<RouteVerdict> {
    let text = format!(
        r#"
trait Step {{
    machine apply(value: u32) -> u32
    requires value > 0
    ensures result < value
    crashes none;
}}
Previous satisfies Step {{
    {member}
}}
{plain}
machine entry() crashes none {{}}
"#
    );
    let typed = typed(&text);
    let entry = entry(&typed);
    let verdicts = crate::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            &typed,
            typed_trees_to_terminal_psi::Compilation {
                declaration_metadata: &[],
                entries: &[entry],
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
    assert_eq!(verdicts.len(), 2, "{verdicts:?}");
    verdicts.into_iter().map(Result::unwrap).collect()
}

#[test]
fn inline_conformers_inherit_the_requirements_logical_contract() {
    // The member deliberately uses another formal name: inherited clauses
    // refer to the same logical input by position, not by its spelling.
    let verdicts = conformer_contracts(
        "machine apply(input: u32) -> u32 crashes none { input - 1 }",
        "",
    );
    for verdict in verdicts {
        assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
    }
}

#[test]
fn an_omitted_member_guarantee_still_has_to_hold_of_the_actual_body() {
    let verdicts = conformer_contracts(
        "machine apply(input: u32) -> u32 crashes none { input }",
        "",
    );
    assert!(
        verdicts.iter().all(|verdict| matches!(
            verdict,
            RouteVerdict::Accepted(_) | RouteVerdict::Rejected { .. }
        )),
        "{verdicts:?}"
    );
    assert!(
        verdicts.iter().any(|verdict| matches!(
            verdict, RouteVerdict::Rejected { unproved, .. }
            if unproved.iter().any(|row| matches!(row.kind, ObligationKind::NormalGuarantee { .. }))
        )),
        "{verdicts:?}"
    );
}

#[test]
fn a_requirement_does_not_replace_written_inputs_or_donate_them_to_a_selection() {
    for (member, plain) in [
        // The originating requirement admits one, so this omission must
        // not infer the narrower input >= 2 that a private helper could.
        (
            "machine apply(input: u32) -> u32 crashes none { input - 2 }",
            "",
        ),
        (
            "machine apply(input: u32) -> u32 requires true crashes none { input - 1 }",
            "",
        ),
        (
            "Step::apply = previous;",
            "machine previous(input: u32) -> u32 requires true crashes none { input - 1 }",
        ),
    ] {
        let verdicts = conformer_contracts(member, plain);
        assert!(
            verdicts.iter().all(|verdict| matches!(
                verdict,
                RouteVerdict::Accepted(_) | RouteVerdict::Rejected { .. }
            )),
            "{verdicts:?}"
        );
        assert!(verdicts.iter().any(|verdict| matches!(
            verdict, RouteVerdict::Rejected { unproved, .. }
            if unproved.iter().any(|row| matches!(row.kind, ObligationKind::OperationSafety { .. }))
        )), "{verdicts:?}");
    }
}

fn declared_source(contract: &str, body: &str, actual: &str, promised: &str) -> String {
    format!(
        r#"
trait Step {{
    machine apply(value: u32) -> u32
    requires value > 0
    ensures result < value
    crashes none;
}}
machine previous(input: u32) -> u32 satisfies Step::apply
{contract}
crashes none
{{ {body} }}
machine entry() -> u32
{promised}
crashes none
{{ previous({actual}) }}
"#
    )
}

fn declared_verdicts(text: &str) -> Vec<RouteVerdict> {
    declared_verdicts_of(&typed(text))
}

fn declared_verdicts_of(typed: &typed_trees::TypedTrees) -> Vec<RouteVerdict> {
    let verdicts = compilation_verdicts_of(typed);
    assert_eq!(verdicts.len(), 2, "{verdicts:?}");
    verdicts
}

fn compilation_verdicts_of(typed: &typed_trees::TypedTrees) -> Vec<RouteVerdict> {
    let entry = entry(typed);
    let verdicts = crate::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            typed,
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
    );
    verdicts.into_iter().map(Result::unwrap).collect()
}

#[test]
fn a_declared_conformers_name_invokes_its_requirement() {
    let text = declared_source("", "input - 1", "7", "ensures result < 7");
    let mut verdicts = declared_verdicts(&text);
    for verdict in &verdicts {
        assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
    }
    let RouteVerdict::Accepted(accepted) = verdicts.remove(0) else {
        unreachable!()
    };
    let publication = accepted.publish_invocation().unwrap();
    let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    let status = instance
        .invoke(&mut psi_work::WorkMeter::with_budget(1_000))
        .unwrap();
    assert_eq!(
        status,
        terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Scalar(
                terminal_interpreter::TerminalScalarValue::Integer {
                    scalar_type: semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        32
                    )
                    .unwrap(),
                    value: semantic_vocabulary::IntegerValue::Unsigned(6),
                }
            )
        )
    );
}

#[test]
fn a_declared_conformers_written_body_contract_is_independently_owed() {
    for (contract, body, expected) in [("", "input", false), ("requires true", "input - 1", true)] {
        let verdicts = declared_verdicts(&declared_source(contract, body, "7", ""));
        assert!(
            matches!(&verdicts[1], RouteVerdict::Rejected { unproved, .. }
            if unproved.iter().any(|row| if expected {
                matches!(row.kind, ObligationKind::OperationSafety { .. })
            } else {
                matches!(row.kind, ObligationKind::NormalGuarantee { .. })
            })),
            "{verdicts:?}"
        );
    }
}

#[test]
fn a_conformer_has_no_callable_direct_contract_beside_its_requirement() {
    for (actual, promised, input_failure) in [("0", "", true), ("7", "ensures result == 0", false)]
    {
        let verdicts = declared_verdicts(&declared_source(
            "requires true ensures result == 0",
            "0",
            actual,
            promised,
        ));
        assert!(
            matches!(&verdicts[1], RouteVerdict::Accepted(_)),
            "{verdicts:?}"
        );
        assert!(
            matches!(&verdicts[0], RouteVerdict::Rejected { unproved, .. }
            if unproved.iter().any(|row| if input_failure {
                matches!(row.kind, ObligationKind::CallRequirement { .. })
            } else {
                matches!(row.kind, ObligationKind::NormalGuarantee { .. })
            })),
            "{verdicts:?}"
        );
    }
}

#[test]
fn recursive_named_calls_in_a_body_witness_keep_the_requirement_inputs() {
    // A postcondition on a call cycle has no served inference rule. This
    // control isolates the named recursive call's input obligation.
    let text = declared_source("requires true", "previous(0)", "7", "")
        .replace("ensures result < value", "");
    let typed = typed(&text);
    let member = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "previous")
        .unwrap();
    let [edge] = typed.machine_trait_conformances(member) else {
        panic!("one declared edge")
    };
    let mut witness = typed_trees_to_terminal_psi::elaborate_witness(
        &typed,
        edge.symbol,
        edge.requirement_symbol,
        &[],
        member.symbol,
    )
    .unwrap();
    let discharged = discharge(&mut witness).unwrap();
    let body = id(2);
    let tables = witness.tables.machines.get(body).unwrap();
    let judgments = discharged.machines.get(body).unwrap();
    assert!(
        tables.obligations.iter().any(|(obligation, row)| matches!(
            row.kind,
            ObligationKind::CallRequirement { .. }
        ) && judgments
            .verdicts
            .get(obligation)
            .is_some_and(Result::is_err)),
        "{judgments:?}"
    );
}

#[test]
fn a_boundary_conformer_is_not_a_named_direct_entry() {
    let text = declared_source("", "input - 1", "7", "")
        .replacen("trait Step", "boundary trait Step", 1)
        .replacen("requires value > 0", "reaches none requires value > 0", 1);
    let typed = typed(&text);
    let entry = entry(&typed);
    let verdicts = crate::judge_compilation(
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
    );
    assert!(
        matches!(verdicts.as_slice(), [Ok(RouteVerdict::FallBack(refusal))]
        if refusal.detail.contains("boundary conformer is callable only through its provider slot")),
        "{verdicts:?}"
    );
}

#[test]
fn a_declared_conformers_call_keeps_the_requirement_reach_ceiling() {
    let typed = typed(
        r#"
boundary trait Service {}
trait Read {
    machine value() -> u32 reaches Service crashes none;
}
machine chosen() -> u32 satisfies Read::value crashes none { 7 }
machine entry() -> u32 reaches none crashes none { chosen() }
"#,
    );
    let entry = entry(&typed);
    let verdicts = crate::judge_compilation(
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
    );
    assert!(
        matches!(&verdicts[0], Ok(RouteVerdict::Rejected { exceeded, .. })
        if !exceeded.is_empty()),
        "{verdicts:?}"
    );
    assert!(
        matches!(&verdicts[1], Ok(RouteVerdict::Accepted(_))),
        "{verdicts:?}"
    );
}

#[test]
fn a_declared_conformer_cannot_be_rebound_by_a_selection_row() {
    let text = declared_source("", "input - 1", "7", "")
        + r#"
trait Other {
    machine apply(value: u32) -> u32
    requires value > 0 ensures result < value crashes none;
}

Rebound satisfies Other { Other::apply = previous; }
"#;
    let typed = typed(&text);
    let entry = entry(&typed);
    let verdicts = crate::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            &typed,
            typed_trees_to_terminal_psi::Compilation {
                declaration_metadata: &[],
                entries: &[entry],
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
    assert!(
        matches!(verdicts.as_slice(), [Ok(RouteVerdict::FallBack(refusal))]
        if refusal.detail.contains("selection row cannot rebind a declared conformer")),
        "the existing bound entry must not acquire another route"
    );
    // Reached dependency obligations use the same witness admission even
    // when this compilation does not claim that declaration as a subject.
    let other = typed
        .traits()
        .iter()
        .find(|declared| declared.name.as_str() == "Other")
        .unwrap();
    let requirement = &typed.trait_machine_signatures(other)[0];
    let member = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "previous")
        .unwrap();
    let Err(refusal) = typed_trees_to_terminal_psi::elaborate_witness(
        &typed,
        other.symbol,
        requirement.symbol,
        &[],
        member.symbol,
    ) else {
        panic!("matching text cannot rebind an existing nominal route");
    };
    assert!(
        refusal
            .detail
            .contains("conformance witness cannot rebind a declared requirement")
    );
}

#[test]
fn an_own_progress_promise_does_not_change_a_silent_requirement_contract() {
    let mut published = Vec::new();
    for contract in ["", "terminates"] {
        let verdicts = declared_verdicts(&declared_source(contract, "input - 1", "7", ""));
        assert!(
            verdicts
                .iter()
                .all(|verdict| matches!(verdict, RouteVerdict::Accepted(_)))
        );
        let RouteVerdict::Accepted(closure) = &verdicts[0] else {
            unreachable!()
        };
        let member = closure.elaboration.module.machine(id(2)).unwrap();
        published.push(member.declaration.as_ref().unwrap().contract);
    }
    assert_eq!(
        published[0], published[1],
        "only the requirement's advertised progress belongs in the routed contract"
    );
}

fn inline_parameter_prepared(actual: &str, guarantee: &str) -> typed_trees::FrozenTypedTrees {
    let text = format!(
        r#"
trait Step {{
    machine apply(value: u32) -> u32
    requires value > 0 ensures result < value crashes none;
}}
Inline satisfies Step {{
    machine apply(input: u32) -> u32
    requires true ensures result == 0 crashes none {{ 0 }}
}}
machine relay<Chosen satisfies Step>() -> u32
requires true
{guarantee}
crashes none
{{ Chosen::apply({actual}) }}
machine entry() -> u32 crashes none {{ relay<Inline>() }}
"#
    );
    // Use the actual specialization pass, retaining the exact source
    // parameter dispatch and selected realization it produces.
    let (prepared, _) = typed_trees_to_typed_trees::prepare_typed_trees(
        typed(&text),
        &typed_trees_to_typed_trees::PreparationRequest::settled(),
    )
    .unwrap();
    prepared
}

fn inline_parameter_read(
    prepared: &typed_trees::FrozenTypedTrees,
    publication: terminal_codec::TerminalPublication,
) -> Checked<typed_trees_to_terminal_psi::Elaboration> {
    let entry = entry(prepared);
    typed_trees_to_terminal_psi::elaborate_for(
        prepared,
        typed_trees_to_terminal_psi::Compilation {
            declaration_metadata: &[],
            entries: &[entry],
            subjects: &[None],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        },
        publication,
    )
}

fn inline_parameter_dispatch(
    actual: &str,
    guarantee: &str,
) -> typed_trees_to_terminal_psi::Elaboration {
    let prepared = inline_parameter_prepared(actual, guarantee);
    let elaboration =
        inline_parameter_read(&prepared, terminal_codec::TerminalPublication::Invocation).unwrap();
    assert_eq!(
        elaboration.selections.len(),
        1,
        "one genuinely prepared parameter dispatch"
    );
    let selected = &elaboration.selections[0];
    assert!(
        !elaboration.local_results.contains(&selected.implementation),
        "the selected requirement does not expose private body summaries"
    );
    elaboration
}

#[test]
fn an_owned_inline_parameter_dispatch_owes_the_requirement_inputs() {
    for (actual, accepted) in [("7", true), ("0", false)] {
        let mut elaboration = inline_parameter_dispatch(actual, "");
        let caller = elaboration.selections[0].caller;
        let found = discharge(&mut elaboration).unwrap();
        let tables = elaboration.tables.machines.get(caller).unwrap();
        let judgments = found.machines.get(caller).unwrap();
        let calls: Vec<_> = tables
            .obligations
            .iter()
            .filter(|(_, row)| matches!(row.kind, ObligationKind::CallRequirement { .. }))
            .collect();
        assert_eq!(
            calls.len(),
            1,
            "own requires true must not erase the requirement input"
        );
        assert_eq!(
            judgments.verdicts.get(calls[0].0).unwrap().is_ok(),
            accepted
        );
    }
}

#[test]
fn an_owned_inline_parameter_dispatch_exposes_only_the_requirement_guarantee() {
    let mut elaboration = inline_parameter_dispatch("7", "ensures result == 0");
    let caller = elaboration.selections[0].caller;
    let found = discharge(&mut elaboration).unwrap();
    let tables = elaboration.tables.machines.get(caller).unwrap();
    let judgments = found.machines.get(caller).unwrap();
    assert!(
        tables.obligations.iter().any(|(obligation, row)| matches!(
            row.kind,
            ObligationKind::NormalGuarantee { .. }
        ) && judgments
            .verdicts
            .get(obligation)
            .is_some_and(Result::is_err)),
        "the implementation's stronger result == 0 is not the routed guarantee"
    );
}

#[test]
fn an_owned_inline_parameter_dispatch_keeps_exact_selection_checks() {
    use typed_trees::expression::ExpressionNode;

    for (corruption, expected) in [
        (0, "requirement dispatch with no requirement"),
        (1, "static requirement dispatch"),
        (2, "requirement dispatch that targets another machine"),
    ] {
        let mut prepared = inline_parameter_prepared("7", "").into_inner();
        let expression = prepared
            .expression_table
            .iter_expressions()
            .find_map(|(expression, node)| {
                matches!(node, ExpressionNode::Call(call) if call.static_requirement_dispatch.is_some())
                    .then_some(expression)
            })
            .expect("one generated requirement dispatch");
        let ExpressionNode::Call(call) = prepared.expression_table.expression_mut(expression)
        else {
            panic!("selected call");
        };
        let dispatch = call.static_requirement_dispatch.as_mut().unwrap();
        match corruption {
            0 => dispatch.requirement = Default::default(),
            1 => dispatch.evidence_parameter = Default::default(),
            2 => dispatch.realization_state = Default::default(),
            _ => unreachable!(),
        }
        let prepared = prepared.into();
        let Err(refusal) =
            inline_parameter_read(&prepared, terminal_codec::TerminalPublication::Invocation)
        else {
            panic!("a changed selection must refuse: {expected}");
        };
        assert!(refusal.detail.contains(expected), "{refusal:?}");
    }
}

#[test]
fn an_anonymous_parameter_application_cannot_transfer_its_template_identity() {
    let prepared = inline_parameter_prepared("7", "");
    let first =
        inline_parameter_read(&prepared, terminal_codec::TerminalPublication::Invocation).unwrap();
    let second =
        inline_parameter_read(&prepared, terminal_codec::TerminalPublication::Invocation).unwrap();
    let template = |elaboration: &typed_trees_to_terminal_psi::Elaboration| {
        elaboration
            .module
            .machines
            .iter()
            .find_map(|machine| machine.declaration.as_ref()?.specialization.as_ref())
            .unwrap()
            .template
            .clone()
    };
    let first_template = template(&first);
    let second_template = template(&second);
    assert!(
        first
            .module
            .identities
            .is_invocation_local(first_template.identity())
    );
    assert!(
        !first_template.same_identity_as(
            &first.module.identities,
            &second_template,
            &second.module.identities,
        ),
        "equal source bytes do not transfer an invocation's declaration authority"
    );
    let Err(refusal) =
        inline_parameter_read(&prepared, terminal_codec::TerminalPublication::Portable)
    else {
        panic!("anonymous specialization must not acquire a hermetic package owner");
    };
    // Portable publication needs a real source owner before the anonymous
    // specialization can even ask for a hermetic template identity.
    assert_eq!(refusal.reason, RefusalReason::Unsupported);
    assert_eq!(refusal.concerns, terminal_check_tables::Construct::Machine);
    assert!(
        refusal
            .detail
            .contains("no exact package or toolchain owner"),
        "{refusal:?}"
    );
}

#[test]
fn an_anonymous_parameter_application_rejects_a_foreign_template_handle() {
    let mut prepared = inline_parameter_prepared("7", "").into_inner();
    let template = prepared.machine_specializations[0].template;
    prepared.machine_specializations[0].template =
        symbols::SymbolHandle::from_parts(template.arena_index(), template.generation() + 1);
    let prepared = prepared.into();
    let Err(refusal) =
        inline_parameter_read(&prepared, terminal_codec::TerminalPublication::Invocation)
    else {
        panic!("a foreign generation cannot supply the specialization's template");
    };
    assert!(
        refusal
            .detail
            .contains("generic machine has no typed-tree declaration"),
        "{refusal:?}"
    );
}

fn required_progress_source(body: &str) -> String {
    format!(
        r#"
trait Finish {{
    machine finish() -> u32 terminates crashes none;
}}
machine finish() -> u32 satisfies Finish::finish crashes none {{ {body} }}
machine entry() -> u32 terminates crashes none {{ finish() }}
"#
    )
}

#[test]
fn a_declared_conformer_inherits_a_bare_required_termination_promise() {
    let mut verdicts = declared_verdicts(&required_progress_source("7"));
    assert!(
        verdicts
            .iter()
            .all(|verdict| matches!(verdict, RouteVerdict::Accepted(_))),
        "{verdicts:?}"
    );
    let RouteVerdict::Accepted(accepted) = verdicts.remove(0) else {
        unreachable!()
    };
    let publication = accepted.publish_invocation().unwrap();
    let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    let status = instance
        .invoke(&mut psi_work::WorkMeter::with_budget(1_000))
        .unwrap();
    assert!(
        matches!(
            status,
            terminal_interpreter::TerminalExecutionStatus::Complete(
                terminal_interpreter::TerminalExecutionResult::Scalar(
                    terminal_interpreter::TerminalScalarValue::Integer {
                        value: semantic_vocabulary::IntegerValue::Unsigned(7),
                        ..
                    }
                )
            )
        ),
        "{status:?}"
    );
}

#[test]
fn an_inherited_termination_promise_does_not_validate_an_unranked_cycle() {
    let source = required_progress_source("finish()");
    // The second caller supplies no separate progress obligation. The
    // callee must still keep the promise inherited from its requirement.
    let silent_caller = source.replacen(
        "machine entry() -> u32 terminates",
        "machine entry() -> u32",
        1,
    );
    assert_ne!(source, silent_caller);
    for source in [&source, &silent_caller] {
        let Err(refusal) = invocation(typed(source)) else {
            panic!("the requirement promise is owed by the actual recursive body");
        };
        assert_eq!(refusal.reason, RefusalReason::Unsupported);
        assert!(
            refusal
                .detail
                .contains("termination promise over a call cycle"),
            "{refusal:?}"
        );
    }
}

fn named_owned_source(actual: &str, promised: &str, body: &str) -> String {
    format!(
        r#"
trait Step {{
    machine apply(value: u32) -> u32
    requires value > 0 ensures result < value crashes none;
}}
Inline satisfies Step {{
    machine apply(input: u32) -> u32
    requires true ensures result == 0 crashes none {{ {body} }}
}}
machine entry() -> u32 {promised} crashes none {{ Inline::apply({actual}) }}
"#
    )
}

#[test]
fn a_named_owned_inline_member_calls_through_its_requirement() {
    let text = named_owned_source("7", "", "0");
    let prepared = typed(&text);
    for node in prepared.expression_table.expression_nodes() {
        if let typed_trees::expression::ExpressionNode::Call(call) = node {
            let selection = prepared
                .authored_declaration_selections()
                .get(call.authored_call_selection.unwrap())
                .unwrap();
            assert!(
                matches!(selection.target(), typed_trees::AuthoredDeclarationSelectionTarget::Resolved(selected)
                if selected.selected_symbol() == call.target_symbol),
                "the public requirement must be closed before lowering"
            );
            let namespace = prepared
                .expression_table
                .name_path_node(call.receiver)
                .unwrap()
                .symbol;
            let (owner, row, _) = prepared
                .named_conformance_member(namespace, call.target_symbol)
                .unwrap()
                .unwrap();
            assert_eq!(owner.symbol, namespace);
            assert_eq!(row.requirement, call.target_symbol);
        }
    }
    let mut verdicts = declared_verdicts(&text);
    assert!(
        verdicts
            .iter()
            .all(|verdict| matches!(verdict, RouteVerdict::Accepted(_))),
        "{verdicts:?}"
    );
    let RouteVerdict::Accepted(accepted) = verdicts.remove(0) else {
        unreachable!()
    };
    let publication = accepted.publish_invocation().unwrap();
    let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    let status = instance
        .invoke(&mut psi_work::WorkMeter::with_budget(1_000))
        .unwrap();
    assert!(
        matches!(
            status,
            terminal_interpreter::TerminalExecutionStatus::Complete(
                terminal_interpreter::TerminalExecutionResult::Scalar(
                    terminal_interpreter::TerminalScalarValue::Integer {
                        value: semantic_vocabulary::IntegerValue::Unsigned(0),
                        ..
                    }
                )
            )
        ),
        "{status:?}"
    );
}

#[test]
fn a_named_owned_member_keeps_public_inputs_and_guarantees() {
    for (actual, promised, bad_input) in [("0", "", true), ("7", "ensures result == 0", false)] {
        let verdicts = declared_verdicts(&named_owned_source(actual, promised, "0"));
        let RouteVerdict::Rejected { unproved, .. } = &verdicts[0] else {
            panic!("{verdicts:?}")
        };
        assert!(
            unproved.iter().any(|row| if bad_input {
                matches!(row.kind, ObligationKind::CallRequirement { .. })
            } else {
                matches!(row.kind, ObligationKind::NormalGuarantee { .. })
            }),
            "{verdicts:?}"
        );
        assert!(
            matches!(verdicts[1], RouteVerdict::Accepted(_)),
            "the own-body witness remains valid: {verdicts:?}"
        );
    }
}

#[test]
fn a_named_owned_member_still_owes_its_actual_body() {
    let verdicts = declared_verdicts(&named_owned_source("7", "", "1"));
    assert!(
        verdicts.iter().any(|verdict| matches!(verdict,
        RouteVerdict::Rejected { unproved, .. } if unproved.iter().any(|row|
            matches!(row.kind, ObligationKind::NormalGuarantee { .. })))),
        "{verdicts:?}"
    );
}

#[test]
fn a_named_owned_resultless_call_keeps_its_requirement() {
    for (actual, accepted) in [(7, true), (0, false)] {
        let text = format!(
            r#"
trait Step {{ machine apply(value: u32) requires value > 0 crashes none; }}
Inline satisfies Step {{ machine apply(input: u32) requires true crashes none {{ }} }}
machine entry() crashes none {{ Inline::apply({actual}); }}
"#
        );
        let verdicts = declared_verdicts(&text);
        assert_eq!(
            matches!(verdicts[0], RouteVerdict::Accepted(_)),
            accepted,
            "{verdicts:?}"
        );
        if !accepted {
            let RouteVerdict::Rejected { unproved, .. } = &verdicts[0] else {
                panic!("{verdicts:?}")
            };
            assert!(
                unproved
                    .iter()
                    .any(|row| matches!(row.kind, ObligationKind::CallRequirement { .. })),
                "{verdicts:?}"
            );
        }
    }
}

#[test]
fn a_named_boundary_implementation_has_no_direct_entry() {
    let text = r#"
boundary trait Step {
    machine apply(value: u32) -> u32
    requires value > 0 ensures result < value reaches none crashes none;
}
Inline satisfies Step {
    machine apply(input: u32) -> u32
    requires true ensures result == 0 reaches none crashes none { 0 }
}
machine entry() -> u32 reaches none crashes none { Inline::apply(7) }
"#;
    let Err(refusal) = invocation(typed(text)) else {
        panic!("a boundary implementation requires its provider slot")
    };
    assert!(
        refusal
            .detail
            .contains("boundary conformer is callable only through its provider slot"),
        "{refusal:?}"
    );
}

fn named_two_packages(public_map: bool) -> typed_trees::TypedTrees {
    let library = named_owned_source("7", "", "0")
        .split("machine entry()")
        .next()
        .unwrap()
        .to_owned()
        .replace("trait Step", "public trait Step")
        .replace(
            "Inline satisfies",
            if public_map {
                "public Inline satisfies"
            } else {
                "Inline satisfies"
            },
        );
    let client = "machine entry() -> u32 crashes none { Inline::apply(7) }";
    let mut sources = source::SourceMap::default();
    let mut syntax = None;
    let mut source_ids = Vec::new();
    for (path, root, text, identity) in [
        ("library/api.omg", "library", library.as_str(), [1; 32]),
        ("client/main.omg", "client", client, [2; 32]),
    ] {
        let id = sources
            .add_checked_instance(
                path.into(),
                text,
                root.into(),
                Some(semantic_vocabulary::PackageKeyIdentity::from_digest(identity).unwrap()),
                source::SourceOrigin::User,
                source::SourceResolutionStratum::Base,
                source::DependencyScope::Product,
            )
            .source_id;
        source_ids.push(id);
        let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
        if let Some(syntax) = &mut syntax {
            tokens_to_syntax_trees::parse(syntax, id, &tokens).unwrap();
        } else {
            syntax = Some(tokens_to_syntax_trees::parse_syntax_trees_with_id(id, &tokens).unwrap());
        }
    }
    let mut request =
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax.unwrap());
    request.sources = Some(Arc::new(sources));
    request
        .top_level_bindings
        .push(symbols::SourceScopedTopLevelBinding::new(
            source_ids[1],
            source_ids[0],
            "Inline",
        ));
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request).unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

#[test]
fn a_named_map_controls_visibility_without_publishing_its_private_body() {
    let prepared = named_two_packages(true);
    let verdicts = declared_verdicts_of(&prepared);
    assert!(
        verdicts
            .iter()
            .all(|verdict| matches!(verdict, RouteVerdict::Accepted(_))),
        "{verdicts:?}"
    );
    let private_map = named_two_packages(false);
    let Err(refusal) = invocation(private_map) else {
        panic!("a private map cannot be named by another package")
    };
    assert!(
        refusal
            .detail
            .contains("private machine of another package"),
        "{refusal:?}"
    );

    // A generated bare call cannot borrow the public map's visibility.
    // Retain the real source and body, removing only the routed selection.
    let mut bare = prepared;
    let (handle, body) = bare
        .expression_table
        .expression_entries()
        .find_map(|(handle, node)| {
            let typed_trees::expression::ExpressionNode::Call(call) = node else {
                return None;
            };
            let namespace = bare.expression_table.name_path_node(call.receiver)?.symbol;
            let (_, row, _) = bare
                .named_conformance_member(namespace, call.target_symbol)
                .ok()??;
            Some((handle, row.realization_machine))
        })
        .unwrap();
    assert!(!bare.machine_by_symbol(body).unwrap().is_public);
    let typed_trees::expression::ExpressionNode::Call(call) =
        bare.expression_table.expression_mut(handle)
    else {
        unreachable!()
    };
    call.target_symbol = body;
    call.receiver = Default::default();
    call.authored_call_selection = None;
    call.authored_call_binding = None;
    call.authored_machine_application = Default::default();
    let Err(refusal) = invocation(bare) else {
        panic!("the private body has no direct foreign entry")
    };
    assert!(
        refusal
            .detail
            .contains("private machine of another package"),
        "{refusal:?}"
    );
}

fn closed_named_source(actual: &str, promised: &str, body: &str) -> String {
    named_owned_source(actual, promised, body)
        .replacen("trait Step {", "trait Step<Value> {", 1)
        .replacen(
            "machine apply(value: u32) -> u32",
            "machine apply(value: Value) -> Value",
            1,
        )
        .replacen("Inline satisfies Step {", "Inline satisfies Step<u32> {", 1)
}

fn assert_named_invocation_value(
    mut verdicts: Vec<RouteVerdict>,
    expected: semantic_vocabulary::IntegerValue,
) {
    assert!(
        verdicts
            .iter()
            .all(|verdict| matches!(verdict, RouteVerdict::Accepted(_))),
        "{verdicts:?}"
    );
    let RouteVerdict::Accepted(accepted) = verdicts.remove(0) else {
        unreachable!()
    };
    let publication = accepted.publish_invocation().unwrap();
    let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    let status = instance
        .invoke(&mut psi_work::WorkMeter::with_budget(1_000))
        .unwrap();
    assert!(
        matches!(
            &status,
            terminal_interpreter::TerminalExecutionStatus::Complete(
                terminal_interpreter::TerminalExecutionResult::Scalar(
                    terminal_interpreter::TerminalScalarValue::Integer {
                        value,
                        ..
                    }
                )
            ) if value == &expected
        ),
        "{status:?}"
    );
}

#[test]
fn a_closed_named_trait_application_keeps_its_applied_contract() {
    assert_named_invocation_value(
        declared_verdicts(&closed_named_source("7", "", "0")),
        semantic_vocabulary::IntegerValue::Unsigned(0),
    );
}

#[test]
fn a_closed_named_member_exposes_only_its_applied_requirement() {
    for (actual, promised, bad_input) in [("0", "", true), ("7", "ensures result == 0", false)] {
        let verdicts = declared_verdicts(&closed_named_source(actual, promised, "0"));
        assert!(
            matches!(&verdicts[0], RouteVerdict::Rejected { unproved, .. }
            if unproved.iter().any(|row| if bad_input {
                matches!(row.kind, ObligationKind::CallRequirement { .. })
            } else {
                matches!(row.kind, ObligationKind::NormalGuarantee { .. })
            })),
            "{verdicts:?}"
        );
        assert!(
            matches!(verdicts[1], RouteVerdict::Accepted(_)),
            "the owning body's private clauses remain valid: {verdicts:?}"
        );
    }
}

#[test]
fn a_closed_named_member_still_owes_its_weaker_written_inputs() {
    let text = closed_named_source("7", "", "input - 1").replacen(
        "ensures result == 0",
        "ensures result < input",
        1,
    );
    let verdicts = declared_verdicts(&text);
    assert!(
        matches!(&verdicts[0], RouteVerdict::Accepted(_)),
        "the runtime body is safe under the public positive input: {verdicts:?}"
    );
    assert!(
        matches!(&verdicts[1], RouteVerdict::Rejected { unproved, .. }
        if unproved.iter().any(|row| matches!(row.kind, ObligationKind::OperationSafety { .. }))),
        "the own body still admits zero under its written requires true: {verdicts:?}"
    );
}

#[test]
fn a_closed_named_member_inherits_its_applied_logical_contract() {
    for (body, accepted) in [("0", true), ("input", false)] {
        let text =
            closed_named_source("7", "", body).replacen("requires true ensures result == 0", "", 1);
        let verdicts = declared_verdicts(&text);
        if accepted {
            assert!(
                verdicts
                    .iter()
                    .all(|verdict| matches!(verdict, RouteVerdict::Accepted(_))),
                "{verdicts:?}"
            );
        } else {
            assert!(
                matches!(&verdicts[1], RouteVerdict::Rejected { unproved, .. }
                if unproved.iter().any(|row| matches!(row.kind, ObligationKind::NormalGuarantee { .. }))),
                "the actual body owes the inherited result clause: {verdicts:?}"
            );
        }
    }
}

#[test]
fn closed_named_members_keep_two_applications_of_one_trait_distinct() {
    let text = r#"
trait Step<Value> {
    machine apply(value: Value) -> Value
    requires value > 0 ensures result < value crashes none;
}
Small satisfies Step<u32> {
    machine apply(input: u32) -> u32
    requires true ensures result == 0 crashes none { 0 }
}
Large satisfies Step<u64> {
    machine apply(input: u64) -> u64
    requires input > 0 ensures result < input crashes none { input - 1 }
}
machine entry() -> u64 crashes none {
    let first: u32 = Small::apply(7);
    Large::apply(9)
}
"#;
    let prepared = typed(text);
    let verdicts = compilation_verdicts_of(&prepared);
    assert_eq!(
        verdicts.len(),
        3,
        "runtime and both owning witnesses: {verdicts:?}"
    );
    assert_named_invocation_value(verdicts, semantic_vocabulary::IntegerValue::Unsigned(8));
}

#[test]
fn a_closed_named_witness_cannot_borrow_another_application_scope() {
    let wrong_application = closed_named_source("7", "", "0").replacen("Step<u32>", "Step<u64>", 1);
    let Err(refusal) = invocation(typed(&wrong_application)) else {
        panic!("a u32 body cannot serve the u64 application")
    };
    assert!(
        refusal
            .detail
            .contains("member signature differs from its requirement"),
        "{refusal:?}"
    );

    let text = format!(
        "{}{}",
        closed_named_source("7", "", "0"),
        r#"
trait Other<Value> {
    machine apply(value: Value) -> Value
    requires value > 0 ensures result < value crashes none;
}
machine plain(input: u32) -> u32
requires input > 0 ensures result < input crashes none { input - 1 }
"#
    );
    let prepared = typed(&text);
    let step = prepared
        .traits()
        .iter()
        .find(|owner| owner.name.as_str() == "Step")
        .unwrap();
    let other = prepared
        .traits()
        .iter()
        .find(|owner| owner.name.as_str() == "Other")
        .unwrap();
    let owned = prepared
        .machines()
        .iter()
        .find(|machine| {
            prepared
                .owned_conformance_realization(machine.symbol)
                .is_some()
        })
        .unwrap();
    let plain = prepared
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "plain")
        .unwrap();
    let Err(refusal) = typed_trees_to_terminal_psi::elaborate_witness(
        &prepared,
        other.symbol,
        prepared.trait_machine_signatures(other)[0].symbol,
        &[],
        owned.symbol,
    ) else {
        panic!("matching shapes cannot rebind the owning requirement")
    };
    assert!(
        refusal.detail.contains("conformance witness cannot rebind"),
        "{refusal:?}"
    );

    let Err(refusal) = typed_trees_to_terminal_psi::elaborate_witness(
        &prepared,
        step.symbol,
        prepared.trait_machine_signatures(step)[0].symbol,
        &[],
        plain.symbol,
    ) else {
        panic!("a plain body cannot infer an application from an available map")
    };
    assert!(
        refusal
            .detail
            .contains("witness needs its exact closed trait application"),
        "{refusal:?}"
    );
}

fn generic_named_map_source() -> &'static str {
    r#"
trait Step<Value> {
    machine apply(value: Value) -> Value reaches none crashes none;
}
Selected<Value> satisfies Step<Value> {
    machine apply(input: Value) -> Value reaches none crashes none { input }
}
machine entry() -> u32 reaches none crashes none { Selected<u32>::apply(7) }
"#
}

fn generic_named_map_call(
    program: &typed_trees::TypedTrees,
) -> &typed_trees::expression::TableCallExpression {
    let mut calls = program
        .expression_table
        .expression_entries()
        .filter_map(|(_, node)| {
            let typed_trees::expression::ExpressionNode::Call(call) = node else {
                return None;
            };
            program
                .expression_table
                .name_path_node(call.receiver)
                .filter(|path| program.conformance_by_symbol(path.symbol).is_some())
                .map(|_| call)
        });
    let call = calls.next().expect("one direct named map call");
    assert!(calls.next().is_none());
    call
}

#[test]
fn generic_named_map_source_retains_its_explicit_application() {
    let program = typed(generic_named_map_source());
    let call = generic_named_map_call(&program);
    let (owner, row, signature) = program
        .named_conformance_call_member(call)
        .unwrap()
        .unwrap();
    assert_eq!(program.conformance_type_parameters(owner).len(), 1);
    assert_eq!(row.requirement, call.target_symbol);
    assert_eq!(signature.symbol, row.requirement);
    assert_eq!(
        program
            .type_reference_table
            .primitive_type(signature.return_type),
        typed_trees::types::PrimitiveType::from_name("u32")
    );
    let selection = program
        .authored_declaration_selections()
        .get(call.authored_call_selection.unwrap())
        .unwrap();
    assert_eq!(
        selection.target().as_resolved().unwrap().selected_symbol(),
        row.requirement
    );
    assert_ne!(row.requirement, row.realization_state);
    assert!(
        program
            .named_conformance_member(owner.symbol, row.requirement)
            .is_err(),
        "a bare generic map cannot borrow the call's explicit application"
    );
}

#[test]
fn generic_named_map_source_refuses_missing_foreign_or_rebound_applications() {
    let program = typed(generic_named_map_source());
    let call = generic_named_map_call(&program);
    let (_, row, _) = program
        .named_conformance_call_member(call)
        .unwrap()
        .unwrap();
    let mut missing = call.clone();
    missing.requirement_application = typed_trees::types::TypeReferenceHandle::invalid();
    assert!(program.named_conformance_call_member(&missing).is_err());
    let mut foreign = call.clone();
    foreign.requirement_application = typed_trees::types::TypeReferenceHandle::from_parts(
        call.requirement_application.arena_index(),
        call.requirement_application.generation() + 1,
    );
    assert!(program.named_conformance_call_member(&foreign).is_err());
    let mut source = call.clone();
    source.authored_machine_application =
        typed_trees::static_applications::StaticMachineApplicationHandle::from_parts(
            call.authored_machine_application.arena_index(),
            call.authored_machine_application.generation() + 1,
        );
    assert!(program.named_conformance_call_member(&source).is_err());
    let mut body = call.clone();
    body.target_symbol = row.realization_state;
    assert!(
        program.named_conformance_call_member(&body).is_err(),
        "the owning body cannot replace its public requirement selection"
    );
}

#[test]
fn generic_named_map_invokes_its_closed_own_telescope() {
    let (prepared, _) = typed_trees_to_typed_trees::prepare_typed_trees(
        typed(generic_named_map_source()),
        &typed_trees_to_typed_trees::PreparationRequest::settled(),
    )
    .unwrap();
    let verdicts = compilation_verdicts_of(&prepared);
    eprintln!("direct generic map verdicts: {verdicts:?}");
    assert_named_invocation_value(verdicts, semantic_vocabulary::IntegerValue::Unsigned(7));
}

#[test]
fn generic_named_map_body_uses_its_own_rigid_telescope() {
    let program = typed(generic_named_map_source());
    let call = generic_named_map_call(&program);
    let (_, row, _) = program
        .named_conformance_call_member(call)
        .unwrap()
        .unwrap();
    let mut form =
        typed_trees_to_terminal_psi::elaborate_generic(&program, row.realization_machine).unwrap();
    let discharged = discharge(&mut form).unwrap();
    let root = discharged
        .machines
        .get(id(1))
        .expect("the actual generic body was judged");
    assert!(root.conflicts.is_empty(), "{:?}", root.conflicts);
    assert!(root.unestablished().next().is_none());
    assert!(
        root.verdicts.values().all(Result::is_ok),
        "{:?}",
        root.verdicts
    );
}

#[test]
fn generic_named_map_body_cannot_be_an_unapplied_runtime_entry() {
    let program = typed(generic_named_map_source());
    let call = generic_named_map_call(&program);
    let (_, row, _) = program
        .named_conformance_call_member(call)
        .unwrap()
        .unwrap();
    let refusal = typed_trees_to_terminal_psi::elaborate_closure(&program, row.realization_machine)
        .unwrap_err();
    assert_eq!(refusal.reason, RefusalReason::Unsupported);
    assert!(
        refusal
            .detail
            .contains("generic map body needs its complete selected application"),
        "{refusal:?}"
    );
}

#[test]
fn generic_named_map_source_keeps_parameter_dispatch_separate() {
    let program = typed(generic_named_map_source());
    let call = generic_named_map_call(&program);
    let (owner, row, _) = program
        .named_conformance_call_member(call)
        .unwrap()
        .unwrap();
    let mut mixed = call.clone();
    mixed.static_requirement_dispatch = Some(typed_trees::StaticRequirementDispatch {
        evidence_parameter: program.conformance_type_parameters(owner)[0].symbol,
        declaring_trait: row.declaring_trait,
        requirement: row.requirement,
        realization_machine: row.realization_machine,
        realization_state: row.realization_state,
    });
    assert!(
        program
            .named_conformance_call_member(&mixed)
            .unwrap()
            .is_none(),
        "a nominal receiver cannot replace an existing parameter dispatch"
    );
    assert!(
        program
            .named_conformance_capture_key(&mixed)
            .unwrap()
            .is_none()
    );
}

#[test]
fn generic_named_map_source_capture_identity_keeps_unused_binders() {
    let text = generic_named_map_source()
        .replacen(
            "Selected<Value> satisfies",
            "Selected<Value, Unused> satisfies",
            1,
        )
        .replacen(
            "Selected<u32>::apply(7)",
            "let first: u32 = Selected<u32, u64>::apply(7); Selected<u32, bool>::apply(8)",
            1,
        );
    let program = typed(&text);
    let calls = program
        .expression_table
        .expression_entries()
        .filter_map(|(_, node)| {
            let typed_trees::expression::ExpressionNode::Call(call) = node else {
                return None;
            };
            program
                .expression_table
                .name_path_node(call.receiver)
                .filter(|path| program.conformance_by_symbol(path.symbol).is_some())
                .map(|_| call)
        })
        .collect::<Vec<_>>();
    let [left, right] = calls[..] else {
        panic!("two explicit applications")
    };
    let left_key = program
        .named_conformance_capture_key(left)
        .unwrap()
        .unwrap();
    let right_key = program
        .named_conformance_capture_key(right)
        .unwrap()
        .unwrap();
    assert_eq!(left_key.0, right_key.0, "one actual lexical source owner");
    assert_ne!(
        left_key.1, right_key.1,
        "the map's unused binder still belongs to its body identity"
    );
    let left_scope = program
        .applied_requirement_scope(left_key.0, left.requirement_application)
        .unwrap();
    let right_scope = program
        .applied_requirement_scope(right_key.0, right.requirement_application)
        .unwrap();
    assert!(
        left_scope.same_application(&program, &right_scope),
        "equal public trait scopes must not collapse the independently bound map tuples"
    );
}
