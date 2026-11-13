//! Interface contracts must survive ordinary source preparation and the
//! actual call, including its borrowed inputs. These tests do not install a
//! provider or claim native execution.

use super::*;
use std::sync::Arc;

pub(super) fn typed(text: &str) -> typed_trees::TypedTrees {
    // The boundary belongs to this fixture's user package. Its declared
    // owner is required by ordinary header publication; it grants no
    // toolchain identity or provider-execution authority.
    let package = semantic_vocabulary::PackageKeyIdentity::from_digest([0xbc; 32])
        .expect("nonempty fixture package identity");
    let mut sources = source::SourceMap::default();
    let source = sources
        .add_checked_instance(
            "boundary_calls/source.omg".into(),
            text,
            "boundary_calls".into(),
            Some(package),
            source::SourceOrigin::User,
            source::SourceResolutionStratum::Base,
            source::DependencyScope::Product,
        )
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

fn invocation(typed: typed_trees::TypedTrees) -> Checked<structure_elaboration::Elaboration> {
    // These are the package's actual declarations in this invocation's
    // activation scope. No provider is installed or executed by the test.
    let entry = entry(&typed);
    let frozen: typed_trees::FrozenTypedTrees = typed.into();
    structure_elaboration::elaborate_for(
        &frozen,
        structure_elaboration::Compilation {
            entries: &[entry],
            subjects: &[],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
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

fn boundary(reach: &str, actual: u32) -> Checked<structure_elaboration::Elaboration> {
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
    let valid = judge(boundary("reaches none", 7)).unwrap();
    assert!(matches!(valid, RouteVerdict::Accepted(_)), "{valid:?}");
    let verdict = judge(boundary("reaches none", 11)).unwrap();
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
        let refusal =
            discharge(&mut elaboration, &mut crate::ProofSearchActivity::default()).unwrap_err();
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
    let valid = judge(call(1)).unwrap();
    assert!(matches!(valid, RouteVerdict::Accepted(_)), "{valid:?}");
    let verdict = judge(call(0)).unwrap();
    assert!(
        matches!(verdict, RouteVerdict::Rejected { ref unproved, .. }
        if unproved.iter().any(|row| matches!(row.kind, ObligationKind::CrashCoverage { .. }))),
        "{verdict:?}"
    );
}

fn borrowed_requirement(second: &str, caller_bound: &str) -> structure_elaboration::Elaboration {
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
    structure_elaboration::elaborate_generic(&typed, entry(&typed)).unwrap()
}

#[test]
fn borrowed_generic_requirements_preserve_input_proofs_and_exclusive_loans() {
    let mut valid = borrowed_requirement("right", "< 10");
    let discharged = discharge(&mut valid, &mut crate::ProofSearchActivity::default()).unwrap();
    let entry = discharged.machines.get(id(1)).unwrap();
    assert!(entry.conflicts.is_empty());
    assert!(entry.unestablished().next().is_none());
    assert!(
        entry.verdicts.values().all(Result::is_ok),
        "{:?}",
        entry.verdicts
    );

    let mut missing_input = borrowed_requirement("right", "<= 10");
    let discharged = discharge(
        &mut missing_input,
        &mut crate::ProofSearchActivity::default(),
    )
    .unwrap();
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
    let discharged = discharge(&mut alias, &mut crate::ProofSearchActivity::default()).unwrap();
    assert!(!discharged.machines.get(id(1)).unwrap().conflicts.is_empty());
}
