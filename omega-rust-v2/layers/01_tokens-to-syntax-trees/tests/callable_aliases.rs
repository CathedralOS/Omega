use omega_lexer::lex;
use omega_parser::{ParseErrorKind, parse_file};

#[test]
fn structural_callable_signatures_have_one_declaration_site() {
    let source = "data Predicate<T> = machine(item:T)->bool;
        data Inspector = machine<'a>(item:@'a u8);
        data Factory = machine()->Predicate<u8> reaches none crashes none;
        machine filter<T, const Keep:Predicate<T>>(item:T)->bool { Keep(item) }
        machine forward<const Keep:machine>() {}
        trait Reader { machine open()->Predicate<u8> terminates; }";
    let bytes = source.as_bytes();
    let parsed = parse_file(bytes, &lex(bytes).unwrap());
    spec_corpus::assert_handoff_eq!("structural-callable/alias-rhs", parsed.map(|_| ()), Ok(()));
}

#[test]
fn each_inline_signature_points_at_the_type_to_extract() {
    for (placement, marked) in [
        ("parameter", "machine filter(keep:|machine(item:u8)->bool) {}"),
        ("const-bound", "machine filter<const Keep:|machine(item:u8)->bool>() {}"),
        ("return", "machine open()->|machine(offset:u32)->u8 requires ready {}"),
        ("field", "data Stream { read:|machine(offset:u32)->u8; }"),
        ("case-field", "data Choice { case Read(value:|machine()); }"),
        ("local", "machine main() { let read:|machine()->u8 = read_byte; }"),
        ("array-element", "data List = [|machine(); 4];"),
        ("type-argument", "data Wrapper = Option<|machine()>;"),
        ("nested-return", "data Factory = machine()->|machine()->u8;"),
        ("nested-const-bound", "data Higher = machine<const Inner:|machine<T>(value:T)>(input:u8);"),
        ("nested-parameter", "data Sink = machine(read:|machine()->u8);"),
    ] {
        let offset = marked.find('|').unwrap();
        let source = marked.replace('|', "");
        let bytes = source.as_bytes();
        let error = parse_file(bytes, &lex(bytes).unwrap()).expect_err(marked);
        assert_eq!(error.get_span().get_start_offset(), offset, "{marked}");
        assert_eq!(error.get_span().slice(bytes), Some(&b"machine"[..]), "{marked}");
        assert_eq!(error.get_kind(), ParseErrorKind::UnexpectedToken, "{marked}");
        assert_eq!(error.get_expected(), "a named callable type (declare its signature with `data Name = machine(...);`)");
        if std::env::var_os("OMEGA_LAYER_REPORT").is_some() {
            let span = error.get_span();
            println!("OMEGA_HANDOFF_ASSERT\tinline-callable/{placement}\t{source}\t{:?}\t{}\t{}\t{}",
                     error.get_kind(), span.get_start_offset(), span.get_end_offset(), error.get_expected());
        }
    }
}
