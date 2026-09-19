use super::{checked, rejected, retained_borrow_program};
use language_semantics::content::{ContentPlaceRoot, ContentPlaceVersion};

fn structural_retention_program(declarations: &str, requirement: &str) -> String {
    format!(
        r#"
        data ByteUnit {{}}
        data CountedQuantity<Unit> {{ magnitude: u64; }}
        trait Content<A> {{ machine project(subject: &Self) -> A; }}

        data Buffer [linear] {{}}
        domain Buffer::Owned;
        machine Owned::content(buffer: &Buffer) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        {{ CountedQuantity {{ magnitude: 1 }} }}

        data PendingWrite [linear] {{}}
        domain PendingWrite::Retained;
        machine Retained::content(pending: &PendingWrite) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        {{ CountedQuantity {{ magnitude: 1 }} }}

        data Receipt {{ pending: PendingWrite in PendingWrite::Retained; }}
        {declarations}
        boundary trait Writer {{ {requirement} }}
        "#,
    )
}

#[test]
fn retained_content_custody_rejects_borrow_only_source_of_record_result() {
    let source = structural_retention_program(
        "",
        "machine submit(buffer: &Buffer in Buffer::Owned) -> Receipt;",
    );
    let diagnostics = rejected(&source);
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("borrowed parameter")
                && diagnostic.message.contains("buffer")
                && diagnostic.message.contains("consumed owned input")
        }),
        "wrapping a result must not create owned custody: {diagnostics:#?}"
    );
}

#[test]
fn retained_content_custody_rejects_borrowed_field_of_owned_input_record() {
    let source = structural_retention_program(
        "data Inputs { buffer: &Buffer in Buffer::Owned; }",
        "machine submit(inputs: Inputs) -> Receipt;",
    );
    let diagnostics = rejected(&source);
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("borrowed parameter")
                && diagnostic.message.contains("inputs")
                && diagnostic.message.contains("consumed owned input")
        }),
        "an owned wrapper must not own its borrowed field: {diagnostics:#?}"
    );
}

#[test]
fn retained_content_custody_accepts_owned_field_beside_borrowed_field() {
    let source = structural_retention_program(
        r#"
        data Inputs {
            borrowed: &Buffer in Buffer::Owned;
            owned: Buffer in Buffer::Owned;
        }
        "#,
        r#"
        machine submit(inputs: Inputs) -> Receipt
        ensures
            Owned::content(old(&inputs.owned)) == Retained::content(&result.pending);
        "#,
    );
    checked(&source);
}

#[test]
fn retained_content_custody_rejects_borrows_through_repeated_generic_wrappers() {
    for requirement in [
        "machine submit(inputs: Wrapper<Wrapper<Borrowed>>) -> Receipt;",
        "machine submit(inputs: &Wrapper<Wrapper<OwnedSlot>>) -> Receipt;",
        "machine submit(buffer: &Buffer in Buffer::Owned) -> Wrapper<Wrapper<Receipt>>;",
    ] {
        let source = structural_retention_program(
            r#"
            data Wrapper<T> { value: T; }
            data Borrowed { buffer: &Buffer in Buffer::Owned; }
            data OwnedSlot { buffer: Buffer in Buffer::Owned; }
            "#,
            requirement,
        );
        let diagnostics = rejected(&source);
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("borrowed parameter")
                    && diagnostic.message.contains("consumed owned input")
            }),
            "finite generic nesting must not hide a loan ({requirement}): {diagnostics:#?}"
        );
    }
}

#[test]
fn retained_content_custody_rejects_expanding_recursive_source_analysis() {
    let source = structural_retention_program(
        "data Holder<T> { value: T; } data Node<T> { next: &Node<Holder<T>>; }",
        "machine submit(buffer: &Buffer in Buffer::Owned, node: Node<u64>) -> Receipt;",
    );
    let diagnostics = rejected(&source);
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot check structural content custody")
            && diagnostic.message.contains("recursive expansion")),
        "expanding types must reject without unbounded traversal: {diagnostics:#?}"
    );
}

#[test]
fn retained_content_custody_empty_array_is_not_an_owned_source() {
    for length in [0, 1] {
        let source = structural_retention_program(
            &format!(
                r#"
                data OwnedSlot {{ buffer: Buffer in Buffer::Owned; }}
                data Inputs {{
                    borrowed: &Buffer in Buffer::Owned;
                    owned: [OwnedSlot; {length}];
                }}
                "#,
            ),
            "machine submit(inputs: Inputs) -> Receipt;",
        );
        if length == 0 {
            let diagnostics = rejected(&source);
            assert!(
                diagnostics.iter().any(|diagnostic| {
                    diagnostic.message.contains("borrowed parameter")
                        && diagnostic.message.contains("consumed owned input")
                }),
                "an empty array supplies no owned element: {diagnostics:#?}"
            );
        } else {
            checked(&source);
        }
    }
}

#[test]
fn retained_content_custody_preserves_root_loan_with_nested_compatible_content() {
    let source = retained_borrow_program(
        r#"
        machine submit<'storage>(
            buffer: &'storage Buffer in Buffer::Owned
        ) -> PendingRead<'storage>
        ensures
            result in PendingRead::Retained;
        "#,
    )
    .replace(
        "data Buffer [linear] {}",
        r#"
        data Nested [linear] {}
        domain Nested::Held;
        machine Held::content(nested: &Nested) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        { CountedQuantity { magnitude: 1 } }
        data Buffer [linear] { nested: Nested in Nested::Held; }
        "#,
    );
    let checked = checked(&source);
    let [custody] = checked
        .facts
        .qualifications
        .content
        .retained_borrow_custodies
        .as_slice()
    else {
        panic!("the exact root lifetime-bound receipt must survive nested content");
    };
    assert!(custody.source.segments.is_empty());
    assert!(custody.result.segments.is_empty());
    assert_eq!(custody.lifetime.as_str(), "storage");
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
    assert_eq!(fact.access, language_semantics::ReferenceAccess::Shared);
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
        language_semantics::content::ContentStructuralPlace {
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
        language_semantics::content::ContentStructuralPlace {
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
