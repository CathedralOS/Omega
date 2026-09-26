use super::{rejected, retained_borrow_program};
use crate::tests::front_end::checked_program;

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
    checked_program(
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
fn retained_content_custody_authorizes_invoked_call_result_spelling() {
    // The invoked requirement spells its result `PendingRead<'storage>` while
    // the domain target is the bare nominal `PendingRead`: the lifetime slot
    // is erased from the semantic identity, so authorization compares head
    // nominals. Without that match the propagated `result in
    // PendingRead::Retained` fact on the call's result could not be
    // established.
    let source = r#"
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
            machine submit<'storage>(
                buffer: &'storage Buffer in Buffer::Owned
            ) -> PendingRead<'storage>
            ensures
                result in PendingRead::Retained;

            machine reclaim<'a>(pending: PendingRead<'a>);
        }

        data Worker {}
        machine Worker::run<'a>(
            &mut self,
            buffer: &'a Buffer in Buffer::Owned
        ) reaches Reader {
            let pending: PendingRead<'a> = Reader::submit(buffer);
            Reader::reclaim(pending);
        }

        data Main {}
        machine Main::main(&mut self) {}
    "#;
    let checked = checked_program(source);
    let [fact] = checked
        .facts
        .qualifications
        .content
        .retained_borrow_custodies
        .as_slice()
    else {
        panic!("the invoked requirement still records its custody fact");
    };
    assert_eq!(fact.lifetime.as_str(), "storage");
}
