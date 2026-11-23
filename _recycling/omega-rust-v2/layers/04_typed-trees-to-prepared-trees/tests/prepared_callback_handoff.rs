#[path = "support/source_package.rs"]
mod source_package;

use omega_preparation::{
    PreparationFailure, PreparationProgress, PreparationReason, PreparationRound, PreparationStep, PreparedTrees, RefusalCause,
    prepare_source_package_diagnosed,
};
use omega_typer::{SourcePackage, TypingProgress, ValueType};
use source_package::source_package;

// Start without any supplied index values. The types inspected below must come
// from L3 after it has consumed L4's answers, not from the provisional forest.
fn resume<'text>(package: &SourcePackage<'text>) -> PreparedTrees<'text> {
    let mut round = PreparationRound::begin(package, &[], None).unwrap();
    assert!(matches!(round.typing(), TypingProgress::Pending(_)));
    loop {
        let answers = round.evaluate(Default::default()).unwrap();
        match round.advance(answers).unwrap() {
            PreparationStep::Evaluate(next) => round = *next,
            PreparationStep::Finished(PreparationProgress::Complete(prepared)) => return prepared,
            PreparationStep::Finished(PreparationProgress::OpenTarget(_)) => panic!("callback needs no layout input"),
        }
    }
}

#[test]
fn a_quiet_selection_under_a_suspending_bound_closes_the_extent() {
    let callbacks = "data Maybe=machine()->u64 suspends; data Immediate=machine()->u64;
        machine ready()->u64{99u64} machine parked()->u64 suspends {99u64}
        machine two()->u64{2u64} machine five()->u64{5u64}
        machine choose<const Wait:Maybe,const Now:Immediate>(wait:bool)->u64 suspends {
            transition wait {true -> suspend Wait(), false -> Now(),}
        }";
    let use_site = "const LENGTH:u64=choose::<ready,two>(false); machine consume(a:[u8;LENGTH]){}";
    for (selection, expected) in [("two", 2), ("five", 5)] {
        let source = use_site.replace("ready,two", &format!("ready,{selection}"));
        let package = source_package([("callbacks.omg", callbacks), ("extent.omg", source.as_str())]);
        let prepared = resume(&package);
        let constant = prepared
            .typed()
            .constants
            .iter()
            .find(|c| c.declaration.path == "LENGTH")
            .unwrap();
        spec_corpus::observe_handoff!(
            "static-invocation-admission/frozen-result",
            assert_eq!(prepared.integer_constant(&constant.declaration), Some(&expected.into()), "{selection}")
        );
        let consume = prepared
            .typed()
            .signatures
            .iter()
            .find(|s| s.machine.path == "consume")
            .unwrap();
        let byte = ValueType::Integer { signed: false, width: Some(8) };
        spec_corpus::observe_handoff!(
            "static-invocation-admission/resumed-quiet-extent",
            assert_eq!(consume.parameters[0].1, ValueType::Array(Box::new(byte), expected), "{selection}")
        );
    }

    // A source selection with fixed suspension is refused by typing, even
    // though Wait() would be skipped. It is not an L4 admission refusal.
    let changed = use_site.replace("ready,two", "parked,two");
    let package = source_package([("callbacks.omg", callbacks), ("extent.omg", changed.as_str())]);
    let failure = prepare_source_package_diagnosed(&package).unwrap_err();
    let (PreparationFailure::Typing(error) | PreparationFailure::ResumedTyping(error)) = failure else {
        panic!("expected typing refusal")
    };
    assert_eq!((error.package.as_str(), error.file.as_str()), ("app", "extent.omg"));
    assert_eq!(error.span.slice(changed.as_bytes()), Some(b"choose::<parked,two>(false)".as_slice()));
    assert_eq!(error.message, "fact or static argument requires an effect-free computation");
}

const WORD: &str = "data Size=machine()->u64 reaches none crashes none;
    machine two()->u64{2u64} machine five()->u64{5u64}
    data Word<const F:Size,const G:Size>{bits:u<G()>;}";

#[test]
fn the_second_callback_slot_closes_each_field_read() {
    let reads = "machine narrow(a:Word<five,two>)->u<2>{a.bits}
                 machine wide(b:Word<two,five>)->u<5>{b.bits}";
    let package = source_package([("word.omg", WORD), ("reads.omg", reads)]);
    let prepared = resume(&package);
    for (read, width) in [("a.bits", 2), ("b.bits", 5)] {
        let projection = prepared
            .typed()
            .expressions
            .iter()
            .find(|e| e.site.file == "reads.omg" && e.site.span.slice(reads.as_bytes()) == Some(read.as_bytes()))
            .expect("typed field read");
        spec_corpus::observe_handoff!(
            "specialized-callback-field/resumed-read",
            assert_eq!(
                projection.ty,
                ValueType::Integer {
                    signed: false,
                    width: Some(width)
                },
                "{read}"
            )
        );
    }
}

#[test]
fn the_selected_field_slot_cannot_supply_a_refinement() {
    let word = WORD.replace("five()->u64{", "five()->u64 requires false {");
    // The same restricted machine may occupy the unused slot. Swap only the
    // arguments: the refusal must now identify the indirect call in Word.
    let read = "machine consume(a:Word<five,two>){}";
    let input = source_package([("word.omg", word.as_str()), ("read.omg", read)]);
    prepare_source_package_diagnosed(&input).unwrap();
    let changed = read.replace("five,two", "two,five");
    let input = source_package([("word.omg", word.as_str()), ("read.omg", changed.as_str())]);
    let PreparationFailure::Preparation(refusal) = prepare_source_package_diagnosed(&input).unwrap_err() else {
        panic!("expected a preparation refusal at the field recipe")
    };
    let start = word.find("G()").unwrap();
    spec_corpus::observe_handoff!(
        "specialized-callback-field/selected-refusal",
        spec_corpus::assert_preparation_cause!(
            refusal,
            RefusalCause::Preparation(PreparationReason::CallbackNeedsRefinement),
            "app",
            "word.omg",
            (start, start + 3),
            "static evaluation cannot admit this invocation: the exact static callback needs a checked refinement of its callable contract"
        )
    );
}

#[test]
fn nested_helper_applications_close_distinct_array_parameters() {
    for call in ["helper(input)", "invoke::<helper>(input)"] {
        let source = format!(
            "data Step=machine(n:u64)->u64;
             machine invoke<const F:Step>(n:u64)->u64{{F(n)}}
             machine outer<const N:u64>(input:u64)->u64{{
                 {call}
                 machine helper(n:u64)->u64{{N +% n}}
             }}
             const SMALL:u64=outer::<3>(2u64);
             const LARGE:u64=outer::<9>(2u64);
             machine receive(a:[u8;SMALL],b:[u8;LARGE]){{}}"
        );
        let package = source_package([("nested.omg", source.as_str())]);
        let prepared = resume(&package);
        let receive = prepared
            .typed()
            .signatures
            .iter()
            .find(|s| s.machine.path == "receive")
            .unwrap();
        let closed: Vec<_> = receive.parameters.iter().map(|(_, ty)| ty.clone()).collect();
        let byte_array = |n| ValueType::Array(Box::new(ValueType::Integer { signed: false, width: Some(8) }), n);
        spec_corpus::observe_handoff!(
            "nested-helpers-and-tail-expressions/resumed-extents",
            assert_eq!(closed, [byte_array(5), byte_array(11)], "{call}")
        );
    }
}
