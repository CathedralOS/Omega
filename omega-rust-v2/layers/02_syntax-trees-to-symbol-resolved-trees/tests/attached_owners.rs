use omega_lexer::lex;
use omega_parser::parse_file;
use omega_resolver::{CompiledInterface, Dependency, NameError, ParsedSource, StageSource, Symbol, SymbolResolvedTrees, resolve_stage_with_exports};

fn resolve<'text>(
    files: &[(&'text str, &'text str, &'text str)],
    dependencies: &[Dependency<'_>],
    interface: &CompiledInterface,
) -> Result<SymbolResolvedTrees<'text>, NameError> {
    let parsed: Vec<_> = files
        .iter()
        .map(|&(_, _, text)| {
            let tokens = lex(text.as_bytes()).unwrap();
            let tree = parse_file(text.as_bytes(), &tokens).unwrap();
            (tokens, tree)
        })
        .collect();
    let sources: Vec<_> = files
        .iter()
        .zip(&parsed)
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
    resolve_stage_with_exports(&sources, dependencies, &Default::default(), interface)
}

#[test]
fn indexed_parent_keeps_its_argument_owner_when_the_carrier_is_omitted() {
    for path in ["Mark<3>::Child", "u8::Mark<3>::Child", "Mark::Child"] {
        let text = format!("domain<const N:u8> u8::Mark<N>; domain u16::Mark; domain u8::Mark::Child; machine f(x:u8 in {path}){{}}");
        let stage = resolve(&[("app", "main.omg", &text)], &[], &Default::default()).unwrap();
        let tree = stage.trees().next().unwrap();
        let reference = tree
            .occurrences()
            .find(|name| {
                name.span.slice(text.as_bytes()) == Some(path.as_bytes()) && matches!(&name.target, Symbol::Declaration(id) if id.path == "u8::Mark::Child")
            })
            .unwrap();
        let applied: Vec<_> = tree
            .applications()
            .filter(|(_, name)| name.node == reference.node)
            .collect();
        if path.contains('<') {
            let [(section, owner)] = applied.as_slice() else {
                panic!("expected one indexed parent in {path}")
            };
            assert!(matches!(&owner.target, Symbol::Declaration(id) if id.path == "u8::Mark"));
            let [argument] = tree.file.nodes[*section].children.as_slice() else {
                panic!("expected one argument in {path}")
            };
            assert_eq!(tree.file.span(*argument).slice(text.as_bytes()), Some(b"3".as_slice()));
        } else {
            assert!(applied.is_empty());
        }
    }
}

#[test]
fn competing_attachments_report_details_in_identity_order() {
    let library = ("lib", "units/defs.omg", "public domain<Z> Z::Flag; public domain<A> A::Flag;");
    let text = "use dep\\units; machine f(x:u8 in Flag){}";
    let requester = ("app", "main.omg", text);
    let dependencies = [Dependency {
        requester: "app",
        alias: "dep",
        package: "lib",
    }];
    let source_error = resolve(&[requester, library], &dependencies, &Default::default()).unwrap_err();
    assert_eq!(source_error.span.slice(text.as_bytes()), Some(b"Flag".as_slice()));
    assert_eq!(
        source_error.message,
        "competing declarations: lib\\units\\A::Flag, lib\\units\\Z::Flag (lib\\units\\A::Flag on generic carrier, import main.omg@4..13) (lib\\units\\Z::Flag on generic carrier, import main.omg@4..13)"
    );
    let interface = resolve(&[library], &[], &Default::default())
        .unwrap()
        .compiled_interface();
    let compiled_error = resolve(&[requester], &dependencies, &interface).unwrap_err();
    assert_eq!(source_error, compiled_error);
}
