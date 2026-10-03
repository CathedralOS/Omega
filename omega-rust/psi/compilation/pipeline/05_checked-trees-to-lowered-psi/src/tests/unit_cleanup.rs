//! Structural Unit cleanup regression families.

use super::{CheckedTrees, LoweringError};
use crate::emission::scalar_types::terminal_scalar_type;
use crate::unit::unit_cleanup::{
    lower_nominal_affine_unit_cleanup_machine, lower_partial_affine_unit_cleanup_machine,
};
use checked_trees::types::PrimitiveType;
use checked_trees::{
    CheckedUnitCallPlan, CheckedUnitCallResult, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralFieldType, CheckedUnitStructuralPathSegment,
    CheckedUnitStructuralTypeShape,
};
use terminal_psi::{
    OperationKind, StructuralFieldType, StructuralPathSegment, StructuralTypeShape, Terminator,
};

/// The operation's call when it is an ordinary call with a Unit result.
fn ordinary_unit_call(
    operation: &mut CheckedUnitEffectOperationPlan,
) -> Option<&mut CheckedUnitCallPlan> {
    operation
        .call_mut()
        .filter(|call| !call.is_boundary() && *call.result() == CheckedUnitCallResult::Unit)
}

fn nominal_affine_unit_checked_fixture() -> CheckedTrees {
    let source = r#"
        data Token {}
        machine Token::drop(owner: &mut Token) {}
        data Root {}
        machine Root::enter(token: Token) {}
    "#;
    crate::front_end::checked_program(source)
}

fn nominal_affine_wide_scalar_unit_checked_fixture() -> CheckedTrees {
    let source = r#"
        data Token { flag: bool; tag: u8; delta: i16; payload: u64; address: addr; }
        machine Token::drop(owner: &mut Token) {}
        data Root {}
        machine Root::enter(token: Token) {}
    "#;
    crate::front_end::checked_program(source)
}

#[test]
fn nominal_affine_wide_scalar_unit_cleanup_retains_exact_field_shape() {
    let checked = nominal_affine_wide_scalar_unit_checked_fixture();
    let [plan] = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .machines
        .as_slice()
    else {
        panic!("expected one checked wide-scalar nominal-cleanup plan")
    };
    let lowered = lower_nominal_affine_unit_cleanup_machine(&checked, plan)
        .expect("wide flat scalar nominal cleanup should lower");
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("terminal entry");
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("wide scalar nominal cleanup requires its distinct terminal return")
    };
    let cleanup_type = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == cleanups[0].structural_type)
        .expect("nominal cleanup structural type");
    let StructuralTypeShape::Record { fields } = &cleanup_type.shape else {
        panic!("nominal scalar cleanup retains a record")
    };
    let [flag, tag, delta, payload, address] = fields.as_slice() else {
        panic!("nominal scalar cleanup retains every flat field")
    };
    for (field, identity, primitive) in [
        (flag, "flag", PrimitiveType::Bool),
        (tag, "tag", PrimitiveType::U8),
        (delta, "delta", PrimitiveType::I16),
        (payload, "payload", PrimitiveType::U64),
        (address, "address", PrimitiveType::Addr),
    ] {
        assert_eq!(field.identity, identity);
        assert!(!field.relevance.is_erased());
        let StructuralFieldType::Scalar(actual) = &field.field_type else {
            panic!("wide nominal cleanup field retains its scalar carrier")
        };
        assert_eq!(
            *actual,
            terminal_scalar_type(primitive).expect("fixture uses terminal-supported fields")
        );
    }

    for bad_field_type in [
        CheckedUnitStructuralFieldType::Scalar(PrimitiveType::F64),
        CheckedUnitStructuralFieldType::Erased {
            type_identity: "named(name(Erased))".to_owned(),
        },
        // `Root::enter` is static and retains no attachment; any structural
        // field is outside the bounded flat record.
        CheckedUnitStructuralFieldType::Structural {
            type_identity: "named(name(Root))".to_owned(),
        },
    ] {
        let mut stale = checked.clone();
        let shape = stale
            .facts
            .flow
            .terminal_nominal_affine_unit_cleanups
            .structural_types
            .iter_mut()
            .find(|shape| shape.identity == plan.cleanups[0].type_identity)
            .expect("nominal cleanup shape");
        let CheckedUnitStructuralTypeShape::Record { fields } = &mut shape.shape else {
            panic!("scalar fixture has a record shape")
        };
        fields[0].field_type = bad_field_type;
        let stale_plan = stale
            .facts
            .flow
            .terminal_nominal_affine_unit_cleanups
            .machines[0]
            .clone();
        assert!(matches!(
            lower_nominal_affine_unit_cleanup_machine(&stale, &stale_plan),
            Err(LoweringError::Unsupported(
                "ordered nominal cleanup parameter shape is outside the bound"
            ))
        ));
    }
}

#[test]
fn nominal_affine_unit_cleanup_lowering_rejects_stale_checked_joins() {
    let checked = nominal_affine_unit_checked_fixture();
    let original = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .machines[0]
        .clone();

    let mut stale = original.clone();
    stale.cleanups[0].source_parameter_index = 1;
    assert!(matches!(
        lower_nominal_affine_unit_cleanup_machine(&checked, &stale),
        Err(LoweringError::Unsupported(
            "ordered nominal cleanup parameter join drifted"
        ))
    ));

    let mut stale = original.clone();
    stale.cleanups[0].cleanup_contract_report_fingerprint ^= 1;
    assert!(matches!(
        lower_nominal_affine_unit_cleanup_machine(&checked, &stale),
        Err(LoweringError::Unsupported(
            "nominal cleanup target selection or borrowed subject drifted"
        ))
    ));

    let mut stale_checked = nominal_affine_unit_checked_fixture();
    let mut stale_plan = stale_checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .machines[0]
        .clone();
    let cleanup_target = stale_checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|machine| machine.machine == stale_plan.cleanups[0].cleanup_machine)
        .expect("cleanup target plan");
    cleanup_target.contract_report_fingerprint ^= 1;
    stale_plan.cleanups[0].cleanup_contract_report_fingerprint =
        cleanup_target.contract_report_fingerprint;
    assert!(matches!(
        lower_nominal_affine_unit_cleanup_machine(&stale_checked, &stale_plan),
        Err(LoweringError::Unsupported(
            "ordered nominal cleanup target is not exact and bounded"
        ))
    ));

    let mut stale_checked = nominal_affine_unit_checked_fixture();
    let stale_plan = stale_checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .machines[0]
        .clone();
    let cleanup_identity = stale_plan.cleanups[0].type_identity.clone();
    let shape = stale_checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .structural_types
        .iter_mut()
        .find(|shape| shape.identity == cleanup_identity)
        .expect("nominal cleanup shape");
    shape.shape = CheckedUnitStructuralTypeShape::FixedArray {
        element_type_identity: cleanup_identity,
        length: 1,
    };
    assert!(matches!(
        lower_nominal_affine_unit_cleanup_machine(&stale_checked, &stale_plan),
        Err(LoweringError::Unsupported(
            "ordered nominal cleanup parameter shape is outside the bound"
        ))
    ));

    let mut stale_checked = nominal_affine_unit_checked_fixture();
    stale_checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .push(original.machine.clone());
    assert!(matches!(
        lower_nominal_affine_unit_cleanup_machine(&stale_checked, &original),
        Err(LoweringError::Unsupported(
            "ordered nominal cleanup caller signature drifted"
        ))
    ));
}

#[test]
fn ordered_nominal_cleanup_lowering_deduplicates_a_shared_helper_across_two_actions() {
    let checked = crate::front_end::checked_program(
        r#"
        data Helper {}
        machine Helper::touch() {}
        data First {}
        machine First::drop(owner: &mut First) { Helper::touch(); }
        data Second {}
        machine Second::drop(owner: &mut Second) { Helper::touch(); }
        data Root {}
        machine Root::enter(first: First, second: Second) {}
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .machines[0]
        .clone();
    let lowered = lower_nominal_affine_unit_cleanup_machine(&checked, &plan)
        .expect("two executable cleanup actions may share one exact helper");
    assert_eq!(
        lowered.semantic_module.machines.len(),
        4,
        "the shared helper appears once in the exact machine closure"
    );
    let entry = &lowered.semantic_module.machines[0];
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("ordered nominal cleanup return")
    };
    let helper_ids = cleanups
        .iter()
        .map(|cleanup| {
            let target = lowered
                .semantic_module
                .machines
                .iter()
                .find(|machine| machine.id == cleanup.cleanup_machine)
                .expect("cleanup target");
            let [operation] = target.blocks[0].operations.as_slice() else {
                panic!("each cleanup target calls one helper")
            };
            let OperationKind::CallUnit { callee, .. } = operation.kind else {
                panic!("cleanup helper call")
            };
            callee
        })
        .collect::<Vec<_>>();
    assert_eq!(helper_ids[0], helper_ids[1]);

    let mut contextual = plan;
    contextual.caller_requirements.push(
        checked_trees::CheckedUnitNominalAffineCallerRequirementPlan {
            source_parameter_index: 0,
            field_identity: "flag".to_owned(),
            expected: true,
        },
    );
    assert!(matches!(
        lower_nominal_affine_unit_cleanup_machine(&checked, &contextual),
        Err(LoweringError::Unsupported(
            "contextual nominal cleanup requirement field is absent, erased, or non-Boolean"
        ))
    ));
}

fn partial_affine_unit_checked_fixture() -> CheckedTrees {
    let source = r#"
        domain [u8; 3]::Utf8
        requires
            valid_utf8(self);
        domain [u8; 8]::Utf8
        requires
            valid_utf8(self);
        data Token { value: u64; }
        data Quartet {
            before: u8;
            before_bytes: [u8; 3] in Utf8;
            before_float: f32;
            first: Token;
            between: bool;
            between_bytes: [u8; 8] in Utf8;
            between_float: f64;
            second: Token;
            third: Token;
            fourth: Token;
            after: u64;
        }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::enter(value: Quartet) {
            Sink::take(value.third);
            Sink::take(value.first);
        }
    "#;
    crate::front_end::checked_program(source)
}

fn nested_partial_affine_unit_checked_fixture() -> CheckedTrees {
    let source = r#"
        data Token { value: u64; }
        data Deep { low: Token; middle: Token; high: Token; }
        data Branch { head: Token; deep: Deep; tail: Token; }
        data Outer { first: Token; left: Branch; right: Branch; last: Token; }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::enter(value: Outer) {
            Sink::take(value.left.deep.middle);
            Sink::take(value.right.tail);
            Sink::take(value.first);
        }
    "#;
    crate::front_end::checked_program(source)
}

#[test]
fn mixed_partial_affine_unit_cleanup_lowers_recursive_maximal_residuals() {
    let checked = nested_partial_affine_unit_checked_fixture();
    let [plan] = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .machines
        .as_slice()
    else {
        panic!("expected one checked mixed partial-cleanup plan")
    };
    let lowered = lower_partial_affine_unit_cleanup_machine(&checked, plan)
        .expect("strict mixed partial cleanup should lower in memory")
        .terminal;
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("terminal entry");
    assert_eq!(
        entry.blocks[0]
            .operations
            .iter()
            .map(|operation| match &operation.kind {
                OperationKind::CallUnit {
                    structural_arguments,
                    ..
                } => structural_arguments[0].path.clone(),
                _ => panic!("mixed partial cleanup calls Unit"),
            })
            .collect::<Vec<_>>(),
        vec![
            vec![
                StructuralPathSegment::Field("left".to_owned()),
                StructuralPathSegment::Field("deep".to_owned()),
                StructuralPathSegment::Field("middle".to_owned()),
            ],
            vec![
                StructuralPathSegment::Field("right".to_owned()),
                StructuralPathSegment::Field("tail".to_owned()),
            ],
            vec![StructuralPathSegment::Field("first".to_owned())],
        ]
    );
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &entry.blocks[0].terminator
    else {
        panic!("mixed partial cleanup retains its distinct return")
    };
    assert_eq!(
        residual_affine_discards
            .iter()
            .map(|discard| discard.path.clone())
            .collect::<Vec<_>>(),
        vec![
            vec![StructuralPathSegment::Field("last".to_owned())],
            vec![
                StructuralPathSegment::Field("right".to_owned()),
                StructuralPathSegment::Field("deep".to_owned()),
            ],
            vec![
                StructuralPathSegment::Field("right".to_owned()),
                StructuralPathSegment::Field("head".to_owned()),
            ],
            vec![
                StructuralPathSegment::Field("left".to_owned()),
                StructuralPathSegment::Field("tail".to_owned()),
            ],
            vec![
                StructuralPathSegment::Field("left".to_owned()),
                StructuralPathSegment::Field("deep".to_owned()),
                StructuralPathSegment::Field("high".to_owned()),
            ],
            vec![
                StructuralPathSegment::Field("left".to_owned()),
                StructuralPathSegment::Field("deep".to_owned()),
                StructuralPathSegment::Field("low".to_owned()),
            ],
            vec![
                StructuralPathSegment::Field("left".to_owned()),
                StructuralPathSegment::Field("head".to_owned()),
            ],
        ]
    );

    let mut stale = plan.clone();
    let Some(call) = ordinary_unit_call(&mut stale.machine.operations[0]) else {
        unreachable!()
    };
    call.structural_arguments[0].path[2] =
        CheckedUnitStructuralPathSegment::Field("missing".to_owned());
    assert!(lower_partial_affine_unit_cleanup_machine(&checked, &stale).is_err());

    let mut overlapping = plan.clone();
    let [_, second, _, _] = overlapping.machine.operations.as_mut_slice() else {
        unreachable!()
    };
    let Some(call) = ordinary_unit_call(second) else {
        unreachable!()
    };
    call.structural_arguments[0].path =
        vec![CheckedUnitStructuralPathSegment::Field("left".to_owned())];
    assert!(matches!(
        lower_partial_affine_unit_cleanup_machine(&checked, &overlapping),
        Err(LoweringError::Unsupported(
            "partial affine Unit cleanup signature or coordinates drifted"
        ))
    ));
}

#[test]
fn partial_affine_unit_cleanup_lowering_rejects_stale_path_type_and_coordinates() {
    let checked = partial_affine_unit_checked_fixture();
    let original = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .machines[0]
        .clone();

    let mut stale = original.clone();
    stale.residual_affine_discards[0]
        .path
        .push(CheckedUnitStructuralPathSegment::Field("nested".to_owned()));
    assert!(matches!(
        lower_partial_affine_unit_cleanup_machine(&checked, &stale),
        Err(LoweringError::Unsupported(
            "partial affine Unit residual field partition drifted"
        ))
    ));

    let mut stale = original.clone();
    stale.residual_affine_discards[0].type_identity = "stale::Token".to_owned();
    assert!(matches!(
        lower_partial_affine_unit_cleanup_machine(&checked, &stale),
        Err(LoweringError::Unsupported(
            "partial affine Unit residual field partition drifted"
        ))
    ));

    let mut stale = original.clone();
    stale.residual_affine_discards.swap(0, 1);
    assert!(matches!(
        lower_partial_affine_unit_cleanup_machine(&checked, &stale),
        Err(LoweringError::Unsupported(
            "partial affine Unit residual field partition drifted"
        ))
    ));

    let mut stale = original.clone();
    let [first, second, _] = stale.machine.operations.as_mut_slice() else {
        unreachable!()
    };
    let Some(first_call) = ordinary_unit_call(first) else {
        unreachable!()
    };
    let first_path = first_call.structural_arguments[0].path.clone();
    let Some(second_call) = ordinary_unit_call(second) else {
        unreachable!()
    };
    second_call.structural_arguments[0].path = first_path;
    assert!(matches!(
        lower_partial_affine_unit_cleanup_machine(&checked, &stale),
        Err(LoweringError::Unsupported(
            "partial affine Unit cleanup signature or coordinates drifted"
        ))
    ));

    let mut stale_checked = partial_affine_unit_checked_fixture();
    let stale_plan = stale_checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .machines[0]
        .clone();
    let source_identity = stale_plan.machine.structural_parameters[0]
        .type_identity
        .clone();
    let shape = stale_checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .structural_types
        .iter_mut()
        .find(|shape| shape.identity == source_identity)
        .expect("partial source shape");
    let CheckedUnitStructuralTypeShape::Record { fields } = &mut shape.shape else {
        unreachable!()
    };
    let mut extra = fields
        .iter()
        .find(|field| {
            matches!(
                field.field_type,
                CheckedUnitStructuralFieldType::Structural { .. }
            )
        })
        .cloned()
        .expect("partial source retains a structural field");
    extra.identity = "extra".to_owned();
    fields.push(extra);
    assert!(matches!(
        lower_partial_affine_unit_cleanup_machine(&stale_checked, &stale_plan),
        Err(LoweringError::Unsupported(
            "partial affine Unit residual field partition drifted"
        ))
    ));

    let mut scalar_as_structural = partial_affine_unit_checked_fixture();
    let scalar_plan = scalar_as_structural
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .machines[0]
        .clone();
    let source_identity = scalar_plan.machine.structural_parameters[0]
        .type_identity
        .clone();
    let shape = scalar_as_structural
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .structural_types
        .iter_mut()
        .find(|shape| shape.identity == source_identity)
        .expect("partial source shape");
    let CheckedUnitStructuralTypeShape::Record { fields } = &mut shape.shape else {
        unreachable!()
    };
    let token_identity = fields
        .iter()
        .find_map(|field| match &field.field_type {
            CheckedUnitStructuralFieldType::Structural { type_identity } => {
                Some(type_identity.clone())
            }
            _ => None,
        })
        .expect("token type identity");
    fields
        .iter_mut()
        .find(|field| field.identity == "before_float")
        .expect("interleaved float field")
        .field_type = CheckedUnitStructuralFieldType::Structural {
        type_identity: token_identity,
    };
    assert!(matches!(
        lower_partial_affine_unit_cleanup_machine(&scalar_as_structural, &scalar_plan),
        Err(LoweringError::Unsupported(
            "partial affine Unit residual field partition drifted"
        ))
    ));

    let mut bounded_as_borrowed = partial_affine_unit_checked_fixture();
    let byte_plan = bounded_as_borrowed
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .machines[0]
        .clone();
    let source_identity = byte_plan.machine.structural_parameters[0]
        .type_identity
        .clone();
    let shape = bounded_as_borrowed
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .structural_types
        .iter_mut()
        .find(|shape| shape.identity == source_identity)
        .expect("partial source shape");
    let CheckedUnitStructuralTypeShape::Record { fields } = &mut shape.shape else {
        unreachable!()
    };
    fields
        .iter_mut()
        .find(|field| field.identity == "before_bytes")
        .expect("bounded byte field")
        .field_type = CheckedUnitStructuralFieldType::ByteSequence(
        checked_trees::CheckedByteSequenceCarrier::BorrowedView {
            access: Some(checked_trees::CheckedStructuralAccess::SharedBorrow),
        },
    );
    assert!(matches!(
        lower_partial_affine_unit_cleanup_machine(&bounded_as_borrowed, &byte_plan),
        Err(LoweringError::Unsupported(
            "partial affine Unit field path or type identity drifted"
        ))
    ));

    let mut moved_as_float = partial_affine_unit_checked_fixture();
    let moved_plan = moved_as_float
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .machines[0]
        .clone();
    let source_identity = moved_plan.machine.structural_parameters[0]
        .type_identity
        .clone();
    let shape = moved_as_float
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .structural_types
        .iter_mut()
        .find(|shape| shape.identity == source_identity)
        .expect("partial source shape");
    let CheckedUnitStructuralTypeShape::Record { fields } = &mut shape.shape else {
        unreachable!()
    };
    fields
        .iter_mut()
        .find(|field| field.identity == "third")
        .expect("moved structural field")
        .field_type = CheckedUnitStructuralFieldType::Scalar(PrimitiveType::F32);
    assert!(matches!(
        lower_partial_affine_unit_cleanup_machine(&moved_as_float, &moved_plan),
        Err(LoweringError::Unsupported(
            "partial affine Unit field path or type identity drifted"
        ))
    ));

    let mut stale = original;
    let Some(call) = ordinary_unit_call(&mut stale.machine.operations[0]) else {
        unreachable!()
    };
    call.coordinate.statement_index = 1;
    assert!(matches!(
        lower_partial_affine_unit_cleanup_machine(&checked, &stale),
        Err(LoweringError::Unsupported(
            "partial affine Unit cleanup signature or coordinates drifted"
        ))
    ));
}

#[test]
fn contextual_nominal_member_contracts_share_the_ordinary_obligation_namespace() {
    for hook_body in ["", "token.ready = false;"] {
        let checked = crate::front_end::checked_program(&format!(
            "data Token {{ ready: bool; }}
             machine Token::drop(token: &mut Token) requires token.ready {{ {hook_body} }}
             machine consume(first: Token, second: Token)
                 requires first.ready; second.ready {{}}
             machine demand(flag: bool) requires flag {{}}
             machine entry(first: Token, second: Token, flag: bool)
                 requires first.ready; second.ready; flag {{
                 demand(flag);
                 consume(first, second);
             }}"
        ));
        let lowered =
            crate::lower_machine(&checked, crate::TerminalMachineSelection::Name("entry"))
                .expect("contextual cleanup is an ordinary called member");
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("each ordinary call and cleanup has independent evidence");
        let module = &lowered.semantic_module;
        let entry = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        assert_eq!(
            entry.contract.requires.len(),
            3,
            "authored caller demands stay exact"
        );
        let (member, cleanups) = module
            .machines
            .iter()
            .find_map(|machine| {
                machine
                    .blocks
                    .iter()
                    .find_map(|block| match &block.terminator {
                        Terminator::ReturnUnitNominalAffine { cleanups, .. } => {
                            Some((machine, cleanups))
                        }
                        _ => None,
                    })
            })
            .expect("consuming member has nominal actions");
        assert_ne!(member.id, module.entry);
        assert_eq!(member.contract.requires.len(), 2);
        assert_eq!(cleanups.len(), 2);
        assert_eq!(cleanups[0].place, member.structural_parameters[1].place);
        assert_eq!(cleanups[1].place, member.structural_parameters[0].place);
        assert_eq!(cleanups[0].cleanup_machine, cleanups[1].cleanup_machine);
        assert_eq!(cleanups[0].cleanup_receiver, cleanups[1].cleanup_receiver);
        assert_ne!(
            cleanups[0].requirement_obligations,
            cleanups[1].requirement_obligations
        );
        let hook = module
            .machines
            .iter()
            .find(|machine| machine.id == cleanups[0].cleanup_machine)
            .unwrap();
        let receiver = cleanups[0]
            .cleanup_receiver
            .expect("contextual proof subject");
        assert_eq!(hook.contract.requires.len(), 1);
        assert_eq!(
            hook.structural_parameters[0].place, receiver,
            "the named source receiver remains a runtime parameter even in an empty body"
        );
        if hook_body.is_empty() {
            // Exercise the receiving representation independently: source lowering
            // retains named parameters, while Terminal also admits a hook whose
            // receiver exists only as the subject of its cleanup prerequisites.
            let mut proof_only = module.clone();
            let target = proof_only
                .machines
                .iter_mut()
                .find(|machine| machine.id == hook.id)
                .unwrap();
            assert!(
                target
                    .blocks
                    .iter()
                    .all(|block| block.operations.is_empty())
            );
            target.structural_parameters.clear();
            target
                .structural_places
                .retain(|place| place.id != receiver);
            terminal_verifier::verify_module(
                &proof_only,
                &lowered.proof_bundle,
                &proof_admission::AdmissionProfile::default(),
            )
            .expect("proof-only cleanup subjects retain exact per-owner evidence");
        }
        let mut identities = cleanups
            .iter()
            .flat_map(|cleanup| cleanup.requirement_obligations.iter().copied())
            .collect::<Vec<_>>();
        for machine in &module.machines {
            for block in &machine.blocks {
                for operation in &block.operations {
                    if let OperationKind::CallUnit {
                        requirement_obligations,
                        ..
                    } = &operation.kind
                    {
                        identities.extend(requirement_obligations.iter().copied());
                    }
                }
            }
        }
        assert!(
            identities.len() >= 5,
            "ordinary and cleanup clauses coexist"
        );
        identities.sort();
        assert!(identities.windows(2).all(|pair| pair[0] != pair[1]));

        let mut wrong_root = lowered.semantic_module.clone();
        let consumer = wrong_root
            .machines
            .iter_mut()
            .find(|machine| machine.id == member.id)
            .unwrap();
        let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
            &mut consumer.blocks[0].terminator
        else {
            unreachable!()
        };
        cleanups[0].place = cleanups[1].place;
        assert!(
            terminal_verifier::validate_module(&wrong_root).is_err(),
            "one owner's prerequisite cannot dispose of another"
        );
    }
}
