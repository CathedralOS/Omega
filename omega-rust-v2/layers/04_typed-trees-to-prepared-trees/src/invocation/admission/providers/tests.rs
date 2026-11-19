use super::*;
use crate::{CheckedPreparationEvidence, PreparationAnswers, PreparedPass, prepare_typing_pass};

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

    let rejected = evaluate(&progress, &slot, &unselected, Some(check())).unwrap_err();
    assert!(
        rejected
            .message
            .contains("does not match the selected target plan"),
        "{rejected:?}"
    );
    let rejected = evaluate(&progress, &slot, &capsule, None).unwrap_err();
    assert!(
        rejected
            .message
            .contains("runtime boundary slot cannot execute"),
        "{rejected:?}"
    );

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
    let rejected = evaluate(&changed, &changed_slot, &capsule, Some(check())).unwrap_err();
    assert!(
        rejected
            .message
            .contains("runtime boundary slot cannot execute"),
        "{rejected:?}"
    );

    let inbound = BoundarySlot {
        direction: SlotDirection::EnvironmentToProgram,
        ..slot.clone()
    };
    let rejected = evaluate(&progress, &inbound, &capsule, Some(check())).unwrap_err();
    assert!(
        rejected
            .message
            .contains("runtime boundary slot cannot execute under different build selections"),
        "{rejected:?}"
    );
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
        ("0", Some("precondition is false")),
        ("10", Some("Trap route remains possible")),
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
        let prepared = prepare_typing_pass(
            &progress,
            std::slice::from_ref(&slot),
            Some(&capsule),
            CheckedPreparationEvidence {
                invocation_proofs: postcondition,
                providers: ProviderAdmission { checked: vec![proof] },
                ..CheckedPreparationEvidence::default()
            },
        );
        if let Some(refusal) = refusal {
            let error = prepared.unwrap_err();
            assert!(error.message.contains(refusal), "{error:?}");
            assert_eq!(error.span.slice(source.as_bytes()), Some(&b"Reading::read(value)"[..]));
        } else {
            let PreparationAnswers::Frozen { constants, .. } = prepared.unwrap().answers else {
                panic!("constant should be frozen")
            };
            assert_eq!(constants[&trees.constants[0].declaration], PreparedValue::Integer(7.into()));
            let proof = check_literal_provider(trees, &slot, &target).unwrap();
            let error = evaluate(&progress, &slot, &capsule, Some(proof)).unwrap_err();
            assert!(
                error
                    .message
                    .contains("postcondition requires a checked proof for the arguments and result"),
                "{error:?}"
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
    let error = prepare(vec![prove(0)]).unwrap_err();
    assert!(error.message.contains("runtime boundary slot cannot execute"), "{error:?}");
    assert_eq!(error.span.slice(source.as_bytes()), Some(&b"Reading::read::<K>(value)"[..]));
}
