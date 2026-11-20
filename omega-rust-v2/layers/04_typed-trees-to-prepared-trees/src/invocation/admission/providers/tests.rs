use super::*;
use crate::{CheckedPreparationEvidence, PreparationAnswers, PreparationReason, PreparationRefusal, PreparedPass, RefusalCause, prepare_typing_pass};
use omega_typer::{RequirementSlot, Site};
use std::sync::Arc;

use omega_resolver::{ParsedSource, StageSource};
use omega_typer::{TypeError, TypingEnvironment, TypingProgress, resolve_source_package, type_source_package_for_preparation};

const SOURCE: &str = "
boundary trait Reading { machine read()->u64 reaches none crashes none terminates; }
machine literal()->u64 reaches none crashes none terminates {7u64}
Capture satisfies Reading {Reading::read = literal;}
machine via()->u64 {Reading::read()}
const USED:u64=via();
const DIRECT:u64=literal();
";
const WORD: ValueType = ValueType::Integer {
    signed: false,
    width: Some(64),
};

fn typed(
    source: &str,
    direction: SlotDirection,
) -> (TypingProgress<'_>, BoundarySlot) {
    let (progress, [slot]) = typed_slots(source, direction, [vec![]]);
    (progress, slot)
}

fn typed_slots<const N: usize>(
    source: &str,
    direction: SlotDirection,
    arguments: [Vec<ValueType>; N],
) -> (TypingProgress<'_>, [BoundarySlot; N]) {
    let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(source.as_bytes(), &tokens).unwrap();
    let sources = [StageSource {
        package: "app",
        source: ParsedSource {
            logical_path: "main.omg",
            text: source,
            tokens: &tokens,
            tree: &tree,
        },
    }];
    let package = resolve_source_package(&sources).unwrap();
    let declaration = package
        .declarations
        .iter()
        .find(|id| id.path == "Reading::read")
        .unwrap()
        .clone();
    let slots = arguments.map(|arguments| BoundarySlot {
        requirement: RequirementSlot {
            declaration: declaration.clone(),
            trait_arguments: vec![],
        },
        arguments,
        direction,
    });
    let environment = TypingEnvironment {
        boundary_slots: &slots,
        ..TypingEnvironment::default()
    };
    (type_source_package_for_preparation(&package, &environment).unwrap(), slots)
}

fn frozen_trees<'stage, 'text>(progress: &'stage TypingProgress<'text>) -> &'stage TypedTrees<'text> {
    let TypingProgress::Complete(trees) = progress else {
        panic!("the provider probe is fully typed")
    };
    trees
}

/// A checker probe for a terminating, pure u64 operation. Its selected local body has a single,
/// representable literal return. The plan has no authority or trust inputs.
fn check_literal_provider<'trees, 'text>(
    trees: &'trees TypedTrees<'text>,
    slot: &BoundarySlot,
    target: &TargetIdentity,
) -> Option<CheckedProvider<'trees, 'text>> {
    check_literal_provider_application(trees, slot, target, None)
}

fn check_literal_provider_application<'trees, 'text>(
    trees: &'trees TypedTrees<'text>,
    slot: &BoundarySlot,
    target: &TargetIdentity,
    caller: Option<&TypedCall>,
) -> Option<CheckedProvider<'trees, 'text>> {
    if slot.direction != SlotDirection::ProgramToProvider {
        return None;
    }
    let (call, selected_application) = trees.calls.iter().find_map(|call| {
        if call.machine != slot.requirement.declaration || call.evidence.is_some() || call.result != WORD {
            return None;
        }
        let application = caller.map(|caller| call.in_caller_application(caller));
        let expected = application.as_ref().unwrap_or(call);
        if expected.type_arguments != slot.arguments
            || expected
                .type_arguments
                .iter()
                .any(|argument| !matches!(argument, ValueType::StaticInteger { carrier, value } if carrier.as_ref() == &WORD && WORD.contains_integer(value)))
        {
            return None;
        }
        if caller.is_none()
            && !trees
                .boundary_calls
                .iter()
                .any(|(site, selected)| site == &call.site && selected == slot)
        {
            return None;
        }
        Some((call, application))
    })?;
    let mut bindings = trees
        .conformance_rows
        .iter()
        .filter(|row| row.requirement == slot.requirement);
    let binding = bindings.next()?;
    if bindings.next().is_some() {
        return None;
    }
    if !trees.obligations.iter().any(|obligation| {
        obligation.node == binding.node
            && matches!(&obligation.kind,
            ObligationKind::RefineRequirement { requirement, implementation }
                if requirement == &binding.requirement && implementation == &binding.implementation)
    }) {
        return None;
    }
    for machine in [&call.machine, &binding.implementation] {
        let signature = trees
            .signatures
            .iter()
            .find(|signature| &signature.machine == machine)?;
        if signature.parameters.iter().any(|(_, carrier)| carrier != &WORD)
            || signature.result != WORD
            || !signature.terminates
            || !signature.reach.as_ref().is_some_and(Vec::is_empty)
            || signature.crash_ceiling.is_none()
        {
            return None;
        }
        let effects = trees
            .machine_effects
            .iter()
            .find(|row| &row.machine == machine)?;
        if !effects.reach.is_empty() || !effects.invokes.is_empty() || effects.pause.suspend || effects.pause.block {
            return None;
        }
    }
    let implementation = trees
        .signatures
        .iter()
        .find(|signature| signature.machine == binding.implementation)?;
    if implementation.requires || implementation.has_guarantee || implementation.crash_ceiling.as_deref() != Some(&[]) {
        return None;
    }
    let requirement = trees
        .signatures
        .iter()
        .find(|signature| signature.machine == call.machine)?;
    if !requirement
        .parameters
        .iter()
        .map(|(_, carrier)| carrier)
        .eq(implementation.parameters.iter().map(|(_, carrier)| carrier))
        || call.operands.len() != requirement.parameters.len()
    {
        return None;
    }
    let requirement_tree = trees.trees().nth(requirement.node.file)?;
    let span = requirement.site.span;
    let checked_contract: Vec<u8> = requirement_tree
        .resolved()
        .file
        .tokens
        .iter()
        .filter(|token| token.span.get_start_offset() >= span.get_start_offset() && token.span.get_end_offset() <= span.get_end_offset())
        .flat_map(|token| token.text.bytes().chain([0]))
        .collect();
    let tree = trees.trees().nth(implementation.node.file)?;
    let file = tree.resolved().file;
    let body = file.child(implementation.node.node, NodeKind::Body)?;
    let [tail] = *file.nodes[body].children.as_slice() else { return None };
    let [literal] = *file.nodes[tail].children.as_slice() else { return None };
    if file.nodes[tail].kind != NodeKind::Tail || file.nodes[literal].kind != NodeKind::Literal {
        return None;
    }
    let value = landed_value(&tree, literal, &WORD).ok()??;
    if requirement.has_guarantee {
        super::super::proofs::probe_literal_guarantee(
            trees,
            InvocationSite {
                call: call.node,
                signature: requirement,
                body: file.node_id(body),
            },
            vec![],
            &value,
        )?;
    }
    Some(CheckedProvider {
        trees,
        call: call.node,
        binding,
        application: selected_application.map(|requirement| {
            // L3 selected this exact conformance and verified both telescopes
            // agree by position. Rebind the implementation's formal owners.
            let mut realization = requirement.clone();
            realization.machine = implementation.machine.clone();
            realization.enclosing_arguments.clear();
            for (operand, (span, _)) in realization.operands.iter_mut().zip(&implementation.parameters) {
                operand.parameter = Site {
                    span: *span,
                    ..implementation.site.clone()
                };
            }
            CheckedProviderApplication {
                requirement,
                implementation: realization,
            }
        }),
        target: target.clone(),
        observations: vec![],
        realization: SelectedRealization {
            requirement: slot.requirement.clone(),
            arguments: slot.arguments.clone(),
            implementation: binding.implementation.clone(),
            conformance: binding.conformance.clone(),
            binding: binding.site.clone(),
            checked_contract: Arc::from(checked_contract),
            selected_plan: Arc::from(value.to_signed_bytes_le()),
        },
    })
}

fn evaluate<'trees, 'text>(
    progress: &'trees TypingProgress<'text>,
    slot: &'trees BoundarySlot,
    target: &'trees crate::TargetSemantics,
    proof: Option<CheckedProvider<'trees, 'text>>,
) -> Result<PreparedPass, TypeError> {
    prepare_typing_pass(
        progress,
        std::slice::from_ref(slot),
        Some(target),
        proof
            .into_iter()
            .fold(CheckedPreparationEvidence::default(), CheckedPreparationEvidence::with_provider),
    )
}

/// Run the typing pass with the supplied evidence and keep the typed cause.
fn refuse_with<'trees, 'text>(
    progress: &'trees TypingProgress<'text>,
    slots: &'trees [BoundarySlot],
    target: &'trees crate::TargetSemantics,
    evidence: CheckedPreparationEvidence<'trees, 'text>,
) -> PreparationRefusal {
    match crate::prepare_typing_pass_with_cause(progress, slots, Some(target), evidence) {
        Ok(_) => panic!("the supplied provider evidence must be refused"),
        Err(refusal) => refusal,
    }
}

fn refuse<'trees, 'text>(
    progress: &'trees TypingProgress<'text>,
    slots: &'trees [BoundarySlot],
    target: &'trees crate::TargetSemantics,
    proofs: Vec<CheckedProvider<'trees, 'text>>,
) -> PreparationRefusal {
    let evidence = proofs
        .into_iter()
        .fold(CheckedPreparationEvidence::default(), CheckedPreparationEvidence::with_provider);
    refuse_with(progress, slots, target, evidence)
}

const ADMISSION: &str = "static evaluation cannot admit this invocation: ";
// Reason texts are written here independently of the reason inventory.
const UNSELECTED: &str = "the selected runtime boundary slot cannot execute during preparation";
const DIFFERENT_SELECTIONS: &str = "the selected runtime boundary slot cannot execute under different build selections";
const WRONG_PLAN: &str = "the checked provider does not match the selected target plan";
const STALE_PROOF: &str = "the checked provider proof depends on a different target selection";
const WRONG_CONFORMANCE: &str = "the checked provider does not certify the selected conformance binding";
const WRONG_APPLICATION: &str = "the checked provider does not preserve the selected positional application";

/// Each fixture reaches the boundary call from the body of `via`.
fn assert_provider_refusal(
    refusal: &PreparationRefusal,
    reason: PreparationReason,
    text: &str,
    source: &str,
    occurrence: &str,
) {
    let start = source
        .find(occurrence)
        .unwrap_or_else(|| panic!("{occurrence:?} does not occur in the source"));
    assert_eq!(source.rfind(occurrence), Some(start), "{occurrence:?} occurs more than once");
    let message = format!("{ADMISSION}{text}\ncall chain: via -> Reading::read");
    spec_corpus::assert_preparation_cause!(
        refusal,
        RefusalCause::Preparation(reason),
        "app",
        "main.omg",
        (start, start + occurrence.len()),
        message.as_str()
    );
}

#[test]
fn selected_literal_provider_retains_the_exact_plan_even_without_a_projection() {
    let (progress, slot) = typed(SOURCE, SlotDirection::ProgramToProvider);
    let trees = frozen_trees(&progress);
    let target = TargetIdentity {
        profile: "windows_x86_64".into(),
        semantics_version: 1,
        realization: Arc::from(&b"probe target"[..]),
    };
    let check = || check_literal_provider(trees, &slot, &target).expect("closed, checked local literal provider");
    let selection = check().realization;
    let unselected = crate::TargetSemantics::new(target.clone(), [], []).unwrap();
    let capsule = crate::TargetSemantics::new(target.clone(), [], [])
        .unwrap()
        .with_selected_realizations([selection.clone()])
        .unwrap();

    let pass = evaluate(&progress, &slot, &capsule, Some(check())).unwrap();
    let PreparationAnswers::Frozen { constants, .. } = pass.answers else {
        panic!("the selected provider should have frozen the package")
    };
    for constant in &trees.constants {
        assert_eq!(constants[&constant.declaration], PreparedValue::Integer(7.into()));
    }
    let observations = pass.target_observations;
    assert_eq!(
        observations,
        [TargetObservation::SelectedRealization {
            target: target.clone(),
            realization: selection
        }]
    );
    assert!(capsule.matches_observation(&observations[0]));
    let direct_source = SOURCE.replace("const USED:u64=via();", "");
    let (direct, direct_slot) = typed(&direct_source, SlotDirection::ProgramToProvider);
    assert!(
        evaluate(&direct, &direct_slot, &capsule, None)
            .unwrap()
            .target_observations
            .is_empty()
    );

    let rejected = refuse(&progress, std::slice::from_ref(&slot), &unselected, vec![check()]);
    assert_provider_refusal(&rejected, PreparationReason::ProviderTargetPlanMismatch, WRONG_PLAN, SOURCE, "Reading::read()");
    let rejected = refuse(&progress, std::slice::from_ref(&slot), &capsule, vec![]);
    assert_provider_refusal(&rejected, PreparationReason::UnselectedBoundarySlot, UNSELECTED, SOURCE, "Reading::read()");

    let changed_source = SOURCE.replace("7u64", "8u64");
    let (changed, changed_slot) = typed(&changed_source, SlotDirection::ProgramToProvider);
    let changed_proof = check_literal_provider(frozen_trees(&changed), &changed_slot, &target).unwrap();
    assert_ne!(
        observations,
        [TargetObservation::SelectedRealization {
            target: target.clone(),
            realization: changed_proof.realization.clone()
        }]
    );
    // A proof checked against another typing pass is not a candidate here.
    let rejected = refuse(&changed, std::slice::from_ref(&changed_slot), &capsule, vec![check()]);
    assert_provider_refusal(&rejected, PreparationReason::UnselectedBoundarySlot, UNSELECTED, &changed_source, "Reading::read()");

    let inbound = BoundarySlot {
        direction: SlotDirection::EnvironmentToProgram,
        ..slot.clone()
    };
    let rejected = refuse(&progress, std::slice::from_ref(&inbound), &capsule, vec![check()]);
    assert_provider_refusal(
        &rejected,
        PreparationReason::BoundarySlotSelectionsDiffer,
        DIFFERENT_SELECTIONS,
        SOURCE,
        "Reading::read()",
    );
}

#[test]
fn only_an_exact_applicable_provider_proof_authorizes_the_slot() {
    let (progress, slot) = typed(SOURCE, SlotDirection::ProgramToProvider);
    let trees = frozen_trees(&progress);
    let target = TargetIdentity {
        profile: "windows_x86_64".into(),
        semantics_version: 1,
        realization: Arc::from(&b"applicable provider proofs"[..]),
    };
    let check = || check_literal_provider(trees, &slot, &target).expect("closed, checked local literal provider");
    let selection = check().realization;
    let float = |operation| crate::FloatRealization {
        format: crate::FloatFormat::Binary64,
        operation,
        semantics: crate::FloatSemantics::Ieee754NearestEvenV1,
    };
    let selected_float = TargetObservation::FloatRealization {
        target: target.clone(),
        realization: float(crate::FloatOperation::Add),
    };
    let unselected_float = TargetObservation::FloatRealization {
        target: target.clone(),
        realization: float(crate::FloatOperation::Multiply),
    };
    let capsule = crate::TargetSemantics::new(target.clone(), [], [float(crate::FloatOperation::Add)])
        .unwrap()
        .with_selected_realizations([selection.clone()])
        .unwrap();
    assert!(capsule.matches_observation(&selected_float));
    assert!(!capsule.matches_observation(&unselected_float));

    // A real checked plan for the same slot in a sibling package whose provider returns 8.
    let changed_source = SOURCE.replace("7u64", "8u64");
    let (changed, changed_slot) = typed(&changed_source, SlotDirection::ProgramToProvider);
    let other_plan = check_literal_provider(frozen_trees(&changed), &changed_slot, &target)
        .unwrap()
        .realization
        .selected_plan;
    assert_ne!(other_plan, selection.selected_plan);
    // The wrong-plan proof depends only on observations the capsule satisfies,
    // so any leak of a rejected candidate's dependencies would be visible.
    let wrong_plan = || {
        let mut proof = check();
        proof.realization.selected_plan = other_plan.clone();
        proof.observations = vec![selected_float.clone()];
        proof
    };
    let stale_dependency = || {
        let mut proof = check();
        proof.observations = vec![unselected_float.clone()];
        proof
    };
    let prepare = |checked: Vec<_>| {
        prepare_typing_pass(
            &progress,
            std::slice::from_ref(&slot),
            Some(&capsule),
            checked
                .into_iter()
                .fold(CheckedPreparationEvidence::default(), CheckedPreparationEvidence::with_provider),
        )
    };
    let expected = [TargetObservation::SelectedRealization {
        target: target.clone(),
        realization: selection.clone(),
    }];
    for checked in [
        vec![wrong_plan(), check()],
        vec![check(), wrong_plan()],
        vec![stale_dependency(), wrong_plan(), check()],
        vec![check(), wrong_plan(), stale_dependency()],
    ] {
        let pass = prepare(checked).unwrap();
        let PreparationAnswers::Frozen { constants, .. } = pass.answers else {
            panic!("the applicable provider proof should freeze the package")
        };
        for constant in &trees.constants {
            assert_eq!(constants[&constant.declaration], PreparedValue::Integer(7.into()));
        }
        spec_corpus::observe_handoff!("bounded-callback-refinements/producer", assert_eq!(pass.target_observations, expected));
    }

    // The same dependency is retained once it belongs to the authorizing proof.
    let mut dependent = check();
    dependent.observations = vec![selected_float.clone()];
    let observations = prepare(vec![wrong_plan(), dependent])
        .unwrap()
        .target_observations;
    assert_eq!(observations.len(), 2, "{observations:?}");
    assert!(observations.contains(&selected_float) && observations.contains(&expected[0]));

    for (checked, refusal, reason, text) in [
        (
            vec![wrong_plan()],
            "does not match the selected target plan",
            PreparationReason::ProviderTargetPlanMismatch,
            WRONG_PLAN,
        ),
        (
            vec![wrong_plan(), wrong_plan()],
            "does not match the selected target plan",
            PreparationReason::ProviderTargetPlanMismatch,
            WRONG_PLAN,
        ),
        (
            vec![stale_dependency()],
            "depends on a different target selection",
            PreparationReason::ProviderProofTargetChanged,
            STALE_PROOF,
        ),
        (
            vec![],
            "runtime boundary slot cannot execute",
            PreparationReason::UnselectedBoundarySlot,
            UNSELECTED,
        ),
    ] {
        let refused = refuse(&progress, std::slice::from_ref(&slot), &capsule, checked);
        let error = &refused.diagnostic;
        spec_corpus::observe_handoff!("bounded-callback-refinements/refusal", assert!(error.message.contains(refusal), "{error:?}"));
        assert_eq!(error.span.slice(SOURCE.as_bytes()), Some(&b"Reading::read()"[..]));
        assert_provider_refusal(&refused, reason, text, SOURCE, "Reading::read()");
    }

    // A certificate for another implementation does not realize this binding.
    // It is refused alone and cannot shadow the applicable proof.
    let wrong_conformance = || {
        let mut proof = check();
        proof.realization.implementation = slot.requirement.declaration.clone();
        proof
    };
    assert_ne!(wrong_conformance().realization.implementation, check().binding.implementation);
    let refused = refuse(&progress, std::slice::from_ref(&slot), &capsule, vec![wrong_conformance()]);
    assert_provider_refusal(
        &refused,
        PreparationReason::ProviderConformanceMismatch,
        WRONG_CONFORMANCE,
        SOURCE,
        "Reading::read()",
    );
    let pass = prepare(vec![wrong_conformance(), check()]).unwrap();
    assert_eq!(pass.target_observations, expected);
}

#[test]
fn checks_provider_requirement_under_the_reevaluated_entry_values() {
    let template = "
        boundary trait Reading {
            machine read(input:u64)->u64 requires input > 0u64 crashes Trap (input >= 10u64) ensures result == 7u64 reaches none terminates;
        }
        machine literal(other:u64)->u64 reaches none crashes none terminates {7u64}
        Capture satisfies Reading {Reading::read = literal;}
        machine via(value:u64)->u64 {Reading::read(value)}
        const USED:u64=via(INPUTu64);
    ";
    let target = TargetIdentity {
        profile: "windows_x86_64".into(),
        semantics_version: 1,
        realization: Arc::from(&b"checked-provider-target"[..]),
    };
    for (input, refusal) in [
        ("2", None),
        (
            "0",
            Some((
                PreparationReason::IntegerPreconditionFalse,
                "the selected integer precondition is false for the supplied arguments",
            )),
        ),
        (
            "10",
            Some((
                PreparationReason::TrapRoutePossible,
                "the selected Trap route remains possible for the supplied arguments",
            )),
        ),
    ] {
        let source = template.replace("INPUT", input);
        let (progress, slot) = typed(&source, SlotDirection::ProgramToProvider);
        let trees = frozen_trees(&progress);
        let proof = check_literal_provider(trees, &slot, &target).expect("the unconditional provider refines the requirement");
        let capsule = crate::TargetSemantics::new(target.clone(), [], [])
            .unwrap()
            .with_selected_realizations([proof.realization.clone()])
            .unwrap();
        let requirement = trees
            .signatures
            .iter()
            .find(|signature| signature.machine == slot.requirement.declaration)
            .unwrap();
        let implementation = trees
            .signatures
            .iter()
            .find(|signature| signature.machine == proof.binding.implementation)
            .unwrap();
        let file = trees.trees().nth(implementation.node.file).unwrap();
        let file = file.resolved().file;
        let body = file.child(implementation.node.node, NodeKind::Body).unwrap();
        let entry = PreparedValue::Integer(input.parse::<u64>().unwrap().into());
        let postcondition = super::super::proofs::probe_literal_guarantee(
            trees,
            InvocationSite {
                call: proof.call,
                signature: requirement,
                body: file.node_id(body),
            },
            vec![entry],
            &7u64.into(),
        )
        .unwrap();
        let evidence = CheckedPreparationEvidence {
            invocation_proofs: postcondition,
            providers: ProviderAdmission { checked: vec![proof] },
            ..CheckedPreparationEvidence::default()
        };
        // The requirement's entry contract is checked against the reevaluated
        // entry values inside the boundary call.
        if let Some((reason, text)) = refusal {
            let refused = refuse_with(&progress, std::slice::from_ref(&slot), &capsule, evidence);
            assert_provider_refusal(&refused, reason, text, &source, "Reading::read(value)");
        } else {
            let prepared = prepare_typing_pass(&progress, std::slice::from_ref(&slot), Some(&capsule), evidence);
            let PreparationAnswers::Frozen { constants, .. } = prepared.unwrap().answers else {
                panic!("constant should be frozen")
            };
            assert_eq!(constants[&trees.constants[0].declaration], PreparedValue::Integer(7.into()));
            let proof = check_literal_provider(trees, &slot, &target).unwrap();
            let refused = refuse(&progress, std::slice::from_ref(&slot), &capsule, vec![proof]);
            assert_provider_refusal(
                &refused,
                PreparationReason::PostconditionNeedsProof,
                "the selected postcondition requires a checked proof for the arguments and result",
                &source,
                "Reading::read(value)",
            );
        }
    }
}

#[test]
fn provider_evidence_is_bound_to_the_exact_closed_application() {
    let source = "
        boundary trait Reading {
            machine read<const Key:u64>(input:u64)->u64 reaches none crashes none terminates;
        }
        machine literal<const OwnKey:u64>(other:u64)->u64 reaches none crashes none terminates {7u64}
        Capture satisfies Reading {Reading::read = literal;}
        machine via<const K:u64>(value:u64)->u64 {Reading::read::<K>(value)}
        const A:u64=via::<1u64>(2u64);
        const B:u64=via::<2u64>(2u64);
    ";
    let arguments = [1u64, 2u64].map(|key| {
        vec![ValueType::StaticInteger {
            carrier: Box::new(WORD),
            value: key.into(),
        }]
    });
    let (progress, slots) = typed_slots(source, SlotDirection::ProgramToProvider, arguments);
    let trees = frozen_trees(&progress);
    let target = TargetIdentity {
        profile: "windows_x86_64".into(),
        semantics_version: 1,
        realization: Arc::from(&b"exact provider applications"[..]),
    };
    let prove = |index: usize| {
        let slot = &slots[index];
        let caller = trees
            .calls
            .iter()
            .find(|call| call.machine.path == "via" && call.type_arguments == slot.arguments)
            .unwrap();
        check_literal_provider_application(trees, slot, &target, Some(caller)).unwrap()
    };
    assert_eq!(prove(0).call, prove(1).call);
    assert_ne!(prove(0).realization, prove(1).realization);
    let capsule = crate::TargetSemantics::new(target.clone(), [], [])
        .unwrap()
        .with_selected_realizations([prove(0).realization, prove(1).realization])
        .unwrap();
    let prepare = |checked: Vec<_>| {
        prepare_typing_pass(
            &progress,
            &slots,
            Some(&capsule),
            checked
                .into_iter()
                .fold(CheckedPreparationEvidence::default(), CheckedPreparationEvidence::with_provider),
        )
    };
    let prepared = prepare(vec![prove(0), prove(1)]).unwrap();
    let PreparationAnswers::Frozen { constants, .. } = prepared.answers else {
        panic!("both checked provider applications freeze their values")
    };
    for (index, constant) in trees.constants.iter().enumerate() {
        assert_eq!(constants[&constant.declaration], PreparedValue::Integer(7.into()));
        let used = prepared
            .dependencies
            .inputs(&crate::PreparedValueOrigin::Constant(constant.declaration.clone()))
            .unwrap();
        assert_eq!(
            used.target_observations,
            [TargetObservation::SelectedRealization {
                target: target.clone(),
                realization: prove(index).realization
            }]
        );
    }
    // B's application has no proof of its own.
    let refused = refuse(&progress, &slots, &capsule, vec![prove(0)]);
    assert_provider_refusal(
        &refused,
        PreparationReason::UnselectedBoundarySlot,
        UNSELECTED,
        source,
        "Reading::read::<K>(value)",
    );

    // A realization that drops A's positional arguments is refused, and it
    // cannot shadow the applicable proof for the same application.
    let misapplied = || {
        let mut proof = prove(0);
        let application = proof.application.as_mut().unwrap();
        assert!(!application.requirement.type_arguments.is_empty());
        application.implementation.type_arguments.clear();
        proof
    };
    let refused = refuse(&progress, &slots, &capsule, vec![misapplied(), prove(1)]);
    assert_provider_refusal(
        &refused,
        PreparationReason::ProviderApplicationMismatch,
        WRONG_APPLICATION,
        source,
        "Reading::read::<K>(value)",
    );
    assert!(prepare(vec![misapplied(), prove(0), prove(1)]).is_ok());
}
