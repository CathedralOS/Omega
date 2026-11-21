use super::*;
use omega_preparation::{PreparationReason, PreparedValue, prepare_typing_pass_with_cause};
use omega_typed::TypedTrees;
use omega_typer::TypingProgress;

#[test]
fn intrinsic_layout_checks_the_declaration_projection_and_result_before_freezing() {
    // Check the whole array: no subsequent scalar read can mask a bad producer.
    use LayoutQuery::{Alignment, Size};
    enum Selection {
        Original(i32),
        Retag(LayoutQuery),
        NarrowResult,
    }
    use Selection::*;
    for (callee, carrier, selected, selection) in [
        ("size_of", "u8", None, Original(42)),
        ("size_of", "u8", None, Retag(Size)),
        ("size_of", "u64", None, Retag(Size)),
        (r"core\size_of", "u64", Some(Size), Original(256)),
        (r"core\align_of", "u64", Some(Alignment), Original(1)),
        (r"core\size_of", "u64", Some(Size), Retag(Alignment)),
        (r"core\size_of", "u64", Some(Size), NarrowResult),
        (r"core\align_of", "u64", Some(Alignment), NarrowResult),
    ] {
        let occurrence = format!("{callee}::<[u8;256]>()");
        let declarations = if selected.is_some() {
            r"use omega\language\core;".into()
        } else {
            format!("machine size_of<T>()->{carrier} {{42{carrier}}}")
        };
        let code = format!("{declarations} const ITEMS:[{carrier};1]=[{occurrence}];");
        let source = resolve(&code);
        let TypingProgress::Complete(trees) = type_source_package_for_preparation(&source, &TypingEnvironment::default()).unwrap() else {
            panic!("expected complete typing");
        };
        let resolved = trees.resolved().clone();
        let mut facts = trees.into_facts();
        let [constant] = facts.constants.as_mut_slice() else { panic!("expected one initializer") };
        let answer = constant.declaration.clone();
        let [call] = facts.calls.as_mut_slice() else { panic!("expected one call") };
        assert_eq!(call.layout_query, selected);
        if let Retag(query) = selection {
            call.layout_query = Some(query);
        }
        if let NarrowResult = selection {
            call.result = BYTE;
            constant.initializer.carrier = ValueType::Array(Box::new(BYTE), 1);
            for expression in &mut facts.expressions {
                if expression.node == call.node {
                    expression.ty = BYTE;
                } else if expression.node == constant.initializer.node {
                    expression.ty = constant.initializer.carrier.clone();
                }
            }
        }
        let subject = ValueType::Array(Box::new(BYTE), 256);
        assert_eq!(call.type_arguments.as_slice(), std::slice::from_ref(&subject));
        let target = identity("selected");
        let capsule = TargetSemantics::new(target.clone(), [layout(&target, subject, LayoutExtent::Fixed(256), Some(1))], []).unwrap();
        let progress = TypingProgress::Complete(TypedTrees::new(resolved, facts));
        let result = prepare_typing_pass_with_cause(&progress, &[], Some(&capsule), CheckedPreparationEvidence::default());
        let Original(expected) = selection else {
            let Err(error) = result else { panic!("expected refusal at {occurrence}") };
            let start = code.find(&occurrence).unwrap();
            spec_corpus::assert_preparation_cause!(
                error,
                RefusalCause::Preparation(PreparationReason::NotOneLayoutQuery),
                "app",
                "main.omg",
                (start, start + occurrence.len()),
                "static evaluation cannot admit this invocation: the request does not select one core layout query",
            );
            continue;
        };
        let pass = result.unwrap();
        assert_eq!(pass.target_observations.len(), usize::from(selected.is_some()));
        let PreparationAnswers::Frozen { constants, .. } = pass.answers else { panic!("expected frozen constants") };
        assert_eq!(constants[&answer], PreparedValue::Array([PreparedValue::Integer(expected.into())].into()));
    }
}
