//! These controls parse real Build binding requests, then supply a completed
//! host event. They check retained admission, without executing a Build.

use super::*;
use language_semantics::declaration_selection::ProductTarget;
use source::{DependencyScope, SourceOrigin, SourceResolutionStratum};
use std::sync::Arc;

const PRELUDE: &str = r#"
public data Build { marker: u64; }
public boundary trait BuildBindingOperations {
    machine prepare(builder: &Build) -> u64 reaches none crashes none;
    machine bind_native<Locator>(activation: u64, target: u64, locator: Locator) reaches none crashes none;
}
public machine build_selection_receiver<'activation>(builder: &'activation mut Build) -> &'activation mut Build { builder }
public data BuildTarget [copy] { case LinuxX86_64; }
public const linux_x86_64: BuildTarget = BuildTarget::LinuxX86_64;
public machine build_selection_target(target: BuildTarget) -> u64 {
    match target { BuildTarget::LinuxX86_64 -> 1u64 }
}
"#;
const REQUIREMENT: &str =
    "public boundary trait Raw { machine read() -> u64 reaches none crashes none; }";

fn package(byte: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([byte; 32]).unwrap()
}

fn typed_requirement(
    requirement: &str,
    origin: SourceOrigin,
    owner: Option<PackageKeyIdentity>,
    stratum: SourceResolutionStratum,
    dependency: bool,
) -> TypedTrees {
    let path = if dependency {
        "dep::Raw::read"
    } else {
        "Raw::read"
    };
    let build = format!(
        "machine build(builder: &mut Build) {{\n\
         let locator: ForeignBinding<0, 0, 0> = ForeignBinding::Syscall {{ number: 9u64 }};\n\
         Build::bind<{path}>(builder, linux_x86_64, locator);\n}}"
    );
    let mut sources = source::SourceMap::default();
    if dependency {
        sources.retain_product_dependency_scope([(package(7), "dep", package(8))]);
    }
    let external = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../source/library/core/external_binding.omg"
    ));
    let inputs = [
        (
            "root/build.omg",
            build.as_str(),
            "root",
            Some(package(7)),
            SourceOrigin::User,
            SourceResolutionStratum::Base,
            DependencyScope::Build,
        ),
        (
            "<build-prelude>",
            PRELUDE,
            "root",
            Some(package(7)),
            SourceOrigin::Toolchain,
            SourceResolutionStratum::Base,
            DependencyScope::Build,
        ),
        (
            "<build-prelude:Build>",
            "module Build;\n",
            "root",
            Some(package(7)),
            SourceOrigin::Toolchain,
            SourceResolutionStratum::Base,
            DependencyScope::Build,
        ),
        (
            "source/library/core/external_binding.omg",
            external,
            "source/library/core",
            None,
            SourceOrigin::Toolchain,
            SourceResolutionStratum::Base,
            DependencyScope::Build,
        ),
        (
            "core/raw.omg",
            requirement,
            "core",
            owner,
            origin,
            stratum,
            DependencyScope::Product,
        ),
    ];
    let mut syntax = None;
    let mut ids = Vec::new();
    for (path, text, root, owner, origin, stratum, scope) in inputs {
        let id = sources
            .add_checked_instance(
                path.into(),
                text,
                root.into(),
                owner,
                origin,
                stratum,
                scope,
            )
            .source_id;
        ids.push(id);
        let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
        match &mut syntax {
            None => {
                syntax =
                    Some(tokens_to_syntax_trees::parse_syntax_trees_with_id(id, &tokens).unwrap())
            }
            Some(syntax) => {
                tokens_to_syntax_trees::parse(syntax, id, &tokens).unwrap();
            }
        }
    }
    let mut request =
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax.unwrap());
    request.sources = Some(Arc::new(sources));
    request.top_level_bindings = vec![
        symbols::SourceScopedTopLevelBinding::new(ids[0], ids[1], "Build"),
        symbols::SourceScopedTopLevelBinding::module_import(ids[0], ids[2], "Build", 0),
        symbols::SourceScopedTopLevelBinding::new(ids[0], ids[3], "ForeignBinding"),
        symbols::SourceScopedTopLevelBinding::new(ids[0], ids[3], "DllImport"),
    ];
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request).unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

fn capture(typed: &TypedTrees) -> (Vec<CapturedNativeDeclaration>, Vec<CapturedNativeIntent>) {
    let author = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "build")
        .unwrap()
        .symbol;
    let request = typed_trees::product_operand::collect_product_selection_occurrences(typed)
        .into_iter()
        .find(|occurrence| occurrence.request.operation == BuildOperation::NativeBinding)
        .unwrap()
        .request;
    let formation = native_binding_formation(typed, author, Some(author), request.clone()).unwrap();
    let event = ExecutedNativeBinding {
        request,
        target: ProductTarget::LinuxX86_64,
        locator: semantic_vocabulary::NativeBindingLocator::Syscall { number: 9 },
        selecting_machine: formation.author,
    };
    capture_package_intents(typed, package(7), &[formation], &[event]).unwrap()
}

fn toolchain_product() -> TypedTrees {
    typed_requirement(
        REQUIREMENT,
        SourceOrigin::Toolchain,
        None,
        SourceResolutionStratum::Base,
        false,
    )
}

fn anchor(typed: &TypedTrees) -> symbols::RetainedDeclaration {
    let owner = typed
        .traits()
        .iter()
        .find(|owner| typed.symbols.display_path(owner.symbol, "::") == "Raw")
        .unwrap();
    symbols::RetainedDeclaration::capture(
        &typed.symbols,
        typed.trait_machine_signatures(owner)[0].symbol,
    )
    .unwrap()
}

fn assert_refusal(
    typed: &mut TypedTrees,
    declarations: &[CapturedNativeDeclaration],
    intents: &[CapturedNativeIntent],
    reason: &str,
) {
    let errors = collect_captured(typed, declarations, intents)
        .expect_err("the captured binding must reject");
    assert!(
        errors.iter().any(|error| error.message.contains(reason)),
        "expected {reason}: {errors:?}"
    );
}

#[test]
fn captured_public_toolchain_product_binding_keeps_its_supplier() {
    let (declarations, intents) = capture(&toolchain_product());
    let mut receiving = toolchain_product();
    let selections = collect_captured(&mut receiving, &declarations, &intents).unwrap();
    let [selection] = selections.as_slice() else {
        panic!("one executed binding must survive")
    };
    assert_eq!(
        receiving
            .symbols
            .symbol_product_package_identity(selection.requirement),
        None
    );
    assert_eq!(selection.supplier_package, package(7));
    assert_eq!(selection.producer, declarations[0].source.producer);
    assert_eq!(
        selection.source_occurrence_digest,
        declarations[0].source.source_occurrence_digest
    );
    assert_eq!(selection.target, intents[0].target);
    assert_eq!(selection.locator, intents[0].locator);
}

#[test]
fn captured_native_bindings_preserve_package_and_dependency_ownership() {
    for dependency in [false, true] {
        let owner = if dependency { package(8) } else { package(7) };
        let mut typed = typed_requirement(
            REQUIREMENT,
            SourceOrigin::User,
            Some(owner),
            SourceResolutionStratum::Base,
            dependency,
        );
        let (declarations, intents) = capture(&typed);
        assert_eq!(
            declarations[0].dependency_alias.as_deref(),
            dependency.then_some("dep")
        );
        let selections = collect_captured(&mut typed, &declarations, &intents).unwrap();
        assert_eq!(selections.len(), 1);
        assert_eq!(
            typed
                .symbols
                .symbol_product_package_identity(selections[0].requirement),
            Some(owner)
        );
    }
}

#[test]
fn captured_toolchain_binding_does_not_authorize_other_ownerless_sources() {
    let (declarations, intents) = capture(&toolchain_product());
    // Even a receipt naming the exact receiving declaration cannot make a
    // private, user-authored, contract-only or temporally hidden source visible.
    for (text, origin, stratum) in [
        (
            "boundary trait Raw { machine read() -> u64 reaches none crashes none; }",
            SourceOrigin::Toolchain,
            SourceResolutionStratum::Base,
        ),
        (
            REQUIREMENT,
            SourceOrigin::User,
            SourceResolutionStratum::Base,
        ),
        (
            REQUIREMENT,
            SourceOrigin::ToolchainContract,
            SourceResolutionStratum::Base,
        ),
        (
            REQUIREMENT,
            SourceOrigin::Toolchain,
            SourceResolutionStratum::CurrentActivationExtension,
        ),
    ] {
        let mut receiving = typed_requirement(text, origin, None, stratum, false);
        let mut forged = declarations.clone();
        forged[0].requirement = anchor(&receiving);
        assert_refusal(
            &mut receiving,
            &forged,
            &intents,
            "not visible under its original Product package and temporal scope",
        );
    }
    let mut receiving = typed_requirement(
        REQUIREMENT,
        SourceOrigin::Toolchain,
        None,
        SourceResolutionStratum::Base,
        true,
    );
    let mut forged = declarations.clone();
    forged[0].source.requirement_path = "dep::Raw::read".into();
    forged[0].dependency_alias = Some("dep".into());
    assert_refusal(
        &mut receiving,
        &forged,
        &intents,
        "not visible under its original Product package and temporal scope",
    );
}

#[test]
fn captured_toolchain_binding_keeps_complete_source_and_signature_custody() {
    let (declarations, intents) = capture(&toolchain_product());
    let mut receiving = toolchain_product();
    let mut changed = declarations.clone();
    changed[0].requirement_identity.push_str(" changed");
    assert_refusal(
        &mut receiving,
        &changed,
        &intents,
        "changed its retained complete requirement identity",
    );
    let mut changed = declarations.clone();
    changed[0].dependency_alias = Some("dep".into());
    assert_refusal(
        &mut receiving,
        &changed,
        &intents,
        "changed its original direct Product dependency scope",
    );
    for changed_source in [
        {
            let mut source = declarations[0].requirement.source().clone();
            source.origin = SourceOrigin::ToolchainContract;
            source
        },
        {
            let mut source = declarations[0].requirement.source().clone();
            source.source = format!("{}\n", source.source).into();
            source
        },
        {
            let mut source = declarations[0].requirement.source().clone();
            source.dependency_scope = DependencyScope::Build;
            source
        },
    ] {
        let mut changed = declarations.clone();
        let original = &declarations[0].requirement;
        changed[0].requirement = symbols::RetainedDeclaration::from_retained_parts(
            changed_source,
            original.span(),
            original.kind(),
        )
        .unwrap();
        assert_refusal(
            &mut receiving,
            &changed,
            &intents,
            "retained declaration is absent or changed",
        );
    }
    let original = &declarations[0].requirement;
    let mut shortened = original.span();
    shortened.end -= 1;
    for (span, kind) in [
        (shortened, original.kind()),
        (original.span(), symbols::SymbolKind::Data),
    ] {
        let mut changed = declarations.clone();
        changed[0].requirement = symbols::RetainedDeclaration::from_retained_parts(
            original.source().clone(),
            span,
            kind,
        )
        .unwrap();
        assert_refusal(
            &mut receiving,
            &changed,
            &intents,
            "retained declaration is absent or changed",
        );
    }
}
