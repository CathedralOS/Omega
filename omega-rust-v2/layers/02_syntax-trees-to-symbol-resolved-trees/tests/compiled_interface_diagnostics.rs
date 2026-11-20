//! Exact refusals for malformed compiled-interface input. Each case starts from the interface
//! that a valid library compiles to and changes one property. Package, file, span and reason
//! are compared independently. A compiled refusal has no source bytes, so its span is 0..0 and
//! its file names the declaration path (`module\Owner::member`) or, for a module row, the
//! module path, both spelled with `\`.
use omega_lexer::{SourceSpan, lex};
use omega_parser::{NodeKind, parse_file};
use omega_resolver::{CompiledInterface, DeclarationId, DeclarationName, InterfaceExport, NameError, NameErrorKind, ParsedSource, StageSource, resolve_stage, resolve_stage_with_exports};
use std::sync::Arc;

/// Two modules: `net` and `proto`. `proto` holds every exported kind the cases mutate.
const LIBRARY: [(&str, &str, &str); 2] = [
    ("wire", "net/link.omg", "public data Link {}"),
    (
        "wire",
        "proto/packet.omg",
        concat!(
            "public data Packet { case Empty; } ",
            "public domain Packet::Ready; ",
            "public trait Codec {} ",
            "public trait Framed: Codec {} ",
            "trait Hidden {} ",
            "public machine send() {} ",
            "public machine receive() {}",
        ),
    ),
];

fn parsed<R>(
    files: &[(&'static str, &'static str, &'static str)],
    body: impl FnOnce(&[StageSource<'static, '_>]) -> R,
) -> R {
    let parses: Vec<_> = files
        .iter()
        .map(|&(_, _, text)| {
            let tokens = lex(text.as_bytes()).unwrap();
            let tree = parse_file(text.as_bytes(), &tokens).unwrap();
            (tokens, tree)
        })
        .collect();
    let sources: Vec<_> = files
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
    body(&sources)
}

/// The authentic interface and the complete declaration roster of the library.
fn library() -> (CompiledInterface, Vec<DeclarationId>) {
    parsed(&LIBRARY, |sources| {
        let stage = resolve_stage(sources, &[]).unwrap();
        (stage.compiled_interface(), stage.declarations.clone())
    })
}

fn export<'i>(
    interface: &'i mut CompiledInterface,
    path: &str,
    kind: NodeKind,
) -> &'i mut InterfaceExport {
    interface
        .declarations
        .iter_mut()
        .find(|e| e.id.path == path && e.id.kind == kind)
        .unwrap_or_else(|| panic!("the library exports no {kind:?} {path}"))
}

fn id(
    interface: &mut CompiledInterface,
    path: &str,
    kind: NodeKind,
) -> DeclarationId {
    export(interface, path, kind).id.clone()
}

fn renamed(
    id: &DeclarationId,
    edit: impl FnOnce(&mut DeclarationName),
) -> DeclarationId {
    let mut name = (*id.name).clone();
    edit(&mut name);
    DeclarationId {
        kind: id.kind,
        name: Arc::new(name),
    }
}

/// Reject the interface in both export orders; the contract keeps the diagnostic order-independent.
fn reject(
    sources: &[StageSource<'_, '_>],
    interface: &mut CompiledInterface,
) -> NameError {
    let error = resolve_stage_with_exports(sources, &[], &Default::default(), &*interface).unwrap_err();
    interface.declarations.reverse();
    let reordered = resolve_stage_with_exports(sources, &[], &Default::default(), &*interface).unwrap_err();
    assert_eq!(error, reordered, "export order changed the diagnostic");
    error
}

/// The refusal's kind is reported only after every field of the refusal matches.
fn assert_refusal(
    error: &NameError,
    kind: NameErrorKind,
    package: &str,
    file: &str,
    reason: &str,
) {
    spec_corpus::assert_resolver_refusal!(error, kind, package, file, SourceSpan::new(0, 0), reason);
}

#[test]
fn the_unmodified_library_interface_is_admitted() {
    let (mut interface, _) = library();
    let modules: Vec<_> = interface
        .modules
        .iter()
        .map(|(package, path)| (package.as_str(), path.as_str()))
        .collect();
    assert_eq!(modules, [("wire", "net"), ("wire", "proto")]);
    for (path, kind) in [
        ("net\\Link", NodeKind::Data),
        ("proto\\Packet", NodeKind::Data),
        ("proto\\Packet::Empty", NodeKind::Case),
        ("proto\\Packet::Ready", NodeKind::Domain),
        ("proto\\Codec", NodeKind::Trait),
        ("proto\\Framed", NodeKind::Trait),
        ("proto\\send", NodeKind::Machine),
        ("proto\\receive", NodeKind::Machine),
    ] {
        let exported = export(&mut interface, path, kind);
        let module = path.rsplit_once('\\').unwrap().0;
        assert_eq!(exported.module, module, "{path}");
        assert_eq!(exported.domain_carrier.is_some(), kind == NodeKind::Domain, "{path}");
        assert_eq!(exported.trait_parents.is_empty(), path != "proto\\Framed", "{path}");
        assert!(exported.id.enclosing_machine.is_none(), "{path}");
    }
    assert!(
        interface
            .declarations
            .iter()
            .all(|e| e.id.path != "proto\\Hidden")
    );
    resolve_stage_with_exports(&[], &[], &Default::default(), &interface).unwrap();
    parsed(&[("app", "main.omg", "machine main() {}")], |sources| {
        resolve_stage_with_exports(sources, &[], &Default::default(), &interface).unwrap();
    });
}

#[test]
fn a_compiled_export_cannot_name_a_nested_declaration() {
    let (mut interface, _) = library();
    let receive = id(&mut interface, "proto\\receive", NodeKind::Machine);
    let send = export(&mut interface, "proto\\send", NodeKind::Machine);
    send.id = renamed(&send.id, |name| name.enclosing_machine = Some(receive));
    assert_refusal(
        &reject(&[], &mut interface),
        NameErrorKind::NestedCompiledExport,
        "wire",
        "proto\\send",
        "compiled export names a nested machine or local declaration",
    );
}

#[test]
fn a_compiled_export_needs_its_own_listed_module() {
    // The root module is an ancestor of every listed module but is not itself listed.
    let (mut interface, _) = library();
    export(&mut interface, "proto\\send", NodeKind::Machine).module = String::new();
    assert_refusal(&reject(&[], &mut interface), NameErrorKind::CompiledExportWithoutModule, "wire", "proto\\send", "compiled export has no defining module");

    let (mut interface, _) = library();
    export(&mut interface, "proto\\Packet::Ready", NodeKind::Domain).module = "proto\\inner".into();
    assert_refusal(
        &reject(&[], &mut interface),
        NameErrorKind::CompiledExportWithoutModule,
        "wire",
        "proto\\Packet::Ready",
        "compiled export has no defining module",
    );

    // The module rows belong to `wire`, so an export from another package has none.
    let (mut interface, _) = library();
    let send = export(&mut interface, "proto\\send", NodeKind::Machine);
    send.id = renamed(&send.id, |name| name.package = "rogue".into());
    assert_refusal(&reject(&[], &mut interface), NameErrorKind::CompiledExportWithoutModule, "rogue", "proto\\send", "compiled export has no defining module");
}

#[test]
fn a_compiled_export_path_must_lie_inside_its_listed_module() {
    let (mut interface, _) = library();
    export(&mut interface, "proto\\send", NodeKind::Machine).module = "net".into();
    assert_refusal(
        &reject(&[], &mut interface),
        NameErrorKind::CompiledExportOutsideModule,
        "wire",
        "proto\\send",
        "compiled export is outside its defining module",
    );

    let (mut interface, _) = library();
    export(&mut interface, "net\\Link", NodeKind::Data).module = "proto".into();
    assert_refusal(
        &reject(&[], &mut interface),
        NameErrorKind::CompiledExportOutsideModule,
        "wire",
        "net\\Link",
        "compiled export is outside its defining module",
    );
}

#[test]
fn a_compiled_domain_carrier_appears_exactly_on_attached_domains() {
    let reason = "compiled domain attachment does not match its declaration";
    let refusal_kind = NameErrorKind::CompiledDomainAttachmentMismatch;

    let (mut interface, _) = library();
    export(&mut interface, "proto\\Packet::Ready", NodeKind::Domain).domain_carrier = None;
    assert_refusal(&reject(&[], &mut interface), refusal_kind, "wire", "proto\\Packet::Ready", reason);

    for (path, kind) in [
        ("proto\\Packet", NodeKind::Data),
        // A case path also contains `::`, but only a domain can be attached.
        ("proto\\Packet::Empty", NodeKind::Case),
        ("proto\\send", NodeKind::Machine),
    ] {
        let (mut interface, _) = library();
        let carrier = export(&mut interface, "proto\\Packet::Ready", NodeKind::Domain)
            .domain_carrier
            .clone();
        assert!(carrier.is_some());
        export(&mut interface, path, kind).domain_carrier = carrier;
        assert_refusal(&reject(&[], &mut interface), refusal_kind, "wire", path, reason);
    }
}

#[test]
fn compiled_module_rows_must_be_package_relative_directory_paths() {
    for module in ["u8", "proto\\u8", "1net", "proto\\", "\\proto", "proto/inner", "proto\\in-ner"] {
        let (mut interface, _) = library();
        assert!(interface.modules.insert(("wire".into(), module.into())), "{module}");
        assert_refusal(&reject(&[], &mut interface), NameErrorKind::InvalidCompiledModulePath, "wire", module, "invalid compiled module path");
    }
}

#[test]
fn only_a_compiled_trait_can_carry_parents() {
    for (path, kind) in [
        ("proto\\Packet", NodeKind::Data),
        ("proto\\Packet::Ready", NodeKind::Domain),
        ("proto\\send", NodeKind::Machine),
    ] {
        let (mut interface, _) = library();
        let codec = id(&mut interface, "proto\\Codec", NodeKind::Trait);
        export(&mut interface, path, kind).trait_parents = vec![codec];
        assert_refusal(
            &reject(&[], &mut interface),
            NameErrorKind::CompiledParentsOnNonTrait,
            "wire",
            path,
            "only a trait can have parents in a compiled interface",
        );
    }
}

#[test]
fn a_compiled_parent_must_be_an_admitted_public_trait() {
    let reason = "unknown parent trait in compiled interface";
    let refusal_kind = NameErrorKind::UnknownCompiledParentTrait;

    // The library's private trait has an authentic identity but is not exported.
    let (mut interface, declarations) = library();
    let hidden = declarations
        .iter()
        .find(|id| id.path == "proto\\Hidden" && id.kind == NodeKind::Trait)
        .unwrap()
        .clone();
    export(&mut interface, "proto\\Framed", NodeKind::Trait).trait_parents = vec![hidden];
    assert_refusal(&reject(&[], &mut interface), refusal_kind, "wire", "proto\\Framed", reason);

    // An exported declaration at the same path is not a trait.
    let (mut interface, _) = library();
    let packet = id(&mut interface, "proto\\Packet", NodeKind::Data);
    let framed = export(&mut interface, "proto\\Framed", NodeKind::Trait);
    framed.trait_parents = vec![DeclarationId {
        kind: NodeKind::Trait,
        ..packet
    }];
    assert_refusal(&reject(&[], &mut interface), refusal_kind, "wire", "proto\\Framed", reason);

    // The parent identity names another package.
    let (mut interface, _) = library();
    let codec = id(&mut interface, "proto\\Codec", NodeKind::Trait);
    export(&mut interface, "proto\\Framed", NodeKind::Trait).trait_parents = vec![renamed(&codec, |name| name.package = "rogue".into())];
    assert_refusal(&reject(&[], &mut interface), refusal_kind, "wire", "proto\\Framed", reason);
}

#[test]
fn a_compiled_trait_cycle_reports_the_first_trait_by_package_and_path() {
    let (mut interface, _) = library();
    let framed = id(&mut interface, "proto\\Framed", NodeKind::Trait);
    export(&mut interface, "proto\\Codec", NodeKind::Trait).trait_parents = vec![framed];
    assert_refusal(&reject(&[], &mut interface), NameErrorKind::CyclicCompiledTraitInheritance, "wire", "proto\\Codec", "cyclic trait inheritance");

    // A self-parent is the shortest cycle.
    let (mut interface, _) = library();
    let codec = id(&mut interface, "proto\\Codec", NodeKind::Trait);
    export(&mut interface, "proto\\Codec", NodeKind::Trait).trait_parents = vec![codec];
    assert_refusal(&reject(&[], &mut interface), NameErrorKind::CyclicCompiledTraitInheritance, "wire", "proto\\Codec", "cyclic trait inheritance");
}

#[test]
fn a_compiled_attached_domain_cannot_share_a_case_name_on_its_owner() {
    let (mut interface, _) = library();
    let ready = export(&mut interface, "proto\\Packet::Ready", NodeKind::Domain);
    ready.id = renamed(&ready.id, |name| name.path = "proto\\Packet::Empty".into());
    assert_refusal(
        &reject(&[], &mut interface),
        NameErrorKind::DuplicateCompiledMember,
        "wire",
        "proto\\Packet::Empty",
        "duplicate member proto\\Packet::Empty",
    );
}

#[test]
fn a_source_package_collides_with_its_compiled_exports_and_its_module_rows() {
    // An export collision names the first export in identity order.
    let (mut interface, _) = library();
    let error = parsed(&LIBRARY, |sources| reject(sources, &mut interface));
    assert_refusal(&error, NameErrorKind::CompiledPackageCollision, "wire", "net\\Link", "source package collides with a compiled interface");

    // Without exports, the first module row of the source package collides.
    let (mut interface, _) = library();
    interface.declarations.clear();
    let error = parsed(&LIBRARY, |sources| reject(sources, &mut interface));
    assert_refusal(&error, NameErrorKind::CompiledPackageCollision, "wire", "net", "source package collides with a compiled interface");

    // One source file of the package is enough.
    let (mut interface, _) = library();
    interface.declarations.clear();
    let error = parsed(&[("wire", "main.omg", "machine main() {}")], |sources| reject(sources, &mut interface));
    assert_refusal(&error, NameErrorKind::CompiledPackageCollision, "wire", "net", "source package collides with a compiled interface");
}
