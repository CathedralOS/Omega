use super::{checked, rejected, retained_self_content_source};
use language_semantics::content::{
    ContentAlgebraIdentity, ContentArithmeticOperator, ContentConservationOwnerKind,
    ContentConservationTerm, ContentPlaceRoot, ContentPlaceSegment, ContentPlaceVersion,
    ContentProjectionExpression, ContentScalarExpression,
};

#[test]
fn scalar_and_content_guarantees_are_checked_independently() {
    let source = retained_self_content_source("7")
        .replace("retain(&mut self)", "retain(&mut self) -> u64")
        .replace(
            "== Owned::content(&self.region)",
            "== Owned::content(&self.region); result == 7",
        );
    let accepted = checked(&source);
    assert_eq!(
        accepted
            .facts
            .qualifications
            .content
            .conservation_plans
            .len(),
        1
    );
    for invalid in [
        source.replace("result == 7", "result == 8"),
        source.replace("{ 7 }", "{ self.region.length = 5; 7 }"),
    ] {
        let diagnostics = rejected(&invalid);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
            "{diagnostics:#?}"
        );
    }
}

#[test]
fn checked_facts_retain_normalized_content_projection() {
    let source = r#"
        data ByteUnit {}
        data CountedQuantity<Unit> { magnitude: u64; }
        trait Content<A> {
            machine project(subject: &Self) -> A;
        }
        data Region [linear] { length: u64; }
        domain Region::Owned;

        machine Owned::content(region: &Region) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        {
            CountedQuantity { magnitude: region.length + 0x1 }
        }

        data Main {}
        machine Main::main(&mut self) {}
    "#;

    let checked = checked(source);
    let [plan] = checked.facts.qualifications.content.plans.as_slice() else {
        panic!("one normalized content projection should be retained");
    };
    assert!(plan.domain.is_valid());
    assert!(plan.machine.is_valid());
    assert_ne!(plan.report_fingerprint, 0);
    assert!(matches!(
        &plan.algebra,
        ContentAlgebraIdentity::CountedQuantity { unit }
            if unit == "named(name(ByteUnit))"
    ));
    let ContentProjectionExpression::CountedQuantity { magnitude } = &plan.expression else {
        panic!("quantity projection shape");
    };
    let ContentScalarExpression::Arithmetic {
        operator: ContentArithmeticOperator::Add,
        left,
        right,
    } = magnitude
    else {
        panic!("normalized addition");
    };
    assert!(
        matches!(
            left.as_ref(),
            ContentScalarExpression::SubjectField(path)
                if matches!(path.as_slice(), [field] if field.name == "length" && field.symbol.is_valid())
        ),
        "normalized subject path: {left:?}"
    );
    assert_eq!(
        right.as_ref(),
        &ContentScalarExpression::Natural("1".to_owned())
    );
}

#[test]
fn checked_facts_lift_a_singleton_into_the_interval_set_algebra() {
    let source = r#"
        data Nat {
            case Zero;
            case Succ(previous: Nat);
        }
        data PhysicalMemory {}
        data IntervalSet<Space> { start: Nat; end: Nat; }
        trait Content<A> {
            machine project(subject: &Self) -> A;
        }
        data Region [linear] { base: u64; length: u64; }
        domain Region::Owned;

        machine Owned::content(region: &Region) -> IntervalSet<PhysicalMemory>
        satisfies Content<IntervalSet<PhysicalMemory>>::project
        {
            IntervalSet {
                start: embed(region.base) as Nat,
                end: (embed(region.base) + embed(region.length)) as Nat
            }
        }

        data Main {}
        machine Main::main(&mut self) {}
    "#;

    let checked = checked(source);
    let [plan] = checked.facts.qualifications.content.plans.as_slice() else {
        panic!("one normalized content projection should be retained");
    };
    assert!(matches!(
        &plan.algebra,
        ContentAlgebraIdentity::IntervalSet { coordinate_space }
            if coordinate_space == "named(name(PhysicalMemory))"
    ));
    let ContentProjectionExpression::IntervalSet { members } = &plan.expression else {
        panic!("interval-set projection shape");
    };
    let [member] = members.as_slice() else {
        panic!("one singleton interval-set member");
    };
    assert!(matches!(
        member.start(),
        ContentScalarExpression::RuntimeScalarEmbedding(path)
            if matches!(path.as_slice(), [field] if field.name == "base")
    ));
    assert!(matches!(
        member.end(),
        ContentScalarExpression::Arithmetic {
            operator: ContentArithmeticOperator::Add,
            ..
        }
    ));
}

#[test]
fn checked_facts_retain_runtime_scalar_embedding() {
    let source = r#"
        data Nat {
            case Zero;
            case Succ(previous: Nat);
        }
        data ByteUnit {}
        data CountedQuantity<Unit> { magnitude: Nat; }
        trait Content<A> {
            machine project(subject: &Self) -> A;
        }
        data Region [linear] { length: u64; }
        domain Region::Owned;

        machine Owned::content(region: &Region) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        {
            CountedQuantity { magnitude: embed(region.length) as Nat }
        }

        data Main {}
        machine Main::main(&mut self) {}
    "#;

    let checked = checked(source);
    let [plan] = checked.facts.qualifications.content.plans.as_slice() else {
        panic!("one normalized content projection should be retained");
    };
    let ContentProjectionExpression::CountedQuantity { magnitude } = &plan.expression else {
        panic!("quantity projection shape");
    };
    assert!(matches!(
        magnitude,
        ContentScalarExpression::RuntimeScalarEmbedding(path)
            if matches!(path.as_slice(), [field] if field.name == "length" && field.symbol.is_valid())
    ));
}

#[test]
fn signed_runtime_embedding_cannot_enter_a_nat_content_algebra() {
    let diagnostics = rejected(
        r#"
        data Nat { case Zero; case Succ(previous: Nat); }
        data ByteUnit {}
        data CountedQuantity<Unit> { magnitude: Nat; }
        trait Content<A> { machine project(subject: &Self) -> A; }
        data Region [linear] { delta: i64; }
        domain Region::Owned;

        machine Owned::content(region: &Region) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        {
            CountedQuantity { magnitude: embed(region.delta) as Nat }
        }

        data Main {}
        machine Main::main(&mut self) {}
        "#,
    );

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("outside the closed projection fragment")
    }));
}

#[test]
fn checked_facts_retain_normalized_content_conservation() {
    let source = r#"
        data ByteUnit {}
        data CountedQuantity<Unit> { magnitude: u64; }
        trait Content<A> {
            machine project(subject: &Self) -> A;
        }
        data Region [linear] { length: u64; }
        domain Region::Owned;

        machine Owned::content(region: &Region) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        {
            CountedQuantity { magnitude: region.length }
        }

        data SplitResult {
            left: Region in Owned;
            right: Region in Owned;
        }

        trait Splitter {
            machine split(whole: Region in Owned) -> SplitResult
            ensures
                Owned::content(old(&whole))
                == separate(
                    Owned::content(&result.right),
                    Owned::content(&result.left),
                );
        }

        data Main {}
        machine Main::main(&mut self) {}
    "#;

    let checked = checked(source);
    let [plan] = checked
        .facts
        .qualifications
        .content
        .conservation_plans
        .as_slice()
    else {
        panic!("one normalized conservation equation should be retained");
    };
    assert_eq!(
        plan.owner_kind,
        ContentConservationOwnerKind::TraitRequirement
    );
    assert!(plan.owner.is_valid());
    assert!(plan.callable.is_valid());
    assert_ne!(plan.report_fingerprint, 0);
    assert!(matches!(
        &plan.algebra,
        ContentAlgebraIdentity::CountedQuantity { unit }
            if unit == "named(name(ByteUnit))"
    ));

    let ContentConservationTerm::Projection { subject, .. } = plan.equation.left() else {
        panic!("canonical equation left side should be the entry projection");
    };
    assert_eq!(subject.version, ContentPlaceVersion::Entry);
    assert!(matches!(
        &subject.root,
        ContentPlaceRoot::Parameter {
            position: 0,
            is_self: false,
            ..
        }
    ));
    assert!(subject.segments.is_empty());

    let ContentConservationTerm::Separate(outputs) = plan.equation.right() else {
        panic!("canonical equation right side should be separated outputs");
    };
    assert_eq!(outputs.len(), 2);
    let output_fields = outputs
        .iter()
        .map(|term| {
            let ContentConservationTerm::Projection { subject, .. } = term else {
                panic!("separate child should be a projection");
            };
            assert_eq!(subject.version, ContentPlaceVersion::Current);
            assert!(matches!(subject.root, ContentPlaceRoot::Result));
            let [ContentPlaceSegment::Field(field)] = subject.segments.as_slice() else {
                panic!("result projection should select one field");
            };
            field.name.as_str()
        })
        .collect::<Vec<_>>();
    assert_eq!(output_fields, ["left", "right"]);
}

#[test]
fn old_is_contextual_even_when_a_callable_parameter_has_the_same_name() {
    let source = r#"
        data ByteUnit {}
        data CountedQuantity<Unit> { magnitude: u64; }
        trait Content<A> { machine project(subject: &Self) -> A; }
        data Region [linear] { length: u64; }
        domain Region::Owned;
        machine Owned::content(region: &Region) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        { CountedQuantity { magnitude: region.length } }

        trait Migration {
            machine retain(whole: Region in Owned, old: u64)
            ensures
                Owned::content(old(&whole)) == Owned::content(&whole);
        }
    "#;

    let checked = checked(source);
    let [plan] = checked
        .facts
        .qualifications
        .content
        .conservation_plans
        .as_slice()
    else {
        panic!("one contextual old conservation equation")
    };
    let ContentConservationTerm::Projection { subject, .. } = plan.equation.left() else {
        panic!("old projection")
    };
    assert_eq!(subject.version, ContentPlaceVersion::Entry);
    assert!(matches!(
        subject.root,
        ContentPlaceRoot::Parameter { position: 0, .. }
    ));
}

#[test]
fn old_retains_an_exact_self_field_place_at_callable_entry() {
    let checked = checked(&retained_self_content_source(""));
    let [plan] = checked
        .facts
        .qualifications
        .content
        .conservation_plans
        .as_slice()
    else {
        panic!("one self-field old conservation equation")
    };
    let ContentConservationTerm::Projection { subject, .. } = plan.equation.left() else {
        panic!("old self-field projection")
    };
    assert_eq!(subject.version, ContentPlaceVersion::Entry);
    assert!(matches!(
        subject.root,
        ContentPlaceRoot::Parameter {
            position: 0,
            is_self: true,
            ..
        }
    ));
    assert!(matches!(
        subject.segments.as_slice(),
        [ContentPlaceSegment::Field(field)] if field.name == "region" && field.symbol.is_valid()
    ));
}

#[test]
fn content_preservation_requires_a_nonmutating_body_not_just_an_equation() {
    for body in [
        "self.region.length = 5;",
        "let alias: &mut Region in Owned = &mut self.region; alias.length = 5;",
    ] {
        let diagnostics = rejected(&retained_self_content_source(body));
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("cannot prove ensures")
                    && diagnostic.message.contains("Owned.content")
            }),
            "{body}: {diagnostics:#?}"
        );
    }
}

#[test]
fn old_rejects_result_local_computed_and_retired_entry_operands() {
    let diagnostics = rejected(
        r#"
        data ByteUnit {}
        data CountedQuantity<Unit> { magnitude: u64; }
        trait Content<A> { machine project(subject: &Self) -> A; }
        data Region [linear] { length: u64; }
        domain Region::Owned;
        machine Owned::content(region: &Region) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        { CountedQuantity { magnitude: region.length } }

        trait InvalidOldOperands {
            machine result_operand(whole: Region in Owned)
            ensures
                Owned::content(old(&result)) == Owned::content(&whole);
            machine local_operand(whole: Region in Owned)
            ensures
                Owned::content(old(&scratch)) == Owned::content(&whole);
            machine computed_operand(whole: Region in Owned)
            ensures
                Owned::content(old(&whole.length + 1)) == Owned::content(&whole);
            machine retired_entry_operand(whole: Region in Owned)
            ensures
                Owned::content(entry(&whole)) == Owned::content(&whole);
        }
        "#,
    );
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(rendered.contains("`old(result)` is invalid"), "{rendered}");
    assert!(
        rendered.contains("projection root `scratch` is not a callable parameter"),
        "{rendered}"
    );
    assert!(
        rendered.contains("is not a parameter, `self`, `result`, or structural subplace"),
        "{rendered}"
    );
    assert!(rendered.contains("entry(&whole)"), "{rendered}");
}
