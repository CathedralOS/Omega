use omega_lexer::lex;
use omega_parser::parse_file;
use omega_resolver::{Dependency, ParsedSource, StageSource, Symbol, resolve_stage};

fn run<'text>(
    files: &[(&'text str, &'text str, &'text str)],
    edges: &[Dependency<'_>],
) -> Result<omega_resolver::SymbolResolvedTrees<'text>, omega_resolver::NameError> {
    let parses: Vec<_> = files
        .iter()
        .map(|&(_, _, text)| {
            let tokens = lex(text.as_bytes()).unwrap();
            let tree = parse_file(text.as_bytes(), &tokens).unwrap();
            (tokens, tree)
        })
        .collect();
    let mut sources: Vec<_> = files
        .iter()
        .zip(&parses)
        .map(|(&(package, logical_path, text), (tokens, tree))| StageSource {
            package,
            source: ParsedSource {
                logical_path,
                text,
                tokens,
                tree,
            },
        })
        .collect();
    let forward = resolve_stage(&sources, edges);
    if let Ok(stage) = &forward {
        support::check_origins(stage, &sources);
    }
    sources.reverse();
    assert_eq!(forward, resolve_stage(&sources, edges));
    forward
}

mod support;

#[test]
fn equal_operand_spellings_keep_distinct_imported_constant_origins() {
    let east = ("app", "east/limits.omg", "public const Limit: u8 = 4;");
    let west = ("app", "west/limits.omg", "public const Limit: u8 = 8;");
    let stage = run(
        &[
            east,
            west,
            ("app", "east.omg", "use east\\Limit; machine read(x: [u8; Limit]) {}"),
            ("app", "west.omg", "use west\\Limit; machine read(x: [u8; Limit]) {}"),
            ("app", "main.omg", "machine visit(x: [u8; 4]) { read(x); }"),
        ],
        &[],
    )
    .unwrap();
    let [first, second] = stage
        .uses()
        .find_map(|name| match &name.target {
            Symbol::Candidates(ids) if ids.iter().all(|id| id.path == "read") => Some(ids.as_slice()),
            _ => None,
        })
        .unwrap()
    else {
        panic!("expected two read declarations");
    };
    assert_eq!(first.operands, second.operands);
    assert_ne!(first, second);
    assert_ne!(first.resolved_signature, second.resolved_signature);

    for (path, constant, qualified) in [
        ("east.omg", east, "machine read(x: [u8; east\\Limit]) {}"),
        ("west.omg", west, "machine read(x: [u8; west\\Limit]) {}"),
    ] {
        let definition = stage
            .trees()
            .find(|tree| tree.file.path == path)
            .unwrap()
            .declaration_nodes()
            .find_map(|(_, id)| (id.path == "read").then_some(id))
            .unwrap();
        let qualified_stage = run(&[constant, ("app", "qualified.omg", qualified)], &[]).unwrap();
        let expected = qualified_stage
            .declarations
            .iter()
            .find(|id| id.path == "read")
            .unwrap();
        assert_eq!(definition, expected, "wrong constant origin in {path}");
    }
}

#[test]
fn short_result_domains_bind_to_the_carriers_that_define_them() {
    let stage = run(
        &[(
            "app",
            "main.omg",
            "domain u8::Ready; domain u16::Ready; machine read() -> u8 in Ready; machine read() -> u16 in Ready; machine visit() { read(); }",
        )],
        &[],
    )
    .unwrap();
    let [first, second] = stage
        .uses()
        .find_map(|name| match &name.target {
            Symbol::Candidates(ids) if ids.iter().all(|id| id.path == "read") => Some(ids.as_slice()),
            _ => None,
        })
        .unwrap()
    else {
        panic!("expected two read declarations");
    };
    assert_eq!(first.result_domains, second.result_domains);
    assert_ne!(first, second);
    assert_ne!(first.resolved_signature, second.resolved_signature);
}

#[test]
fn reordering_and_qualifying_the_same_result_domains_keeps_one_identity() {
    let text = "domain u8::Ready; domain u8::Checked; machine read() -> u8 in Ready & Checked; machine read() -> u8 in u8::Checked & u8::Ready;";
    let error = run(&[("app", "main.omg", text)], &[]).unwrap_err();
    spec_corpus::observe_handoff!("overloads-and-operators/refusal", assert_eq!(error.message, "duplicate declaration read"));
    assert_eq!(error.span.get_start_offset(), text.rfind("read").unwrap());
}

#[test]
fn naming_the_same_constant_with_qualification_keeps_one_operand_identity() {
    let definition = ("app", "east/limits.omg", "public const Limit: u8 = 4;");
    let unqualified = ("app", "a.omg", "use east\\Limit; public machine read(x: [u8; Limit]) {}");
    let qualified = ("app", "b.omg", "public machine read(x: [u8; east\\Limit]) {}");
    let resolve_id = |producer| {
        run(&[definition, producer], &[])
            .unwrap()
            .compiled_interface()
            .declarations
            .into_iter()
            .find(|export| export.id.path == "read")
            .unwrap()
            .id
    };
    let first = resolve_id(unqualified);
    let second = resolve_id(qualified);
    assert_ne!(first.operands, second.operands);
    assert_eq!(first, second);
    assert!(first.cmp(&second).is_eq());
    assert_eq!(std::collections::HashSet::from([first, second]).len(), 1);
    let error = run(&[definition, unqualified, qualified], &[]).unwrap_err();
    assert_eq!(error.message, "duplicate declaration read");
}

#[test]
fn qualification_of_applied_parent_domains_preserves_the_overload_identity() {
    let domains = "domain<const N:u8> u8::Mark<N>;
                   domain<const M:u8> u8::Mark<3>::Middle<M>;
                   domain u8::Mark<3>::Middle<4>::Child;
                   domain<const N:u8> u8::Other<N>;
                   domain<const M:u8> u8::Other<3>::Middle<M>;
                   domain u8::Other<3>::Middle<4>::Child;";
    let first = "machine read(x:u8 in Mark<3>::Middle<4>::Child){}";
    let duplicate = "machine read(x:u8 in u8::Mark<3>::Middle<4>::Child){}";
    let text = format!("{domains} {first} {duplicate}");
    let error = run(&[("app", "main.omg", &text)], &[]).unwrap_err();
    assert_eq!(error.message, "duplicate declaration read");
    assert_eq!(error.span.get_start_offset(), text.rfind("read").unwrap());

    for distinct in [
        "machine read(x:u8 in u8::Mark<5>::Middle<4>::Child){}",
        "machine read(x:u8 in u8::Mark<3>::Middle<5>::Child){}",
        "machine read(x:u8 in u8::Other<3>::Middle<4>::Child){}",
    ] {
        let text = format!("{domains} {first} {distinct}");
        let stage = run(&[("app", "main.omg", &text)], &[]).unwrap();
        let signatures: std::collections::HashSet<_> = stage
            .declarations
            .iter()
            .filter(|id| id.path == "read")
            .collect();
        assert_eq!(signatures.len(), 2);
    }
}

#[test]
fn indexed_operand_and_result_domains_use_the_runtime_parameter_position() {
    for (definition, signature) in [
        ("data Frame<N:u8>{}", "machine read({first}:u8,{second}:u8,x:Frame<{index}>){}"),
        ("domain<N:u8> u8::Ready<N>;", "machine read({first}:u8,{second}:u8)->u8 in Ready<{index}>;"),
    ] {
        let declaration = |first: &str, second: &str, index: &str| {
            signature
                .replace("{first}", first)
                .replace("{second}", second)
                .replace("{index}", index)
        };
        let first = declaration("n", "k", "n");
        let distinct = declaration("k", "n", "n");
        let source = format!("{definition} {first} {distinct}");
        let stage = run(&[("app", "main.omg", &source)], &[]).unwrap();
        let signatures: std::collections::HashSet<_> = stage
            .declarations
            .iter()
            .filter(|id| id.path == "read")
            .collect();
        assert_eq!(signatures.len(), 2);

        let duplicate = declaration("renamed", "other", "renamed");
        let source = format!("{definition} {first} {duplicate}");
        let error = run(&[("app", "main.omg", &source)], &[]).unwrap_err();
        assert_eq!(error.message, "duplicate declaration read");
        assert_eq!(error.span.get_start_offset(), source.rfind("read").unwrap());
    }
}

#[test]
fn generic_domain_qualification_uses_the_bound_carrier_in_operands_and_results() {
    let domains = "domain<T,const N:u8> T::Level<N>;";
    for (first, equivalent, distinct) in [
        (
            "machine read<U,W>(x:U in Level<2>){}",
            "machine read<V,W>(x:V in V::Level<2>){}",
            "machine read<V,W>(x:V in W::Level<2>){}",
        ),
        (
            "machine read<U,W>() -> U in Level<2>;",
            "machine read<V,W>() -> V in V::Level<2>;",
            "machine read<V,W>() -> V in W::Level<2>;",
        ),
    ] {
        let source = format!("{domains} {first} {equivalent}");
        let error = run(&[("app", "main.omg", &source)], &[]).unwrap_err();
        assert_eq!(error.message, "duplicate declaration read");
        assert_eq!(error.span.get_start_offset(), source.rfind("read").unwrap());

        let source = format!("{domains} {first} {distinct}");
        let stage = run(&[("app", "main.omg", &source)], &[]).unwrap();
        let signatures: std::collections::HashSet<_> = stage
            .declarations
            .iter()
            .filter(|id| id.path == "read")
            .collect();
        assert_eq!(signatures.len(), 2);
    }
}
