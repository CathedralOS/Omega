#[path = "support/source_package.rs"]
mod support;

use omega_preparation::{CheckedPreparationEvidence, PreparationAnswers, prepare_source_package, prepare_typing_pass_with_cause};
use omega_typer::{StaticValue, StaticValueDemand, TypingEnvironment, TypingProgress, type_source_package, type_source_package_for_preparation};

const SOURCE: &str = include_str!("fixtures/pending_operator.omg");

#[test]
fn a_ready_overload_cannot_win_by_default_while_its_competitor_waits() {
    let code = "machine size()->u64{16u64} const SIZE:u64=size();
        domain u<8>::Ready; domain u<16>::Ready; data Box<T> [copy] {value:u8;}
        machine + wide(x:Box<u<SIZE> in Ready>,y:u8)->u8{x.value}
        machine + narrow(x:Box<u8 in Ready>,y:u8)->u8{9u8}
        machine first()->u8{Box::<u8 in Ready>{value:42u8}+3u8}
        machine second()->u8{Box::<u16 in Ready>{value:42u8}+3u8}
        const A:u8=first(); const B:u8=second();";
    let package = support::source_package([("family.omg", code)]);
    let TypingProgress::Pending(first) = type_source_package_for_preparation(&package, &Default::default()).unwrap() else {
        panic!("one overload depends on SIZE")
    };
    assert!(
        first
            .trees()
            .signatures
            .iter()
            .any(|s| s.machine.path == "narrow" && s.operator.is_some())
    );
    assert!(
        !first
            .trees()
            .calls
            .iter()
            .any(|c| matches!(c.machine.path.as_str(), "narrow" | "wide"))
    );

    let prepared = prepare_source_package(&package).unwrap();
    for (name, expected) in [("A", 9), ("B", 42)] {
        let constant = prepared
            .typed()
            .constants
            .iter()
            .find(|c| c.declaration.path == name)
            .unwrap();
        assert_eq!(prepared.integer_constant(&constant.declaration), Some(&expected.into()));
    }

    // When the pending width becomes 8 instead, the same two declarations conflict.
    let duplicate = code.replace("{16u64}", "{8u64}");
    let package = support::source_package([("family.omg", &duplicate)]);
    let error = prepare_source_package(&package).unwrap_err();
    assert_eq!(error.message, "duplicate normalized operator binding");
    assert_eq!(error.span.get_start_offset(), duplicate.find("+ narrow").unwrap());
}

#[test]
fn a_pending_operator_binder_can_precede_even_the_partial_signature() {
    let code = "machine size()->u64{8u64} const SIZE:u64=size();
        domain u<8>::Ready; domain u<16>::Ready; data Box{} trait Tag<T>{}
        data Id<T> = T; data ReadyByte = Id<u<SIZE> in Ready>;
        machine + add<const N:Tag<ReadyByte>>(x:Box,y:Box)->Box{x}";
    let package = support::source_package([("binder.omg", code)]);
    let TypingProgress::Pending(first) = type_source_package_for_preparation(&package, &Default::default()).unwrap() else {
        panic!("the binder needs the width")
    };
    assert!(!first.trees().signatures.iter().any(|s| s.machine.path == "add"));
    let prepared = prepare_source_package(&package).unwrap();
    let add = prepared
        .typed()
        .signatures
        .iter()
        .find(|s| s.machine.path == "add")
        .unwrap();
    assert_eq!(add.operator.as_ref().unwrap().token, "+");
}

#[test]
fn computed_operand_domain_defers_the_binding_and_its_use() {
    let package = support::source_package([("op.omg", SOURCE)]);
    let error = type_source_package(&package).unwrap_err();
    assert_eq!(error.message, "static expression requires an admitted evaluation result");
    assert_eq!(error.span.slice(SOURCE.as_bytes()), Some(b"SIZE".as_slice()));

    let initial = type_source_package_for_preparation(&package, &TypingEnvironment::default()).unwrap();
    let TypingProgress::Pending(first) = &initial else {
        panic!("the operand needs its width")
    };
    let [StaticValueDemand::Closed(width)] = first.demands() else {
        panic!("only the width should execute")
    };
    assert_eq!(width.expression.span.slice(SOURCE.as_bytes()), Some(b"SIZE".as_slice()));
    assert!(!first.trees().signatures.iter().any(|s| s.machine.path == "add"));
    let at_use = |site: &omega_typer::Site| site.file == "op.omg" && site.span.slice(SOURCE.as_bytes()) == Some(b"x+y".as_slice());
    assert!(!first.trees().calls.iter().any(|c| at_use(&c.site)));
    assert!(!first.trees().operations.iter().any(|op| at_use(&op.site)));

    let pass = prepare_typing_pass_with_cause(&initial, &[], None, CheckedPreparationEvidence::default()).unwrap();
    let PreparationAnswers::Pending(answers) = pass.answers else {
        panic!("expected a closed width receipt")
    };
    assert_eq!(answers, [(first.demands()[0].clone(), StaticValue::Integer(8.into()))]);
    let evaluated = [(width.clone(), answers[0].1.clone())];
    let environment = TypingEnvironment {
        evaluated_indices: &evaluated,
        ..Default::default()
    };
    let TypingProgress::Complete(resumed) = type_source_package_for_preparation(&package, &environment).unwrap() else {
        panic!("width settles selection")
    };
    let prepared = prepare_source_package(&package).unwrap();
    for trees in [&resumed, prepared.typed()] {
        let add = trees
            .signatures
            .iter()
            .find(|s| s.machine.path == "add")
            .unwrap();
        let [domain] = add.parameters[1].1.domains() else {
            panic!("expected the narrow domain")
        };
        let binding = add.operator.as_ref().unwrap();
        assert_eq!(binding.token, "+");
        assert_eq!(binding.home.as_ref(), Some(&domain.declaration));
        let use_site = trees.calls.iter().find(|call| at_use(&call.site)).unwrap();
        assert_eq!(use_site.machine, add.machine);
        assert!(!trees.operations.iter().any(|op| at_use(&op.site)));
    }
}

#[test]
fn computed_operator_home_waits_for_the_carrier_even_with_a_ready_signature() {
    let headers = "machine width()->u64{4u64+4u64} const SIZE:u64=width();";
    let operator = "machine + u8::Ready::add(x:u8,y:u8)->u8{x}";
    let qualified_operator = "machine + u8::Ready::add(x:u8 in u8::Ready,y:u8 in u8::Ready)->u8 in u8::Ready{x}";
    let caller = "machine read(x:u8 in u8::Ready)->u8 in u8::Ready{x+x}";
    for declarations in [
        "domain u<SIZE>::Ready; domain u<16>::Ready;",
        "domain u<16>::Ready; domain u<SIZE>::Ready;",
    ] {
        for (declaration, use_site) in [(operator, ""), (qualified_operator, caller)] {
            let source = format!("{headers} {declarations} {declaration} {use_site}");
            let package = support::source_package([("home.omg", source.as_str())]);
            let TypingProgress::Pending(first) = type_source_package_for_preparation(&package, &Default::default()).unwrap() else {
                panic!("the explicit home depends on SIZE")
            };
            let at_use = |site: &omega_typer::Site| site.span.slice(source.as_bytes()) == Some(b"x+x".as_slice());
            assert!(!first.trees().calls.iter().any(|c| at_use(&c.site)));
            assert!(!first.trees().operations.iter().any(|op| at_use(&op.site)));
            if use_site.is_empty() {
                assert!(
                    first
                        .trees()
                        .signatures
                        .iter()
                        .any(|s| s.machine.path.ends_with("::add") && s.operator.is_none())
                );
            }

            let prepared = prepare_source_package(&package).unwrap();
            let trees = prepared.typed();
            let (domain, _) = trees
                .domain_carriers
                .iter()
                .find(|(_, ty)| **ty == omega_typer::ValueType::Integer { signed: false, width: Some(8) })
                .unwrap();
            let add = trees
                .signatures
                .iter()
                .find(|s| s.machine.path.ends_with("::add"))
                .unwrap();
            assert_eq!(add.operator.as_ref().unwrap().home.as_ref(), Some(domain));
            if !use_site.is_empty() {
                assert_eq!(trees.calls.iter().find(|c| at_use(&c.site)).unwrap().machine, add.machine);
            }
        }
    }
}
