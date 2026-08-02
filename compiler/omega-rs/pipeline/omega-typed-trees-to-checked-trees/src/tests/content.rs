use super::*;
use omega_core::content::{
    ContentAlgebraIdentity, ContentArithmeticOperator, ContentConservationOwnerKind,
    ContentConservationTerm, ContentPlaceRoot, ContentPlaceSegment, ContentPlaceVersion,
    ContentProjectionExpression, ContentScalarExpression,
};

fn checked(source: &str) -> omega_checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

fn rejected(source: &str) -> Vec<omega_core::diagnostics::Diagnostic> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect_err("checked lowering should reject")
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
    assert_ne!(plan.fingerprint, 0);
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
        boundary machine embed<T>(value: T) -> Nat;
        data Region [linear] { base: u64; length: u64; }
        domain Region::Owned;

        machine Owned::content(region: &Region) -> IntervalSet<PhysicalMemory>
        satisfies Content<IntervalSet<PhysicalMemory>>::project
        {
            IntervalSet {
                start: embed(region.base),
                end: embed(region.base) + embed(region.length)
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
        boundary machine embed<T>(value: T) -> Nat;
        data Region [linear] { length: u64; }
        domain Region::Owned;

        machine Owned::content(region: &Region) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        {
            CountedQuantity { magnitude: embed(region.length) }
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
                Owned::content(entry(&whole))
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
    assert_ne!(plan.fingerprint, 0);
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
fn retained_content_custody_rejects_borrow_only_source() {
    let diagnostics = rejected(
        r#"
        data ByteUnit {}
        data CountedQuantity<Unit> { magnitude: u64; }
        trait Content<A> {
            machine project(subject: &Self) -> A;
        }

        data Buffer [linear] {}
        domain Buffer::Owned;
        machine Owned::content(buffer: &Buffer) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        {
            CountedQuantity { magnitude: 1 }
        }

        data PendingWrite [linear] {}
        domain PendingWrite::Retained {
            Writer::submit;
        }
        machine Retained::content(pending: &PendingWrite) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        {
            CountedQuantity { magnitude: 1 }
        }

        boundary trait Writer {
            machine submit(buffer: &Buffer in Buffer::Owned) -> PendingWrite
            ensures
                result in PendingWrite::Retained;
        }

        data Main {}
        machine Main::main(&mut self) {}
        "#,
    );

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("returns content-bearing custody `PendingWrite::Retained`")
                && diagnostic.message.contains("borrowed parameter `buffer`")
                && diagnostic.message.contains("consumed owned input")
        }),
        "borrow-only retained-custody diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn retained_content_custody_accepts_consumed_owned_source() {
    checked(
        r#"
        data ByteUnit {}
        data CountedQuantity<Unit> { magnitude: u64; }
        trait Content<A> {
            machine project(subject: &Self) -> A;
        }

        data Buffer [linear] {}
        domain Buffer::Owned;
        machine Owned::content(buffer: &Buffer) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        {
            CountedQuantity { magnitude: 1 }
        }

        data PendingWrite [linear] {}
        domain PendingWrite::Retained {
            Writer::submit;
        }
        machine Retained::content(pending: &PendingWrite) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        {
            CountedQuantity { magnitude: 1 }
        }

        boundary trait Writer {
            machine submit(buffer: Buffer in Buffer::Owned) -> PendingWrite
            ensures
                result in PendingWrite::Retained;
        }

        data Main {}
        machine Main::main(&mut self) {}
        "#,
    );
}
