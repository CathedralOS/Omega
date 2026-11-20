use omega_preparation::{
    CheckedPreparationEvidence, PreparationFailure, PreparationReason, RefusalCause, SelectedRealization, TargetIdentity, TargetSemantics,
    TargetSemanticsError, prepare_source_package_diagnosed_with_inputs, prepare_typing_pass_with_cause,
};
use omega_resolver::{ParsedSource, StageSource};
use omega_typed::{BoundarySlot, ConformanceRow, RequirementSlot, SlotDirection, TypedTrees};
use omega_typer::{SourcePackage, TypingEnvironment, TypingProgress, resolve_source_package, type_stage_in_environment};

const SOURCE: &str = "boundary trait Reading { machine read()->u64 reaches none crashes none terminates; }
    machine first()->u64 reaches none crashes none terminates {7u64}
    machine second()->u64 reaches none crashes none terminates {8u64}
    A satisfies Reading {Reading::read = first;}
    B satisfies Reading {Reading::read = second;}
    machine via()->u64 {Reading::read()}
    const USED:u64=via();";

fn input() -> (SourcePackage<'static>, BoundarySlot, TypedTrees<'static>) {
    let tokens = omega_lexer::lex(SOURCE.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(SOURCE.as_bytes(), &tokens).unwrap();
    let source = resolve_source_package(&[StageSource {
        package: "reader",
        source: ParsedSource {
            logical_path: "providers.omg",
            text: SOURCE,
            tokens: &tokens,
            tree: &tree,
        },
    }])
    .unwrap();
    let declaration = source
        .declarations
        .iter()
        .find(|d| d.path == "Reading::read")
        .unwrap()
        .clone();
    let slot = BoundarySlot {
        requirement: RequirementSlot {
            declaration,
            trait_arguments: vec![],
        },
        arguments: vec![],
        direction: SlotDirection::ProgramToProvider,
    };
    let environment = TypingEnvironment {
        boundary_slots: std::slice::from_ref(&slot),
        ..Default::default()
    };
    let trees = type_stage_in_environment(&source, &environment).unwrap();
    let [(site, published)] = trees.boundary_calls.as_slice() else {
        panic!("one boundary call")
    };
    assert_eq!(published, &slot);
    assert_eq!(site.span.slice(SOURCE.as_bytes()), Some(&b"Reading::read()"[..]));
    (source, slot, trees)
}

fn target() -> TargetSemantics {
    TargetSemantics::new(
        TargetIdentity {
            profile: "windows_x86_64".into(),
            semantics_version: 1,
            realization: b"test profile".as_slice().into(),
        },
        [],
        [],
    )
    .unwrap()
}

// Only target metadata, not a CheckedProvider. Source typing identifies the candidates;
// a consumer cannot issue a refinement or authorize execution by retaining these rows.
fn selection(row: &ConformanceRow) -> SelectedRealization {
    SelectedRealization {
        requirement: row.requirement.clone(),
        arguments: vec![],
        implementation: row.implementation.clone(),
        conformance: row.conformance.clone(),
        binding: row.site.clone(),
        checked_contract: b"read contract".as_slice().into(),
        selected_plan: row.implementation.path.as_bytes().into(),
    }
}

#[test]
fn competing_realizations_stop_at_target_selection() {
    let (_, _, trees) = input();
    let [left, right] = trees.conformance_rows.as_slice() else {
        panic!("two candidates")
    };
    assert_ne!(left.implementation, right.implementation);
    assert_ne!(left.conformance, right.conformance);
    for (a, b) in [(left, right), (right, left)] {
        let capsule = target().with_selected_realizations([selection(a)]).unwrap();
        let Err(error) = capsule.with_selected_realizations([selection(b)]) else {
            panic!("two plans for one slot")
        };
        assert_eq!(error, TargetSemanticsError::DuplicateRealizationSelection);
        assert_eq!(
            error.to_string(),
            "the target capsule selects multiple realizations for one requirement application"
        );
    }
}

#[test]
fn target_selection_is_not_checked_provider_evidence_even_without_the_selected_signature() {
    let (source, slot, trees) = input();
    let first = trees
        .conformance_rows
        .iter()
        .find(|r| r.implementation.path == "first")
        .unwrap();
    let selected = selection(first);
    let implementation = selected.implementation.clone();
    let target = target().with_selected_realizations([selected]).unwrap();
    let slots = std::slice::from_ref(&slot);
    let occurrence = "Reading::read()";
    let start = SOURCE.find(occurrence).unwrap();
    let range = (start, start + occurrence.len());
    let cause = RefusalCause::Preparation(PreparationReason::UnselectedBoundarySlot);
    let message = "static evaluation cannot admit this invocation: the selected runtime boundary slot cannot execute during preparation\ncall chain: via -> Reading::read";

    let failure = prepare_source_package_diagnosed_with_inputs(&source, slots, Some(&target)).unwrap_err();
    assert!(matches!(failure, PreparationFailure::Preparation(_)));
    spec_corpus::assert_preparation_refusal!(failure, cause, "reader", "providers.omg", range, message);

    let resolved = trees.resolved().clone();
    let mut facts = trees.into_facts();
    let before = facts.signatures.len();
    facts.signatures.retain(|s| s.machine != implementation);
    assert_eq!(facts.signatures.len() + 1, before);
    assert!(facts.library.is_none(), "no imported obligations in this typed stage");
    let changed = TypingProgress::Complete(TypedTrees::new(resolved, facts, 0));
    let refusal = prepare_typing_pass_with_cause(&changed, slots, Some(&target), CheckedPreparationEvidence::default()).unwrap_err();
    spec_corpus::assert_preparation_cause!(refusal, cause, "reader", "providers.omg", range, message);
}
