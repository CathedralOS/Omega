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

const OMEGA_REQUEST: &str = include_str!("../../../../../source/omega/application/ocreq/request.omg");
const OMEGA_OUTCOME: &str = include_str!("../../../../../source/omega/application/ocreq/outcome.omg");

#[test]
fn omega_compiler_storage_and_outcomes_publish_psi() {
    // Compile the product declarations directly so a broken hosted Build
    // cannot hide a regression in their field types or accessors.
    let sources = include_str!("../../../../../source/psi/foundation/source.omg");
    let (staging, _) = OMEGA_REQUEST.split_once("machine u32_at(").unwrap();
    let (storage, _) = sources
        .split_once("public machine source_set_add(")
        .unwrap();

    let package = PackageKeyIdentity::from_digest([0x61; 32]).unwrap();
    for (name, text) in [
        ("request staging", staging),
        ("source storage", storage),
        ("outcomes", OMEGA_OUTCOME),
    ] {
        let (files, root) = captured(package, text);
        compile(product(files, root, package, &[]))
            .unwrap_or_else(|errors| panic!("{name} could not publish Psi: {errors:?}"));
    }
}

#[test]
#[ignore = "checks the complete request decoder and encoder together, which takes several minutes"]
fn complete_omega_request_protocol_publishes_psi() {
    let package = PackageKeyIdentity::from_digest([0x61; 32]).unwrap();
    let (mut files, root) = captured(package, OMEGA_REQUEST);
    let outcome = files
        .add_checked_instance(
            "product/outcome.omg".into(),
            OMEGA_OUTCOME,
            "product".into(),
            Some(package),
            SourceOrigin::User,
            SourceResolutionStratum::Base,
            DependencyScope::Product,
        )
        .source_id;
    let mut stage = product(files, root, package, &[]);
    stage.roots.push(outcome);
    let program = compile(stage).unwrap();
    assert!(!program.to_bytes().is_empty());
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

#[test]
fn publication_checks_the_same_grammar_in_root_and_imported_files() {
    let main = "use kit::answer;
        machine main() -> u32 {
            transition true { true -> kit::answer(), false -> 0u32 }
        }";
    let helper = "machine id<T>(x: T) -> T { x }
        public machine answer() -> u32 {
            match true { true -> id::<u32>(7u32), false -> 0u32 }
        }";
    let package = PackageKeyIdentity::from_digest([0x63; 32]).unwrap();
    let publish = |main: &str, helper: &str| {
        let (mut files, root) = captured(package, main);
        let dependency = files
            .add_checked_instance(
                "product/kit/values.omg".into(),
                helper,
                "product".into(),
                Some(package),
                SourceOrigin::User,
                SourceResolutionStratum::Base,
                DependencyScope::Product,
            )
            .source_id;
        let mut stage = product(files, root, package, &[]);
        stage.roots.push(dependency);
        compile(stage)
    };

    assert!(!publish(main, helper).unwrap().to_bytes().is_empty());
    for (main, helper, expected) in [
        (
            main.to_owned(),
            format!("module kit;\n{helper}"),
            "`module` headers",
        ),
        (
            main.to_owned(),
            helper.replace("id::<u32>", "id<u32>"),
            "comparison",
        ),
        (
            main.to_owned(),
            helper.replace("7u32),", "7u32)"),
            "between arms",
        ),
        (
            main.replace("answer(),", "answer()"),
            helper.to_owned(),
            "between arms",
        ),
    ] {
        let diagnostics = publish(&main, &helper).unwrap_err();
        assert!(
            diagnostics.iter().any(|d| d.message.contains(expected)),
            "{diagnostics:?}"
        );
    }
}
