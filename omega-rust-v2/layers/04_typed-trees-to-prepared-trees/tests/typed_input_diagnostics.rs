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
        let trees = typed_packages::typed(self.library, self.client);
        let answer = trees.constants.iter().find(|c| c.declaration.path == "ANSWER").unwrap().declaration.clone();
        let control = TypingProgress::Complete(trees);
        let PreparationAnswers::Frozen { constants, .. } = prepare(&control).unwrap() else { panic!("expected frozen constants") };
        assert_eq!(constants[&answer], PreparedValue::Integer(42u8.into()));

        let TypingProgress::Complete(trees) = control else { unreachable!() };
        let resolved = trees.resolved().clone();
        let mut facts = trees.into_facts();
        assert!(facts.library.is_none(), "these packages are typed together");
        change(&mut facts);
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
