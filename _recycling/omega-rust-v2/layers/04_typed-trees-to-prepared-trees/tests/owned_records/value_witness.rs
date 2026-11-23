use omega_preparation::{
    CheckedPreparationEvidence, PreparationAnswers, PreparationReason, PreparationRefusal, PreparedValue, RefusalCause, prepare_typing_pass_with_cause,
};
use omega_typed::{StaticValue, TypedStage, TypedTrees, ValueType};
use omega_typer::TypingProgress;

const BYTE: ValueType = ValueType::Integer { signed: false, width: Some(8) };

fn evaluate(progress: &TypingProgress<'_>) -> Result<PreparationAnswers, PreparationRefusal> {
    Ok(prepare_typing_pass_with_cause(progress, &[], None, CheckedPreparationEvidence::default())?.answers)
}

fn witness(
    source: &str,
    control: impl FnOnce(&PreparedValue),
    change: impl FnOnce(&mut TypedStage),
    (context, site): (&str, &str),
    reason: PreparationReason,
    message: &str,
) {
    // Obtain the completed typing pass, including any closed static arguments.
    // Neither evaluation below receives the package's prepared values.
    let trees = super::prepare(source).unwrap().into_typed();
    let answer = trees.constants.iter().find(|c| c.declaration.path == "ANSWER").unwrap().declaration.clone();
    let progress = TypingProgress::Complete(trees);
    let PreparationAnswers::Frozen { constants, .. } = evaluate(&progress).unwrap() else {
        panic!("expected a frozen control")
    };
    control(&constants[&answer]);

    let TypingProgress::Complete(trees) = progress else { unreachable!() };
    let resolved = trees.resolved().clone();
    let mut facts = trees.into_facts();
    change(&mut facts);
    let changed = TypingProgress::Complete(TypedTrees::new(resolved, facts));
    let start = source.rfind(context).unwrap() + context.find(site).unwrap();
    spec_corpus::assert_preparation_cause!(
        evaluate(&changed).unwrap_err(),
        RefusalCause::Preparation(reason),
        "app",
        "main.omg",
        (start, start + site.len()),
        format!("static evaluation cannot admit this invocation: {message}"),
    );
}

fn change_selected_part(facts: &mut TypedStage) {
    let call = facts.calls.iter_mut().find(|call| call.machine.path == "get").unwrap();
    let [ValueType::StaticValue { value: StaticValue::Record { fields, .. }, .. }] = call.type_arguments.as_mut_slice() else {
        panic!("expected a closed Part")
    };
    // Only the selected application's snapshot uses wide's value for n.
    assert_eq!(fields.as_ref(), &[StaticValue::Integer(42.into()), StaticValue::Integer(256.into())]);
    let fields = std::sync::Arc::make_mut(fields);
    fields[0] = fields[1].clone();
}

const OUTSIDE: &str = "a closed argument is outside its selected carrier";

#[test]
fn an_out_of_range_static_integer_reports_the_parameter_read() {
    let source = "machine get<const N:u8>()->u8{N} const ANSWER:u8=get::<42u8>();";
    witness(
        source,
        |value| assert_eq!(*value, PreparedValue::Integer(42.into())),
        |facts| {
            let read = facts.expressions.iter().find(|row| row.site.span.slice(source.as_bytes()) == Some(b"N")).unwrap();
            assert_eq!(read.static_argument_slot, Some(0));
            assert_eq!(read.ty, BYTE);
            assert!(!facts.landings.iter().any(|landing| landing.node == read.node));
            let call = facts.calls.iter_mut().find(|call| call.machine.path == "get").unwrap();
            let [ValueType::StaticInteger { value, carrier }] = call.type_arguments.as_mut_slice() else {
                panic!("expected a closed integer argument")
            };
            assert_eq!(carrier.as_ref(), &BYTE);
            assert_eq!(*value, 42.into());
            *value = 256.into();
        },
        ("{N}", "N"),
        PreparationReason::ArgumentOutsideCarrier,
        OUTSIDE,
    );
}

#[test]
fn an_invalid_closed_argument_cannot_freeze_a_scalar_or_a_whole_record() {
    for (ty, body) in [("u8", "P.n"), ("Part", "P"), ("u16", "P.wide")] {
        let source = format!("data Part [copy]{{n:u8; wide:u16;}} machine get<const P:Part>()->{ty} {{{body}}} const ANSWER:{ty}=get::<Part{{n:42u8,wide:256u16}}>();");
        witness(
            &source,
            |value| match ty {
                "Part" => {
                    let PreparedValue::Record { owner, fields } = value else { panic!("expected Part") };
                    assert_eq!(owner.path, "Part");
                    spec_corpus::observe_handoff!(
                        "closed-argument-fields/producer",
                        assert_eq!(fields.as_ref(), &[PreparedValue::Integer(42.into()), PreparedValue::Integer(256.into())]),
                    );
                }
                "u16" => assert_eq!(*value, PreparedValue::Integer(256.into())),
                _ => assert_eq!(*value, PreparedValue::Integer(42.into())),
            },
            change_selected_part,
            (&format!("{{{body}}}"), "P"),
            PreparationReason::ArgumentOutsideCarrier,
            OUTSIDE,
        );
    }
}

#[test]
fn an_invalid_closed_argument_is_refused_before_direct_or_nested_named_reads() {
    let prefix = "data Part [copy]{n:u8; wide:u16;} machine get<const P:Part>()->u8 {P.n}
                  const BYTE:u8=get::<Part{n:42u8,wide:256u16}>(); ";
    for suffix in [
        "const ANSWER:u8=0u8 +% BYTE;",
        "machine read()->u8{0u8 +% BYTE} machine outer()->u8{read()} const ANSWER:u8=outer();",
    ] {
        spec_corpus::observe_handoff!(
            "closed-argument-fields/refusal",
            witness(
                &format!("{prefix}{suffix}"),
                |v| assert_eq!(*v, PreparedValue::Integer(42.into())),
                change_selected_part,
                ("{P.n}", "P"),
                PreparationReason::ArgumentOutsideCarrier,
                OUTSIDE,
            ),
        );
    }
}

#[test]
fn a_representable_snapshot_is_not_proof_of_the_selected_calls_predicate() {
    let call = "get::<Part{n:42u8,wide:256u16}>()";
    let source = format!("data Part [copy]{{n:u8; wide:u16;}} machine get<const P:Part>()->u8 requires P.n == 42u8 {{P.n}} const ANSWER:u8={call};");
    witness(
        &source,
        |v| assert_eq!(*v, PreparedValue::Integer(42.into())),
        |facts| {
            let call = facts.calls.iter_mut().find(|call| call.machine.path == "get").unwrap();
            let [ValueType::StaticValue { value: StaticValue::Record { fields, .. }, .. }] = call.type_arguments.as_mut_slice() else { panic!("expected Part") };
            std::sync::Arc::make_mut(fields)[0] = StaticValue::Integer(41.into());
        },
        (call, call),
        PreparationReason::IntegerPreconditionFalse,
        "the selected integer precondition is false for the supplied arguments",
    );
}

#[test]
fn nested_snapshot_uses_active_case_and_applied_field_carriers_before_tuple_storage() {
    use std::sync::Arc;

    let source = "data Cell<T [copy]> [copy]{n:T;} data Choice [copy]{case Item(cell:Cell<u8>); case Empty;}
                  data Other [copy]{case Item(cell:Cell<u8>);}
                  machine keep<const P:[[Choice;1];1]>()->[[Choice;1];1]{P}
                  const ANSWER=(false,keep::<[[Choice::Item{cell:Cell::<u8>{n:255u8}}]]>());";
    for fault in [
        "array length", "nested array length", "case owner", "foreign case", "sum as record", "case arity", "record owner", "record arity", "field type",
        "field range", "tuple carrier",
    ] {
        witness(
            source,
            |value| {
                let PreparedValue::Tuple(pair) = value else { panic!("expected tuple") };
                let [PreparedValue::Bool(false), PreparedValue::Array(outer)] = pair.as_ref() else { panic!("expected owned array") };
                let [PreparedValue::Array(rows)] = outer.as_ref() else { panic!("expected nested array") };
                let [PreparedValue::Sum { owner, case, fields }] = rows.as_ref() else { panic!("expected active case") };
                assert_eq!((&*owner.path, &*case.path), ("Choice", "Choice::Item"));
                let [PreparedValue::Record { owner, fields }] = fields.as_ref() else { panic!("expected cell") };
                assert_eq!(owner.path, "Cell");
                assert_eq!(fields.as_ref(), &[PreparedValue::Integer(255.into())]);
            },
            |facts| {
                if fault == "tuple carrier" {
                    // Tuples are prepared values but have no StaticValue variant.
                    // Neither a scalar nor an array is an encoding of a tuple.
                    let (_, cell) = facts.static_data_shapes.iter_mut().find(|(ty, _)| matches!(ty, ValueType::Applied(owner, args) if owner.path == "Cell" && args == &[BYTE])).unwrap();
                    assert_eq!(cell.fields, [BYTE]);
                    cell.fields[0] = ValueType::Tuple(vec![BYTE]);
                    return;
                }
                let (foreign, other) = facts.case_owners.iter().find(|(case, _)| case.path == "Other::Item").unwrap();
                let call = facts.calls.iter_mut().find(|call| call.machine.path == "keep").unwrap();
                let [ValueType::StaticValue { value: StaticValue::Array(rows), .. }] = call.type_arguments.as_mut_slice() else { panic!("expected array argument") };
                if fault == "array length" {
                    *rows = [].into();
                    return;
                }
                let StaticValue::Array(inner) = &mut Arc::make_mut(rows)[0] else { panic!("expected nested array") };
                if fault == "nested array length" {
                    *inner = [].into();
                    return;
                }
                let row = &mut Arc::make_mut(inner)[0];
                let StaticValue::Sum { owner, case, fields } = row else { panic!("expected Choice::Item") };
                match fault {
                    "case owner" => *owner = other.clone(),
                    "foreign case" => *case = foreign.clone(),
                    "sum as record" => *row = StaticValue::Record { owner: owner.clone(), fields: fields.clone() },
                    "case arity" => *fields = [].into(),
                    _ => {
                        let StaticValue::Record { owner, fields } = &mut Arc::make_mut(fields)[0] else { panic!("expected Cell<u8>") };
                        match fault {
                            "record owner" => *owner = other.clone(),
                            "record arity" => *fields = [].into(),
                            "field type" => Arc::make_mut(fields)[0] = StaticValue::Bool(true),
                            "field range" => Arc::make_mut(fields)[0] = StaticValue::Integer(256.into()),
                            _ => unreachable!(),
                        }
                    }
                }
            },
            ("{P}", "P"),
            PreparationReason::ArgumentOutsideCarrier,
            OUTSIDE,
        );
    }
}

#[test]
fn named_constant_reads_use_the_carrier_of_the_selected_occurrence() {
    let cases: [(&str, fn(&PreparedValue)); 4] = [
        ("const BASE:u16=256u16; const ANSWER:u16=0u16 +% BASE;", |v| assert_eq!(*v, PreparedValue::Integer(256.into()))),
        ("const BASE:bool=true; const ANSWER:bool=!BASE;", |v| assert_eq!(*v, PreparedValue::Bool(false))),
        ("const BASE:f32=1.0f32; const ANSWER:f32=BASE;", |v| {
            assert_eq!(v.float(), Some(omega_preparation::PreparedFloat::Binary32(1.0_f32.to_bits())));
        }),
        ("const BASE:[u8;2]=[2u8,3u8]; const ANSWER=(false,BASE);", |v| {
            let PreparedValue::Tuple(pair) = v else { panic!("expected the tuple") };
            let [PreparedValue::Bool(false), PreparedValue::Array(elements)] = pair.as_ref() else { panic!("expected the array field") };
            assert_eq!(elements.as_ref(), &[PreparedValue::Integer(2.into()), PreparedValue::Integer(3.into())]);
        }),
    ];
    for (source, control) in cases {
        witness(
            source,
            control,
            |facts| {
                let [(node, _)] = facts.constant_references() else { panic!("expected one BASE read") };
                let node = *node;
                let read = facts.expressions.iter_mut().find(|e| e.node == node).unwrap();
                assert_ne!(read.ty, BYTE);
                read.ty = BYTE;
            },
            ("BASE", "BASE"),
            PreparationReason::ConstantCarrierMismatch,
            "the selected constant has a different carrier",
        );
    }
}

#[test]
fn a_bad_initializer_landing_is_refused_before_the_frozen_read() {
    let source = "const BYTE:u8=42u8; const ANSWER:u8=0u8 +% BYTE;";
    witness(
        source,
        |v| assert_eq!(*v, PreparedValue::Integer(42.into())),
        |facts| {
            let landing = facts.landings.iter_mut().find(|l| l.site.span.slice(source.as_bytes()) == Some(b"42u8")).unwrap();
            assert_eq!(landing.ty, BYTE);
            landing.value = num_bigint::BigInt::from(256).into();
        },
        ("42u8", "42u8"),
        PreparationReason::LandingOutsideCarrier,
        "the typed landing exceeds its carrier",
    );
}

#[test]
fn complete_type_admission_and_zero_storage_fail_at_different_sites() {
    let source = "data Part [copy]{case Some(n:u8); case None;} data Envelope [copy]{part:Part;} data Cover [copy]{inner:Envelope;} const ANSWER:Cover=Cover{inner:Envelope{..}};";
    let uninhabited = ValueType::Integer { signed: true, width: Some(0) };
    assert!(!uninhabited.contains_integer(&0.into()));
    for (payload_type, site, reason, text) in [
        (
            uninhabited,
            "Cover{inner:Envelope{..}}",
            PreparationReason::ConstantTypeOutsideFragment,
            "the complete constant type is outside the prepared value fragment",
        ),
        (
            ValueType::Array(Box::new(BYTE), u64::MAX),
            "Envelope{..}",
            PreparationReason::ZeroArrayNeedsStorage,
            "zero-filling an array requires checked preparation storage",
        ),
    ] {
        witness(
            source,
            |v| {
                let PreparedValue::Record { fields, .. } = v else { panic!("expected Cover") };
                let [PreparedValue::Record { fields, .. }] = fields.as_ref() else { panic!("expected Envelope") };
                let [PreparedValue::Sum { case, fields, .. }] = fields.as_ref() else { panic!("expected omitted Part") };
                assert_eq!(case.path, "Part::Some");
                assert_eq!(fields.as_ref(), &[PreparedValue::Integer(0.into())]);
            },
            |facts| {
                let (_, shape) = facts.static_data_shapes.iter_mut().find(|(ty, _)| matches!(ty, ValueType::Nominal(owner) if owner.path == "Part")).unwrap();
                let (case, payload) = &mut shape.cases[0];
                assert_eq!(case.path, "Part::Some");
                assert_eq!(payload, &[BYTE]);
                payload[0] = payload_type;
            },
            (site, site),
            reason,
            text,
        );
    }
}
