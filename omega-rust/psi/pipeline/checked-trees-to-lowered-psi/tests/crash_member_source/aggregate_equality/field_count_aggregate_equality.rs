use crate::crash_member_source::{
    EIGHT_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
    ELEVEN_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
    FIVE_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
    FOUR_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
    FOURTEEN_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
    NINE_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
    SEVEN_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
    SIX_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
    TEN_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
    THIRTEEN_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
    THREE_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
    TWELVE_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
    TWO_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
    assert_nested_mixed_aggregate_equality_replays_every_prefixed_path,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, Proposition, ScalarTerm, StructuralFieldId,
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
use terminal_psi::{CrashRouteGuard, OperationKind, StructuralFieldType, StructuralTypeShape};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::CheckingRequest;
use typed_trees_to_checked_trees::lower_typed_trees;

#[test]
fn two_field_nested_mixed_aggregate_equality_replays_every_prefixed_path() {
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

    let tokens = Lexer::new(TWO_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check");
    let equal = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("two-field nested mixed equality lowers through the whole-root call");
    let different = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Different::enter"),
    )
    .expect("two-field nested mixed inequality lowers through the whole-root call");

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
        let inner = fields
            .iter()
            .find(|field| field.identity == "inner")
            .expect("inner field");
        let StructuralFieldType::Structural(inner_type) = inner.field_type else {
            panic!("inner field retains its structural type")
        };
        let inner_declaration = lowered
            .semantic_module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == inner_type)
            .expect("Inner structural type");
        let StructuralTypeShape::Record { fields } = &inner_declaration.shape else {
            panic!("Inner remains a record")
        };
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
            panic!("two-field nested mixed equality publishes one predicate")
        };
        let equality = if is_different {
            let Proposition::Implication {
                premise,
                conclusion,
            } = route.proposition()
            else {
                panic!("two-field nested mixed inequality is an implication")
            };
            assert!(matches!(conclusion.as_ref(), Proposition::Falsehood));
            premise.as_ref()
        } else {
            route.proposition()
        };
        let Proposition::Conjunction(canonical) = equality else {
            panic!("two-field nested mixed equality is one canonical conjunction")
        };
        assert_eq!(canonical.len(), 2);
        assert!(matches!(
            canonical.last(),
            Some(Proposition::Disjunction(_))
        ));

        let mixed_prefix = [
            CanonicalStructuralPathSegment::Field(inner.id),
            CanonicalStructuralPathSegment::Field(message.id),
        ];
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
                && path == &mixed_prefix
                && cases.iter().any(|candidate| candidate.id == *case)
        }));
        assert_eq!(boolean.len(), 2);
        assert!(boolean.iter().all(|(root, path)| {
            machine
                .structural_parameters
                .iter()
                .any(|parameter| parameter.place == *root)
                && path
                    == &[
                        mixed_prefix[0],
                        mixed_prefix[1],
                        CanonicalStructuralPathSegment::Field(active.id),
                    ]
        }));
        assert_eq!(integer.len(), 2);
        assert!(integer.iter().all(|(root, path)| {
            machine
                .structural_parameters
                .iter()
                .any(|parameter| parameter.place == *root)
                && path
                    == &[
                        mixed_prefix[0],
                        mixed_prefix[1],
                        CanonicalStructuralPathSegment::Case(data.id),
                        CanonicalStructuralPathSegment::Field(value.id),
                    ]
        }));

        let OperationKind::CallUnit {
            crash_continuations,
            ..
        } = &machine.blocks[0].operations[0].kind
        else {
            panic!("two-field nested mixed caller emits one Unit call")
        };
        assert_eq!(crash_continuations, &machine.contract.crash_routes);

        let verified = terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .expect("verifier replays every two-field-prefixed mixed path");
        let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
            .expect("two-field nested mixed equality has fixed fuel");
        validate_fixed_entry_fuel(&verified, &fixed)
            .expect("two-field nested mixed fixed fuel recomputes");
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
        let arguments = machine
            .structural_parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| TerminalStructuralValue {
                opaque_identity: 901 + u64::try_from(index).expect("small parameter index"),
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
        .expect("verified two-field nested mixed equality remains executable metadata");
        assert_eq!(measured.value(), TerminalExecutionResult::Unit);
        assert_eq!(measured.usage().total_units(), fixed.ceiling_units());
    }

    let machine = &equal.semantic_module.machines[0];
    let envelope_type = machine.structural_parameters[0].structural_type;
    let envelope = equal
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == envelope_type)
        .expect("Envelope structural type");
    let StructuralTypeShape::Record { fields } = &envelope.shape else {
        panic!("Envelope remains a record")
    };
    let StructuralFieldType::Structural(inner_type) = fields[0].field_type else {
        panic!("inner field retains its structural type")
    };

    let mut outer_field_drift = equal.semantic_module.clone();
    let StructuralTypeShape::Record { fields } = &mut outer_field_drift
        .structural_types
        .iter_mut()
        .find(|declaration| declaration.id == envelope_type)
        .expect("Envelope structural type")
        .shape
    else {
        panic!("Envelope remains a record")
    };
    fields[0].id = StructuralFieldId::new(u64::MAX).expect("nonzero outer field");
    let outer_result = terminal_verifier::validate_module(&outer_field_drift);
    assert!(
        matches!(
            outer_result,
            Err(terminal_verifier::ModuleError::InvalidBooleanFieldTerm { .. })
                | Err(terminal_verifier::ModuleError::InvalidStructuralCaseMembership { .. })
        ),
        "unexpected outer-field drift result: {outer_result:?}"
    );

    let mut inner_field_drift = equal.semantic_module.clone();
    let StructuralTypeShape::Record { fields } = &mut inner_field_drift
        .structural_types
        .iter_mut()
        .find(|declaration| declaration.id == inner_type)
        .expect("Inner structural type")
        .shape
    else {
        panic!("Inner remains a record")
    };
    fields[0].id = StructuralFieldId::new(u64::MAX - 1).expect("nonzero inner field");
    let inner_result = terminal_verifier::validate_module(&inner_field_drift);
    assert!(
        matches!(
            inner_result,
            Err(terminal_verifier::ModuleError::InvalidBooleanFieldTerm { .. })
                | Err(terminal_verifier::ModuleError::InvalidStructuralCaseMembership { .. })
        ),
        "unexpected inner-field drift result: {inner_result:?}"
    );
}

#[test]
fn three_field_nested_mixed_aggregate_equality_replays_every_prefixed_path() {
    assert_nested_mixed_aggregate_equality_replays_every_prefixed_path(
        THREE_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
        &["middle", "inner", "message"],
    );
}

#[test]
fn four_field_nested_mixed_aggregate_equality_replays_every_prefixed_path() {
    assert_nested_mixed_aggregate_equality_replays_every_prefixed_path(
        FOUR_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
        &["envelope", "middle", "inner", "message"],
    );
}

#[test]
fn five_field_nested_mixed_aggregate_equality_replays_every_prefixed_path() {
    assert_nested_mixed_aggregate_equality_replays_every_prefixed_path(
        FIVE_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
        &["exterior", "envelope", "middle", "inner", "message"],
    );
}

#[test]
fn six_field_nested_mixed_aggregate_equality_replays_every_prefixed_path() {
    assert_nested_mixed_aggregate_equality_replays_every_prefixed_path(
        SIX_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
        &[
            "outside", "exterior", "envelope", "middle", "inner", "message",
        ],
    );
}

#[test]
fn seven_field_nested_mixed_aggregate_equality_replays_every_prefixed_path() {
    assert_nested_mixed_aggregate_equality_replays_every_prefixed_path(
        SEVEN_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
        &[
            "beyond", "outside", "exterior", "envelope", "middle", "inner", "message",
        ],
    );
}

#[test]
fn eight_field_nested_mixed_aggregate_equality_replays_every_prefixed_path() {
    assert_nested_mixed_aggregate_equality_replays_every_prefixed_path(
        EIGHT_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
        &[
            "further", "beyond", "outside", "exterior", "envelope", "middle", "inner", "message",
        ],
    );
}

#[test]
fn nine_field_nested_mixed_aggregate_equality_replays_every_prefixed_path() {
    assert_nested_mixed_aggregate_equality_replays_every_prefixed_path(
        NINE_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
        &[
            "furthest", "further", "beyond", "outside", "exterior", "envelope", "middle", "inner",
            "message",
        ],
    );
}

#[test]
fn ten_field_nested_mixed_aggregate_equality_replays_every_prefixed_path() {
    assert_nested_mixed_aggregate_equality_replays_every_prefixed_path(
        TEN_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
        &[
            "ultimate", "furthest", "further", "beyond", "outside", "exterior", "envelope",
            "middle", "inner", "message",
        ],
    );
}

#[test]
fn eleven_field_nested_mixed_aggregate_equality_replays_every_prefixed_path() {
    assert_nested_mixed_aggregate_equality_replays_every_prefixed_path(
        ELEVEN_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
        &[
            "outermost",
            "ultimate",
            "furthest",
            "further",
            "beyond",
            "outside",
            "exterior",
            "envelope",
            "middle",
            "inner",
            "message",
        ],
    );
}

#[test]
fn twelve_field_nested_mixed_aggregate_equality_replays_every_prefixed_path() {
    assert_nested_mixed_aggregate_equality_replays_every_prefixed_path(
        TWELVE_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
        &[
            "final",
            "outermost",
            "ultimate",
            "furthest",
            "further",
            "beyond",
            "outside",
            "exterior",
            "envelope",
            "middle",
            "inner",
            "message",
        ],
    );
}

#[test]
fn thirteen_field_nested_mixed_aggregate_equality_replays_every_prefixed_path() {
    assert_nested_mixed_aggregate_equality_replays_every_prefixed_path(
        THIRTEEN_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
        &[
            "absolute",
            "final",
            "outermost",
            "ultimate",
            "furthest",
            "further",
            "beyond",
            "outside",
            "exterior",
            "envelope",
            "middle",
            "inner",
            "message",
        ],
    );
}

#[test]
fn fourteen_field_nested_mixed_aggregate_equality_replays_every_prefixed_path() {
    assert_nested_mixed_aggregate_equality_replays_every_prefixed_path(
        FOURTEEN_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
        &[
            "supreme",
            "absolute",
            "final",
            "outermost",
            "ultimate",
            "furthest",
            "further",
            "beyond",
            "outside",
            "exterior",
            "envelope",
            "middle",
            "inner",
            "message",
        ],
    );
}
