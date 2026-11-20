//! `u8` and `u<8>` name one carrier, but only typing evaluates a width. When the written
//! or implicit carrier of a domain path is an applied integer, L2 binds every visible domain
//! of that name on a same-family builtin carrier and leaves the choice to typing. Unapplied
//! spellings match exactly unless the declaration itself applies a width.

use omega_lexer::lex;
use omega_parser::parse_file;
use omega_resolver::{Dependency, NameError, NameErrorKind, NameRole, ParsedSource, StageSource, Symbol, SymbolResolvedTrees, resolve_stage};

fn resolve<'text>(
    files: &[(&'text str, &'text str, &'text str)],
    dependencies: &[Dependency<'_>],
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
    resolve_stage(&sources, dependencies)
}

/// The reference bound at the written domain path, as (package\path, deferred) pairs.
fn bound_domains(
    stage: &SymbolResolvedTrees<'_>,
    text: &str,
    path: &str,
) -> Vec<(String, bool)> {
    let usage = stage
        .uses()
        .find(|u| u.role == NameRole::Reference && u.span.slice(text.as_bytes()) == Some(path.as_bytes()))
        .unwrap_or_else(|| panic!("no reference {path} in {text}"));
    let mut bound: Vec<_> = match &usage.target {
        Symbol::Declaration(id) => vec![(format!("{}\\{}", id.package, id.path), false)],
        Symbol::DomainCandidates { candidates, .. } => candidates
            .iter()
            .map(|c| (format!("{}\\{}", c.declaration.package, c.declaration.path), c.carrier_deferred))
            .collect(),
        other => panic!("{text}: unexpected target {other:?}"),
    };
    bound.sort();
    bound
}

fn owned(pairs: &[(&str, bool)]) -> Vec<(String, bool)> {
    pairs.iter().map(|&(path, deferred)| (path.to_owned(), deferred)).collect()
}

#[test]
fn an_applied_integer_root_binds_its_single_same_family_domain() {
    // The reported case no longer stops at L2.
    let text = "domain u8::Ready; machine f(x:u8 in u<8>::Ready){}";
    let stage = resolve(&[("app", "main.omg", text)], &[]).unwrap();
    assert_eq!(bound_domains(&stage, text, "u<8>::Ready"), owned(&[("app\\u8::Ready", false)]));
    let tree = stage.trees().next().unwrap();
    let applications: Vec<_> = tree.applications().collect();
    let [(_, owner)] = applications.as_slice() else {
        panic!("expected the carrier argument list only")
    };
    assert!(matches!(&owner.target, Symbol::Builtin(name) if name == "u"), "{owner:?}");
}

#[test]
fn a_compact_integer_use_retains_an_applied_declaration_for_typing() {
    let text = "public domain u<8>::Ready; domain i<8>::Ready; domain u16::Ready; machine f(x:u8 in u8::Ready){}";
    let stage = resolve(&[("app", "main.omg", text)], &[]).unwrap();
    assert_eq!(bound_domains(&stage, text, "u8::Ready"), owned(&[("app\\u::Ready", false)]));
    let interface = stage.compiled_interface();
    let domain = interface.declarations.iter().find(|d| d.id.path == "u::Ready").unwrap();
    assert!(matches!(&domain.domain_carrier, Some(omega_resolver::DomainCarrier::Builtin { name, applied: true }) if name == "u"));
}

#[test]
fn applied_integer_roots_defer_every_same_family_carrier_to_typing() {
    let defs = "domain u8::Ready; domain u16::Ready; domain i8::Ready; domain bool::Ready;";
    let unsigned = owned(&[("app\\u16::Ready", true), ("app\\u8::Ready", true)]);
    for (use_site, path) in [
        ("machine f(x:u8 in u<8>::Ready){}", "u<8>::Ready"),
        ("machine f(x:u16 in u<16>::Ready){}", "u<16>::Ready"),
        // Width arguments are not evaluated here: symbolic and invalid widths reach typing.
        ("machine f<const N:u64>(x:u<N> in u<N>::Ready){}", "u<N>::Ready"),
        ("machine f(x:u8 in u<4 + 4>::Ready){}", "u<4 + 4>::Ready"),
        ("machine f(x:u8 in u<0>::Ready){}", "u<0>::Ready"),
        ("machine f(x:u8 in u8<1>::Ready){}", "u8<1>::Ready"),
        // Implicit carrier.
        ("machine f(x:u<8> in Ready){}", "Ready"),
        // A value or static binder spelled u leaves the builtin carrier in the type role.
        ("machine f(u:u16, x:u8 in u<8>::Ready){}", "u<8>::Ready"),
        ("machine f<const u:u64>(x:u8 in u<8>::Ready){}", "u<8>::Ready"),
    ] {
        let text = format!("{defs} {use_site}");
        let stage = resolve(&[("app", "main.omg", &text)], &[]).unwrap_or_else(|error| panic!("{text}: {error:?}"));
        assert_eq!(bound_domains(&stage, &text, path), unsigned, "{text}");
    }
    let text = format!("{defs} machine f(x:i8 in i<8>::Ready){{}}");
    let stage = resolve(&[("app", "main.omg", &text)], &[]).unwrap();
    assert_eq!(bound_domains(&stage, &text, "i<8>::Ready"), owned(&[("app\\i8::Ready", false)]));
    // Unapplied decimal spellings keep the exact carrier.
    for (use_site, path, selected) in [
        ("machine f(x:u8 in u8::Ready){}", "u8::Ready", "app\\u8::Ready"),
        ("machine f(x:u16 in Ready){}", "Ready", "app\\u16::Ready"),
    ] {
        let text = format!("{defs} {use_site}");
        let stage = resolve(&[("app", "main.omg", &text)], &[]).unwrap();
        assert_eq!(bound_domains(&stage, &text, path), owned(&[(selected, false)]), "{text}");
    }
}

#[test]
fn integer_family_and_the_unbounded_spelling_still_refuse_at_l2() {
    let defs = "domain u8::Ready;";
    for (use_site, path) in [
        ("machine f(x:u8 in i<8>::Ready){}", "i::Ready"),
        ("machine f(x:u8 in u::Ready){}", "u::Ready"),
        ("machine f(x:i<8> in Ready){}", "Ready"),
    ] {
        let text = format!("{defs} {use_site}");
        let error = resolve(&[("app", "main.omg", &text)], &[]).unwrap_err();
        assert_eq!(error.kind, NameErrorKind::UnresolvedName, "{text}: {error:?}");
        assert_eq!(error.message, format!("unresolved name {path}"), "{text}");
    }
}

#[test]
fn a_family_found_parent_owns_its_indices_as_candidates() {
    let defs = "domain<const K:u8> u8::Limit<K>; domain<const K:u16> u16::Limit<K>; domain u8::Limit<2>::Pair; domain u16::Limit<2>::Pair;";
    let text = format!("{defs} machine f(x:u8 in u<8>::Limit<2>::Pair){{}}");
    let text = text.as_str();
    let stage = resolve(&[("app", "main.omg", text)], &[]).unwrap();
    assert_eq!(
        bound_domains(&stage, text, "u<8>::Limit<2>::Pair"),
        owned(&[("app\\u16::Limit::Pair", true), ("app\\u8::Limit::Pair", true)])
    );
    let tree = stage.trees().next().unwrap();
    let use_start = defs.len();
    let parent = tree
        .applications()
        .find(|&(section, _)| {
            let [only] = tree.file.nodes[section].children.as_slice() else { return false };
            tree.file.span(*only).slice(text.as_bytes()) == Some(b"2".as_slice()) && tree.file.span(section).get_start_offset() > use_start
        })
        .map(|(_, owner)| owner)
        .unwrap();
    assert_eq!(parent.role, NameRole::Qualifier);
    let Symbol::DomainCandidates { candidates, .. } = &parent.target else {
        panic!("{parent:?}")
    };
    let mut parents: Vec<_> = candidates
        .iter()
        .map(|c| (c.declaration.path.as_str(), c.carrier_deferred))
        .collect();
    parents.sort();
    assert_eq!(parents, [("u16::Limit", true), ("u8::Limit", true)]);
}

#[test]
fn width_domain_candidates_respect_package_visibility_and_public_signatures() {
    let edge = Dependency {
        requester: "app",
        alias: "dep",
        package: "lib",
    };
    let library = ("lib", "units/defs.omg", "public domain u16::Ready; domain u32::Ready;");
    let text = "use dep\\units; domain u8::Ready; machine f(x:u8 in u<8>::Ready){}";
    let stage = resolve(&[library, ("app", "main.omg", text)], &[edge]).unwrap();
    // The private foreign domain is not visible, so it never competes.
    assert_eq!(
        bound_domains(&stage, text, "u<8>::Ready"),
        owned(&[("app\\u8::Ready", true), ("lib\\units\\u16::Ready", true)])
    );

    // A private candidate is retained for typing to refuse only if selected.
    let text = "public domain u8::Ready; domain u16::Ready; public machine f(x:u8 in u<8>::Ready){}";
    let stage = resolve(&[("app", "main.omg", text)], &[]).unwrap();
    let tree = stage.trees().next().unwrap();
    let reference = tree
        .occurrences()
        .find(|u| u.role == NameRole::Reference && u.span.slice(text.as_bytes()) == Some(b"u<8>::Ready".as_slice()))
        .unwrap();
    let [private] = tree.private_signature_candidates(reference.node.node) else {
        panic!("expected one retained private candidate")
    };
    assert_eq!(private.path, "u16::Ready");
    // With no public candidate the signature refuses as before.
    let text = "domain u8::Ready; domain u16::Ready; public machine f(x:u8 in u<8>::Ready){}";
    let error = resolve(&[("app", "main.omg", text)], &[]).unwrap_err();
    assert_eq!(error.kind, NameErrorKind::PrivateDeclarationInPublicSignature, "{error:?}");
    assert_eq!(error.message, "public signature exposes private declaration u::Ready");
}
