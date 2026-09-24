use crate::crash_member_source::{
    MIXED_AGGREGATE_EQUALITY_SOURCE, NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
    NESTED_PAYLOAD_SUM_EQUALITY_SOURCE, WHOLE_AGGREGATE_EQUALITY_SOURCE,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, Proposition, ScalarTerm, StructuralFieldId,
};
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_section};
use terminal_fixed_fuel::{derive_fixed_entry_fuel, validate_fixed_entry_fuel};
use terminal_interpreter::TerminalStructuralInputs;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecutionResult,
    TerminalStructuralValue, interpret_terminal_artifact_measured,
};
use terminal_psi::{
    CrashPredicateTerm, CrashRouteGuard, OperationKind, StructuralFieldType, StructuralTypeShape,
};

#[test]
fn whole_aggregate_equality_expands_and_reconstructs_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn field_roots(
        proposition: &Proposition,
    ) -> Vec<(semantic_vocabulary::PlaceId, semantic_vocabulary::PlaceId)> {
        let Proposition::Conjunction(conjuncts) = proposition else {
            panic!("aggregate equality is one flat conjunction")
        };
        let mut roots = Vec::new();
        for conjunct in conjuncts {
            let Proposition::Equal(ScalarTerm::Boolean(true), term) = conjunct else {
                panic!("aggregate field compare is asserted true")
            };
            match term {
                ScalarTerm::BooleanEqual { left, right } => {
                    let (
                        ScalarTerm::BooleanField {
                            root: left_root, ..
                        },
                        ScalarTerm::BooleanField {
                            root: right_root, ..
                        },
                    ) = (left.as_ref(), right.as_ref())
                    else {
                        panic!("Boolean aggregate fields retain paths")
                    };
                    roots.push((*left_root, *right_root));
                }
                ScalarTerm::IntegerEqual { left, right, .. } => {
                    let (
                        ScalarTerm::IntegerField {
                            root: left_root, ..
                        },
                        ScalarTerm::IntegerField {
                            root: right_root, ..
                        },
                    ) = (left.as_ref(), right.as_ref())
                    else {
                        panic!("integer aggregate fields retain paths")
                    };
                    roots.push((*left_root, *right_root));
                }
                _ => panic!("aggregate equality uses only member equality terms"),
            }
        }
        roots
    }

    let checked = crate::front_end::checked_program(WHOLE_AGGREGATE_EQUALITY_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("whole aggregate equality lowers");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one aggregate equality route")
    };
    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one aggregate equality route")
    };
    let root_field_roots = field_roots(root_route.proposition());
    let helper_field_roots = field_roots(helper_route.proposition());
    assert_eq!(root_field_roots.len(), 3);
    assert!(root_field_roots.iter().all(|roots| {
        *roots
            == (
                root.structural_parameters[0].place,
                root.structural_parameters[1].place,
            )
    }));
    assert_eq!(helper_field_roots.len(), 3);
    assert!(helper_field_roots.iter().all(|roots| {
        *roots
            == (
                helper.structural_parameters[0].place,
                helper.structural_parameters[1].place,
            )
    }));

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    assert_eq!(structural_arguments.len(), 2);
    assert!(
        structural_arguments
            .iter()
            .all(|argument| argument.path.is_empty())
    );
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call retains aggregate equality continuation")
    };
    assert_eq!(continuation, root_route);

    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently substitutes both aggregate roots");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("aggregate equality route has an acyclic fixed-fuel certificate");
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
            opaque_identity: 31 + u64::try_from(index).unwrap(),
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
    .expect("aggregate equality remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut redirected = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut redirected.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let CrashRouteGuard::Predicate(predicate) = &mut crash_continuations[0].alternatives[0] else {
        unreachable!()
    };
    let mut proposition = predicate.proposition().clone();
    let Proposition::Conjunction(conjuncts) = &mut proposition else {
        unreachable!()
    };
    let Some(Proposition::Equal(_, ScalarTerm::IntegerEqual { right, .. })) =
        conjuncts.iter_mut().find(|conjunct| {
            matches!(
                conjunct,
                Proposition::Equal(_, ScalarTerm::IntegerEqual { .. })
            )
        })
    else {
        unreachable!()
    };
    let ScalarTerm::IntegerField {
        root: right_root, ..
    } = right.as_mut()
    else {
        unreachable!()
    };
    *right_root = root.structural_parameters[0].place;
    *predicate = CrashPredicateTerm::new(proposition);
    let invalid_result = terminal_verifier::validate_module(&redirected);
    assert!(
        matches!(
            invalid_result,
            Err(terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected aggregate equality validation result: {invalid_result:?}"
    );
}

#[test]
fn nested_payload_sum_equality_retains_exact_record_case_payload_paths_end_to_end() {
    fn collect_paths(
        proposition: &Proposition,
        memberships: &mut Vec<(
            semantic_vocabulary::PlaceId,
            Vec<CanonicalStructuralPathSegment>,
            semantic_vocabulary::StructuralCaseId,
        )>,
        integer_fields: &mut Vec<(
            semantic_vocabulary::PlaceId,
            Vec<CanonicalStructuralPathSegment>,
        )>,
    ) {
        match proposition {
            Proposition::StructuralCaseMembership { subject, case } => {
                memberships.push((subject.root(), subject.path().to_vec(), *case));
            }
            Proposition::Equal(_, ScalarTerm::IntegerEqual { left, right, .. }) => {
                for operand in [left.as_ref(), right.as_ref()] {
                    if let ScalarTerm::IntegerField { root, path, .. } = operand {
                        integer_fields.push((*root, path.clone()));
                    }
                }
            }
            Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
                for child in children {
                    collect_paths(child, memberships, integer_fields);
                }
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                collect_paths(premise, memberships, integer_fields);
                collect_paths(conclusion, memberships, integer_fields);
            }
            Proposition::Truth
            | Proposition::Falsehood
            | Proposition::Atom(_)
            | Proposition::Equal(_, _)
            | Proposition::LessThan(_, _)
            | Proposition::LessOrEqual(_, _)
            | Proposition::IntegerMathEqual(_, _)
            | Proposition::IntegerMathLessThan(_, _)
            | Proposition::IntegerMathLessOrEqual(_, _)
            | Proposition::IeeeFloatComparison { .. }
            | Proposition::ScalarIeeeFloatComparison { .. }
            | Proposition::ByteSequenceEqual { .. }
            | Proposition::ContentConservation(_) => {}
        }
    }

    fn redirect_right_payload_field(
        proposition: &mut Proposition,
        from: StructuralFieldId,
        to: StructuralFieldId,
    ) -> bool {
        match proposition {
            Proposition::Equal(_, ScalarTerm::IntegerEqual { right, .. }) => {
                let ScalarTerm::IntegerField { path, .. } = right.as_mut() else {
                    return false;
                };
                let Some(CanonicalStructuralPathSegment::Field(field)) = path.last_mut() else {
                    return false;
                };
                if *field != from {
                    return false;
                }
                *field = to;
                true
            }
            Proposition::Conjunction(children) | Proposition::Disjunction(children) => children
                .iter_mut()
                .any(|child| redirect_right_payload_field(child, from, to)),
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                redirect_right_payload_field(premise, from, to)
                    || redirect_right_payload_field(conclusion, from, to)
            }
            _ => false,
        }
    }

    let checked = crate::front_end::checked_program(NESTED_PAYLOAD_SUM_EQUALITY_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Whole::enter"),
    )
    .expect("whole Envelope equality lowers through its sum field");

    let root = &lowered.semantic_module.machines[0];
    let envelope = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == root.structural_parameters[0].structural_type)
        .expect("Envelope structural type");
    let StructuralTypeShape::Record { fields } = &envelope.shape else {
        panic!("Envelope is a record")
    };
    let message_field = fields
        .iter()
        .find(|field| field.identity == "message")
        .expect("message field");
    let StructuralFieldType::Structural(message_type) = message_field.field_type else {
        panic!("message field names the nested structural sum")
    };
    let message = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == message_type)
        .expect("Message structural type");
    let StructuralTypeShape::Sum { cases } = &message.shape else {
        panic!("Message is a sum")
    };
    let data_case = cases
        .iter()
        .find(|case| case.identity == "Data")
        .expect("Data case");
    let value_field = data_case
        .fields
        .iter()
        .find(|field| field.identity == "value")
        .expect("value field");
    let checksum_field = data_case
        .fields
        .iter()
        .find(|field| field.identity == "checksum")
        .expect("checksum field");

    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("whole Envelope equality publishes one predicate")
    };
    let mut root_memberships = Vec::new();
    let mut root_fields = Vec::new();
    collect_paths(
        root_route.proposition(),
        &mut root_memberships,
        &mut root_fields,
    );
    assert_eq!(root_memberships.len(), 4);
    assert!(root_memberships.iter().all(|(root_place, path, _)| {
        [
            root.structural_parameters[0].place,
            root.structural_parameters[1].place,
        ]
        .contains(root_place)
            && path == &[CanonicalStructuralPathSegment::Field(message_field.id)]
    }));
    assert_eq!(root_fields.len(), 4);
    assert!(root_fields.iter().all(|(root_place, path)| {
        [
            root.structural_parameters[0].place,
            root.structural_parameters[1].place,
        ]
        .contains(root_place)
            && matches!(
                path.as_slice(),
                [
                    CanonicalStructuralPathSegment::Field(message),
                    CanonicalStructuralPathSegment::Case(case),
                    CanonicalStructuralPathSegment::Field(field),
                ] if *message == message_field.id
                    && *case == data_case.id
                    && [value_field.id, checksum_field.id].contains(field)
            )
    }));

    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier accepts exact nested record-to-sum paths");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("nested equality has fixed fuel");
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

    let mut redirected = lowered.semantic_module.clone();
    let CrashRouteGuard::Predicate(predicate) =
        &mut redirected.machines[0].contract.crash_routes[0].alternatives[0]
    else {
        unreachable!()
    };
    let mut proposition = predicate.proposition().clone();
    let wrong_field = StructuralFieldId::new(u64::MAX).expect("nonzero redirected field");
    assert!(redirect_right_payload_field(
        &mut proposition,
        value_field.id,
        wrong_field,
    ));
    *predicate = CrashPredicateTerm::new(proposition);
    let invalid_result = terminal_verifier::validate_module(&redirected);
    assert!(
        matches!(
            invalid_result,
            Err(terminal_verifier::ModuleError::InvalidIntegerFieldTerm { .. })
        ),
        "unexpected redirected nested payload validation result: {invalid_result:?}"
    );
}

#[test]
fn mixed_aggregate_equality_retains_common_fields_cases_and_call_rebasing_end_to_end() {
    fn collect_scalar_paths(
        term: &ScalarTerm,
        boolean: &mut Vec<(
            semantic_vocabulary::PlaceId,
            Vec<CanonicalStructuralPathSegment>,
        )>,
        integer: &mut Vec<(
            semantic_vocabulary::PlaceId,
            Vec<CanonicalStructuralPathSegment>,
        )>,
    ) {
        match term {
            ScalarTerm::BooleanField { root, path } => boolean.push((*root, path.clone())),
            ScalarTerm::IntegerField { root, path, .. } => integer.push((*root, path.clone())),
            ScalarTerm::BooleanEqual { left, right }
            | ScalarTerm::IntegerEqual { left, right, .. } => {
                collect_scalar_paths(left, boolean, integer);
                collect_scalar_paths(right, boolean, integer);
            }
            _ => {}
        }
    }

    fn collect(
        proposition: &Proposition,
        memberships: &mut Vec<(
            semantic_vocabulary::PlaceId,
            Vec<CanonicalStructuralPathSegment>,
            semantic_vocabulary::StructuralCaseId,
        )>,
        boolean: &mut Vec<(
            semantic_vocabulary::PlaceId,
            Vec<CanonicalStructuralPathSegment>,
        )>,
        integer: &mut Vec<(
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
                collect_scalar_paths(left, boolean, integer);
                collect_scalar_paths(right, boolean, integer);
            }
            Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
                for child in children {
                    collect(child, memberships, boolean, integer);
                }
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                collect(premise, memberships, boolean, integer);
                collect(conclusion, memberships, boolean, integer);
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

    let checked = crate::front_end::checked_program(MIXED_AGGREGATE_EQUALITY_SOURCE);
    let equal = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("mixed equality lowers through the direct call");
    let different = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Different::enter"),
    )
    .expect("mixed inequality lowers through the direct call");

    for (lowered, is_different) in [(&equal, false), (&different, true)] {
        let machine = &lowered.semantic_module.machines[0];
        let declaration = lowered
            .semantic_module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == machine.structural_parameters[0].structural_type)
            .expect("Message structural type");
        let StructuralTypeShape::Mixed { fields, cases } = &declaration.shape else {
            panic!("Message retains its mixed shape")
        };
        let active = fields
            .iter()
            .find(|field| field.identity == "active")
            .expect("common active field");
        let data = cases
            .iter()
            .find(|case| case.identity == "Data")
            .expect("Data case");
        let value = data
            .fields
            .iter()
            .find(|field| field.identity == "value")
            .expect("Data value field");
        let [CrashRouteGuard::Predicate(route)] =
            machine.contract.crash_routes[0].alternatives.as_slice()
        else {
            panic!("mixed equality publishes one predicate")
        };
        let equality = if is_different {
            let Proposition::Implication {
                premise,
                conclusion,
            } = route.proposition()
            else {
                panic!("mixed inequality is an implication")
            };
            assert!(matches!(conclusion.as_ref(), Proposition::Falsehood));
            premise.as_ref()
        } else {
            route.proposition()
        };
        let Proposition::Conjunction(canonical) = equality else {
            panic!("mixed equality is one canonical conjunction")
        };
        assert_eq!(canonical.len(), 2);
        assert!(matches!(
            &canonical[0],
            Proposition::Equal(_, ScalarTerm::BooleanEqual { .. })
        ));
        assert!(matches!(
            &canonical[1],
            Proposition::Disjunction(arms) if arms.len() == 2
        ));
        if is_different {
            assert!(matches!(
                route.proposition(),
                Proposition::Implication { conclusion, .. }
                    if matches!(conclusion.as_ref(), Proposition::Falsehood)
            ));
        }
        let mut memberships = Vec::new();
        let mut boolean = Vec::new();
        let mut integer = Vec::new();
        collect(
            route.proposition(),
            &mut memberships,
            &mut boolean,
            &mut integer,
        );
        assert_eq!(memberships.len(), 4);
        assert!(memberships.iter().all(|(root, path, case)| {
            machine
                .structural_parameters
                .iter()
                .any(|parameter| parameter.place == *root)
                && path.is_empty()
                && cases.iter().any(|candidate| candidate.id == *case)
        }));
        assert_eq!(boolean.len(), 2);
        assert!(boolean.iter().all(|(root, path)| {
            machine
                .structural_parameters
                .iter()
                .any(|parameter| parameter.place == *root)
                && path == &[CanonicalStructuralPathSegment::Field(active.id)]
        }));
        assert_eq!(integer.len(), 2);
        assert!(integer.iter().all(|(root, path)| {
            machine
                .structural_parameters
                .iter()
                .any(|parameter| parameter.place == *root)
                && path
                    == &[
                        CanonicalStructuralPathSegment::Case(data.id),
                        CanonicalStructuralPathSegment::Field(value.id),
                    ]
        }));
        let verified = terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .expect("verifier replays the exact mixed paths");
        let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
            .expect("mixed equality has fixed fuel");
        validate_fixed_entry_fuel(&verified, &fixed).expect("mixed equality fuel recomputes");
        let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
        assert_eq!(
            decode_module(&semantics),
            Ok(lowered.semantic_module.clone())
        );
    }

    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }
    let verified = terminal_verifier::verify_module(
        &equal.semantic_module,
        &equal.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("mixed equality verifies before interpretation");
    let fixed = derive_fixed_entry_fuel(&verified, equal.semantic_module.entry)
        .expect("mixed equality has fixed fuel");
    let semantics = encode_module(&equal.semantic_module).expect("semantic encode");
    let proof =
        encode_proof_section(&equal.semantic_module, &equal.proof_bundle).expect("proof encode");
    let arguments = equal.semantic_module.machines[0]
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| TerminalStructuralValue {
            opaque_identity: 701 + u64::try_from(index).expect("small parameter index"),
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
    .expect("verified mixed equality remains executable metadata");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut redirected = equal.semantic_module.clone();
    let cases = redirected
        .structural_types
        .iter_mut()
        .find_map(|declaration| match &mut declaration.shape {
            StructuralTypeShape::Mixed { cases, .. } => Some(cases),
            _ => None,
        })
        .expect("Message remains a mixed structural type");
    cases[0].id = semantic_vocabulary::StructuralCaseId::new(u64::MAX).expect("nonzero case");
    let redirected_result = terminal_verifier::validate_module(&redirected);
    assert!(
        matches!(
            redirected_result,
            Err(terminal_verifier::ModuleError::InvalidStructuralCaseMembership { .. })
        ),
        "unexpected redirected mixed case result: {redirected_result:?}"
    );

    let mut common_field_drift = equal.semantic_module.clone();
    let fields = common_field_drift
        .structural_types
        .iter_mut()
        .find_map(|declaration| match &mut declaration.shape {
            StructuralTypeShape::Mixed { fields, .. } => Some(fields),
            _ => None,
        })
        .expect("Message common fields");
    fields[0].id = StructuralFieldId::new(u64::MAX).expect("nonzero field");
    assert!(matches!(
        terminal_verifier::validate_module(&common_field_drift),
        Err(terminal_verifier::ModuleError::InvalidBooleanFieldTerm { .. })
    ));

    let mut payload_field_drift = equal.semantic_module.clone();
    let cases = payload_field_drift
        .structural_types
        .iter_mut()
        .find_map(|declaration| match &mut declaration.shape {
            StructuralTypeShape::Mixed { cases, .. } => Some(cases),
            _ => None,
        })
        .expect("Message cases");
    let data = cases
        .iter_mut()
        .find(|case| case.identity == "Data")
        .expect("Data case");
    data.fields[0].id = StructuralFieldId::new(u64::MAX).expect("nonzero field");
    assert!(matches!(
        terminal_verifier::validate_module(&payload_field_drift),
        Err(terminal_verifier::ModuleError::InvalidIntegerFieldTerm { .. })
    ));

    let mut noncanonical = equal.semantic_module.clone();
    let cases = noncanonical
        .structural_types
        .iter_mut()
        .find_map(|declaration| match &mut declaration.shape {
            StructuralTypeShape::Mixed { cases, .. } => Some(cases),
            _ => None,
        })
        .expect("Message cases");
    cases.swap(0, 1);
    assert!(matches!(
        encode_module(&noncanonical),
        Err(terminal_codec::CodecError::NonCanonicalOrder(
            "mixed structural cases by StructuralCaseId"
        ))
    ));
}

#[test]
fn nested_mixed_aggregate_equality_prefixes_every_path_and_rebases_whole_root_calls() {
    fn collect_scalar_paths(
        term: &ScalarTerm,
        boolean: &mut Vec<(
            semantic_vocabulary::PlaceId,
            Vec<CanonicalStructuralPathSegment>,
        )>,
        integer: &mut Vec<(
            semantic_vocabulary::PlaceId,
            Vec<CanonicalStructuralPathSegment>,
        )>,
    ) {
        match term {
            ScalarTerm::BooleanField { root, path } => boolean.push((*root, path.clone())),
            ScalarTerm::IntegerField { root, path, .. } => integer.push((*root, path.clone())),
            ScalarTerm::BooleanEqual { left, right }
            | ScalarTerm::IntegerEqual { left, right, .. } => {
                collect_scalar_paths(left, boolean, integer);
                collect_scalar_paths(right, boolean, integer);
            }
            _ => {}
        }
    }

    fn collect(
        proposition: &Proposition,
        memberships: &mut Vec<(
            semantic_vocabulary::PlaceId,
            Vec<CanonicalStructuralPathSegment>,
            semantic_vocabulary::StructuralCaseId,
        )>,
        boolean: &mut Vec<(
            semantic_vocabulary::PlaceId,
            Vec<CanonicalStructuralPathSegment>,
        )>,
        integer: &mut Vec<(
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
                collect_scalar_paths(left, boolean, integer);
                collect_scalar_paths(right, boolean, integer);
            }
            Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
                for child in children {
                    collect(child, memberships, boolean, integer);
                }
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                collect(premise, memberships, boolean, integer);
                collect(conclusion, memberships, boolean, integer);
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

    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    let checked = crate::front_end::checked_program(NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE);
    let equal = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("nested mixed equality lowers through the whole-root call");
    let different = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Different::enter"),
    )
    .expect("nested mixed inequality lowers through the whole-root call");

    for (lowered, is_different) in [(&equal, false), (&different, true)] {
        let machine = &lowered.semantic_module.machines[0];
        let envelope = lowered
            .semantic_module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == machine.structural_parameters[0].structural_type)
            .expect("Envelope structural type");
        let StructuralTypeShape::Record { fields } = &envelope.shape else {
            panic!("Envelope remains a record")
        };
        let selected = fields
            .iter()
            .find(|field| field.identity == "selected")
            .expect("selected field");
        let message = fields
            .iter()
            .find(|field| field.identity == "message")
            .expect("message field");
        let StructuralFieldType::Structural(message_type) = message.field_type else {
            panic!("message field retains its structural type")
        };
        let message_declaration = lowered
            .semantic_module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == message_type)
            .expect("Message structural type");
        let StructuralTypeShape::Mixed { fields, cases } = &message_declaration.shape else {
            panic!("Message retains its mixed shape")
        };
        let active = fields
            .iter()
            .find(|field| field.identity == "active")
            .expect("message active field");
        let data = cases
            .iter()
            .find(|case| case.identity == "Data")
            .expect("Data case");
        let value = data
            .fields
            .iter()
            .find(|field| field.identity == "value")
            .expect("Data value field");
        let [CrashRouteGuard::Predicate(route)] =
            machine.contract.crash_routes[0].alternatives.as_slice()
        else {
            panic!("nested mixed equality publishes one predicate")
        };
        let equality = if is_different {
            let Proposition::Implication {
                premise,
                conclusion,
            } = route.proposition()
            else {
                panic!("nested mixed inequality is an implication")
            };
            assert!(matches!(conclusion.as_ref(), Proposition::Falsehood));
            premise.as_ref()
        } else {
            route.proposition()
        };
        let Proposition::Conjunction(canonical) = equality else {
            panic!("nested mixed equality is one canonical conjunction")
        };
        assert_eq!(canonical.len(), 3);
        assert!(matches!(
            canonical.last(),
            Some(Proposition::Disjunction(_))
        ));

        let mut memberships = Vec::new();
        let mut boolean = Vec::new();
        let mut integer = Vec::new();
        collect(
            route.proposition(),
            &mut memberships,
            &mut boolean,
            &mut integer,
        );
        assert_eq!(memberships.len(), 4);
        assert!(memberships.iter().all(|(root, path, case)| {
            machine
                .structural_parameters
                .iter()
                .any(|parameter| parameter.place == *root)
                && path == &[CanonicalStructuralPathSegment::Field(message.id)]
                && cases.iter().any(|candidate| candidate.id == *case)
        }));
        assert_eq!(boolean.len(), 4);
        assert_eq!(
            boolean
                .iter()
                .filter(|(_, path)| {
                    path.as_slice() == [CanonicalStructuralPathSegment::Field(selected.id)]
                })
                .count(),
            2
        );
        assert_eq!(
            boolean
                .iter()
                .filter(|(_, path)| {
                    path.as_slice()
                        == [
                            CanonicalStructuralPathSegment::Field(message.id),
                            CanonicalStructuralPathSegment::Field(active.id),
                        ]
                })
                .count(),
            2
        );
        assert_eq!(integer.len(), 2);
        assert!(integer.iter().all(|(root, path)| {
            machine
                .structural_parameters
                .iter()
                .any(|parameter| parameter.place == *root)
                && path
                    == &[
                        CanonicalStructuralPathSegment::Field(message.id),
                        CanonicalStructuralPathSegment::Case(data.id),
                        CanonicalStructuralPathSegment::Field(value.id),
                    ]
        }));

        let OperationKind::CallUnit {
            crash_continuations,
            ..
        } = &machine.blocks[0].operations[0].kind
        else {
            panic!("nested mixed caller emits one Unit call")
        };
        assert_eq!(crash_continuations, &machine.contract.crash_routes);

        let verified = terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .expect("verifier independently replays every prefixed mixed path");
        let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
            .expect("nested mixed equality has fixed fuel");
        validate_fixed_entry_fuel(&verified, &fixed).expect("nested mixed fixed fuel recomputes");
        let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
        assert_eq!(
            decode_module(&semantics),
            Ok(lowered.semantic_module.clone())
        );
    }

    let verified = terminal_verifier::verify_module(
        &equal.semantic_module,
        &equal.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("nested mixed equality verifies before interpretation");
    let fixed = derive_fixed_entry_fuel(&verified, equal.semantic_module.entry)
        .expect("nested mixed equality has fixed fuel");
    let semantics = encode_module(&equal.semantic_module).expect("semantic encode");
    let proof =
        encode_proof_section(&equal.semantic_module, &equal.proof_bundle).expect("proof encode");
    let arguments = equal.semantic_module.machines[0]
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| TerminalStructuralValue {
            opaque_identity: 801 + u64::try_from(index).expect("small parameter index"),
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
    .expect("verified nested mixed equality remains executable metadata");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut redirected = equal.semantic_module.clone();
    let envelope = redirected
        .structural_types
        .iter_mut()
        .find_map(|declaration| match &mut declaration.shape {
            StructuralTypeShape::Record { fields }
                if fields.iter().any(|field| {
                    field.identity == "message"
                        && matches!(field.field_type, StructuralFieldType::Structural(_))
                }) =>
            {
                Some(fields)
            }
            _ => None,
        })
        .expect("Envelope fields");
    envelope
        .iter_mut()
        .find(|field| field.identity == "message")
        .expect("message field")
        .id = StructuralFieldId::new(u64::MAX).expect("nonzero redirected field");
    let redirected_result = terminal_verifier::validate_module(&redirected);
    assert!(
        matches!(
            redirected_result,
            Err(terminal_verifier::ModuleError::InvalidBooleanFieldTerm { .. })
                | Err(terminal_verifier::ModuleError::InvalidStructuralCaseMembership { .. })
        ),
        "unexpected redirected nested-mixed prefix result: {redirected_result:?}"
    );
}
