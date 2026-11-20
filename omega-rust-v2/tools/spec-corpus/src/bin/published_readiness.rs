//! Check the intended verdicts of published examples that still fail before their primary judgment.
//! Each example runs its positive and negative inputs, even if the first one fails.
use omega_lexer::{SourceSpan, lex};
use omega_parser::parse_file;
use omega_resolver::{Dependency, ParsedSource, StageSource, Symbol, SymbolResolvedTrees, resolve_stage};
use omega_toolchain::resolve_source_package;
use omega_typer::{TypedStage, TypedTrees, ValueType, type_source_package};

type Source<'a> = (&'a str, &'a str, &'a str);
struct Failure {
    stage: &'static str,
    package: String,
    file: String,
    span: SourceSpan,
    message: String,
}

fn compile<'text>(
    files: &[Source<'text>],
    edges: &[Dependency<'_>],
) -> Result<TypedTrees<'text>, Failure> {
    let mut parsed = Vec::new();
    for &(package, file, text) in files {
        let earlier = |stage, span, message| Failure {
            stage,
            package: package.to_owned(),
            file: file.to_string(),
            span,
            message,
        };
        let tokens = lex(text.as_bytes()).map_err(|e| earlier("L0", e.get_span(), e.get_kind().get_message().to_owned()))?;
        let tree = parse_file(text.as_bytes(), &tokens).map_err(|e| earlier("L1", e.get_span(), e.get_kind().get_message(e.get_expected())))?;
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
    let name_failure = |e: omega_resolver::NameError| Failure {
        stage: "L2",
        package: e.package,
        file: e.file,
        span: e.span,
        message: e.message,
    };
    let typed = if edges.is_empty() {
        type_source_package(&resolve_source_package(&sources).map_err(name_failure)?)
    } else {
        omega_typer::type_stage(&resolve_stage(&sources, edges).map_err(name_failure)?)
    }
    .map_err(|e| Failure {
        stage: "L3",
        package: e.package,
        file: e.file,
        span: e.span,
        message: e.message,
    })?;
    Ok(typed)
}

fn block(
    chapter: &str,
    line: usize,
) -> String {
    let examples = spec_corpus::published_chapter(chapter).unwrap();
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
        .find(|(package, name, _)| *package == failure.package && *name == failure.file)
        .map(|(_, _, text)| *text);
    let Some(source) = source else {
        return format!("{} {}/{}: {}", failure.stage, failure.package, failure.file, failure.message);
    };
    let pos = omega_lexer::LineIndex::new(source.as_bytes()).get_position(failure.span.get_start_offset());
    format!("{} {}:{}:{}: {} (bytes {}..{})", failure.stage, failure.file, pos.get_line(), pos.get_column(), failure.message, failure.span.get_start_offset(), failure.span.get_end_offset())
}

fn accept(
    edges: &[Dependency<'_>],
    files: &[Source<'_>],
    verify: impl FnOnce(&SymbolResolvedTrees<'_>, &TypedStage) -> bool,
) -> Result<Option<String>, String> {
    let typed = compile(files, edges).map_err(|e| describe(&e, files))?;
    if verify(typed.resolved(), &typed) {
        Ok(None)
    } else {
        Err("L3 accepted but the expected typed product is missing".into())
    }
}

fn reject(
    edges: &[Dependency<'_>],
    files: &[Source<'_>],
    file: &str,
    site: &str,
    reason: &str,
) -> Result<Option<String>, String> {
    let &(package, _, source) = files.iter().find(|(_, name, _)| *name == file).unwrap();
    let mut occurrences = source.match_indices(site);
    let (start, _) = occurrences.next().expect("negative operation absent");
    assert!(occurrences.next().is_none(), "ambiguous expected occurrence: {site}");
    let expected = format!("expected L3 to refuse {package}/{file} at {site} for {reason}");
    let Err(error) = compile(files, edges) else {
        return Err(format!("accepted the negative input; {expected}"));
    };
    if error.stage == "L3"
        && error.package == package
        && error.file == file
        && error.span == SourceSpan::new(start, start + site.len())
        && error.message == reason
    {
        Ok(Some(describe(&error, files)))
    } else {
        Err(format!("{}; {expected}", describe(&error, files)))
    }
}

fn report(
    label: &str,
    result: Result<Option<String>, String>,
) -> bool {
    match result {
        Ok(detail) => {
            if let Some(observed) = detail {
                println!("  PASS {label} — {observed}");
            } else {
                println!("  PASS {label}");
            }
            true
        }
        Err(message) => {
            println!("  FAIL {label} — {message}");
            false
        }
    }
}

fn codec() -> bool {
    let spec = format!("use model\\Message; use model\\Codec;\n{}", block("language/conformances.md", 204));
    let edges = [
        ("example", "model", "messages"),
        ("example", "lib_a", "encoder"),
        ("example", "lib_b", "decoder"),
        ("encoder", "model", "messages"),
        ("decoder", "model", "messages"),
    ]
    .map(|(requester, alias, package)| Dependency { requester, alias, package });
    let defs = "public data Message{} public trait Codec<T>{machine encode(m:T)->u8; machine decode(n:u8)->T;}";
    let encode = "use model\\Message; use model\\Codec; public machine encode(m:Message)->u8 satisfies Codec<Message>::encode {0}";
    let decode = "use model\\Message; use model\\Codec; public machine decode(n:u8)->Message satisfies Codec<Message>::decode {Message{}}";
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
                            && r.requirement.declaration.package == "messages"
                            && r.implementation.package == *package
                            && r.requirement.declaration.path == *req
                            && r.implementation.path == *body
                            && r.requirement
                                .trait_arguments
                                .iter()
                                .map(ToString::to_string)
                                .eq(["messages\\Message"])
                    })
                })
            },
        ),
    );
    let mut ok = valid;
    for (from, to, requirement) in [
        ("lib_a\\encode", "lib_b\\decode", "encode"),
        ("lib_b\\decode", "lib_a\\encode", "decode"),
    ] {
        let wrong = spec.replacen(from, to, 1);
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
                &format!("({to} satisfies Codec<Message>::{requirement})"),
                "machine has no matching requirement signature",
            ),
        );
    }
    ok
}

fn reader() -> bool {
    let chapter = "language/some_packages.md";
    let schema = block(chapter, 15);
    let init = block(chapter, 41);
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
        accept(&[], &files(defs), |_, stage| {
            let [package] = stage.packages.as_slice() else { return false };
            package.site.file == "pack.omg" && matches!(package.witnesses.as_slice(), [ValueType::Nominal(id)] if id.package == "example" && id.path == "File")
        }),
    );
    let invalid = report(
        "refuse a callable over Socket at the initializer",
        reject(&[], &files(&wrong), "pack.omg", "read_file", "inconsistent package witness"),
    );
    valid && invalid
}

fn reflection() -> bool {
    let selectors = block("language/reflection.md", 77);
    let defs = "data Controller{status:u8;} data Buffer<T>{used:T;} trait ControllerWriter{machine write();}";
    let mut ok = true;
    let expected = [
        ("Controller::status", "FieldInfo"),
        ("Buffer::used", "FieldInfo"),
        ("Controller\\update", "CallableInfo"),
        ("ControllerWriter::write", "CallableInfo"),
    ];
    let lines: Vec<_> = selectors.lines().collect();
    assert_eq!(lines.len(), expected.len());
    for (selector, (path, kind)) in lines.into_iter().zip(expected) {
        let source = format!("use omega\\language\\core\\reflect;\nmachine inspect() {{ let descriptor={selector}; }}");
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
                    .trees()
                    .filter(|tree| tree.file.path == "selector.omg")
                    .flat_map(|tree| tree.occurrences())
                    .any(|u| matches!(&u.target,Symbol::Declaration(id) if id.path==path));
                let expression = s.expressions.iter().find(|e| e.site.file == "selector.omg"
                    && e.site.span.slice(source.as_bytes()) == Some(selector.as_bytes()));
                let Some(expression) = expression else { return false };
                let descriptor_type = matches!(&expression.ty, ValueType::Applied(id, lifetimes)
                    if id.package == "@omega/toolchain" && id.path == format!("language\\core\\reflect\\{kind}") && lifetimes.len() == 1);
                use omega_typer::DeclarationSelection::*;
                let exact = match (path, s.declaration_selections.get(&expression.node)) {
                    ("Controller::status", Some(Field { owner: ValueType::Nominal(owner), field, field_type, position: 0, case: None })) =>
                        owner.package == "example" && owner.path == "Controller" && field.path == path && matches!(field_type, ValueType::Integer { signed: false, width: Some(8) }),
                    ("Buffer::used", Some(Field { owner: ValueType::Applied(owner, args), field, field_type, position: 0, case: None })) =>
                        owner.package == "example" && owner.path == "Buffer" && field.path == path
                            && args.as_slice() == std::slice::from_ref(field_type) && matches!(field_type, ValueType::Integer { signed: false, width: Some(32) }),
                    (_, Some(Machine(application))) if path == "Controller\\update" =>
                        application.machine.package == "example" && application.machine.path == path && application.arguments.is_empty(),
                    (_, Some(Requirement { owner, application })) if path == "ControllerWriter::write" =>
                        owner.path == "ControllerWriter" && application.machine.path == path
                            && application.enclosing_arguments.iter().any(|(scope, args)| scope == owner && args.is_empty()),
                    _ => false,
                };
                selected && descriptor_type && exact && s.calls.iter().all(|call| call.node != expression.node)
            }),
        );
        let (from, to, reason) = if kind == "FieldInfo" {
            ("reflect\\field", "reflect\\callable", "expected a callable declaration")
        } else {
            ("reflect\\callable", "reflect\\field", "expected a field declaration")
        };
        let other = selector.replace(from, to);
        let wrong = source.replace(selector, &other);
        let argument = selector.strip_prefix(from).unwrap().strip_prefix('(').unwrap().strip_suffix(')').unwrap();
        ok &= report(
            &format!("reject {other}"),
            reject(
                &[],
                &files(&wrong),
                "selector.omg",
                argument,
                reason,
            ),
        );
    }
    // An unresolved selector can mean either a missing public query or a failed
    // lookup of a query the toolchain supplied. Inspect the actual interface.
    let interface = omega_toolchain::core_interface().exports();
    for member in ["field", "callable", "FieldInfo", "CallableInfo"] {
        let path = format!("language\\core\\reflect\\{member}");
        let mut kinds: Vec<_> = interface
            .declarations
            .iter()
            .filter(|export| export.id.package == omega_toolchain::TOOLCHAIN_PACKAGE && export.id.path == path)
            .map(|export| format!("{:?}", export.id.kind))
            .collect();
        if interface
            .declaration_queries
            .contains(&(omega_toolchain::TOOLCHAIN_PACKAGE.into(), path.clone()))
        {
            kinds.push("DeclarationQuery".into());
        }
        let result = if kinds.is_empty() { "absent".to_owned() } else { kinds.join(",") };
        println!(
            "OMEGA_CONTEXT_EXPORT\tlanguage/reflection.md:77\t{}\t{path}\t{result}",
            omega_toolchain::TOOLCHAIN_PACKAGE
        );
    }
    ok
}

fn main() {
    let mut failed = 0;
    for (name, check) in [
        ("language/conformances.md:204", codec as fn() -> bool),
        ("language/some_packages.md:41", reader),
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
