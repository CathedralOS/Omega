//! Static declarations never construct an implicit receiver binding.
use crate::parse_syntax_trees;
use source_files_to_tokens::Lexer;

#[test]
fn retired_receiver_parameters_reject_in_every_signature_owner() {
    for parameter in ["self", "mut self", "&self", "&mut self", "&write self"] {
        for source in [
            format!("data Counter {{ value: u32; }} machine Counter::step({parameter}) {{}}"),
            format!("trait Operation {{ machine step({parameter}); }}"),
            format!("boundary trait Operation {{ machine step({parameter}); }}"),
            format!("machine run() {{ state step({parameter}) {{}} }}"),
        ] {
            let tokens = Lexer::new(&source).tokenize().unwrap();
            let error = parse_syntax_trees(&tokens).expect_err("receiver must reject");
            assert!(
                error.message.contains("`self` receivers are retired"),
                "{source}: {}",
                error.message
            );
        }
    }
}

#[test]
fn implicit_self_expressions_and_transition_targets_reject() {
    for body in [
        "self",
        "self.value",
        "transition { _ -> self }",
        "transition { _ -> self() }",
    ] {
        let source = format!("machine run() {{ {body} }}");
        let tokens = Lexer::new(&source).tokenize().unwrap();
        let error = parse_syntax_trees(&tokens).expect_err("implicit subject must reject");
        assert!(
            error.message.contains("implicit `self` is retired"),
            "{source}: {}",
            error.message
        );
    }
}

#[test]
fn explicit_owned_shared_mutable_and_write_only_parameters_parse() {
    let source = r#"
        data Counter { value: u32; }
        trait Readable { machine read(counter: &Self) -> u32; }
        machine consume(counter: Counter) {}
        machine read(counter: &Counter) -> u32 { counter.value }
        machine write(counter: &write Counter) { counter.value = 1; }
        machine Counter::clear(counter: &mut Counter) {
            transition { _ -> finish(counter) }
            state finish(counter: &mut Counter) { counter.value = 0; }
        }
    "#;
    let tokens = Lexer::new(source).tokenize().unwrap();
    parse_syntax_trees(&tokens).expect("ordinary explicit parameters remain valid");
}
