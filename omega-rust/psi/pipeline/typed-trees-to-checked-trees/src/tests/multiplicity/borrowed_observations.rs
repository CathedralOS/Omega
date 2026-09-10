use super::*;

fn check_source(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    lower_typed_trees(typed)
}

#[test]
fn intrinsic_enum_equality_observes_two_borrowed_fields() {
    for operator in ["==", "!="] {
        let source = format!(
            "data Kind {{ case A; case B; }} data Pair {{ left: Kind; right: Kind; }} machine Pair::same(&self) -> bool {{ self.left {operator} self.right }}"
        );
        check_source(&source).expect("intrinsic equality observes both enum tags");
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
    check_source(&source.replace("            consume(token);", ""))
        .expect("single owned operator argument followed by borrowing its result is admitted");
    let diagnostics = match check_source(source) {
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
fn intrinsic_equality_preserves_owned_nested_call_arguments() {
    let source = r#"
        data Kind { case A; case B; }
        data Token {}
        data Pair { left: Kind; token: Token; }
        machine choose(token: Token) -> Kind { Kind::A }
        machine Pair::same(&self) -> bool { self.left == choose(self.token) }
    "#;
    let diagnostics = match check_source(source) {
        Ok(_) => panic!("nested call transfers borrowed token"),
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
fn borrowed_index_operator_result_preserves_owned_collection_consumption() {
    let source = r#"
        data Buffer { value: i32; }
        operator [] Buffer::index(items: Buffer, index: u64) -> i32;
        machine consume(items: Buffer) {}
        machine read(items: Buffer, index: u64) {
            let view: &i32 = &items[index];
            consume(items);
        }
    "#;
    check_source(&source.replace("            consume(items);", ""))
        .expect("borrowing one owned index operator result is admitted");
    let direct = source.replace(
        "let view: &i32 = &items[index];",
        "let value: i32 = items[index];",
    );
    check_source(&direct.replace("            consume(items);", ""))
        .expect("one direct index result consumes its collection once");
    assert!(
        check_source(&direct).is_err(),
        "direct index result must retain collection consumption"
    );
    let diagnostics = match check_source(source) {
        Ok(_) => panic!("index operator consumes its owned collection"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("items")
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
    let diagnostics = match check_source(source) {
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
fn authored_case_equality_cannot_consume_borrowed_operand() {
    let source = r#"
        data Kind { case A; case B; }
        operator == Kind::equal(left: Kind, right: Kind) -> bool;
        data Pair { left: Kind; }
        machine Pair::same(&self) -> bool { self.left == Kind::A }
    "#;
    let diagnostics = match check_source(source) {
        Ok(_) => panic!("authored equality consumes its owned operand"),
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
fn indexed_operand_access_preserves_shared_collection_and_owned_index() {
    let source = r#"
        data Buffer { value: i32; }
        data Index {}
        operator [] Buffer::index(items: &Buffer, index: Index) -> i32;
        machine consume(index: Index) {}
        data Main { buffer: Buffer; }
        machine Main::read(&self, index: Index) {
            let view: &i32 = &self.buffer[index];
            consume(index);
        }
    "#;
    check_source(&source.replace("            consume(index);", ""))
        .expect("shared collection remains observed while index moves once");
    let diagnostics = match check_source(source) {
        Ok(_) => panic!("authored index consumes its owned index operand"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("index")
                && (diagnostic.message.contains("move") || diagnostic.message.contains("consum"))),
        "{diagnostics:#?}"
    );
}
