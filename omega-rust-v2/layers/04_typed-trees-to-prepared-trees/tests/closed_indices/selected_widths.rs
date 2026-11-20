use super::*;
use omega_preparation::{PreparationFailure, PreparationReason, RefusalCause, prepare_source_package_diagnosed};
use omega_typed::StaticValue;

const SOURCE: &str = "data Size=machine()->u64 reaches none crashes none;
machine two()->u64{2u64}
data Word<const F:Size,const G:Size>{x:u<F()>;}
machine other()->u64{let x=Word::<two,other>{x:2u2}; 3u64}
machine consume(x:Word<two,other>){}";

fn assert_width(
    source: &str,
    recipe: &str,
    expected_owner: &str,
    expected: [&str; 2],
) {
    let prepared = prepare_source_package_diagnosed(&resolve(source)).unwrap();
    let [answer] = prepared.specialized_indices() else {
        panic!("expected one specialized field width")
    };
    assert_eq!(answer.request.expression.span.slice(source.as_bytes()), Some(recipe.as_bytes()));
    assert_eq!(answer.value, StaticValue::Integer(2.into()));
    let trees = prepared.typed();
    let consume = trees
        .signatures
        .iter()
        .find(|s| s.machine.path == "consume")
        .unwrap();
    let word = &consume.parameters[0].1;
    let (owner, arguments) = word.nominal_parts().unwrap();
    assert_eq!(owner.path, expected_owner);
    assert_eq!(arguments, answer.arguments);
    let [ValueType::MachineSelection(f), ValueType::MachineSelection(g)] = arguments else {
        panic!("expected both selected identities")
    };
    assert_eq!([f.machine.path.as_str(), g.machine.path.as_str()], expected);
    if expected[0] == "relay" {
        let [ValueType::MachineSelection(a), ValueType::MachineSelection(b)] = f.arguments.as_slice() else {
            panic!("expected nested selections")
        };
        assert_eq!((&*a.machine.path, &*b.machine.path), ("other", "two"));
    }
    assert_eq!(trees.static_data_shapes[word].fields, [ValueType::Integer { signed: false, width: Some(2) }]);
}

#[test]
fn an_unused_callback_is_part_of_the_identity_not_a_field_dependency() {
    let same = SOURCE.replace("two,other>", "two,two>");
    assert_width(&same, "F()", "Word", ["two", "two"]);
    assert_width(SOURCE, "F()", "Word", ["two", "other"]);
    let second = SOURCE
        .replace("u<F()>", "u<G()>")
        .replace("two,other>", "other,two>");
    assert_width(&second, "G()", "Word", ["other", "two"]);
}

#[test]
fn a_call_operand_uses_the_inner_binder_not_the_outer_binder_with_the_same_index() {
    let source = "data Size=machine()->u64 reaches none crashes none;
        machine two()->u64{2u64} machine measure(n:u64)->u64{n}
        data Outer<const G:Size>{data Word<const F:Size>{x:u<measure(F())>;}}
        machine other()->u64{let x=Outer::<other>::Word::<two>{x:2u2}; 3u64}
        machine consume(x:Outer<other>::Word<two>){}";
    assert_width(source, "measure(F())", "Outer::Word", ["other", "two"]);
    let cycle = source.replace("measure(F())", "measure(G())");
    assert_cycle(&cycle, "measure(G())");
}

#[test]
fn nested_and_forwarded_widths_wait_only_on_the_called_slot() {
    let source = SOURCE.replace("data Word", "machine relay<const A:Size,const B:Size>()->u64{B()}\ndata Word");
    let nested = source.replace("two,other>", "relay::<other,two>,other>");
    assert_width(&nested, "F()", "Word", ["relay", "other"]);
    let forwarded = source.replace("u<F()>", "u<relay::<G,F>()>");
    assert_width(&forwarded, "relay::<G,F>()", "Word", ["two", "other"]);

    // Now the forwarded second slot really does select the unfinished body.
    let cycle = forwarded.replace("relay::<G,F>()", "relay::<F,G>()");
    assert_cycle(&cycle, "relay::<F,G>()");
}

fn assert_cycle(
    source: &str,
    site: &str,
) {
    let failure = prepare_source_package_diagnosed(&resolve(source)).unwrap_err();
    assert_eq!(failure.cause(), None);
    let PreparationFailure::Typing(error) = failure else {
        panic!("expected an initial typing dependency cycle: {failure:?}")
    };
    let start = source.find(site).unwrap();
    assert_eq!((error.package.as_str(), error.file.as_str()), ("app", "main.omg"));
    assert_eq!((error.span.get_start_offset(), error.span.get_end_offset()), (start, start + site.len()));
    assert_eq!(error.message, "static evaluation is blocked by a typing dependency cycle");
}

#[test]
fn a_nested_callback_refinement_uses_the_initializer_arguments() {
    let source = SOURCE.replace(
        "data Word",
        "machine relay<const A:Size,const B:Size>()->u64{B()} machine run<const C:Size>()->u64{C()} data Word",
    );
    let recipe = "run::<relay::<G,F>>()";
    let source = source.replace("u<F()>", &format!("u<{recipe}>"));
    assert_width(&source, recipe, "Word", ["two", "other"]);
    let cycle = "run::<relay::<F,G>>()";
    assert_cycle(&source.replace(recipe, cycle), cycle);
}

#[test]
fn ignoring_an_unused_slot_does_not_supply_evidence_for_the_selected_one() {
    let source = SOURCE.replace("two()->u64{", "two()->u64 requires false {");
    let failure = prepare_source_package_diagnosed(&resolve(&source)).unwrap_err();
    assert!(matches!(failure, PreparationFailure::Preparation(_)), "{failure:?}");
    let start = source.find("F()").unwrap();
    spec_corpus::assert_preparation_refusal!(
        failure,
        RefusalCause::Preparation(PreparationReason::CallbackNeedsRefinement),
        "app",
        "main.omg",
        (start, start + 3),
        "static evaluation cannot admit this invocation: the exact static callback needs a checked refinement of its callable contract"
    );
}
