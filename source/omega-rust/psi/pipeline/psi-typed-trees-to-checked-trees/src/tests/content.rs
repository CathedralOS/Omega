use super::*;
use psi_language_semantics::content::{
    ContentAlgebraIdentity, ContentArithmeticOperator, ContentConservationOwnerKind,
    ContentConservationTerm, ContentPlaceRoot, ContentPlaceSegment, ContentPlaceVersion,
    ContentProjectionExpression, ContentScalarExpression,
};

fn checked(source: &str) -> psi_checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

fn rejected(source: &str) -> Vec<psi_diagnostics::Diagnostic> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect_err("checked lowering should reject")
}

fn retained_borrow_program(signature: &str) -> String {
    [
        r#"
        data ByteUnit {}
        data CountedQuantity<Unit> { magnitude: u64; }
        trait Content<A> { machine project(subject: &Self) -> A; }

        data Buffer [linear] {}
        domain Buffer::Owned;
        machine Owned::content(buffer: &Buffer) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        { CountedQuantity { magnitude: 1 } }

        data PendingRead<'storage> [linear] {}
        domain PendingRead::Retained
        established by Reader::submit;
        machine Retained::content(pending: &PendingRead) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        { CountedQuantity { magnitude: 1 } }

        boundary trait Reader {
        "#,
        signature,
        r#"
        }

        data Main {}
        machine Main::main(&mut self) {}
        "#,
    ]
    .join("\n")
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
fn compiler_owned_embedding_proves_exact_unsigned_and_signed_carrier_ranges() {
    let source = r#"
        machine unsigned_range(value: u8)
        ensures embed(value) >= 0 && embed(value) <= 255
        {
        }

        machine signed_range(value: i8)
        ensures embed(value) >= -128 && embed(value) <= 127
        {
        }

        data Main {}
        machine Main::main(&mut self) {}
    "#;

    let checked = checked(source);
    let embed = checked
        .typed
        .symbols
        .builtin_function_symbol(psi_symbols::BuiltinFunction::ContentEmbed)
        .expect("compiler-owned embed symbol");
    assert!(
        checked
            .typed
            .expression_table
            .iter_expressions()
            .any(|(_, expression)| {
                matches!(expression, psi_typed_trees::expression::ExpressionNode::Call(call)
            if call.target_symbol == embed && call.target.as_str() == "embed")
            })
    );
    assert!(checked.typed.machines().iter().all(|machine| {
        machine.symbol != embed
            && checked
                .typed
                .machine_states(machine)
                .iter()
                .all(|state| state.symbol != embed)
    }));
    assert!(checked.facts.nominal_machine_uses.uses.is_empty());
    assert!(checked.facts.operators.boundary_applications.is_empty());
    assert!(
        checked
            .facts
            .operators
            .symbolic_boundary_applications
            .is_empty()
    );
    assert!(checked.facts.intrinsic_calls.is_empty());
}

#[test]
fn compiler_owned_embedding_rejects_runtime_and_non_integer_uses() {
    for source in [
        r#"
            data Main {}
            machine Main::main(&mut self) {
                let value: u64 = embed(1u64);
            }
        "#,
        r#"
            machine invalid(value: bool)
            ensures embed(value) == embed(value)
            {
            }
            data Main {}
            machine Main::main(&mut self) {}
        "#,
        r#"
            machine invalid(value: u64)
            ensures embed(value, value) == 0
            {
            }
            data Main {}
            machine Main::main(&mut self) {}
        "#,
        r#"
            machine invalid(value: u64)
            ensures embed<u64>(value) == embed(value)
            {
            }
            data Main {}
            machine Main::main(&mut self) {}
        "#,
    ] {
        let diagnostics = rejected(source);
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("compiler-owned `embed")
                    || diagnostic.message.contains("compiler-owned `embed(value)`")
                    || diagnostic
                        .message
                        .contains("supplies static machine arguments")
            }),
            "unexpected diagnostics: {diagnostics:#?}"
        );
    }
}

#[test]
fn authored_qualified_embed_lookalike_cannot_supply_the_proof_term() {
    let diagnostics = rejected(
        r#"
        data Fake {}
        boundary machine Fake::embed(value: u64) -> Int;

        machine invalid(value: u64)
        ensures Fake.embed(value) == Fake.embed(value)
        {
        }

        data Main {}
        machine Main::main(&mut self) {}
        "#,
    );

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("authored, package-qualified, or same-spelled call")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );
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
    let source = r#"
        data ByteUnit {}
        data CountedQuantity<Unit> { magnitude: u64; }
        trait Content<A> { machine project(subject: &Self) -> A; }
        data Region [linear] { length: u64; }
        domain Region::Owned;
        machine Owned::content(region: &Region) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        { CountedQuantity { magnitude: region.length } }

        data Store { region: Region in Owned; }
        machine Store::retain(&mut self)
        ensures
            Owned::content(old(&self.region)) == Owned::content(&self.region)
        {}
    "#;

    let checked = checked(source);
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
        domain PendingWrite::Retained
        established by Writer::submit;
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
fn retained_content_custody_records_exact_shared_lifetime_bound_source() {
    let source = retained_borrow_program(
        r#"
            machine submit<'storage>(
                buffer: &'storage Buffer in Buffer::Owned
            ) -> PendingRead<'storage>
            ensures
                result in PendingRead::Retained;
        "#,
    );
    let mut checked = checked(&source);
    let [fact] = checked
        .facts
        .qualifications
        .content
        .retained_borrow_custodies
        .as_slice()
    else {
        panic!("one exact retained shared-borrow custody fact should be recorded");
    };
    assert!(fact.callable.is_valid());
    assert_eq!(fact.access, psi_language_semantics::ReferenceAccess::Shared);
    assert_eq!(fact.lifetime.as_str(), "storage");
    assert_eq!(fact.callable_lifetime_parameter_ordinal, 0);
    assert!(fact.result_data.is_valid());
    assert_eq!(fact.result_lifetime_argument_ordinal, 0);
    assert!(fact.retained_semantic_domain.is_valid());
    assert_eq!(
        fact.source_projection.algebra,
        fact.result_projection.algebra
    );
    assert_ne!(
        fact.source_projection.semantic_domain,
        fact.result_projection.semantic_domain
    );
    assert_eq!(
        fact.result_projection.semantic_domain,
        fact.retained_semantic_domain
    );
    assert!(matches!(
        &fact.source,
        psi_language_semantics::content::ContentStructuralPlace {
            version: ContentPlaceVersion::Entry,
            root: ContentPlaceRoot::Parameter {
                position: 0,
                name,
                is_self: false,
                ..
            },
            segments,
        } if name == "buffer" && segments.is_empty()
    ));
    assert!(matches!(
        &fact.result,
        psi_language_semantics::content::ContentStructuralPlace {
            version: ContentPlaceVersion::Current,
            root: ContentPlaceRoot::Result,
            segments,
        } if segments.is_empty()
    ));

    let expected = fact.clone();
    checked
        .facts
        .qualifications
        .content
        .retained_borrow_custodies
        .clear();
    crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
        .expect("checked validation should independently reconstruct retained borrow custody");
    assert_eq!(
        checked
            .facts
            .qualifications
            .content
            .retained_borrow_custodies,
        vec![expected]
    );
}

#[test]
fn retained_content_custody_rejects_different_source_and_result_lifetimes() {
    let source = retained_borrow_program(
        r#"
            machine submit<'source, 'result>(
                buffer: &'source Buffer in Buffer::Owned
            ) -> PendingRead<'result>
            ensures
                result in PendingRead::Retained;
        "#,
    );
    let diagnostics = rejected(&source);

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("borrowed parameter `buffer`")
                && diagnostic.message.contains("lifetime `'source'`")
                && diagnostic
                    .message
                    .contains("does not match result lifetime `'result'`")
        }),
        "mismatched retained-borrow lifetime diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn retained_content_custody_rejects_ambiguous_shared_sources() {
    let source = retained_borrow_program(
        r#"
            machine submit<'storage>(
                left: &'storage Buffer in Buffer::Owned,
                right: &'storage Buffer in Buffer::Owned
            ) -> PendingRead<'storage>
            ensures
                result in PendingRead::Retained;
        "#,
    );
    let diagnostics = rejected(&source);

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("ambiguous compatible borrowed inputs `left`, `right`")
                && diagnostic.message.contains("whole direct shared source")
        }),
        "ambiguous retained-borrow source diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn retained_content_custody_rejects_mutable_lifetime_bound_source() {
    let source = retained_borrow_program(
        r#"
            machine submit<'storage>(
                buffer: &'storage mut Buffer in Buffer::Owned
            ) -> PendingRead<'storage>
            ensures
                result in PendingRead::Retained;
        "#,
    );
    let diagnostics = rejected(&source);

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("borrowed parameter `buffer`")
                && diagnostic.message.contains("Mutable access")
                && diagnostic.message.contains("shared access only")
        }),
        "mutable retained-borrow source diagnostic: {diagnostics:#?}"
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
        domain PendingWrite::Retained
        established by Writer::submit;
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

#[test]
fn retained_content_custody_rejects_ambiguous_owned_sources() {
    let diagnostics = rejected(
        r#"
        data ByteUnit {}
        data CountedQuantity<Unit> { magnitude: u64; }
        trait Content<A> { machine project(subject: &Self) -> A; }

        data Buffer [linear] {}
        domain Buffer::Owned;
        machine Owned::content(buffer: &Buffer) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        { CountedQuantity { magnitude: 1 } }

        data PendingWrite [linear] {}
        domain PendingWrite::Retained
        established by Writer::submit;
        machine Retained::content(pending: &PendingWrite) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        { CountedQuantity { magnitude: 1 } }

        boundary trait Writer {
            machine submit(
                left: Buffer in Buffer::Owned,
                right: Buffer in Buffer::Owned
            ) -> PendingWrite
            ensures
                result in PendingWrite::Retained;
        }
        "#,
    );

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("returns content-bearing custody `PendingWrite::Retained`")
                && diagnostic
                    .message
                    .contains("ambiguous compatible consumed inputs")
                && diagnostic.message.contains("`left`, `right`")
                && diagnostic
                    .message
                    .contains("exact postcondition correspondence")
        }),
        "ambiguous retained-custody diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn retained_content_custody_accepts_exact_authored_source_correspondence() {
    checked(
        r#"
        data ByteUnit {}
        data CountedQuantity<Unit> { magnitude: u64; }
        trait Content<A> { machine project(subject: &Self) -> A; }

        data Buffer [linear] {}
        domain Buffer::Owned;
        machine Owned::content(buffer: &Buffer) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        { CountedQuantity { magnitude: 1 } }

        data PendingWrite [linear] {}
        domain PendingWrite::Retained
        established by Writer::submit;
        machine Retained::content(pending: &PendingWrite) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        { CountedQuantity { magnitude: 1 } }

        boundary trait Writer {
            machine submit(
                selected: Buffer in Buffer::Owned,
                other: Buffer in Buffer::Owned
            ) -> PendingWrite
            ensures
                result in PendingWrite::Retained
                Owned::content(old(&selected)) == Retained::content(&result);
        }
        "#,
    );
}

#[test]
fn retained_content_custody_rejects_authored_borrow_correspondence() {
    let diagnostics = rejected(
        r#"
        data ByteUnit {}
        data CountedQuantity<Unit> { magnitude: u64; }
        trait Content<A> { machine project(subject: &Self) -> A; }

        data Buffer [linear] {}
        domain Buffer::Owned;
        machine Owned::content(buffer: &Buffer) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        { CountedQuantity { magnitude: 1 } }

        data PendingWrite [linear] {}
        domain PendingWrite::Retained
        established by Writer::submit;
        machine Retained::content(pending: &PendingWrite) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        { CountedQuantity { magnitude: 1 } }

        boundary trait Writer {
            machine submit(buffer: &Buffer in Buffer::Owned) -> PendingWrite
            ensures
                result in PendingWrite::Retained
                Owned::content(old(&buffer)) == Retained::content(&result);
        }
        "#,
    );

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("borrowed parameter `buffer`")
                && diagnostic.message.contains("consumed owned input")
        }),
        "an authored equality cannot convert a borrow into retained custody: {diagnostics:#?}"
    );
}

#[test]
fn checked_facts_infer_exact_content_reshuffles_through_transparent_paths() {
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

        data Wrapped { region: Region in Owned; }

        data Main {}
        machine Main::forward(region: Region in Owned) -> Region in Owned {
            region
        }
        machine Main::pack(region: Region in Owned) -> Wrapped {
            Wrapped { region: region }
        }
        machine Main::unpack(wrapped: Wrapped) -> Region in Owned {
            wrapped.region
        }
        machine Main::main(&mut self) {}
    "#;

    let checked = checked(source);
    let reshuffles = &checked.facts.qualifications.content.identity_reshuffles;
    assert_eq!(reshuffles.len(), 3, "identity reshuffles: {reshuffles:#?}");

    let row_for = |state_name: &str| {
        let state_symbol = checked
            .machines()
            .iter()
            .flat_map(|machine| checked.machine_states(machine))
            .find(|state| state.name.as_str() == state_name)
            .expect("named state")
            .symbol;
        reshuffles
            .iter()
            .find(|row| row.state_symbol == state_symbol)
            .expect("inferred reshuffle row")
    };

    let forward = row_for("forward");
    assert_ne!(
        forward.claim_identity,
        psi_language_semantics::PermissionClaimIdentity::Unknown
    );
    assert!(matches!(
        forward.plan.equation.left(),
        ContentConservationTerm::Projection { subject, .. }
            if subject.version == ContentPlaceVersion::Entry
                && subject.segments.is_empty()
    ));
    assert!(matches!(
        forward.plan.equation.right(),
        ContentConservationTerm::Projection { subject, .. }
            if subject.version == ContentPlaceVersion::Current
                && subject.segments.is_empty()
    ));

    let pack = row_for("pack");
    let pack_paths = [pack.plan.equation.left(), pack.plan.equation.right()]
        .into_iter()
        .map(|term| match term {
            ContentConservationTerm::Projection { subject, .. } => subject,
            ContentConservationTerm::Separate(_) => panic!("reshuffles never infer separation"),
        })
        .collect::<Vec<_>>();
    assert!(pack_paths.iter().any(|subject| {
        subject.version == ContentPlaceVersion::Entry && subject.segments.is_empty()
    }));
    assert!(pack_paths.iter().any(|subject| {
        subject.version == ContentPlaceVersion::Current
            && matches!(subject.segments.as_slice(), [ContentPlaceSegment::Field(field)] if field.name == "region")
    }));

    let unpack = row_for("unpack");
    let unpack_paths = [unpack.plan.equation.left(), unpack.plan.equation.right()]
        .into_iter()
        .map(|term| match term {
            ContentConservationTerm::Projection { subject, .. } => subject,
            ContentConservationTerm::Separate(_) => panic!("reshuffles never infer separation"),
        })
        .collect::<Vec<_>>();
    assert!(unpack_paths.iter().any(|subject| {
        subject.version == ContentPlaceVersion::Entry
            && matches!(subject.segments.as_slice(), [ContentPlaceSegment::Field(field)] if field.name == "region")
    }));
    assert!(unpack_paths.iter().any(|subject| {
        subject.version == ContentPlaceVersion::Current && subject.segments.is_empty()
    }));
}

#[test]
fn checked_facts_compose_authored_partitions_through_a_direct_wrapper() {
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
        data Pair {
            left: Region in Owned;
            right: Region in Owned;
        }

        boundary trait Splitter {
            machine partition(
                left: Region in Owned,
                right: Region in Owned
            ) -> Pair
            ensures
                separate(
                    Owned::content(old(&left)),
                    Owned::content(old(&right)),
                )
                == separate(
                    Owned::content(&result.left),
                    Owned::content(&result.right),
                );
        }
        data Main { splitter: Splitter; }
        machine Main::forward(&mut self, pair: Pair) -> Pair
        requires
            pair.left in Region::Owned;
            pair.right in Region::Owned
        {
            self.splitter.partition(pair.left, pair.right)
        }
        machine Main::main(&mut self) {}
    "#;

    let checked = checked(source);
    let state_symbol = |name: &str| {
        checked
            .machines()
            .iter()
            .flat_map(|machine| checked.machine_states(machine))
            .find(|state| state.name.as_str() == name)
            .expect("named state")
            .symbol
    };
    let partition = checked
        .traits()
        .iter()
        .flat_map(|trait_definition| checked.trait_machine_signatures(trait_definition))
        .find(|signature| signature.name.as_str() == "partition")
        .expect("partition requirement")
        .symbol;
    let forward = state_symbol("forward");

    assert_eq!(
        checked
            .facts
            .qualifications
            .content
            .conservation_plans
            .len(),
        1,
        "only the primitive authors a source theorem"
    );
    let compositions = &checked.facts.qualifications.content.partition_compositions;
    assert_eq!(
        compositions.len(),
        1,
        "the authored theorem should instantiate through the direct wrapper: {compositions:#?}\nauthored: {:#?}\npermissions: {:#?}",
        checked.facts.qualifications.content.conservation_plans,
        checked.facts.flow.ownership.permissions,
    );
    let forward_row = compositions
        .iter()
        .find(|row| row.state_symbol == forward)
        .expect("direct wrapper composition");
    assert_eq!(forward_row.source_callable, partition);
    assert_eq!(
        forward_row.source_report_fingerprint,
        forward_row.source_plan.report_fingerprint
    );
    assert_eq!(forward_row.call_ordinal, 0);
    assert_eq!(forward_row.input_claim_identities.len(), 2);
    assert_eq!(forward_row.input_claim_bindings.len(), 2);
    assert!(forward_row.input_claim_bindings.iter().all(|binding| {
        binding.entry_place.version == ContentPlaceVersion::Entry
            && matches!(binding.entry_place.root, ContentPlaceRoot::Parameter { .. })
            && forward_row
                .input_claim_identities
                .contains(&binding.claim_identity)
    }));
    assert!(forward_row.result_rewrites.is_empty());
    assert_eq!(
        forward_row.substitutions.len(),
        4,
        "both source inputs and both result fields have exact substitutions"
    );
    assert!(
        forward_row.input_claim_identities.iter().all(|identity| {
            *identity != psi_language_semantics::PermissionClaimIdentity::Unknown
        })
    );

    for row in compositions {
        assert!(matches!(
            row.plan.equation.left(),
            ContentConservationTerm::Separate(children) if children.len() == 2
        ));
        assert!(matches!(
            row.plan.equation.right(),
            ContentConservationTerm::Separate(children) if children.len() == 2
        ));
    }
}

#[test]
fn checked_facts_compose_partitions_through_exact_staged_result_rewrites() {
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

        data Pair {
            left: Region in Owned;
            right: Region in Owned;
        }
        data Envelope { pair: Pair; }
        data Double { first: Pair; second: Pair; }

        boundary trait Splitter {
            machine partition(
                left: Region in Owned,
                right: Region in Owned
            ) -> Pair
            ensures
                separate(
                    Owned::content(old(&left)),
                    Owned::content(old(&right)),
                )
                == separate(
                    Owned::content(&result.left),
                    Owned::content(&result.right),
                );
        }
        boundary trait PairSplitter {
            machine partition(pair: Pair) -> Pair
            ensures
                separate(
                    Owned::content(old(&pair.left)),
                    Owned::content(old(&pair.right)),
                )
                == separate(
                    Owned::content(&result.left),
                    Owned::content(&result.right),
                );
        }
        data Main {
            splitter: Splitter;
            pair_splitter: PairSplitter;
        }
        machine Main::repack(&mut self, pair: Pair) -> Pair
        requires
            pair.left in Region::Owned;
            pair.right in Region::Owned
        {
            let result: Pair = self.splitter.partition(pair.left, pair.right);
            result
        }
        machine Main::envelope(&mut self, pair: Pair) -> Envelope
        requires
            pair.left in Region::Owned;
            pair.right in Region::Owned
        {
            let result: Pair = self.splitter.partition(pair.left, pair.right);
            Envelope { pair: result }
        }
        machine Main::two_hop(&mut self, pair: Pair) -> Pair
        requires
            pair.left in Region::Owned;
            pair.right in Region::Owned
        {
            let result: Pair = self.splitter.partition(pair.left, pair.right);
            let forwarded: Pair = result;
            forwarded
        }
        machine Main::aggregate_argument(
            &mut self,
            left: Region in Owned,
            right: Region in Owned
        ) -> Pair
        {
            self.pair_splitter.partition(Pair { left: left, right: right })
        }
        machine Main::two_calls(&mut self, first: Pair, second: Pair) -> Double
        requires
            first.left in Region::Owned;
            first.right in Region::Owned;
            second.left in Region::Owned;
            second.right in Region::Owned
        {
            let left: Pair = self.splitter.partition(first.left, first.right);
            let right: Pair = self.splitter.partition(second.left, second.right);
            Double { first: left, second: right }
        }
        machine Main::forward_double(&mut self, first: Pair, second: Pair) -> Double
        requires
            first.left in Region::Owned;
            first.right in Region::Owned;
            second.left in Region::Owned;
            second.right in Region::Owned
        {
            self.two_calls(first, second)
        }
        machine Main::main(&mut self) {}
    "#;

    let checked_program = checked(source);
    let compositions = &checked_program
        .facts
        .qualifications
        .content
        .partition_compositions;
    assert_eq!(compositions.len(), 8, "compositions: {compositions:#?}");
    let state_symbol = |name: &str| {
        checked_program
            .machines()
            .iter()
            .flat_map(|machine| checked_program.machine_states(machine))
            .find(|state| state.name.as_str() == name)
            .expect("named state")
            .symbol
    };
    let composition = compositions
        .iter()
        .find(|composition| composition.state_symbol == state_symbol("repack"))
        .expect("staged-local composition");
    assert_eq!(composition.statement_index, 0);
    assert_eq!(composition.call_ordinal, 0);
    assert_eq!(composition.input_claim_identities.len(), 2);
    assert_eq!(composition.result_rewrites.len(), 2);
    assert_eq!(composition.substitutions.len(), 4);
    assert!(
        composition
            .result_rewrites
            .iter()
            .all(|rewrite| rewrite.claim_identity
                != psi_language_semantics::PermissionClaimIdentity::Unknown)
    );
    assert!(matches!(
        composition.plan.equation.left(),
        ContentConservationTerm::Separate(children) if children.len() == 2
    ));
    assert!(matches!(
        composition.plan.equation.right(),
        ContentConservationTerm::Separate(children) if children.len() == 2
    ));

    let envelope = compositions
        .iter()
        .find(|composition| composition.state_symbol == state_symbol("envelope"))
        .expect("nested aggregate composition");
    assert_eq!(envelope.result_rewrites.len(), 2);
    let output_substitutions = envelope
        .substitutions
        .iter()
        .filter(|substitution| substitution.source.root == ContentPlaceRoot::Result)
        .collect::<Vec<_>>();
    assert_eq!(output_substitutions.len(), 2);
    assert!(output_substitutions.iter().all(|substitution| {
        matches!(
            substitution.target.segments.as_slice(),
            [ContentPlaceSegment::Field(outer), ContentPlaceSegment::Field(_)]
                if outer.name == "pair"
        )
    }));
    let two_hop = compositions
        .iter()
        .find(|composition| composition.state_symbol == state_symbol("two_hop"))
        .expect("multi-hop local composition");
    assert_eq!(two_hop.result_rewrites.len(), 2);
    assert!(two_hop.result_rewrites.iter().all(|rewrite| {
        rewrite.claim_identity != psi_language_semantics::PermissionClaimIdentity::Unknown
            && rewrite.source == rewrite.target
    }));
    let aggregate_argument = compositions
        .iter()
        .find(|composition| composition.state_symbol == state_symbol("aggregate_argument"))
        .expect("record aggregate argument composition");
    assert_eq!(aggregate_argument.input_claim_identities.len(), 2);
    assert!(aggregate_argument.result_rewrites.is_empty());
    let input_substitutions = aggregate_argument
        .substitutions
        .iter()
        .filter(|substitution| {
            matches!(substitution.source.root, ContentPlaceRoot::Parameter { .. })
        })
        .collect::<Vec<_>>();
    assert_eq!(input_substitutions.len(), 2);
    assert!(
        input_substitutions
            .iter()
            .all(|substitution| substitution.target.segments.is_empty())
    );
    let mut two_calls = compositions
        .iter()
        .filter(|composition| composition.state_symbol == state_symbol("two_calls"))
        .collect::<Vec<_>>();
    two_calls.sort_by_key(|composition| composition.statement_index);
    assert_eq!(two_calls.len(), 2);
    for (index, composition) in two_calls.into_iter().enumerate() {
        assert_eq!(composition.statement_index, index);
        assert_eq!(composition.call_ordinal, 0);
        assert_eq!(composition.input_claim_identities.len(), 2);
        assert_eq!(composition.result_rewrites.len(), 2);
        let outer = if index == 0 { "first" } else { "second" };
        assert!(composition.result_rewrites.iter().all(|rewrite| matches!(
            rewrite.target.segments.as_slice(),
            [ContentPlaceSegment::Field(field), ContentPlaceSegment::Field(_)]
                if field.name == outer
        )));
    }
    let forward_double = compositions
        .iter()
        .filter(|composition| composition.state_symbol == state_symbol("forward_double"))
        .collect::<Vec<_>>();
    assert_eq!(forward_double.len(), 2);
    assert!(
        forward_double.iter().all(|composition| {
            composition.source_callable == state_symbol("two_calls")
                && composition.source_derivation_depth == 1
                && composition.result_rewrites.len() == 2
        }),
        "forwarded rows: {forward_double:#?}"
    );
}

#[test]
fn checked_facts_compose_partitions_through_exact_array_and_case_arguments() {
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

        data Pair {
            left: Region in Owned;
            right: Region in Owned;
        }
        data SumPair {
            case Pair(left: Region in Owned, right: Region in Owned);
            case Mirror(left: Region in Owned, right: Region in Owned);
        }
        boundary trait ArraySplitter {
            machine partition(pair: [Region in Owned; 2]) -> Pair
            ensures
                separate(
                    Owned::content(old(&pair[0])),
                    Owned::content(old(&pair[1])),
                )
                == separate(
                    Owned::content(&result.left),
                    Owned::content(&result.right),
                );
        }
        boundary trait CaseSplitter {
            machine partition(pair: SumPair) -> Pair
            ensures
                separate(
                    Owned::content(old(&pair.left)),
                    Owned::content(old(&pair.right)),
                )
                == separate(
                    Owned::content(&result.left),
                    Owned::content(&result.right),
                );
        }
        data Main {
            array_splitter: ArraySplitter;
            case_splitter: CaseSplitter;
        }
        machine Main::array_argument(
            &mut self,
            left: Region in Owned,
            right: Region in Owned
        ) -> Pair
        {
            self.array_splitter.partition([left, right])
        }
        machine Main::case_argument(
            &mut self,
            left: Region in Owned,
            right: Region in Owned
        ) -> Pair
        {
            self.case_splitter.partition(SumPair::Pair {
                left: left,
                right: right,
            })
        }
        machine Main::wrong_case_argument(
            &mut self,
            left: Region in Owned,
            right: Region in Owned
        ) -> Pair
        {
            self.case_splitter.partition(SumPair::Mirror {
                left: left,
                right: right,
            })
        }
        machine Main::main(&mut self) {}
    "#;

    let checked_program = checked(source);
    let compositions = &checked_program
        .facts
        .qualifications
        .content
        .partition_compositions;
    assert_eq!(compositions.len(), 2, "compositions: {compositions:#?}");
    let state_symbol = |name: &str| {
        checked_program
            .machines()
            .iter()
            .flat_map(|machine| checked_program.machine_states(machine))
            .find(|state| state.name.as_str() == name)
            .expect("named state")
            .symbol
    };
    let array = compositions
        .iter()
        .find(|composition| composition.state_symbol == state_symbol("array_argument"))
        .expect("fixed-array argument composition");
    let case = compositions
        .iter()
        .find(|composition| composition.state_symbol == state_symbol("case_argument"))
        .expect("active-case argument composition");

    for composition in [array, case] {
        assert_eq!(composition.input_claim_identities.len(), 2);
        assert!(composition.result_rewrites.is_empty());
        let input_substitutions = composition
            .substitutions
            .iter()
            .filter(|substitution| {
                matches!(substitution.source.root, ContentPlaceRoot::Parameter { .. })
            })
            .collect::<Vec<_>>();
        assert_eq!(input_substitutions.len(), 2);
        assert!(
            input_substitutions
                .iter()
                .all(|substitution| substitution.target.segments.is_empty())
        );
    }
    assert!(array.substitutions.iter().any(|substitution| matches!(
        substitution.source.segments.as_slice(),
        [ContentPlaceSegment::FixedIndex(0)]
    )));
    assert!(case.substitutions.iter().any(|substitution| matches!(
        substitution.source.segments.as_slice(),
        [ContentPlaceSegment::Case(case), ContentPlaceSegment::Field(field)]
            if case.name == "Pair" && field.name == "left"
    )));
    assert!(
        compositions
            .iter()
            .all(|composition| composition.state_symbol != state_symbol("wrong_case_argument"))
    );
}

#[test]
fn checked_facts_infer_exact_content_reshuffles_through_sum_case_paths() {
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

        data Envelope {
            case Empty;
            case Present(region: Region in Owned);
        }

        data Main {}
        machine Main::forward(value: Envelope) -> Envelope {
            value
        }
        machine Main::main(&mut self) {}
    "#;

    let checked = checked(source);
    let [reshuffle] = checked
        .facts
        .qualifications
        .content
        .identity_reshuffles
        .as_slice()
    else {
        panic!("one active-payload reshuffle should be inferred");
    };
    for term in [
        reshuffle.plan.equation.left(),
        reshuffle.plan.equation.right(),
    ] {
        let ContentConservationTerm::Projection { subject, .. } = term else {
            panic!("an identity reshuffle must remain a direct projection equality");
        };
        assert!(matches!(
            subject.segments.as_slice(),
            [ContentPlaceSegment::Case(case), ContentPlaceSegment::Field(field)]
                if case.name == "Present"
                    && case.symbol.is_valid()
                    && field.name == "region"
                    && field.symbol.is_valid()
        ));
    }
}

#[test]
fn checked_facts_do_not_infer_content_for_fresh_claim_establishment() {
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

        data Main {}
        machine Main::issue() -> Region in Owned {
            Region { length: 1 }
        }
        machine Main::main(&mut self) {}
    "#;

    let checked = checked(source);
    assert!(
        checked
            .facts
            .qualifications
            .content
            .identity_reshuffles
            .is_empty(),
        "fresh establishment requires a sealed introduction row, not an inferred reshuffle"
    );
}

#[test]
fn checked_facts_keep_independent_same_algebra_reshuffles_separate() {
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

        data Pair {
            first: Region in Owned;
            second: Region in Owned;
        }

        data Main {}
        machine Main::swap(pair: Pair) -> Pair {
            Pair { first: pair.second, second: pair.first }
        }
        machine Main::main(&mut self) {}
    "#;

    let checked = checked(source);
    let reshuffles = &checked.facts.qualifications.content.identity_reshuffles;
    assert_eq!(reshuffles.len(), 2, "one row per preserved claim identity");
    assert!(reshuffles.iter().all(|row| {
        !matches!(
            row.plan.equation.left(),
            ContentConservationTerm::Separate(_)
        ) && !matches!(
            row.plan.equation.right(),
            ContentConservationTerm::Separate(_)
        )
    }));
    let ownership = &checked.facts.flow.ownership;
    assert!(reshuffles.iter().all(|row| {
        ownership.segments.span_or_empty(row.input_segments)
            != ownership.segments.span_or_empty(row.output_segments)
    }));
}

#[test]
fn checked_facts_do_not_equate_distinct_content_projection_identities() {
    let source = r#"
        data ByteUnit {}
        data CountedQuantity<Unit> { magnitude: u64; }
        trait Content<A> {
            machine project(subject: &Self) -> A;
        }

        data Region [linear] { length: u64; }
        domain Region::Left;
        domain Region::Right;
        machine Left::content(region: &Region) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        {
            CountedQuantity { magnitude: region.length }
        }
        machine Right::content(region: &Region) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        {
            CountedQuantity { magnitude: region.length }
        }

        data Main {}
        machine Main::retag(region: Region in Left) -> Region in Right {
            region
        }
        machine Main::main(&mut self) {}
    "#;

    let checked = checked(source);
    assert!(
        checked
            .facts
            .qualifications
            .content
            .identity_reshuffles
            .is_empty(),
        "matching carrier and algebra cannot replace exact projection identity"
    );
}

#[test]
fn checked_facts_infer_reshuffles_from_ordinary_qualification_contracts() {
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

        data Main {}
        machine Main::forward(region: Region) -> Region in Owned
        requires
            region in Region::Owned
        {
            region
        }
        machine Main::main(&mut self) {}
    "#;

    let checked = checked(source);
    assert_eq!(
        checked
            .facts
            .qualifications
            .content
            .identity_reshuffles
            .len(),
        1,
        "an ordinary requires qualification should select the exact input projection"
    );
}
