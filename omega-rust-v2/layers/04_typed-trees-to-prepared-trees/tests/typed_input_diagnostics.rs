//! These tests change one record in a successfully typed package. They exercise
//! L4's public input contract, including refusals a valid source cannot produce.
#[path = "support/typed_packages.rs"]
mod typed_packages;
#[path = "typed_input_diagnostics/providers.rs"]
mod providers;

use omega_preparation::{
    CheckedPreparationEvidence, PreparationAnswers, PreparationReason, PreparedValue, RefusalCause,
    prepare_typing_pass_with_cause,
};
use omega_typed::{TypedStage, TypedTrees, ValueType};
use omega_typer::TypingProgress;

fn prepare(progress: &TypingProgress<'_>) -> Result<PreparationAnswers, omega_preparation::PreparationRefusal> {
    let pass = prepare_typing_pass_with_cause(progress, &[], None, CheckedPreparationEvidence::default())?;
    Ok(pass.answers)
}

struct Program {
    library: &'static str,
    client: &'static str,
    occurrence: &'static str,
}

impl Program {
    fn check(
        &self,
        expected: PreparationReason,
        text: &str,
        change: impl FnOnce(&mut TypedStage),
    ) {
        self.check_changed(expected, text, |trees| {
            let mut facts = trees.into_facts();
            change(&mut facts);
            facts
        });
    }

    fn check_changed(
        &self,
        expected: PreparationReason,
        text: &str,
        change: impl FnOnce(TypedTrees<'_>) -> TypedStage,
    ) {
        let trees = typed_packages::typed(self.library, self.client);
        let answer = trees.constants.iter().find(|c| c.declaration.path == "ANSWER").unwrap().declaration.clone();
        let control = TypingProgress::Complete(trees);
        let PreparationAnswers::Frozen { constants, .. } = prepare(&control).unwrap() else { panic!("expected frozen constants") };
        assert_eq!(constants[&answer], PreparedValue::Integer(42u8.into()));

        let TypingProgress::Complete(trees) = control else { unreachable!() };
        let resolved = trees.resolved().clone();
        assert!(trees.library.is_none(), "these packages are typed together");
        let facts = change(trees);
        let changed = TypingProgress::Complete(TypedTrees::new(resolved, facts, 0));
        let error = prepare(&changed).unwrap_err();
        // The expected occurrence is the first spelling in the test's client file.
        let start = self.client.find(self.occurrence).unwrap();
        spec_corpus::assert_preparation_cause!(
            error, RefusalCause::Preparation(expected), "client", "client.omg", (start, start + self.occurrence.len()),
            format!("static evaluation cannot admit this invocation: {text}"),
        );
    }
}

const READ: Program = Program {
    library: "",
    client: "const ANSWER:u8=1u8 +% BASE; const BASE:u8=41u8;",
    occurrence: "BASE",
};

#[test]
fn a_selected_constant_needs_its_definition() {
    READ.check(PreparationReason::ConstantWithoutInitializer, "the selected constant has no local typed initializer", |facts| {
        facts.constants.retain(|c| c.declaration.path != "BASE");
    });
}

#[test]
fn a_constant_read_needs_one_selection() {
    READ.check(PreparationReason::AmbiguousConstantReference, "ambiguous constant reference", |facts| {
        let [(node, declaration)] = facts.constant_references() else { panic!("expected one constant read") };
        facts.record_constant_reference(*node, declaration.clone());
    });
}

#[test]
fn a_constant_read_needs_its_declared_carrier() {
    READ.check(PreparationReason::ConstantCarrierMismatch, "the selected constant has a different carrier", |facts| {
        let base = facts.constants.iter_mut().find(|c| c.declaration.path == "BASE").unwrap();
        base.initializer.carrier = ValueType::Bool;
    });
}

#[test]
fn a_constant_read_inside_nested_calls_preserves_both_the_read_and_call_chain() {
    let program = Program {
        library: "",
        client: "const ANSWER:u8=outer(); machine outer()->u8{inner()} machine inner()->u8{1u8 +% BASE} const BASE:u8=41u8;",
        occurrence: "BASE",
    };
    program.check(PreparationReason::ConstantWithoutInitializer, "the selected constant has no local typed initializer\ncall chain: outer -> inner", |facts| {
        facts.constants.retain(|c| c.declaration.path != "BASE");
    });
}

const CALL: Program = Program {
    library: "public machine increment(n:u8)->u8 reaches none crashes none terminates {n +% 1u8}",
    client: "const ANSWER:u8=1u8 +% geo\\increment(40u8);",
    occurrence: "geo\\increment(40u8)",
};

#[test]
fn the_selected_call_result_must_agree_with_the_operand_type() {
    CALL.check(PreparationReason::NotOneOrdinaryInvocation, "the request does not select one ordinary static invocation", |facts| {
        let [call] = facts.calls.as_mut_slice() else { panic!("expected one call") };
        call.result = ValueType::Bool;
    });
}

#[test]
fn an_invocation_needs_the_selected_library_signature() {
    CALL.check(PreparationReason::DeclarationWithoutSignature, "the selected declaration has no typed signature", |facts| {
        facts.signatures.retain(|signature| signature.machine.path != "increment");
    });
}

#[test]
fn a_library_signature_needs_a_body_in_its_own_file() {
    CALL.check(PreparationReason::DeclarationWithoutBody, "the selected declaration has no local body", |facts| {
        let [signature] = facts.signatures.as_mut_slice() else { panic!("expected one signature") };
        std::sync::Arc::make_mut(signature).node.file = usize::MAX;
    });
}

#[test]
fn a_library_body_needs_its_effect_row() {
    CALL.check(PreparationReason::BodyWithoutEffectRow, "the selected body has no complete typed effect row", |facts| {
        facts.machine_effects.retain(|row| row.machine.path != "increment");
    });
}

#[test]
fn a_reachable_service_in_the_selected_body_prevents_static_execution() {
    let program = Program {
        library: "boundary trait Sensor{} machine sense() reaches Sensor {} public machine increment(n:u8)->u8 reaches none crashes none terminates {n +% 1u8}",
        ..CALL
    };
    spec_corpus::observe_handoff!(
        "boundary-service-reach/consumer",
        program.check(
            PreparationReason::InvocationNotEffectFree,
            "the selected invocation is not effect-free during preparation",
            |facts| {
                let sensor = facts
                    .machine_effects
                    .iter()
                    .find(|row| row.machine.path == "sense")
                    .unwrap();
                let [service] = sensor.reach.as_slice() else {
                    panic!("expected one reachable service")
                };
                assert_eq!(service.service.path, "Sensor");
                let reach = sensor.reach.clone();
                let increment = facts
                    .machine_effects
                    .iter_mut()
                    .find(|row| row.machine.path == "increment")
                    .unwrap();
                assert!(increment.reach.is_empty());
                std::sync::Arc::get_mut(increment).unwrap().reach = reach;
            }
        ),
    );
}

#[test]
fn a_blocking_library_body_cannot_execute_during_preparation() {
    CALL.check(PreparationReason::InvocationNotEffectFree, "the selected invocation is not effect-free during preparation", |facts| {
        let [effects] = facts.machine_effects.as_mut_slice() else { panic!("expected one effect row") };
        std::sync::Arc::get_mut(effects).unwrap().fixed_pause.block = true;
    });
}

#[test]
fn a_call_needs_the_operands_of_its_selected_signature() {
    CALL.check(PreparationReason::CallParametersDisagree, "the typed call and selected parameters disagree", |facts| {
        let [call] = facts.calls.as_mut_slice() else { panic!("expected one call") };
        call.operands.clear();
    });
}

#[test]
fn an_evaluation_request_must_identify_an_existing_file_and_expression() {
    let program = Program { library: "", client: "const ANSWER:u8=42u8;", occurrence: "42u8" };
    program.check(PreparationReason::RequestWithoutTypedFile, "the evaluation request has no typed source file", |facts| {
        facts.constants[0].initializer.node.file = usize::MAX;
    });
    // A missing node in a valid file still has the request's original source span.
    program.check(PreparationReason::RequestWithoutTypedNode, "the evaluation request has no expression at the selected node", |facts| {
        facts.constants[0].initializer.node.node = usize::MAX;
    });
}

const RECORD: Program = Program {
    library: "",
    client: "data Pair [copy]{left:u8; right:u8;} machine first(p:Pair)->u8{p.left} const ANSWER:u8=first(Pair{right:1u8,left:42u8});",
    occurrence: "Pair{right:1u8,left:42u8}",
};

fn changed_record(
    reason: PreparationReason,
    text: &str,
    change: impl FnOnce(&mut omega_typed::TypedConstruction, &mut TypedStage),
) {
    RECORD.check_changed(reason, text, |trees| {
        let (node, mut construction) = trees.trees().find_map(|tree| {
            let file = tree.resolved().file;
            (0..file.nodes.len()).find_map(|i| Some((file.node_id(i), tree.construction(i)?.clone())))
        }).expect("one typed Pair construction");
        let mut facts = trees.into_facts();
        change(&mut construction, &mut facts);
        facts.record_construction(node, construction);
        facts
    });
}

#[test]
fn a_constructor_in_a_call_reports_its_own_inconsistent_inputs() {
    changed_record(PreparationReason::ConstructorCarrierMismatch, "the selected constructor has a different carrier", |construction, _| {
        construction.carrier = ValueType::Bool;
    });
    changed_record(PreparationReason::CaseOutsideCarrier, "the selected case does not belong to the carrier", |construction, _| {
        construction.case = Some(construction.owner.clone());
    });
    changed_record(PreparationReason::ConstructorFieldWithoutInitializer, "the selected constructor has a field without an initializer or typed zero request", |construction, _| {
        construction.initializers.pop();
    });
}

#[test]
fn admitting_an_aggregate_argument_needs_its_complete_field_shape() {
    let request = Program { occurrence: "first(Pair{right:1u8,left:42u8})", ..RECORD };
    request.check(PreparationReason::ConstantTypeWithoutFieldShape, "the complete constant type has no finite typed field shape", |facts| {
        assert_eq!(facts.static_data_shapes.len(), 1);
        facts.static_data_shapes.clear();
    });
}

#[test]
fn a_constructor_can_only_schedule_its_own_initializers_once_per_field() {
    for outside in [0, usize::MAX] {
        changed_record(PreparationReason::InitializerOutsideConstruction, "the selected field initializer is outside its construction", |construction, _| {
            construction.initializers[0].expression = outside;
        });
    }
    for position in [0, usize::MAX] {
        changed_record(PreparationReason::InvalidConstructorFieldSlot, "the selected field initializer has no unique position in this construction", |construction, _| {
            assert_eq!(construction.initializers[1].position, 0);
            construction.initializers[0].position = position;
        });
    }
}

#[test]
fn a_constant_request_cannot_freeze_another_expressions_value() {
    for library in ["", "public const FOREIGN:u8=99u8;"] {
        let program = Program { library, client: "const ANSWER:u8=42u8; const OTHER:u8=7u8;", occurrence: "42u8" };
        program.check(PreparationReason::RequestSiteMismatch, "the evaluation request selects a different source occurrence", |facts| {
            let wrong = if library.is_empty() { "OTHER" } else { "FOREIGN" };
            let other = facts.constants.iter().find(|c| c.declaration.path == wrong).unwrap().initializer.node;
            let answer = facts.constants.iter_mut().find(|c| c.declaration.path == "ANSWER").unwrap();
            answer.initializer.node = other;
        });
    }
}

#[test]
fn an_integer_literal_reports_its_own_invalid_typed_landing() {
    use num_rational::BigRational;
    let program = Program {
        library: "",
        client: "const ANSWER:u8=outer(); machine outer()->u8{inner()} machine inner()->u8{1u8 +% 41u8}",
        occurrence: "41u8",
    };
    for (reason, message, value, duplicate) in [
        (PreparationReason::LandingNotIntegral, "the typed landing does not supply an integral value", BigRational::new(1.into(), 2.into()), false),
        (PreparationReason::LandingOutsideCarrier, "the typed landing exceeds its carrier", BigRational::from_integer(256.into()), false),
        (PreparationReason::ConflictingIntegerLandings, "conflicting integer landings at one typed occurrence", BigRational::from_integer(40.into()), true),
    ] {
        program.check(reason, &format!("{message}\ncall chain: outer -> inner"), |facts| {
            let landing = facts.landings.iter_mut().find(|l| l.site.span.slice(program.client.as_bytes()) == Some(b"41u8")).unwrap();
            if duplicate {
                let extra = omega_typed::TypedNumericLanding { node: landing.node, site: landing.site.clone(), ty: landing.ty.clone(), value };
                facts.landings.push(extra);
            } else {
                landing.value = value;
            }
        });
    }
}

#[test]
fn static_callbacks_require_the_selected_signature_and_its_effects() {
    let program = Program {
        library: "",
        client: "data NumberStep=machine(n:u64)->u64; machine identity(n:u64)->u64{n} machine apply<const F:NumberStep>(n:u64)->u64{F(n)} const ANSWER:u64=apply::<identity>(42u64);",
        occurrence: "apply::<identity>(42u64)",
    };
    program.check(PreparationReason::RefinementApplicationMismatch, "the static callable refinement does not identify the selected application", |facts| {
        let index = facts.obligations.iter_mut().find_map(|obligation| match &mut obligation.kind {
            omega_typed::ObligationKind::RefineCallable { formal: Some((_, index)), .. } => Some(index),
            _ => None,
        }).expect("a refinement obligation for identity");
        *index = usize::MAX;
    });
    program.check(PreparationReason::CallbackWithoutBound, "the static callback has no typed bound", |facts| {
        facts.callable_bounds.retain(|(owner, index, _)| !(owner.path == "apply" && *index == 0));
    });
    program.check(PreparationReason::RefinementWithoutCaller, "the static callable refinement has no closed caller application", |facts| {
        let apply = facts.calls.iter_mut().find(|call| call.machine.path == "apply").unwrap();
        apply.type_arguments.clear();
    });
    program.check(PreparationReason::CallbackWithoutSignature, "the static callback has no typed signature", |facts| {
        facts.signatures.retain(|s| s.machine.path != "identity");
    });
    program.check(PreparationReason::CallbackWithoutOperationalBounds, "the exact static callback has no complete operational bounds", |facts| {
        facts.machine_effects.retain(|row| row.machine.path != "identity");
    });
    program.check(PreparationReason::CallbackEffectsNotEmpty, "the exact static callback has unresolved or nonempty operational effects", |facts| {
        let effects = facts.machine_effects.iter_mut().find(|row| row.machine.path == "identity").unwrap();
        std::sync::Arc::get_mut(effects).unwrap().fixed_pause.block = true;
    });
    program.check(PreparationReason::ForwardedCallbackNeedsRefinement, "the exact static callback needs checked refinement of its callable contract", |facts| {
        let signature = facts.signatures.iter_mut().find(|s| s.machine.path == "identity").unwrap();
        std::sync::Arc::make_mut(signature).parameters[0].1 = ValueType::Bool;
    });
}

#[test]
fn a_skipped_callback_cannot_have_a_fixed_suspension_effect() {
    let program = Program {
        library: "",
        client: "data Step=machine()->u8 suspends; machine ready()->u8{0u8}
                 machine choose<const F:Step>(run:bool)->u8 suspends {transition run {true -> suspend F(), false -> 42u8,}}
                 const ANSWER:u8=choose::<ready>(false);",
        occurrence: "choose::<ready>(false)",
    };
    // Both passes start at L4 with a complete typed input. The acknowledged
    // branch is skipped in both; only the selected body's fixed effect changes.
    spec_corpus::observe_handoff!(
        "selected-callback-effects/consumer",
        program.check(PreparationReason::CallbackEffectsNotEmpty, "the exact static callback has unresolved or nonempty operational effects", |facts| {
            let [call] = facts.indirect_calls.as_slice() else { panic!("expected the template invocation") };
            assert!(call.acknowledgements.suspend && call.contract.pause.suspend);
            let row = facts.machine_effects.iter_mut().find(|row| row.machine.path == "ready").unwrap();
            assert!(!row.fixed_pause.suspend);
            std::sync::Arc::get_mut(row).unwrap().fixed_pause.suspend = true;
        })
    );
}

#[test]
fn an_indirect_call_reports_which_operand_or_selection_it_cannot_admit() {
    let program = Program {
        library: "",
        client: "data Step=machine(n:u64)->u64; machine identity(n:u64)->u64{n} machine apply<const F:Step>(n:u64)->u64{F(n)} const ANSWER:u64=apply::<identity>(42u64);",
        occurrence: "F(n)",
    };
    let chain = "\ncall chain: apply -> <indirect or unsupported call>";
    program.check(PreparationReason::CallbackDisagreesWithCall, &format!("the selected static callback disagrees with the typed call{chain}"), |facts| {
        facts.indirect_calls[0].operands.clear();
    });
    program.check(PreparationReason::RuntimeCallableArgument, &format!("preparation cannot materialize a runtime callable argument{chain}"), |facts| {
        let call = &mut facts.indirect_calls[0];
        call.operands[0].callable = Some(call.entry.clone());
    });
    program.check(PreparationReason::CallbackWithoutSelection, &format!("the static callback has no closed selection{chain}"), |facts| {
        let omega_typed::InvocationTarget::StaticParameter { index, .. } = &mut facts.indirect_calls[0].entry else {
            panic!("expected F to select a static parameter");
        };
        *index = 1;
    });
    program.check(PreparationReason::InexactCallbackSelection, &format!("preparation requires an exact static callback selection{chain}"), |facts| {
        let call = &mut facts.indirect_calls[0];
        call.entry = omega_typed::InvocationTarget::StoredCallable(call.site.clone());
    });
}

#[test]
fn static_call_admission_reports_the_missing_operand_or_invalid_carrier() {
    CALL.check(PreparationReason::NoLocalScalarOperand, "the static call does not have a local scalar operand", |facts| {
        facts.calls[0].operands[0].node.file = usize::MAX;
    });
    CALL.check(PreparationReason::ArgumentOutsideCarrier, "a closed argument is outside its selected carrier", |facts| {
        let carrier = facts.signatures[0].parameters[0].1.clone();
        facts.calls[0].operands[0].subject = ValueType::StaticInteger { value: 256u16.into(), carrier: Box::new(carrier) };
    });
    CALL.check(PreparationReason::ParameterOutsideIntegerFragment, "a selected parameter is outside the bounded integer fragment", |facts| {
        std::sync::Arc::make_mut(&mut facts.signatures[0]).parameters[0].1 = ValueType::NoResult;
    });
}

#[test]
fn a_tuple_reports_its_mismatched_nested_initializer() {
    let program = Program {
        library: "",
        client: "const ANSWER:u8=42u8; const INPUT=((1u8,2u8),0u8);",
        occurrence: "(1u8,2u8)",
    };
    program.check(PreparationReason::TupleWithoutInitializer, "the selected tuple has no matching typed initializer", |facts| {
        let input = facts.constants.iter_mut().find(|constant| constant.declaration.path == "INPUT").unwrap();
        let byte = ValueType::Integer { signed: false, width: Some(8) };
        let triple = ValueType::Tuple(vec![byte.clone(); 3]);
        input.initializer.carrier = ValueType::Tuple(vec![triple, byte]);
    });
}

#[test]
fn an_array_argument_reports_its_own_mismatched_length() {
    for (client, occurrence, reason, message) in [
        (r#"machine keep(x:[u8;2])->u8{42u8} const ANSWER:u8=keep("AB");"#, r#""AB""#, PreparationReason::ByteLiteralTypeMismatch, "the selected byte literal has a different array type"),
        ("machine keep(x:[u8;2])->u8{42u8} const ANSWER:u8=keep([1u8,2u8]);", "[1u8,2u8]", PreparationReason::ArrayWithoutInitializer, "the selected array value has no supported initializer"),
    ] {
        Program { library: "", client, occurrence }.check(reason, message, |facts| {
            let parameter = &mut std::sync::Arc::make_mut(&mut facts.signatures[0]).parameters[0].1;
            let ValueType::Array(_, length) = parameter else { panic!("expected an array parameter") };
            *length = 3;
        });
    }
}

#[test]
fn a_direct_call_cannot_evaluate_another_expressions_value() {
    let program = Program {
        library: "",
        client: "const ANSWER:u8=outer(); machine outer()->u8{keep(42u8) +% 0u8} machine keep(n:u8)->u8{n}",
        occurrence: "keep(42u8)",
    };
    program.check(PreparationReason::OperandSiteMismatch, "the call operand selects a different source occurrence\ncall chain: outer -> keep", |facts| {
        let other = facts.landings.iter().find(|l| l.site.span.slice(program.client.as_bytes()) == Some(b"0u8")).unwrap();
        let call = facts.calls.iter_mut().find(|call| call.machine.path == "keep").unwrap();
        call.operands[0].node = other.node;
    });
}

#[test]
fn a_static_callback_cannot_evaluate_another_expressions_value() {
    let program = Program {
        library: "",
        client: "data Step=machine(n:u64)->u64; machine identity(n:u64)->u64{n} machine apply<const F:Step>(n:u64)->u64{F(n) +% 0u64} const ANSWER:u64=apply::<identity>(42u64);",
        occurrence: "F(n)",
    };
    program.check(PreparationReason::OperandSiteMismatch, "the call operand selects a different source occurrence\ncall chain: apply -> <indirect or unsupported call>", |facts| {
        let other = facts.landings.iter().find(|l| l.site.span.slice(program.client.as_bytes()) == Some(b"0u64")).unwrap();
        facts.indirect_calls[0].operands[0].node = other.node;
    });
}

#[test]
fn a_floating_literal_reports_conflicting_typed_values_at_its_own_site() {
    let program = Program {
        library: "",
        client: "machine pick(x:f32)->u8{42u8} const ANSWER:u8=pick(0.5);",
        occurrence: "0.5",
    };
    program.check(PreparationReason::ConflictingFloatLandings, "conflicting floating landings at one typed occurrence", |facts| {
        let half = facts.landings.iter().find(|l| l.ty == ValueType::Float32).unwrap();
        facts.landings.push(omega_typed::TypedNumericLanding {
            node: half.node, site: half.site.clone(), ty: half.ty.clone(), value: num_rational::BigRational::from_integer(1.into()),
        });
    });
}

#[test]
fn invocation_contracts_require_their_typed_records() {
    const CLIENT: &str = "const ANSWER:u8=geo\\guarded(42u8);";
    const CALL_SITE: &str = "geo\\guarded(42u8)";
    let guarded = Program {
        library: "public machine guarded(x:u8)->u8 reaches none crashes Trap x == 0 terminates {x}",
        client: CLIENT,
        occurrence: CALL_SITE,
    };

    guarded.check(
        PreparationReason::CrashCauseWithoutRoute,
        "the selected crash cause has no typed route",
        |facts| {
            assert_eq!(facts.crash_routes.len(), 1);
            facts.crash_routes.clear();
        },
    );
    guarded.check(
        PreparationReason::CrashGuardWithoutPredicate,
        "the selected crash guard has no typed predicate",
        |facts| {
            let [route] = facts.crash_routes.as_slice() else {
                panic!("expected one crash route");
            };
            let guard = route.guard.clone().expect("expected a guarded route");
            let before = facts.obligations.len();
            facts.obligations.retain(|o| o.site != guard);
            assert_eq!(facts.obligations.len() + 1, before);
        },
    );
}

#[test]
fn owned_field_refusals_blame_the_selector() {
    use PreparationReason::*;
    use omega_typed::{PlaceStep, Projection};

    let program = Program {
        occurrence: "p.left",
        ..RECORD
    };
    for (reason, message) in [
        (FieldNotInActiveValue, "the selected field is not in the active owned value"),
        (OwnedFieldMismatch, "the selected owned field has a different result type or source tree"),
        (OwnedFieldWithoutRootType, "the selected owned field has no supported root type"),
        (FieldReadNeedsProof, "the selected field read requires proof or borrowed storage"),
    ] {
        program.check_changed(reason, message, |trees| {
            let (selector, mut step, field) = trees
                .trees()
                .find_map(|tree| {
                    let file = tree.resolved().file;
                    (0..file.nodes.len()).find_map(|index| {
                        let node = file.node_id(index);
                        let (_, path) = trees.place_path(node);
                        let [step] = path.as_slice() else { return None };
                        let Some(Projection::Field(field)) = &step.projection else { return None };
                        (file.span(index).slice(program.client.as_bytes()) == Some(b"p.left")).then(|| {
                            let original = PlaceStep {
                                base: step.base,
                                through: step.through.clone(),
                                projection: step.projection.clone(),
                            };
                            (node, original, field.as_ref().clone())
                        })
                    })
                })
                .expect("one typed field selector");

            let mut facts = trees.into_facts();
            match reason {
                FieldNotInActiveValue => {
                    let fields = facts
                        .record_fields
                        .values_mut()
                        .find(|fields| fields.iter().any(|entry| entry.declaration == field))
                        .expect("the selected field belongs to the control shape");
                    std::sync::Arc::make_mut(fields).retain(|entry| entry.declaration != field);
                }
                OwnedFieldMismatch => {
                    step.base.file = usize::MAX;
                    facts.record_place_step(selector, step);
                }
                OwnedFieldWithoutRootType => {
                    let [signature] = facts.signatures.as_slice() else { panic!("expected first") };
                    assert_eq!(signature.node.file, selector.file);
                    step.base = signature.node;
                    facts.record_place_step(selector, step);
                }
                FieldReadNeedsProof => {
                    let answer = facts.constants[0].declaration.clone();
                    facts.record_constant_reference(selector, answer);
                }
                _ => unreachable!(),
            }
            facts
        });
    }
}

#[test]
fn a_field_chain_reports_the_first_component_missing_from_the_active_value() {
    let client = "data Inner [copy]{value:u8;} data Outer [copy]{inner:Inner;} machine read(p:Outer)->u8{p.inner.value} const ANSWER:u8=read(Outer{inner:Inner{value:42u8}});";
    for (shape, occurrence) in [("Outer", "p.inner"), ("Inner", "p.inner.value")] {
        let program = Program {
            library: "",
            client,
            occurrence,
        };
        program.check(
            PreparationReason::FieldNotInActiveValue,
            "the selected field is not in the active owned value",
            |facts| {
                let (_, fields) = facts
                    .record_fields
                    .iter_mut()
                    .find(|(id, _)| id.path == shape)
                    .unwrap();
                assert_eq!(fields.len(), 1);
                std::sync::Arc::make_mut(fields).clear();
            },
        );
    }
}

#[test]
fn an_authored_postcondition_needs_both_the_signature_flag_and_its_proposition() {
    let source = "const ANSWER:u8=keep(42u8); machine keep(n:u8)->u8 ensures result == 0u8 {n}";
    let trees = typed_packages::typed("", source);
    let original = TypingProgress::Complete(trees);
    let refusal = prepare(&original).unwrap_err();
    let start = source.find("keep(42u8)").unwrap();
    let call = (start, start + "keep(42u8)".len());
    spec_corpus::assert_preparation_cause!(
        refusal, RefusalCause::Preparation(PreparationReason::PostconditionNeedsProof), "client", "client.omg", call,
        "static evaluation cannot admit this invocation: the selected postcondition requires a checked proof for the arguments and result",
    );

    let TypingProgress::Complete(trees) = original else { unreachable!() };
    let resolved = trees.resolved().clone();
    let mut facts = trees.into_facts();
    let signature = &mut facts.signatures[0];
    assert!(signature.has_guarantee);
    std::sync::Arc::make_mut(signature).has_guarantee = false;
    let changed = TypingProgress::Complete(TypedTrees::new(resolved.clone(), facts, 0));
    let refusal = prepare(&changed).unwrap_err();
    spec_corpus::assert_preparation_cause!(
        refusal, RefusalCause::Preparation(PreparationReason::PostconditionDeclarationMismatch), "client", "client.omg", call,
        "static evaluation cannot admit this invocation: the signature and declaration disagree about postconditions",
    );

    let TypingProgress::Complete(trees) = changed else { unreachable!() };
    let mut facts = trees.into_facts();
    std::sync::Arc::make_mut(&mut facts.signatures[0]).has_guarantee = true;
    let before = facts.obligations.len();
    facts.obligations.retain(|o| !matches!(o.kind, omega_typed::ObligationKind::ProveGuarantee { .. }));
    assert_eq!(facts.obligations.len() + 1, before);
    let changed = TypingProgress::Complete(TypedTrees::new(resolved, facts, 0));
    let refusal = prepare(&changed).unwrap_err();
    spec_corpus::assert_preparation_cause!(
        refusal, RefusalCause::Preparation(PreparationReason::PostconditionWithoutProposition), "client", "client.omg", call,
        "static evaluation cannot admit this invocation: the selected postcondition has no typed proposition",
    );
}

#[test]
fn a_signature_cannot_invent_an_authored_postcondition() {
    CALL.check(PreparationReason::PostconditionDeclarationMismatch, "the signature and declaration disagree about postconditions", |facts| {
        std::sync::Arc::make_mut(&mut facts.signatures[0]).has_guarantee = true;
    });
}

#[test]
fn preparation_checks_the_closed_type_against_its_selected_multiplicity() {
    let program = Program {
        library: "",
        client: "machine keep<T [copy]>(x:T)->T{x} const ANSWER:u8=keep(42u8);",
        occurrence: "keep(42u8)",
    };
    for missing in [false, true] {
        program.check(PreparationReason::MultiplicityBoundMismatch, "the selected type argument does not satisfy its multiplicity bound", |facts| {
            let bound = &mut std::sync::Arc::make_mut(&mut facts.signatures[0]).multiplicity_bounds[0];
            if missing {
                bound.0 = usize::MAX;
            } else {
                bound.1 = omega_typed::Multiplicity::Linear;
            }
        });
    }
}

#[test]
fn a_selected_generic_callback_checks_its_own_closed_bound() {
    let program = Program {
        library: "",
        client: "data Step=machine(n:u64)->u64; machine keep<T [copy]>(n:T)->T{n} machine apply<const F:Step>(n:u64)->u64{F(n)} const ANSWER:u64=apply::<keep::<u64> >(42u64);",
        occurrence: "apply::<keep::<u64> >(42u64)",
    };
    program.check(PreparationReason::MultiplicityBoundMismatch, "the selected type argument does not satisfy its multiplicity bound", |facts| {
        let callback = facts.signatures.iter_mut().find(|s| s.machine.path == "keep").unwrap();
        std::sync::Arc::make_mut(callback).multiplicity_bounds[0].1 = omega_typed::Multiplicity::Linear;
    });
}

#[test]
fn a_generic_constructor_cannot_discharge_another_machines_return_obligation() {
    let program = Program {
        library: "",
        client: "data Boxed<T [copy]> [copy]{value:T;} machine wrap<T [copy]>(x:T)->Boxed<T>{Boxed::<T>{value:x}} machine other()->u8{0u8} const INPUT=wrap(7u8); const ANSWER:u8=42u8;",
        occurrence: "Boxed::<T>{value:x}",
    };
    program.check(PreparationReason::ConstructionNeedsProof, "the selected constructor requires proof outside the prepared value fragment", |facts| {
        let other = &facts.signatures.iter().find(|signature| signature.machine.path == "other").unwrap().machine;
        let obligation = facts.obligations.iter_mut().find_map(|obligation| match &mut obligation.kind {
            omega_typed::ObligationKind::ReturnBorrow { machine } if machine.path == "wrap" => Some(machine),
            _ => None,
        }).expect("a return obligation for wrap");
        *obligation = other.clone();
    });
}

#[test]
fn a_projection_rejects_a_field_slot_of_a_different_carrier() {
    let program = Program {
        library: "",
        client: "data Pair [copy]{left:u8; right:u16;} machine first(p:Pair)->u8{p.left} const ANSWER:u8=first(Pair{right:256u16,left:42u8});",
        occurrence: "p.left",
    };
    program.check(PreparationReason::OwnedFieldMismatch, "the selected owned field has a different result type or source tree", |facts| {
        let (_, fields) = facts.record_fields.iter_mut().find(|(owner, _)| owner.path == "Pair").unwrap();
        let [left, right] = std::sync::Arc::make_mut(fields).as_mut_slice() else { panic!("expected the two record fields") };
        std::mem::swap(&mut left.declaration, &mut right.declaration);
    });
}
