//! These tests change one record in a successfully typed package. They exercise
//! L4's public input contract, including refusals a valid source cannot produce.
#[path = "support/typed_packages.rs"]
mod typed_packages;

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
fn incompatible_callback_parameters_refuse_the_instantiated_application() {
    let program = Program {
        library: "",
        client: "data NumberStep=machine(n:u64)->u64; machine identity(n:u64)->u64{n} machine apply<const F:NumberStep>(n:u64)->u64{F(n)} const ANSWER:u64=apply::<identity>(42u64);",
        occurrence: "apply::<identity>(42u64)",
    };
    program.check(PreparationReason::ForwardedCallbackNeedsRefinement, "the exact static callback needs checked refinement of its callable contract", |facts| {
        let signature = facts.signatures.iter_mut().find(|s| s.machine.path == "identity").unwrap();
        std::sync::Arc::make_mut(signature).parameters[0].1 = ValueType::Bool;
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
