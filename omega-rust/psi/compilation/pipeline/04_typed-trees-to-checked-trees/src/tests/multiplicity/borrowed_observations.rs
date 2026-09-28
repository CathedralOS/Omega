use crate::tests::front_end::checked_program_result;

/// A derived `equals(&self, other: &Self)` lends both operands exactly as
/// `==` observes them: neither receiver field nor argument field is moved out
/// of `&mut self`, so a following boundary call sees both present.
#[test]
fn derived_equals_call_observes_its_borrowed_receiver_and_argument() {
    for data in [
        "data Kind { case A; case B; }",
        "data Kind { case A; case B(value: i32); }",
    ] {
        let source = format!(
            "boundary trait Trace {{ machine observe(value: bool); }}
             trait Equatable {{ machine equals(&self, other: &Self) -> bool; }}
             {data}
             KindEquatable: Kind satisfies Equatable;
             data Pair {{ left: Kind; right: Kind; }}
             machine Pair::run(&mut self) reaches Trace {{
                 let same: bool = self.left.equals(&self.right);
                 Trace::observe(same);
             }}"
        );
        checked_program_result(&source)
            .unwrap_or_else(|errors| panic!("{data}: equals borrows its operands: {errors:?}"));
    }
}

#[test]
fn borrowed_operator_result_preserves_owned_argument_consumption() {
    let source = r#"
        data Token {}
        data Cell { value: u64; }
        data View { body: &mut Cell; }
        data Main { cells: [Cell; 2]; }
        boundary operator Index::choose(token: Token) -> Cell;
        machine consume(token: Token) {}
        machine Main::run(&mut self, token: Token) {
            let view: View = View { body: &mut Index::choose(token) };
            consume(token);
        }
    "#;
    checked_program_result(&source.replace("            consume(token);", ""))
        .expect("single owned operator argument followed by borrowing its result is admitted");
    let diagnostics = match checked_program_result(source) {
        Ok(_) => panic!("operator computation consumes token"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("token")
                && (diagnostic.message.contains("move") || diagnostic.message.contains("consum"))),
        "{diagnostics:#?}"
    );
}

#[test]
fn borrowed_index_operator_result_cannot_consume_borrowed_collection() {
    let source = r#"
        data Buffer { value: i32; }
        operator [] Buffer::index(items: Buffer, index: u64) -> i32;
        data Main { buffer: Buffer; }
        machine Main::read(&self, index: u64) {
            let view: &i32 = &self.buffer[index];
        }
    "#;
    let diagnostics = match checked_program_result(source) {
        Ok(_) => panic!("index operator cannot consume a borrowed collection"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot transfer a non-copy value out of borrowed storage")),
        "{diagnostics:#?}"
    );
}

#[test]
fn borrowed_indexed_collection_reborrows_into_the_attached_receiver() {
    for access in ["&", "&mut"] {
        for declaration in [
            "machine [] Buffer::index(&self, index: Index) -> i32 { self.value }",
            "machine [..] Buffer::range(&self, start: Index, end: Index) -> i32 { self.value }",
        ] {
            let (operands, tail) = if declaration.contains("[..]") {
                ("start: Index, end: Index", "buffer[start..end]")
            } else {
                ("index: Index", "buffer[index]")
            };
            let source = format!(
                "data Buffer {{ value: i32; }}
                 data Index {{}}
                 {declaration}
                 machine read(buffer: {access} Buffer, {operands}) -> i32 {{ {tail} }}"
            );
            checked_program_result(&source).unwrap_or_else(|diagnostics| {
                panic!("borrowed collection reborrows into the attached receiver: {diagnostics:#?}")
            });
        }
    }
}

#[test]
fn borrowed_indexed_collection_cannot_move_into_an_owned_receiver() {
    let source = r#"
        data Buffer { value: i32; }
        data Index {}
        machine [] Buffer::take(self, index: Index) -> i32 { self.value }
        machine read(buffer: &Buffer, index: Index) -> i32 { buffer[index] }
    "#;
    let diagnostics = match checked_program_result(source) {
        Ok(_) => panic!("borrowed storage cannot move into an owned `self` receiver"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics.iter().any(
            |diagnostic| diagnostic.message.contains("remained unresolved")
                || diagnostic.message.contains("move")
                || diagnostic.message.contains("consum")
                || diagnostic.message.contains("borrowed")
        ),
        "{diagnostics:#?}"
    );
}
