//! Structural Unit cleanup regression families.

use super::*;

fn nominal_affine_unit_checked_fixture() -> CheckedTrees {
    let source = r#"
        data Token {}
        machine Token::drop(&mut self) {}
        data Root {}
        machine Root::enter(token: Token) {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

fn nominal_affine_wide_scalar_unit_checked_fixture() -> CheckedTrees {
    let source = r#"
        data Token { flag: bool; tag: u8; delta: i16; payload: u64; address: addr; }
        machine Token::drop(&mut self) {}
        data Root {}
        machine Root::enter(token: Token) {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

fn ordered_one_executable_nominal_affine_checked_fixture() -> CheckedTrees {
    let source = r#"
        data Helper {}
        machine Helper::touch() {}
        data First {}
        machine First::drop(&mut self) { Helper::touch(); }
        data Second {}
        machine Second::drop(&mut self) {}
        data Root {}
        machine Root::enter(first: First, second: Second) {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

#[test]
fn nominal_affine_unit_cleanup_lowers_exact_target_into_terminal_closure() {
    let checked = nominal_affine_unit_checked_fixture();
    let [plan] = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .machines
        .as_slice()
    else {
        panic!("expected one checked nominal-cleanup plan")
    };
    let lowered = lower_nominal_affine_unit_cleanup_machine(&checked, plan)
        .expect("strict checked nominal cleanup should lower in memory");
    assert_eq!(
        lowered.semantic_module.machines.len(),
        2,
        "cleanup target must be retained as executable closure work"
    );
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("terminal entry");
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("nominal cleanup requires its distinct terminal return")
    };
    assert_eq!(cleanups[0].place, entry.structural_parameters[0].place);
    assert_eq!(
        cleanups[0].structural_type,
        entry.structural_parameters[0].structural_type
    );
    let target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == cleanups[0].cleanup_machine)
        .expect("terminal cleanup target");
    assert_eq!(target.attachment, Some(cleanups[0].structural_type));
    assert!(target.structural_parameters.is_empty());
    assert!(target.blocks[0].operations.is_empty());
    assert!(matches!(
        &target.blocks[0].terminator,
        Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        } if trivial_affine_discards.is_empty()
    ));
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("independent verifier accepts exact nominal cleanup closure");
    let bytes = terminal_codec::encode_module(&lowered.semantic_module)
        .expect("verified nominal cleanup should encode canonically");
    assert_eq!(
        terminal_codec::decode_module(&bytes).expect("canonical nominal cleanup should decode"),
        lowered.semantic_module
    );

    let entry_name = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find(|selection| selection.machine == plan.machine.machine)
        .expect("nominal cleanup terminal selection")
        .name
        .clone();
    let public = lower_machine(&checked, &entry_name)
        .expect("source nominal cleanup should cross the public lowering entry");
    assert!(matches!(
        public.semantic_module.machines[0].blocks[0].terminator,
        Terminator::ReturnUnitNominalAffine { .. }
    ));
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
        CheckedUnitStructuralFieldType::Structural {
            type_identity: plan
                .machine
                .attachment_type_identity
                .clone()
                .expect("nominal cleanup fixture remains attached"),
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
                "nominal affine Unit parameter is outside the bounded record shape"
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
            "nominal affine Unit cleanup signature or coordinates drifted"
        ))
    ));

    let mut stale = original.clone();
    stale.cleanups[0].cleanup_contract_report_fingerprint ^= 1;
    assert!(matches!(
        lower_nominal_affine_unit_cleanup_machine(&checked, &stale),
        Err(LoweringError::Unsupported(
            "nominal cleanup target identity or bounded signature drifted"
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
            "nominal cleanup target identity or bounded signature drifted"
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
            "nominal affine Unit parameter is outside the bounded record shape"
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
            "nominal affine Unit machine is also published in the trivial lane"
        ))
    ));
}

#[test]
fn ordered_nominal_cleanup_lowering_deduplicates_a_shared_helper_across_two_actions() {
    let mut checked = ordered_one_executable_nominal_affine_checked_fixture();
    let plan = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .machines[0]
        .clone();
    let [empty_cleanup, executable_cleanup] = plan.cleanups.as_slice() else {
        panic!("fixture has two ordered cleanup actions")
    };
    let executable_operation = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(executable_cleanup.cleanup_machine)
        .expect("executable cleanup target")
        .operations[0]
        .clone();
    let empty_target = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|target| target.machine == empty_cleanup.cleanup_machine)
        .expect("empty cleanup target");
    empty_target.operations.insert(0, executable_operation);
    let CheckedUnitEffectOperationPlan::ReturnUnit {
        statement_index, ..
    } = empty_target
        .operations
        .last_mut()
        .expect("cleanup target return")
    else {
        panic!("cleanup target ends in Unit return")
    };
    *statement_index = 1;

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
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
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
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

#[test]
fn partial_affine_unit_cleanup_lowers_exact_terminal_paths_before_verification() {
    let checked = partial_affine_unit_checked_fixture();
    let [plan] = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .machines
        .as_slice()
    else {
        panic!("expected one checked partial-cleanup plan")
    };
    let lowered = lower_partial_affine_unit_cleanup_machine(&checked, plan)
        .expect("strict checked partial cleanup should lower in memory");
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("terminal entry");
    let [first_call, second_call] = entry.blocks[0].operations.as_slice() else {
        panic!("partial cleanup entry should contain both source-ordered calls")
    };
    let moved_paths = [first_call, second_call]
        .into_iter()
        .map(|call| {
            let OperationKind::CallUnit {
                structural_arguments,
                claim_transfers,
                ..
            } = &call.kind
            else {
                panic!("partial cleanup entry should call Unit")
            };
            assert!(claim_transfers.is_empty());
            structural_arguments[0].path.clone()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        moved_paths,
        vec![
            vec![StructuralPathSegment::Field("third".to_owned())],
            vec![StructuralPathSegment::Field("first".to_owned())],
        ]
    );
    let root_type = entry.structural_parameters[0].structural_type;
    let root = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == root_type)
        .expect("mixed float partial root type");
    let StructuralTypeShape::Record { fields } = &root.shape else {
        panic!("mixed float partial root remains a record")
    };
    assert_eq!(
        fields
            .iter()
            .filter_map(|field| match field.field_type {
                StructuralFieldType::IeeeFloat(format) => {
                    Some((field.identity.as_str(), format))
                }
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![
            (
                "before_float",
                semantic_vocabulary::IeeeFloatFormat::Binary32
            ),
            (
                "between_float",
                semantic_vocabulary::IeeeFloatFormat::Binary64
            ),
        ]
    );
    assert_eq!(
        fields
            .iter()
            .filter_map(|field| match field.field_type {
                StructuralFieldType::ByteSequence(carrier) => {
                    Some((field.identity.as_str(), carrier))
                }
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![
            (
                "before_bytes",
                terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity: 3 },
            ),
            (
                "between_bytes",
                terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity: 8 },
            ),
        ],
        "Terminal identity retains both exact bounded byte capacities"
    );
    let Terminator::ReturnUnitPartialAffine {
        trivial_affine_discards,
        residual_affine_discards,
        ..
    } = &entry.blocks[0].terminator
    else {
        panic!("partial cleanup requires its distinct terminal return")
    };
    assert!(trivial_affine_discards.is_empty());
    assert_eq!(residual_affine_discards.len(), 2);
    assert_eq!(
        residual_affine_discards
            .iter()
            .map(|discard| discard.path.clone())
            .collect::<Vec<_>>(),
        vec![
            vec![StructuralPathSegment::Field("fourth".to_owned())],
            vec![StructuralPathSegment::Field("second".to_owned())],
        ]
    );
    for residual in residual_affine_discards {
        assert_eq!(residual.place, entry.structural_parameters[0].place);
        assert!(
            lowered
                .semantic_module
                .structural_types
                .iter()
                .any(|declaration| declaration.id == residual.structural_type
                    && declaration.identity.contains("Token"))
        );
    }
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("independent verifier proves moved field plus residual cleanup exhausts root");
    let bytes = terminal_codec::encode_module(&lowered.semantic_module)
        .expect("verified partial affine cleanup should encode canonically");
    assert_eq!(
        terminal_codec::decode_module(&bytes)
            .expect("canonical partial affine cleanup should decode"),
        lowered.semantic_module
    );
    let entry_name = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find(|selection| selection.machine == plan.machine.machine)
        .expect("partial cleanup terminal selection")
        .name
        .clone();
    lower_machine(&checked, &entry_name)
        .expect("verified partial affine cleanup should cross the ordinary lowering entry");
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
        .expect("strict mixed partial cleanup should lower in memory");
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
    let CheckedUnitEffectOperationPlan::CallUnit {
        structural_arguments,
        ..
    } = &mut stale.machine.operations[0]
    else {
        unreachable!()
    };
    structural_arguments[0].path[2] = CheckedUnitStructuralPathSegment::Field("missing".to_owned());
    assert!(lower_partial_affine_unit_cleanup_machine(&checked, &stale).is_err());

    let mut overlapping = plan.clone();
    let [_, second, _, _] = overlapping.machine.operations.as_mut_slice() else {
        unreachable!()
    };
    let CheckedUnitEffectOperationPlan::CallUnit {
        structural_arguments,
        ..
    } = second
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![CheckedUnitStructuralPathSegment::Field("left".to_owned())];
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
    let CheckedUnitEffectOperationPlan::CallUnit {
        structural_arguments: first_arguments,
        ..
    } = first
    else {
        unreachable!()
    };
    let first_path = first_arguments[0].path.clone();
    let CheckedUnitEffectOperationPlan::CallUnit {
        structural_arguments: second_arguments,
        ..
    } = second
    else {
        unreachable!()
    };
    second_arguments[0].path = first_path;
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
        checked_trees::CheckedByteSequenceCarrier::BorrowedView,
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
    let CheckedUnitEffectOperationPlan::CallUnit { coordinate, .. } =
        &mut stale.machine.operations[0]
    else {
        unreachable!()
    };
    coordinate.statement_index = 1;
    assert!(matches!(
        lower_partial_affine_unit_cleanup_machine(&checked, &stale),
        Err(LoweringError::Unsupported(
            "partial affine Unit cleanup signature or coordinates drifted"
        ))
    ));
}
