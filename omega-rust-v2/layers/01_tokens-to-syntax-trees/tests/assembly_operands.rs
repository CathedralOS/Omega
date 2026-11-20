//! Instruction operands are `expression % ','`, so, like every comma list in GRAMMAR.md, they may
//! end in a comma.

mod support;

use omega_lexer::lex;
use omega_parser::parse_file;

fn get_dump(source: &str) -> String {
    let tokens = lex(source.as_bytes()).unwrap();
    let tree = parse_file(source.as_bytes(), &tokens).unwrap_or_else(|error| panic!("{source}: {error}"));
    support::dump_tree(source.as_bytes(), &tokens, &tree)
}

#[test]
fn a_trailing_operand_comma_adds_only_its_token() {
    // `INSTRUCTION` is followed by each token that can end an instruction: `;`, `}` and a nested assembly machine.
    for template in [
        "asm machine f() { INSTRUCTION; ret }",
        "asm machine f() { ret; INSTRUCTION }",
        "asm machine f() { INSTRUCTION asm machine g() { ret } }",
        "machine f() { asm { INSTRUCTION; ret } }",
        "machine f() { asm { INSTRUCTION } }",
    ] {
        for (operands, last_operand) in [
            ("add x, y", "PathExpression 'y'"),
            ("push x", "PathExpression 'x'"),
        ] {
            let without_comma = get_dump(&template.replace("INSTRUCTION", operands));
            let source = template.replace("INSTRUCTION", &format!("{operands}, "));
            // The comma sits directly after the last operand, at that operand's depth inside the
            // Instruction. No other node in these sources has the operand's spelling.
            let last_operand_line = without_comma
                .lines()
                .find(|line| line.trim_start() == last_operand)
                .unwrap();
            let indent = &last_operand_line[..last_operand_line.len() - last_operand.len()];
            let expected = without_comma.replacen(&format!("{last_operand_line}\n"), &format!("{last_operand_line}\n{indent}','\n"), 1);
            assert_eq!(get_dump(&source), expected, "{source}");
        }
    }
}

#[test]
fn an_operand_list_still_needs_an_operand_between_commas() {
    // `% ','` allows one comma after the last operand, never an empty operand.
    for (source, offending_token) in [
        ("asm machine f() { add x,, y; ret }", ","),
        ("asm machine f() { add x,,; ret }", ","),
        ("asm machine f() { add ,; ret }", ","),
    ] {
        let tokens = lex(source.as_bytes()).unwrap();
        let error = parse_file(source.as_bytes(), &tokens).expect_err(source);
        let span = error.get_span();
        assert_eq!(&source[span.get_start_offset()..span.get_end_offset()], offending_token, "{source}");
    }
}

#[test]
fn a_clobber_separator_needs_a_register_name() {
    for (header, body) in [("asm machine f clobbers ", "{}"), ("machine f() { asm clobbers ", "{} }")] {
        let valid = format!("{header}rax, rbx {body}");
        parse_file(valid.as_bytes(), &lex(valid.as_bytes()).unwrap()).unwrap();
        let invalid = valid.replace("rbx", "7");
        let error = parse_file(invalid.as_bytes(), &lex(invalid.as_bytes()).unwrap()).unwrap_err();
        assert_eq!(error.get_kind(), omega_parser::ParseErrorKind::UnexpectedToken);
        assert_eq!(error.get_expected(), "a register name");
        assert_eq!(error.get_span().slice(invalid.as_bytes()), Some(b"7".as_slice()));

        let comma_before_body = valid.replace("rbx ", "");
        let error = parse_file(comma_before_body.as_bytes(), &lex(comma_before_body.as_bytes()).unwrap()).unwrap_err();
        assert_eq!(error.get_kind(), omega_parser::ParseErrorKind::SeparatorBeforeBody);
        assert_eq!(error.get_span().slice(comma_before_body.as_bytes()), Some(b",".as_slice()));
    }
}
