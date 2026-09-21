use super::{
    ADDRESS_RECORD_EQUALITY_SOURCE, BYTE_SEQUENCE_AGGREGATE_EQUALITY_SOURCE,
    EMPTY_RECORD_EQUALITY_SOURCE, FIXED_INDEX_SOURCE, IEEE_FLOAT_AGGREGATE_EQUALITY_SOURCE,
    MIXED_AGGREGATE_EQUALITY_FENCE_SOURCES, NESTED_MIXED_AGGREGATE_EQUALITY_FENCE_SOURCES,
    NESTED_RECORD_PAYLOAD_SUM_EQUALITY_SOURCE, NESTED_SOURCE,
    NESTED_SUM_PAYLOAD_SUM_EQUALITY_SOURCE,
};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IeeeFloatFormat, Proposition, ScalarTerm, StructuralFieldId,
};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_section};
use terminal_fixed_fuel::{derive_fixed_entry_fuel, validate_fixed_entry_fuel};
use terminal_interpreter::TerminalStructuralInputs;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecutionResult,
    TerminalStructuralValue, interpret_terminal_artifact_measured,
};
use terminal_psi::{
    CrashPredicateTerm, CrashRouteGuard, OperationKind, StructuralFieldType, StructuralPathSegment,
    StructuralTypeShape,
};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

#[test]
fn unsupported_mixed_aggregate_equality_shapes_remain_fenced() {
    for source in MIXED_AGGREGATE_EQUALITY_FENCE_SOURCES
        .into_iter()
        .chain(NESTED_MIXED_AGGREGATE_EQUALITY_FENCE_SOURCES)
    {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
        let typed = match lower_symbol_resolved_trees(&resolved) {
            Ok(typed) => typed,
            Err(_) => continue,
        };
        let checked = match lower_typed_trees(typed) {
            Ok(checked) => checked,
            Err(diagnostics) => {
                assert!(!diagnostics.is_empty());
                continue;
            }
        };
        let result = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter");
        assert!(matches!(
            result,
            Err(checked_trees_to_lowered_psi::LoweringError::Unsupported(
                "structural crash route is outside checked Boolean member lowering"
            ))
        ));
    }
}

#[test]
fn payload_sum_nested_record_equality_rebases_and_replays_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn collect_scalar_fields(
        term: &ScalarTerm,
        fields: &mut Vec<(
            semantic_vocabulary::PlaceId,
            Vec<CanonicalStructuralPathSegment>,
        )>,
    ) {
        match term {
            ScalarTerm::BooleanField { root, path }
            | ScalarTerm::IntegerField { root, path, .. } => {
                fields.push((*root, path.clone()));
            }
            ScalarTerm::BooleanEqual { left, right }
            | ScalarTerm::IntegerEqual { left, right, .. } => {
                collect_scalar_fields(left, fields);
                collect_scalar_fields(right, fields);
            }
            _ => {}
        }
    }

    fn collect_paths(
        proposition: &Proposition,
        memberships: &mut Vec<(
            semantic_vocabulary::PlaceId,
            Vec<CanonicalStructuralPathSegment>,
            semantic_vocabulary::StructuralCaseId,
        )>,
        fields: &mut Vec<(
            semantic_vocabulary::PlaceId,
            Vec<CanonicalStructuralPathSegment>,
        )>,
    ) {
        match proposition {
            Proposition::StructuralCaseMembership { subject, case } => {
                memberships.push((subject.root(), subject.path().to_vec(), *case));
            }
            Proposition::Equal(left, right)
            | Proposition::LessThan(left, right)
            | Proposition::LessOrEqual(left, right)
            | Proposition::ScalarIeeeFloatComparison { left, right, .. } => {
                collect_scalar_fields(left, fields);
                collect_scalar_fields(right, fields);
            }
            Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
                for child in children {
                    collect_paths(child, memberships, fields);
                }
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                collect_paths(premise, memberships, fields);
                collect_paths(conclusion, memberships, fields);
            }
            Proposition::Truth
            | Proposition::Falsehood
            | Proposition::Atom(_)
            | Proposition::IntegerMathEqual(_, _)
            | Proposition::IntegerMathLessThan(_, _)
            | Proposition::IntegerMathLessOrEqual(_, _)
            | Proposition::IeeeFloatComparison { .. }
            | Proposition::ByteSequenceEqual { .. }
            | Proposition::ContentConservation(_) => {}
        }
    }

    fn redirect_integer_leaf(
        proposition: &mut Proposition,
        from: StructuralFieldId,
        to: StructuralFieldId,
    ) -> bool {
        fn redirect_term(
            term: &mut ScalarTerm,
            from: StructuralFieldId,
            to: StructuralFieldId,
        ) -> bool {
            match term {
                ScalarTerm::IntegerField { path, .. } => {
                    let Some(CanonicalStructuralPathSegment::Field(field)) = path.last_mut() else {
                        return false;
                    };
                    if *field != from {
                        return false;
                    }
                    *field = to;
                    true
                }
                ScalarTerm::BooleanEqual { left, right }
                | ScalarTerm::IntegerEqual { left, right, .. } => {
                    redirect_term(left, from, to) || redirect_term(right, from, to)
                }
                _ => false,
            }
        }

        match proposition {
            Proposition::Equal(left, right)
            | Proposition::LessThan(left, right)
            | Proposition::LessOrEqual(left, right) => {
                redirect_term(left, from, to) || redirect_term(right, from, to)
            }
            Proposition::Conjunction(children) | Proposition::Disjunction(children) => children
                .iter_mut()
                .any(|child| redirect_integer_leaf(child, from, to)),
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                redirect_integer_leaf(premise, from, to)
                    || redirect_integer_leaf(conclusion, from, to)
            }
            _ => false,
        }
    }

    let tokens = Lexer::new(NESTED_RECORD_PAYLOAD_SUM_EQUALITY_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("payload-sum equality expands the nested record and lowers");
    let different = checked_trees_to_lowered_psi::lower_machine(&checked, "Different::enter")
        .expect("payload-sum inequality expands the nested record and lowers");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let message = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == root.structural_parameters[0].structural_type)
        .expect("Message structural type");
    let StructuralTypeShape::Sum { cases } = &message.shape else {
        panic!("Message is a sum")
    };
    let data_case = cases
        .iter()
        .find(|case| case.identity == "Data")
        .expect("Data case");
    let detail_field = data_case
        .fields
        .iter()
        .find(|field| field.identity == "detail")
        .expect("detail payload field");
    let StructuralFieldType::Structural(detail_type) = detail_field.field_type else {
        panic!("detail payload retains its record type")
    };
    let detail = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == detail_type)
        .expect("Detail structural type");
    let StructuralTypeShape::Record {
        fields: detail_fields,
    } = &detail.shape
    else {
        panic!("Detail is a record")
    };
    let active_field = detail_fields
        .iter()
        .find(|field| field.identity == "active")
        .expect("active field");
    let counter_field = detail_fields
        .iter()
        .find(|field| field.identity == "counter")
        .expect("counter field");
    let StructuralFieldType::Structural(counter_type) = counter_field.field_type else {
        panic!("counter field retains its record type")
    };
    let counter = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == counter_type)
        .expect("Counter structural type");
    let StructuralTypeShape::Record {
        fields: counter_fields,
    } = &counter.shape
    else {
        panic!("Counter is a record")
    };
    let count_field = counter_fields
        .iter()
        .find(|field| field.identity == "count")
        .expect("count leaf");

    for module in [&lowered.semantic_module, &different.semantic_module] {
        let entry = &module.machines[0];
        let [CrashRouteGuard::Predicate(route)] =
            entry.contract.crash_routes[0].alternatives.as_slice()
        else {
            panic!("entry publishes one nested-record payload predicate")
        };
        let mut memberships = Vec::new();
        let mut field_paths = Vec::new();
        collect_paths(route.proposition(), &mut memberships, &mut field_paths);
        assert_eq!(memberships.len(), 4);
        assert!(memberships.iter().all(|(place, path, _)| {
            entry
                .structural_parameters
                .iter()
                .any(|parameter| parameter.place == *place)
                && path.is_empty()
        }));
        assert_eq!(field_paths.len(), 4);
        assert!(field_paths.iter().all(|(place, path)| {
            entry
                .structural_parameters
                .iter()
                .any(|parameter| parameter.place == *place)
                && match path.as_slice() {
                    [
                        CanonicalStructuralPathSegment::Case(case),
                        CanonicalStructuralPathSegment::Field(payload),
                        CanonicalStructuralPathSegment::Field(leaf),
                    ] => {
                        *case == data_case.id
                            && *payload == detail_field.id
                            && *leaf == active_field.id
                    }
                    [
                        CanonicalStructuralPathSegment::Case(case),
                        CanonicalStructuralPathSegment::Field(payload),
                        CanonicalStructuralPathSegment::Field(record),
                        CanonicalStructuralPathSegment::Field(leaf),
                    ] => {
                        *case == data_case.id
                            && *payload == detail_field.id
                            && *record == counter_field.id
                            && *leaf == count_field.id
                    }
                    _ => false,
                }
        }));
    }

    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        unreachable!()
    };
    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        unreachable!()
    };
    let mut helper_memberships = Vec::new();
    let mut helper_fields = Vec::new();
    collect_paths(
        helper_route.proposition(),
        &mut helper_memberships,
        &mut helper_fields,
    );
    assert!(helper_fields.iter().all(|(place, _)| {
        helper
            .structural_parameters
            .iter()
            .any(|parameter| parameter.place == *place)
    }));
    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("Root emits the helper call")
    };
    assert_eq!(structural_arguments.len(), 2);
    assert!(
        structural_arguments
            .iter()
            .all(|argument| argument.path.is_empty())
    );
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call retains one rebased crash continuation")
    };
    assert_eq!(continuation, root_route);

    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier reconstructs nested payload-record paths through the helper call");
    terminal_verifier::verify_module(
        &different.semantic_module,
        &different.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier accepts nested payload-record inequality");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("nested payload-record equality has fixed fuel");
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");
    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let arguments = root
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| TerminalStructuralValue {
            opaque_identity: 71 + u64::try_from(index).unwrap(),
            structural_type: parameter.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let measured = interpret_terminal_artifact_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &arguments,
            ..Default::default()
        },
        &mut Accept,
    )
    .expect("nested payload-record equality remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut redirected = lowered.semantic_module.clone();
    let CrashRouteGuard::Predicate(predicate) =
        &mut redirected.machines[0].contract.crash_routes[0].alternatives[0]
    else {
        unreachable!()
    };
    let mut proposition = predicate.proposition().clone();
    assert!(redirect_integer_leaf(
        &mut proposition,
        count_field.id,
        active_field.id,
    ));
    *predicate = CrashPredicateTerm::new(proposition);
    let invalid_result = terminal_verifier::validate_module(&redirected);
    assert!(
        matches!(
            invalid_result,
            Err(terminal_verifier::ModuleError::CallCrashContinuationUncovered { .. })
        ),
        "redirecting the exact nested leaf must break the independently reconstructed call continuation: {invalid_result:?}"
    );
}

#[test]
fn payload_sum_nested_sum_equality_replays_end_to_end() {
    fn collect_scalar_paths(
        term: &ScalarTerm,
        paths: &mut Vec<Vec<CanonicalStructuralPathSegment>>,
    ) {
        match term {
            ScalarTerm::BooleanField { path, .. } | ScalarTerm::IntegerField { path, .. } => {
                paths.push(path.clone())
            }
            ScalarTerm::BooleanEqual { left, right }
            | ScalarTerm::IntegerEqual { left, right, .. } => {
                collect_scalar_paths(left, paths);
                collect_scalar_paths(right, paths);
            }
            _ => {}
        }
    }

    fn collect_paths(
        proposition: &Proposition,
        paths: &mut Vec<Vec<CanonicalStructuralPathSegment>>,
    ) {
        match proposition {
            Proposition::Equal(left, right)
            | Proposition::LessThan(left, right)
            | Proposition::LessOrEqual(left, right)
            | Proposition::ScalarIeeeFloatComparison { left, right, .. } => {
                collect_scalar_paths(left, paths);
                collect_scalar_paths(right, paths);
            }
            Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
                for child in children {
                    collect_paths(child, paths);
                }
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                collect_paths(premise, paths);
                collect_paths(conclusion, paths);
            }
            Proposition::Truth
            | Proposition::Falsehood
            | Proposition::Atom(_)
            | Proposition::IntegerMathEqual(_, _)
            | Proposition::IntegerMathLessThan(_, _)
            | Proposition::IntegerMathLessOrEqual(_, _)
            | Proposition::StructuralCaseMembership { .. }
            | Proposition::IeeeFloatComparison { .. }
            | Proposition::ByteSequenceEqual { .. }
            | Proposition::ContentConservation(_) => {}
        }
    }

    let tokens = Lexer::new(NESTED_SUM_PAYLOAD_SUM_EQUALITY_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("payload-sum equality expands the nested sum and lowers");

    let root = &lowered.semantic_module.machines[0];
    let [CrashRouteGuard::Predicate(route)] = root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("nested sum equality publishes one predicate")
    };
    let mut paths = Vec::new();
    collect_paths(route.proposition(), &mut paths);
    assert_eq!(paths.len(), 2, "both integer roots remain explicit");
    assert!(paths.iter().all(|path| {
        matches!(
            path.as_slice(),
            [
                CanonicalStructuralPathSegment::Case(_),
                CanonicalStructuralPathSegment::Field(_),
                CanonicalStructuralPathSegment::Case(_),
                CanonicalStructuralPathSegment::Field(_),
            ]
        )
    }));

    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier accepts exact nested-sum payload paths");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("nested-sum equality has fixed fuel");
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");
    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
}

#[test]
fn ieee_float_aggregate_equality_is_atomic_and_canonical_end_to_end() {
    let tokens = Lexer::new(IEEE_FLOAT_AGGREGATE_EQUALITY_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("IEEE aggregate equality lowers");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one IEEE aggregate route")
    };
    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one IEEE aggregate route")
    };
    let inspect = |proposition: &Proposition,
                   left_root: semantic_vocabulary::PlaceId,
                   right_root: semantic_vocabulary::PlaceId| {
        let Proposition::Conjunction(conjuncts) = proposition else {
            panic!("two-float equality is one conjunction")
        };
        assert_eq!(conjuncts.len(), 2);
        let mut formats = Vec::new();
        for conjunct in conjuncts {
            let Proposition::IeeeFloatComparison {
                format,
                left,
                right,
                ..
            } = conjunct
            else {
                panic!("float leaves remain atomic IEEE propositions")
            };
            assert_eq!((left.root(), right.root()), (left_root, right_root));
            assert_eq!(left.path().len(), 1);
            assert_eq!(right.path().len(), 1);
            formats.push(*format);
        }
        formats.sort();
        assert_eq!(
            formats,
            [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64]
        );
    };
    inspect(
        root_route.proposition(),
        root.structural_parameters[0].place,
        root.structural_parameters[1].place,
    );
    inspect(
        helper_route.proposition(),
        helper.structural_parameters[0].place,
        helper.structural_parameters[1].place,
    );

    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call retains the IEEE continuation")
    };
    assert_eq!(continuation, root_route);

    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier checks IEEE leaf formats and substituted roots");
    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );

    let reversed = checked_trees_to_lowered_psi::lower_machine(&checked, "Reverse::enter")
        .expect("reversed IEEE operands lower canonically");
    let [CrashRouteGuard::Predicate(reversed_route)] =
        reversed.semantic_module.machines[0].contract.crash_routes[0]
            .alternatives
            .as_slice()
    else {
        unreachable!()
    };
    let Proposition::Conjunction(reversed_conjuncts) = reversed_route.proposition() else {
        unreachable!()
    };
    assert!(reversed_conjuncts.iter().all(|item| matches!(
        item,
        Proposition::IeeeFloatComparison { left, right, .. } if left <= right
    )));
    encode_module(&reversed.semantic_module).expect("canonical reversed semantic encode");

    let different = checked_trees_to_lowered_psi::lower_machine(&checked, "Different::enter")
        .expect("direct IEEE inequality lowers atomically");
    let [CrashRouteGuard::Predicate(different_route)] =
        different.semantic_module.machines[0].contract.crash_routes[0]
            .alternatives
            .as_slice()
    else {
        unreachable!()
    };
    assert!(matches!(
        different_route.proposition(),
        Proposition::IeeeFloatComparison {
            kind: semantic_vocabulary::IeeeFloatComparisonKind::NotEqual,
            format: IeeeFloatFormat::Binary32,
            ..
        }
    ));
    terminal_verifier::verify_module(
        &different.semantic_module,
        &different.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier retains direct IEEE inequality");
    let different_bytes = encode_module(&different.semantic_module).expect("inequality encode");
    assert_eq!(
        decode_module(&different_bytes),
        Ok(different.semantic_module)
    );

    let aggregate_different =
        checked_trees_to_lowered_psi::lower_machine(&checked, "AggregateDifferent::enter")
            .expect("aggregate IEEE inequality lowers as canonical negation");
    let aggregate_root = &aggregate_different.semantic_module.machines[0];
    let [CrashRouteGuard::Predicate(aggregate_route)] = aggregate_root.contract.crash_routes[0]
        .alternatives
        .as_slice()
    else {
        unreachable!()
    };
    let Proposition::Implication {
        premise,
        conclusion,
    } = aggregate_route.proposition()
    else {
        panic!("aggregate IEEE inequality is equality implying falsehood")
    };
    assert_eq!(conclusion.as_ref(), &Proposition::Falsehood);
    let Proposition::Conjunction(equalities) = premise.as_ref() else {
        panic!("two-field aggregate equality remains the negated conjunction")
    };
    assert!(equalities.iter().all(|proposition| matches!(
        proposition,
        Proposition::IeeeFloatComparison {
            kind: semantic_vocabulary::IeeeFloatComparisonKind::Equal,
            ..
        }
    )));
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &aggregate_root.blocks[0].operations[0].kind
    else {
        panic!("aggregate inequality caller emits one structural Unit call")
    };
    let [CrashRouteGuard::Predicate(aggregate_continuation)] =
        crash_continuations[0].alternatives.as_slice()
    else {
        unreachable!()
    };
    assert_eq!(aggregate_continuation, aggregate_route);
    terminal_verifier::verify_module(
        &aggregate_different.semantic_module,
        &aggregate_different.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier reconstructs aggregate IEEE negation through the call");

    let mut redirected_aggregate = aggregate_different.semantic_module.clone();
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut redirected_aggregate.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let CrashRouteGuard::Predicate(predicate) = &mut crash_continuations[0].alternatives[0] else {
        unreachable!()
    };
    let mut proposition = predicate.proposition().clone();
    let Proposition::Implication { premise, .. } = &mut proposition else {
        unreachable!()
    };
    let Proposition::Conjunction(conjuncts) = premise.as_mut() else {
        unreachable!()
    };
    let Some(Proposition::IeeeFloatComparison { left, right, .. }) = conjuncts
        .iter_mut()
        .find(|item| matches!(item, Proposition::IeeeFloatComparison { .. }))
    else {
        unreachable!()
    };
    *right = semantic_vocabulary::IeeeFloatStructuralField::new(left.root(), right.path().to_vec())
        .expect("redirected IEEE field path remains nonempty");
    *predicate = CrashPredicateTerm::new(proposition);
    let invalid_result = terminal_verifier::validate_module(&redirected_aggregate);
    assert!(
        matches!(
            invalid_result,
            Err(terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected aggregate IEEE inequality validation result: {invalid_result:?}"
    );

    let aggregate_bytes =
        encode_module(&aggregate_different.semantic_module).expect("aggregate inequality encode");
    assert_eq!(
        decode_module(&aggregate_bytes),
        Ok(aggregate_different.semantic_module)
    );

    let projected_different =
        checked_trees_to_lowered_psi::lower_machine(&checked, "ProjectedDifferent::enter")
            .expect("projected aggregate IEEE inequality lowers");
    let projected_root = &projected_different.semantic_module.machines[0];
    let [CrashRouteGuard::Predicate(projected_route)] = projected_root.contract.crash_routes[0]
        .alternatives
        .as_slice()
    else {
        unreachable!()
    };
    let Proposition::Implication { premise, .. } = projected_route.proposition() else {
        panic!("projected aggregate inequality retains canonical negation")
    };
    let Proposition::Conjunction(projected_equalities) = premise.as_ref() else {
        unreachable!()
    };
    assert!(projected_equalities.iter().all(|proposition| matches!(
        proposition,
        Proposition::IeeeFloatComparison { left, right, .. }
            if left.path().len() == 3 && right.path().len() == 3
    )));
    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &projected_root.blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(structural_arguments[0].path.len(), 1);
    let [CrashRouteGuard::Predicate(projected_continuation)] =
        crash_continuations[0].alternatives.as_slice()
    else {
        unreachable!()
    };
    assert_eq!(projected_continuation, projected_route);
    terminal_verifier::verify_module(
        &projected_different.semantic_module,
        &projected_different.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier reconstructs nonempty prefixes below aggregate IEEE negation");

    let mut wrong_format = reversed.semantic_module.clone();
    let [CrashRouteGuard::Predicate(predicate)] = wrong_format.machines[0].contract.crash_routes[0]
        .alternatives
        .as_mut_slice()
    else {
        unreachable!()
    };
    let mut proposition = predicate.proposition().clone();
    let Proposition::Conjunction(conjuncts) = &mut proposition else {
        unreachable!()
    };
    let Some(Proposition::IeeeFloatComparison { format, .. }) = conjuncts.iter_mut().find(|item| {
        matches!(
            item,
            Proposition::IeeeFloatComparison {
                format: IeeeFloatFormat::Binary32,
                ..
            }
        )
    }) else {
        unreachable!()
    };
    *format = IeeeFloatFormat::Binary64;
    *predicate = CrashPredicateTerm::new(proposition);
    assert!(matches!(
        terminal_verifier::validate_module(&wrong_format),
        Err(terminal_verifier::ModuleError::InvalidIeeeFloatFieldTerm { .. })
    ));
}

#[test]
fn byte_sequence_aggregate_equality_is_content_atomic_end_to_end() {
    let tokens = Lexer::new(BYTE_SEQUENCE_AGGREGATE_EQUALITY_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("borrowed byte-sequence aggregate equality lowers");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let byte_equality = |proposition: &Proposition| {
        let Proposition::Conjunction(conjuncts) = proposition else {
            panic!("Boolean and byte-sequence fields form one conjunction")
        };
        assert_eq!(conjuncts.len(), 2);
        conjuncts
            .iter()
            .find_map(|proposition| match proposition {
                Proposition::ByteSequenceEqual { left, right } => {
                    Some((left.clone(), right.clone()))
                }
                _ => None,
            })
            .expect("byte-sequence equality remains one semantic atom")
    };
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("root publishes one byte-sequence aggregate route")
    };
    let (left, right) = byte_equality(root_route.proposition());
    assert_eq!(
        (left.root(), right.root()),
        (
            root.structural_parameters[0].place,
            root.structural_parameters[1].place
        )
    );
    assert_eq!((left.path().len(), right.path().len()), (1, 1));

    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("helper publishes one byte-sequence aggregate route")
    };
    let (helper_left, helper_right) = byte_equality(helper_route.proposition());
    assert_eq!(
        (helper_left.root(), helper_right.root()),
        (
            helper.structural_parameters[0].place,
            helper.structural_parameters[1].place
        )
    );

    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("root emits one structural Unit call")
    };
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call retains one byte-sequence crash continuation")
    };
    assert_eq!(continuation, root_route);

    assert!(
        lowered
            .semantic_module
            .structural_types
            .iter()
            .any(|declaration| {
                matches!(&declaration.shape, StructuralTypeShape::Record { fields }
                if fields.iter().any(|field| matches!(
                    field.field_type,
                    StructuralFieldType::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView)
                )))
            })
    );
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier checks byte-sequence leaves and substituted roots");
    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );

    let bounded = checked_trees_to_lowered_psi::lower_machine(&checked, "BoundedRoot::enter")
        .expect("bounded byte-sequence aggregate equality lowers");
    assert!(
        bounded
            .semantic_module
            .structural_types
            .iter()
            .any(|declaration| {
                matches!(&declaration.shape, StructuralTypeShape::Record { fields }
                if fields.iter().any(|field| matches!(
                    field.field_type,
                    StructuralFieldType::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity: 8 }
                    )
                )))
            })
    );
    terminal_verifier::verify_module(
        &bounded.semantic_module,
        &bounded.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("bounded carrier verifies");
    let bounded_bytes = encode_module(&bounded.semantic_module).expect("bounded semantic encode");
    assert_eq!(decode_module(&bounded_bytes), Ok(bounded.semantic_module));

    let mut redirected = lowered.semantic_module;
    let field = redirected
        .structural_types
        .iter_mut()
        .find_map(|declaration| match &mut declaration.shape {
            StructuralTypeShape::Record { fields } => fields
                .iter_mut()
                .find(|field| matches!(field.field_type, StructuralFieldType::ByteSequence(_))),
            StructuralTypeShape::Reference { .. }
            | StructuralTypeShape::PrimitiveScalar(_)
            | StructuralTypeShape::ByteSequence(_)
            | StructuralTypeShape::FixedArray { .. }
            | StructuralTypeShape::Sum { .. }
            | StructuralTypeShape::Mixed { .. } => None,
        })
        .expect("borrowed byte-sequence field");
    field.field_type = StructuralFieldType::Scalar(semantic_vocabulary::ScalarType::Boolean);
    assert!(matches!(
        terminal_verifier::validate_module(&redirected),
        Err(terminal_verifier::ModuleError::InvalidByteSequenceFieldTerm { .. })
    ));
}

#[test]
fn empty_record_equality_reuses_boolean_constants_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    let tokens = Lexer::new(EMPTY_RECORD_EQUALITY_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("empty-record equality lowers through the existing Boolean constant carrier");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    for machine in [root, helper] {
        let [CrashRouteGuard::Predicate(predicate)] =
            machine.contract.crash_routes[0].alternatives.as_slice()
        else {
            panic!("empty-record equality should retain one predicate")
        };
        assert_eq!(
            predicate.proposition(),
            &Proposition::Equal(ScalarTerm::Boolean(true), ScalarTerm::Boolean(true))
        );
    }

    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call retains the empty-record equality continuation")
    };
    let CrashRouteGuard::Predicate(root_route) = &root.contract.crash_routes[0].alternatives[0]
    else {
        unreachable!()
    };
    assert_eq!(continuation, root_route);

    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier reconstructs the root-free constant continuation");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("empty-record equality route has fixed fuel");
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let arguments = root
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| TerminalStructuralValue {
            opaque_identity: 41 + u64::try_from(index).unwrap(),
            structural_type: parameter.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let measured = interpret_terminal_artifact_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &arguments,
            ..Default::default()
        },
        &mut Accept,
    )
    .expect("constant equality remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut tampered = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut tampered.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    crash_continuations[0].alternatives[0] = CrashRouteGuard::Predicate(CrashPredicateTerm::new(
        Proposition::Equal(ScalarTerm::Boolean(true), ScalarTerm::Boolean(false)),
    ));
    let invalid_result = terminal_verifier::validate_module(&tampered);
    assert!(
        matches!(
            invalid_result,
            Err(terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected empty-record continuation result: {invalid_result:?}"
    );
}

#[test]
fn address_record_equality_remains_fenced_before_terminal_lowering() {
    let tokens = Lexer::new(ADDRESS_RECORD_EQUALITY_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");

    let result = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter");
    assert!(
        matches!(
            &result,
            Err(checked_trees_to_lowered_psi::LoweringError::Unsupported(
                "structural crash route is outside checked Boolean member lowering"
            ))
        ),
        "unexpected lowering result: {result:?}"
    );
}

#[test]
fn fixed_index_argument_prefix_is_canonical_and_rebases_member_crash_routes_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    let tokens = Lexer::new(FIXED_INDEX_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("fixed-index structural member crash route lowers");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    assert_eq!(
        structural_arguments[0].path,
        [StructuralPathSegment::FixedIndex(0)]
    );
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("callee member route survives the fixed-index call")
    };
    let Proposition::Equal(
        _,
        ScalarTerm::BooleanField {
            root: continuation_root,
            path: continuation_path,
        },
    ) = continuation.proposition()
    else {
        panic!("continuation is a canonical structural Boolean path")
    };
    assert_eq!(*continuation_root, root.structural_parameters[0].place);
    let [
        CanonicalStructuralPathSegment::FixedIndex(0),
        CanonicalStructuralPathSegment::Field(leaf),
    ] = continuation_path.as_slice()
    else {
        panic!("fixed index precedes the callee-relative Boolean field")
    };
    let [CrashRouteGuard::Predicate(helper_predicate)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one Boolean member route")
    };
    let Proposition::Equal(
        _,
        ScalarTerm::BooleanField {
            path: helper_path, ..
        },
    ) = helper_predicate.proposition()
    else {
        panic!("callee route retains its member")
    };
    assert_eq!(helper_path, &[CanonicalStructuralPathSegment::Field(*leaf)]);

    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently composes the fixed index and callee member");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("fixed-index member route has an acyclic fixed-fuel certificate");
    assert_eq!(fixed.ceiling_units(), 4);
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("proof encode");
    let argument = TerminalStructuralValue {
        opaque_identity: 13,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[argument],
            ..Default::default()
        },
        &mut Accept,
    )
    .expect("fixed-index member contract remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut out_of_bounds = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut out_of_bounds.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let CrashRouteGuard::Predicate(predicate) = &mut crash_continuations[0].alternatives[0] else {
        unreachable!()
    };
    let Proposition::Equal(_, ScalarTerm::BooleanField { root, path }) = predicate.proposition()
    else {
        unreachable!()
    };
    let root = *root;
    let mut path = path.clone();
    path[0] = CanonicalStructuralPathSegment::FixedIndex(1);
    *predicate = CrashPredicateTerm::new(Proposition::Equal(
        ScalarTerm::boolean(true),
        ScalarTerm::boolean_field_path(root, path),
    ));
    let invalid_result = terminal_verifier::validate_module(&out_of_bounds);
    assert!(
        matches!(
            invalid_result,
            Err(terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected fixed-index validation result: {invalid_result:?}"
    );
}

#[test]
fn verifier_rejects_empty_truncated_and_mistyped_boolean_field_paths() {
    let tokens = Lexer::new(NESTED_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("nested Boolean member crash route lowers");
    let CrashRouteGuard::Predicate(predicate) =
        &lowered.semantic_module.machines[0].contract.crash_routes[0].alternatives[0]
    else {
        panic!("nested route is a predicate")
    };
    let Proposition::Equal(_, ScalarTerm::BooleanField { path, .. }) = predicate.proposition()
    else {
        panic!("nested route is a Boolean field path")
    };
    let [outer, leaf] = path.as_slice() else {
        panic!("nested route has two fields")
    };

    for invalid_path in [
        Vec::new(),
        vec![*outer],
        vec![*leaf, *outer],
        vec![*outer, *leaf, *leaf],
    ] {
        let mut malformed = lowered.semantic_module.clone();
        let replace_path = |predicate: &mut CrashPredicateTerm| {
            let Proposition::Equal(_, ScalarTerm::BooleanField { root, .. }) =
                predicate.proposition()
            else {
                panic!("member route remains a Boolean field predicate")
            };
            let root = *root;
            *predicate = CrashPredicateTerm::new(Proposition::Equal(
                ScalarTerm::boolean(true),
                ScalarTerm::boolean_field_path(root, invalid_path.clone()),
            ));
        };
        for machine in &mut malformed.machines {
            for route in &mut machine.contract.crash_routes {
                for alternative in &mut route.alternatives {
                    let CrashRouteGuard::Predicate(predicate) = alternative else {
                        continue;
                    };
                    replace_path(predicate);
                }
            }
            for operation in &mut machine.blocks[0].operations {
                let OperationKind::CallUnit {
                    crash_continuations,
                    ..
                } = &mut operation.kind
                else {
                    continue;
                };
                for route in crash_continuations {
                    for alternative in &mut route.alternatives {
                        let CrashRouteGuard::Predicate(predicate) = alternative else {
                            continue;
                        };
                        replace_path(predicate);
                    }
                }
            }
        }
        let result = terminal_verifier::validate_module(&malformed);
        assert!(
            matches!(
                result,
                Err(terminal_verifier::ModuleError::InvalidBooleanFieldTerm { .. })
            ),
            "unexpected validation result: {result:?}"
        );
    }
}
