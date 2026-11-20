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

/// A builtin or bound carrier at the root of a domain path owns the arguments
/// written on it, as the same carrier does when written alone. Typing decides
/// whether that carrier accepts them, as for u<8> versus u8<1>.
#[test]
fn builtin_and_bound_carrier_roots_own_their_written_arguments() {
    use omega_resolver::NameRole::{ApplicationPrefix, Qualifier, Reference};
    let limit = "domain<const K:u8> u8::Limit<K>; domain u8::Limit<2>::Pair;";
    for (text, root, argument, role) in [
        ("machine f(x:u8<1>){}".to_owned(), "u8", "1", Reference),
        ("domain u8::Ready; machine f(x:u8 in u8<1>::Ready){}".to_owned(), "u8", "1", Qualifier),
        ("domain<T> T::Ready; machine f(x:u8 in u<8>::Ready){}".to_owned(), "u", "8", Qualifier),
        ("domain<T> T::Ready; machine f(x:u8){ let y = x as +u<8>::Ready; }".to_owned(), "u", "8", Qualifier),
        (format!("{limit} machine f(x:u8 in u8<1>::Limit<2>::Pair){{}}"), "u8", "1", ApplicationPrefix),
        ("domain<T> T::Ready; machine f<T>(x:T in T<3>::Ready){}".to_owned(), "T", "3", Qualifier),
    ] {
        let stage = resolve(&[("app", "main.omg", &text)], &[], &Default::default()).unwrap_or_else(|error| panic!("{text}: {error:?}"));
        let tree = stage.trees().next().unwrap();
        let owners: Vec<_> = tree
            .applications()
            .filter(|(section, _)| {
                matches!(tree.file.nodes[*section].children.as_slice(), [only] if tree.file.span(*only).slice(text.as_bytes()) == Some(argument.as_bytes()))
            })
            .collect();
        let [(_, owner)] = owners.as_slice() else {
            panic!("expected one owner of <{argument}> in {text}")
        };
        assert_eq!(owner.role, role, "{text}");
        assert_eq!(owner.span.slice(text.as_bytes()), Some(root.as_bytes()), "{text}");
        match &owner.target {
            Symbol::Builtin(name) => assert_eq!(name, root, "{text}"),
            Symbol::Binding { .. } => assert_eq!(root, "T", "{text}"),
            other => panic!("{text}: unexpected owner {other:?}"),
        }
    }
    // The parent's own arguments still belong to the parent declaration.
    let text = format!("{limit} machine f(x:u8 in u8<1>::Limit<2>::Pair){{}}");
    let stage = resolve(&[("app", "main.omg", &text)], &[], &Default::default()).unwrap();
    let tree = stage.trees().next().unwrap();
    assert!(tree.applications().any(|(section, owner)| {
        tree.file.nodes[section].children.len() == 1
            && tree.file.span(tree.file.nodes[section].children[0]).slice(text.as_bytes()) == Some(b"2".as_slice())
            && matches!(&owner.target, Symbol::Declaration(id) if id.path == "u8::Limit")
    }));
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
