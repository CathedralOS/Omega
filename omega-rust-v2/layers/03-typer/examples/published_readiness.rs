//! Check the intended verdicts of published examples that still fail before their primary judgment.
//! Each example runs its positive and negative inputs, even if the first one fails.
use omega_lexer::{SourceSpan, lex};
use omega_parser::parse_file;
use omega_resolver::{Dependency, ParsedSource, ResolvedStage, StageSource, Symbol, resolve_stage};
use omega_typer::{TypedStage, ValueType, resolve_source_package, type_source_package};

type Source<'a> = (&'a str, &'a str, &'a str);
struct Failure {
    stage: &'static str,
    file: String,
    span: Option<SourceSpan>,
    message: String,
}

fn compile(
    files: &[Source<'_>],
    edges: &[Dependency<'_>],
) -> Result<(ResolvedStage, TypedStage), Failure> {
    let mut parsed = Vec::new();
    for &(package, file, text) in files {
        let earlier = |stage, message| Failure {
            stage,
            file: file.to_string(),
            span: None,
            message,
        };
        let tokens = lex(text.as_bytes()).map_err(|e| earlier("L0", format!("{e:?}")))?;
        let tree = parse_file(text.as_bytes(), &tokens).map_err(|e| earlier("L1", format!("{e:?}")))?;
        parsed.push((package, file, text, tokens, tree));
    }
    let sources: Vec<_> = parsed
        .iter()
        .map(|(package, file, text, tokens, tree)| StageSource {
            package,
            source: ParsedSource {
                logical_path: file,
                text,
                tokens,
                tree,
            },
        })
        .collect();
    let resolved = if edges.is_empty() {
        resolve_source_package(&sources)
    } else {
        resolve_stage(&sources, edges)
    }
    .map_err(|e| Failure {
        stage: "L2",
        file: e.file,
        span: Some(e.span),
        message: e.message,
    })?;
    let typed = if edges.is_empty() {
        type_source_package(&sources, &resolved)
    } else {
        omega_typer::type_stage(&sources, &resolved)
    }
    .map_err(|e| Failure {
        stage: "L3",
        file: e.file,
        span: Some(e.span),
        message: e.message,
    })?;
    Ok((resolved, typed))
}

fn block(
    chapter: &str,
    line: usize,
) -> String {
    let mut examples = Vec::new();
    spec_corpus::extract_examples_from_markdown("chapter", chapter, &mut examples);
    examples
        .into_iter()
        .find(|e| e.fence_line_number == line)
        .unwrap_or_else(|| panic!("missing spec block at {line}"))
        .source_text
}

fn describe(
    failure: &Failure,
    files: &[Source<'_>],
) -> String {
    let source = files
        .iter()
        .find(|(_, name, _)| *name == failure.file)
        .map(|(_, _, text)| *text);
    let before = source
        .zip(failure.span)
        .and_then(|(s, span)| s.get(..span.get_start_offset()));
    let position = before
        .map(|s| {
            format!(
                ":{}:{}",
                1 + s.bytes().filter(|&b| b == b'\n').count(),
                1 + s.rsplit('\n').next().unwrap().chars().count()
            )
        })
        .unwrap_or_default();
    format!("{} {}{}: {}", failure.stage, failure.file, position, failure.message)
}

fn accept(
    edges: &[Dependency<'_>],
    files: &[Source<'_>],
    verify: impl FnOnce(&ResolvedStage, &TypedStage) -> bool,
) -> Result<(), String> {
    let (names, typed) = compile(files, edges).map_err(|e| describe(&e, files))?;
    if verify(&names, &typed) {
        Ok(())
    } else {
        Err("L3 accepted but the expected typed product is missing".into())
    }
}

fn reject(
    edges: &[Dependency<'_>],
    files: &[Source<'_>],
    file: &str,
    site: &str,
    reason: &[&str],
) -> Result<(), String> {
    let Err(error) = compile(files, edges) else {
        return Err("accepted the negative input".into());
    };
    let source = files.iter().find(|(_, name, _)| *name == file).unwrap().2;
    let start = source.find(site).expect("negative operation absent");
    if error.stage == "L3"
        && error.file == file
        && error
            .span
            .is_some_and(|span| span.get_start_offset() >= start && span.get_end_offset() <= start + site.len())
        && reason.iter().any(|word| error.message.contains(word))
    {
        Ok(())
    } else {
        Err(describe(&error, files))
    }
}

fn report(
    label: &str,
    result: Result<(), String>,
) -> bool {
    match result {
        Ok(()) => {
            println!("  PASS {label}");
            true
        }
        Err(message) => {
            println!("  FAIL {label} — {message}");
            false
        }
    }
}

fn codec() -> bool {
    let spec = format!(
        "use model::Message; use model::Codec;\n{}",
        block(include_str!("../../../wiki/spec/language/conformances.md"), 204)
    );
    let edges = [
        ("example", "model", "messages"),
        ("example", "lib_a", "encoder"),
        ("example", "lib_b", "decoder"),
        ("encoder", "model", "messages"),
        ("decoder", "model", "messages"),
    ]
    .map(|(requester, alias, package)| Dependency { requester, alias, package });
    let defs = "public data Message{} public trait Codec<T>{machine encode(m:T)->u8; machine decode(n:u8)->T;}";
    let encode = "use model::Message; use model::Codec; public machine encode(m:Message)->u8 satisfies Codec<Message>::encode {0}";
    let decode = "use model::Message; use model::Codec; public machine decode(n:u8)->Message satisfies Codec<Message>::decode {Message{}}";
    let context = [
        ("messages", "types.omg", defs),
        ("encoder", "a.omg", encode),
        ("decoder", "b.omg", decode),
    ];
    let valid = report(
        "encode and decode retain their selected requirements",
        accept(
            &edges,
            &[
                ("example", "codec.omg", &spec),
                context[0],
                context[1],
                context[2],
            ],
            |_, s| {
                [
                    ("Codec::encode", "encoder", "encode"),
                    ("Codec::decode", "decoder", "decode"),
                ]
                .iter()
                .all(|(req, package, body)| {
                    s.conformance_rows.iter().any(|r| {
                        r.site.file == "codec.omg"
                            && r.requirement.package == "messages"
                            && r.implementation.package == *package
                            && r.requirement.path == *req
                            && r.implementation.path == *body
                            && r.trait_arguments
                                .iter()
                                .map(ToString::to_string)
                                .eq(["messages::Message"])
                    })
                })
            },
        ),
    );
    let mut ok = valid;
    for (from, to) in [
        ("lib_a::encode", "lib_b::decode"),
        ("lib_b::decode", "lib_a::encode"),
    ] {
        let wrong = spec.replacen(from, to, 1);
        let line = wrong
            .lines()
            .find(|l| l.contains(&format!("{to} satisfies")) && l.contains(if from.ends_with("encode") { "::encode)" } else { "::decode)" }))
            .unwrap()
            .trim();
        let start = line.find('(').unwrap();
        ok &= report(
            &format!("reject {to} in the {from} slot"),
            reject(
                &edges,
                &[
                    ("example", "codec.omg", &wrong),
                    context[0],
                    context[1],
                    context[2],
                ],
                "codec.omg",
                &line[start..line.len() - 1],
                &["requirement", "signature", "satisf", "conformance"],
            ),
        );
    }
    ok
}

fn reader() -> bool {
    let chapter = include_str!("../../../wiki/spec/language/some_packages.md");
    let schema = block(chapter, 15);
    let init = block(chapter, 39);
    let source = format!("machine pack() {{ let mut file:File;\n{init}\n}}");
    let defs =
        "data File {} data Socket {} machine read_file(state:@mut File,output:@mut [u8])->u64 ensures result <= output.len reaches none crashes none {0}";
    let wrong = defs.replace("state:@mut File", "state:@mut Socket");
    let files = |declarations| {
        [
            ("example", "reader.omg", schema.as_str()),
            ("example", "state.omg", declarations),
            ("example", "pack.omg", source.as_str()),
        ]
    };
    let valid = report(
        "one File witness supplies both record fields",
        accept(
            &[],
            &files(defs),
            |_, s| matches!(s.packages.as_slice(),[p] if p.site.file=="pack.omg" && p.witnesses.iter().map(ToString::to_string).eq(["example::File"])),
        ),
    );
    let invalid = report(
        "refuse a callable over Socket at the initializer",
        reject(&[], &files(&wrong), "pack.omg", "read: read_file", &["witness", "callable", "signature"]),
    );
    valid && invalid
}

fn reflection() -> bool {
    let selectors = block(include_str!("../../../wiki/spec/language/reflection.md"), 77);
    let defs = "data Controller{status:u8;} data Buffer<T>{used:T;} trait ControllerWriter{machine write();}";
    let mut ok = true;
    let expected = [
        ("Controller::status", "FieldInfo"),
        ("Buffer::used", "FieldInfo"),
        ("Controller::update", "CallableInfo"),
        ("ControllerWriter::write", "CallableInfo"),
    ];
    let lines: Vec<_> = selectors.lines().collect();
    assert_eq!(lines.len(), expected.len());
    for (selector, (path, kind)) in lines.into_iter().zip(expected) {
        let source = format!("use omega::language::core::reflect;\nmachine inspect() {{ let descriptor={selector}; }}");
        let files = |text| {
            [
                ("example", "owners.omg", defs),
                ("example", "Controller/update.omg", "public machine update(){}"),
                ("example", "selector.omg", text),
            ]
        };
        ok &= report(
            selector,
            accept(&[], &files(&source), |n, s| {
                let selected = n
                    .uses
                    .iter()
                    .any(|u| u.file == "selector.omg" && matches!(&u.target,Symbol::Declaration(id) if id.path==path));
                let applied = path != "Buffer::used"
                    || n.applications.iter().any(|a| {
                        a.site.file == "selector.omg"
                            && matches!(&a.site.target,Symbol::Declaration(id) if id.path=="Buffer")
                            && a.arguments
                                .iter()
                                .any(|span| span.slice(source.as_bytes()) == Some(b"u32"))
                    });
                let typed=s.expressions.iter().any(|e|e.site.file=="selector.omg" && e.site.span.slice(source.as_bytes())==Some(selector.as_bytes()) && matches!(&e.ty,ValueType::Nominal(id)|ValueType::Applied(id,_) if id.package=="@omega/toolchain" && id.path==format!("language::core::reflect::{kind}")));
                selected && applied && typed
            }),
        );
        let (from, to) = if kind == "FieldInfo" {
            ("reflect::field", "reflect::callable")
        } else {
            ("reflect::callable", "reflect::field")
        };
        let other = selector.replace(from, to);
        let wrong = source.replace(selector, &other);
        ok &= report(
            &format!("reject {other}"),
            reject(
                &[],
                &files(&wrong),
                "selector.omg",
                &other,
                &[
                    "field declaration",
                    "callable declaration",
                    "expected a field",
                    "expected a callable",
                    "machine declaration",
                ],
            ),
        );
    }
    ok
}

fn main() {
    let mut failed = 0;
    for (name, check) in [
        ("language/conformances.md:204", codec as fn() -> bool),
        ("language/some_packages.md:39", reader),
        ("language/reflection.md:77", reflection),
    ] {
        println!("{name}");
        if check() {
            println!("PASS {name}");
        } else {
            println!("FAIL {name}");
            failed += 1;
        }
    }
    println!("{} / 3 published examples reached their intended verdicts", 3 - failed);
    if failed != 0 {
        std::process::exit(1)
    }
}
