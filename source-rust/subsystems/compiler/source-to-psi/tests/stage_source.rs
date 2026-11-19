//! Exercise the public source compilation boundary with independently acquired inputs.
use semantic_vocabulary::{PackageKeyIdentity, source_selections::ProductTarget};
use source_admission::{RetainedDeclaration, SourceOrigin, SourceResolutionStratum};
use source_compiler::declaration_selection::{
    DescribedProductEntry, ExecutedRootBinding, RootBindingSelection,
};
use source_compiler::product::ProductSourceInputs;
use source_compiler::source_inputs::SourceMap;
use source_compiler::{StageSource, compile};
use source_provenance::DependencyScope;
use source_text::{SourceId, SourceSpan, Span};
use std::sync::Arc;

fn captured(package: PackageKeyIdentity, text: &str) -> (SourceMap, SourceId) {
    let mut files = SourceMap::default();
    let id = files
        .add_checked_instance(
            "product/main.omg".into(),
            text,
            "product".into(),
            Some(package),
            SourceOrigin::User,
            SourceResolutionStratum::Base,
            DependencyScope::Product,
        )
        .source_id;
    (files, id)
}

fn product<'a>(
    files: SourceMap,
    root: SourceId,
    package: PackageKeyIdentity,
    selections: &'a [ExecutedRootBinding],
) -> StageSource<'a> {
    StageSource {
        files: Arc::new(files),
        roots: vec![root],
        imports: Vec::new(),
        selections: ProductSourceInputs {
            generated: None,
            targets: &[],
            calling_targets: &[],
            subject: Some(package),
            root_binding_requests: selections,
            policy: Default::default(),
            default_provider_selections: &[],
            native_bindings: Default::default(),
            layout_selection_authority: None,
            optimizations: Default::default(),
        },
        build_accounting: None,
        declaration_authority: None,
        build_host: ProductTarget::LinuxX86_64,
    }
}

#[test]
fn an_entryless_checked_product_publishes_reopenable_psi() {
    let package = PackageKeyIdentity::from_digest([0x61; 32]).unwrap();
    let (files, root) = captured(
        package,
        "const VALUE: u32 = value();
        machine value() -> u32 crashes none { 17 }
        machine main() -> u32 crashes none { VALUE }",
    );
    let program = compile(product(files, root, package, &[])).unwrap();
    let bytes = program.to_bytes();
    let reopened = terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes).unwrap();
    assert_eq!(reopened.to_bytes(), bytes);
}

#[test]
fn a_description_for_the_same_name_and_span_in_another_package_is_refused() {
    let text = "machine main() {}";
    let package = PackageKeyIdentity::from_digest([0x61; 32]).unwrap();
    let (files, root) = captured(package, text);
    let (other, other_root) = captured(PackageKeyIdentity::from_digest([0x62; 32]).unwrap(), text);
    let start = text.find("main").unwrap();
    let span = Span::new(start, start + "main".len());
    let declaration = RetainedDeclaration::from_source_occurrence(
        other.get(other_root).unwrap().clone(),
        span,
        symbols::SymbolKind::Machine,
    );
    let slot = "linux_x86_64::ProgramEntry";
    let executed = ExecutedRootBinding {
        source_span: SourceSpan::new(root, span),
        slot: slot.into(),
        implementation: RootBindingSelection::Described(DescribedProductEntry {
            declaration,
            machine_name: "main".into(),
            slot: slot.into(),
        }),
    };
    let diagnostics = compile(product(files, root, package, &[executed])).unwrap_err();
    assert!(
        diagnostics[0]
            .message
            .contains("retained declaration is absent or changed"),
        "{diagnostics:?}"
    );
}

#[test]
fn a_build_activation_rejoins_its_authored_machine_in_the_selected_package() {
    let package = PackageKeyIdentity::from_digest([0x61; 32]).unwrap();
    let (mut files, root) = captured(package, "machine main() {}");
    let build = files.add_checked_instance(
        "product/build.omg".into(), "machine build() {}", "product".into(), Some(package),
        SourceOrigin::User, SourceResolutionStratum::Base, DependencyScope::Build,
    ).source_id;
    let declaration = RetainedDeclaration::from_retained_parts(
        files.get(build).unwrap().clone(), Span::new(8, 13), symbols::SymbolKind::Machine,
    ).unwrap();
    let mut stage = product(files.clone(), root, package, &[]);
    stage.selections.policy.activation = Some(&declaration);
    compile(stage).unwrap();

    let mut changed = declaration.source().clone();
    changed.package_identity = Some(PackageKeyIdentity::from_digest([0x62; 32]).unwrap());
    let foreign = RetainedDeclaration::from_retained_parts(
        changed, declaration.span(), declaration.kind(),
    ).unwrap();
    let mut stage = product(files, root, package, &[]);
    stage.selections.policy.activation = Some(&foreign);
    let diagnostics = compile(stage).unwrap_err();
    assert!(diagnostics[0].message.contains("retained declaration is absent or changed"),
        "{diagnostics:?}");
}
