use omega_lexer::lex;
use omega_parser::{FragmentKind, parse_fragment};
use omega_resolver::{ParsedSource, StageSource, resolve_stage};
use std::path::Path;

#[test]
fn spec_fragments_have_reviewed_typing_outcomes() {
    let spec = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../wiki/spec");
    let examples = spec_corpus::extract_omega_examples(&spec).unwrap();
    let mut outcomes = String::new();
    assert_eq!(examples.len(), 132);
    for example in examples {
        let label = example.get_label();
        let kind = match label.as_str() {
            "language/generics.md:306" | "language/termination.md:35" | "resources/content_custody.md:129" => FragmentKind::Clauses,
            "language/state_contracts.md:86" => FragmentKind::GenericParam,
            _ => FragmentKind::Statements,
        };
        let text = example.source_text.replace("{ ... }", "{     }");
        let tokens = lex(text.as_bytes()).unwrap();
        let tree = parse_fragment(text.as_bytes(), &tokens, kind).unwrap();
        let input = StageSource {
            package: "example",
            source: ParsedSource {
                logical_path: "example.omg",
                text: &text,
                tokens: &tokens,
                tree: &tree,
            },
        };
        let files = [input];
        let outcome = match resolve_stage(&files, &[]) {
            Ok(stage) => match omega_typer::type_stage(&files, &stage) {
                Ok(_) => "accepted".to_owned(),
                Err(e) => {
                    assert_eq!(e.file, "example.omg");
                    let text = std::str::from_utf8(e.span.slice(text.as_bytes()).unwrap()).unwrap();
                    format!("L3 {} | {}", e.message, text.lines().next().unwrap_or(""))
                }
            },
            Err(error) => {
                let row = format!("{label}  {}", error.message);
                assert!(
                    include_str!("../../02-resolver/fixtures/spec-outcomes.txt")
                        .lines()
                        .any(|l| l == row),
                    "{row}"
                );
                continue;
            }
        };
        outcomes.push_str(&format!("{label}  {outcome}\n"));
    }
    assert_eq!(
        outcomes.lines().collect::<Vec<_>>(),
        include_str!("../fixtures/spec-outcomes.txt")
            .lines()
            .collect::<Vec<_>>()
    );
}

mod support;

#[test]
fn the_published_vector_operator_selects_the_same_machine_as_a_named_call() {
    let spec = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../wiki/spec");
    let example = spec_corpus::extract_omega_examples(&spec)
        .unwrap()
        .into_iter()
        .find(|example| example.get_label() == "language/expressions.md:151")
        .unwrap();

    // Supply the types and helper omitted by the example. A second overload
    // makes choosing the first declaration by accident observable.
    let definitions = "public data Vec2{} public data Other{} machine add_vectors(a:Vec2,b:Vec2)->Vec2{a}";
    let other = "public machine + add(a:Other,b:Other)->Other{a}";
    for carrier in ["Vec2", "Other"] {
        for spelling in ["a+b", "add(a,b)"] {
            let caller = format!("machine f(a:{carrier},b:{carrier})->{carrier}{{{spelling}}}");
            let files = [
                ("vectors.omg", example.source_text.as_str()),
                ("defs.omg", definitions),
                ("other.omg", other),
                ("caller.omg", &caller),
            ];
            let stage = support::check(&files).unwrap();
            let selected = stage
                .calls
                .iter()
                .find(|call| call.site.file == "caller.omg")
                .unwrap();
            assert_eq!(selected.machine.path, "add");
            assert_eq!(selected.machine.operands, [carrier, carrier]);
            let mut reversed = files;
            reversed.reverse();
            assert_eq!(stage, support::check(&reversed).unwrap());

            let opposite = if carrier == "Vec2" { "Other" } else { "Vec2" };
            let wrong = caller.replace(&format!("b:{carrier}"), &format!("b:{opposite}"));
            let mut files = files;
            files[3].1 = &wrong;
            let error = support::check(&files).unwrap_err();
            let reason = if spelling == "a+b" {
                "no matching operator +"
            } else {
                "no matching overload for add"
            };
            assert!(error.message.starts_with(reason), "{error:?}");
            assert_eq!(error.file, "caller.omg");
        }
    }
}
