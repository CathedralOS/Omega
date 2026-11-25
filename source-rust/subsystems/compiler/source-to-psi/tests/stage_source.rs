//! Exercise the public source compilation boundary with independently acquired inputs.
use declaration_scope::ProductDeclarationScope;
use semantic_vocabulary::{PackageKeyIdentity, source_selections::ProductTarget};
use source_admission::{RetainedDeclaration, SourceOrigin, SourceResolutionStratum};
use source_compiler::representations::declaration_selection::{
    DescribedProductEntry, ProductEntryBinding, RootBindingSelection,
};
use source_compiler::compile_product::ProductSourceInputs;
use source_compiler::representations::source_inputs::SourceMap;
use source_compiler::representations::source_symbols::SourceScopedTopLevelBinding;
use source_compiler::{StageSource, compile};
use source_provenance::DependencyScope;
use source_text::{SourceId, SourceSpan, Span};
use std::path::Path;
use std::sync::Arc;

fn capture_file(
    files: &mut SourceMap,
    owner: &str,
    identity: PackageKeyIdentity,
    name: &str,
    text: &str,
) -> SourceId {
    files
        .add_checked_instance(
            Path::new(owner).join(name),
            text,
            owner.into(),
            Some(identity),
            SourceOrigin::User,
            SourceResolutionStratum::Base,
            DependencyScope::Product,
        )
        .source_id
}

fn captured(package: PackageKeyIdentity, text: &str) -> (SourceMap, SourceId) {
    let mut files = SourceMap::default();
    let id = capture_file(&mut files, "product", package, "main.omg", text);
    (files, id)
}

fn product<'a>(
    files: SourceMap,
    root: SourceId,
    package: PackageKeyIdentity,
    selections: &'a [ProductEntryBinding],
) -> Result<StageSource<'a>, Vec<diagnostics::Diagnostic>> {
    Ok(StageSource {
        parsed: Box::new(source_compiler::ParsedStageSources::parse(
            Arc::new(files),
            vec![root],
            vec![],
        )?),
        frontends: Default::default(),
        product: ProductSourceInputs {
            build_accounting: None,
            declaration_scope: None,
            build_host: ProductTarget::LinuxX86_64,
            generated: None,
            targets: &[],
            calling_targets: &[],
            subject: Some(package),
            root_binding_requests: selections,
            policy: Default::default(),
            default_provider_selections: &[],
            native_bindings: Default::default(),
            optimizations: Default::default(),
        },
    })
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
    let program = compile(product(files, root, package, &[]).unwrap(), None).unwrap();
    let bytes = program.to_bytes();
    let reopened = terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes).unwrap();
    assert_eq!(reopened.to_bytes(), bytes);
}

#[test]
fn a_requirement_reads_nested_nominal_arguments_in_its_own_type_scope() {
    let package = PackageKeyIdentity::from_digest([0x72; 32]).unwrap();
    let source = r#"
        data Reading {}
        data Writing {}
        data Stamp<T> { value: u32; }
        boundary trait Echo<T> {
            machine carry(input: Stamp<Stamp<T>>) -> Stamp<Stamp<T>> reaches none crashes none;
        }
        First satisfies Echo<Reading> {
            machine carry(input: Stamp<Stamp<Reading>>) -> Stamp<Stamp<Reading>> { input }
        }
        Second satisfies Echo<Writing> {
            machine carry(input: Stamp<Stamp<Writing>>) -> Stamp<Stamp<Writing>> { input }
        }
        machine main() {}
    "#;
    let check = |source: &str| {
        let (files, root) = captured(package, source);
        source_compiler::check(
            product(files, root, package, &[]).unwrap(),
            &mut Default::default(),
        )
    };
    check(source).unwrap();
    let wrong = source.replace(
        "-> Stamp<Stamp<Writing>> { input }",
        "-> Stamp<Stamp<Reading>> { input }",
    );
    let errors = check(&wrong).expect_err("the second provider promises a different record");
    assert!(
        errors.iter().any(|error| error.message.contains("differs from its requirement")),
        "{errors:?}"
    );
}

#[test]
fn a_requirement_keeps_its_domain_index_inside_a_record_result() {
    let package = PackageKeyIdentity::from_digest([0x73; 32]).unwrap();
    let source = r#"
        data Read {}
        data Write {}
        data Token { value: u32; }
        domain<P> Token::Issued<P>;
        data Receipt<T> { value: T; }
        boundary trait Forward<P> {
            machine pass(input: Receipt<Token in Issued<P>>) -> Receipt<Token in Issued<P>>
                reaches none crashes none;
        }
        Reader satisfies Forward<Read> {
            machine pass(input: Receipt<Token in Issued<Read>>) -> Receipt<Token in Issued<Read>> { input }
        }
        Writer satisfies Forward<Write> {
            machine pass(input: Receipt<Token in Issued<Write>>) -> Receipt<Token in Issued<Write>> { input }
        }
        machine main() {}
    "#;
    let check = |source: &str| {
        let (files, root) = captured(package, source);
        source_compiler::check(
            product(files, root, package, &[]).unwrap(),
            &mut Default::default(),
        )
    };
    check(source).unwrap();
    let wrong = source.replace(
        "-> Receipt<Token in Issued<Write>> {",
        "-> Receipt<Token in Issued<Read>> {",
    );
    let errors = check(&wrong).expect_err("the writer returns a receipt issued to another policy");
    assert!(
        errors.iter().any(|error| error.message.contains("differs from its requirement")),
        "{errors:?}"
    );
}

#[test]
fn inherited_provider_methods_keep_the_declaring_application_and_result() {
    let package = PackageKeyIdentity::from_digest([0x72; 32]).unwrap();
    let source = "public boundary trait Reading<T> {
            machine read() -> T reaches none crashes none;
        }
        public boundary trait OpenChild<T>: Reading<T> {}
        public boundary trait Narrow: Reading<u32> {}
        public NarrowSupply satisfies Narrow {
            machine read() -> u32 reaches none crashes none { 17u32 }
        }
        public WideSupply satisfies Reading<u64> {
            machine read() -> u64 reaches none crashes none { 29u64 }
        }";
    let (files, root) = captured(package, source);
    let artifact = compile(product(files, root, package, &[]).unwrap(), None).unwrap();
    let module = artifact.semantic_module();
    let candidates = &module.interfaces.provider_candidates;
    assert_eq!(candidates.len(), 2);
    assert_eq!(
        candidates[0].source_requirement_identity,
        candidates[1].source_requirement_identity
    );
    assert_ne!(
        candidates[0].receiving_application,
        candidates[1].receiving_application
    );
    for candidate in candidates {
        assert_eq!(
            candidate.declaration_application,
            candidate.receiving_application
        );
        let boundary = module.boundary_machine(candidate.boundary).unwrap();
        let service = boundary.owning_service.unwrap();
        let receiver = module
            .interfaces
            .services
            .iter()
            .find(|row| row.id == service)
            .unwrap();
        assert_eq!(receiver.identity, candidate.receiving_application);
    }

    let wrong = source.replace(
        "-> u32 reaches none crashes none { 17u32 }",
        "-> u64 reaches none crashes none { 17u64 }",
    );
    let (files, root) = captured(package, &wrong);
    assert!(compile(product(files, root, package, &[]).unwrap(), None).is_err());
}

#[test]
fn selected_requirement_reach_keeps_its_receiver_arguments() {
    let package = PackageKeyIdentity::from_digest([0x72; 32]).unwrap();
    let source = "public boundary trait Reading<T> {
            machine read() -> u64 reaches Reading<T> crashes none;
        }
        public Narrow satisfies Reading<u32> {
            machine read() -> u64 reaches none crashes none { 17u64 }
        }
        public Wide satisfies Reading<u64> {
            machine read() -> u64 reaches none crashes none { 29u64 }
        }";
    let (files, root) = captured(package, source);
    let artifact = compile(product(files, root, package, &[]).unwrap(), None).unwrap();
    let module = artifact.semantic_module();
    let candidates = &module.interfaces.provider_candidates;
    assert_eq!(candidates.len(), 2);
    let mut applications = Vec::new();
    for candidate in candidates {
        let boundary = module.boundary_machine(candidate.boundary).unwrap();
        let receiver = boundary.owning_service.unwrap();
        assert_eq!(boundary.published_service_ceiling, [receiver]);
        assert!(boundary.published_synchronous_entries.is_empty());
        let selected = module.machine(candidate.candidate).unwrap();
        assert_eq!(selected.contract.published_service_ceiling, [receiver]);
        applications.push(receiver);
    }
    assert_ne!(applications[0], applications[1]);
}

#[test]
fn const_service_terms_are_values_in_the_callers_scope() {
    let package = PackageKeyIdentity::from_digest([0x72; 32]).unwrap();
    let source = "public boundary trait Sensor<const Id: u64> {
            machine read() -> u64 reaches none crashes none;
        }
        public boundary trait Reader {
            machine poll() reaches Sensor<17u64> + Sensor<29u64> invokes Sensor<17u64> crashes none;
        }
        public machine published() reaches Sensor<17u64> + Sensor<29u64> invokes Sensor<17u64> crashes none {}";
    let (files, root) = captured(package, source);
    let artifact = compile(product(files, root, package, &[]).unwrap(), None).unwrap();
    let module = artifact.semantic_module();
    let published = module
        .machines
        .iter()
        .find(|machine| machine.contract.published_service_ceiling.len() == 2)
        .unwrap();
    let reachable = &published.contract.published_service_ceiling;
    let entries = &published.contract.published_synchronous_entries;
    assert_eq!(entries.len(), 1);
    assert!(reachable.contains(&entries[0]));
    let receiving = module
        .interfaces
        .services
        .iter()
        .find(|service| service.id == entries[0])
        .unwrap();
    let identity = module.identities.text(receiving.identity).unwrap();
    assert!(identity.contains("arguments(integer(17))"), "{identity}");
}

#[test]
fn a_generic_call_forwards_the_callers_const_service_argument() {
    let package = PackageKeyIdentity::from_digest([0x72; 32]).unwrap();
    let source = "public boundary trait Sensor<const Id: u64> {
            machine read() -> u64 reaches Sensor<Id> crashes none;
        }
        public machine poll<const K: u64>() -> u64 reaches Sensor<K> crashes none { Sensor::<K>::read() }
        public machine first() -> u64 reaches Sensor<17u64> crashes none { poll::<17u64>() }
        public machine second() -> u64 reaches Sensor<29u64> crashes none { poll::<29u64>() }";
    let (files, root) = captured(package, source);
    compile(product(files, root, package, &[]).unwrap(), None).unwrap();

    let wrong = source.replace(
        "second() -> u64 reaches Sensor<29u64>",
        "second() -> u64 reaches Sensor<17u64>",
    );
    let (files, root) = captured(package, &wrong);
    assert!(compile(product(files, root, package, &[]).unwrap(), None).is_err());
}

#[test]
fn invocation_ceiling_follows_callable_parameter_aliases() {
    let package = PackageKeyIdentity::from_digest([0x61; 32]).unwrap();
    let check = |ceiling: &str, body: &str| {
        let source = format!(
            "data Callback = machine() -> u64 reaches none crashes none;
             machine forward(selected: Callback) -> u64 invokes selected {{ selected() }}
             machine probe(other: Callback, selected: Callback) -> u64 {ceiling} {{ {body} }}"
        );
        let (files, root) = captured(package, &source);
        compile(product(files, root, package, &[]).unwrap(), None)
    };
    let alias = "let alias: Callback = selected; alias()";
    let forwarded = "let first: Callback = selected; let second: Callback = first; forward(second)";
    let arm = "let saved: Callback = selected; transition true { true -> saved(), _ -> 7u64 }";
    check("invokes selected", arm).unwrap();
    check("invokes selected", alias).unwrap();
    let rebound = "let mutable slot: Callback = other; slot = selected; forward(slot)";
    for body in ["selected()", alias, forwarded, rebound, arm] {
        let artifact = check("", body).unwrap();
        let module = artifact.semantic_module();
        let probe = module
            .machines
            .iter()
            .find(|machine| machine.contract.parameter_order.len() == 2)
            .unwrap();
        let ceiling = module
            .execution
            .callables
            .machine_envelope(probe.id)
            .unwrap();
        assert_eq!(
            ceiling.invocations,
            [terminal_psi::TerminalCallableInvocation::Parameter(1)]
        );
    }
    check(
        "invokes other",
        "let mutable slot: Callback = other; let saved: Callback = slot;
         slot = selected; forward(saved)",
    )
    .unwrap();
    check("invokes other", "let alias: Callback = selected; 7u64").unwrap();
    for body in [alias, forwarded, arm] {
        let errors = check("invokes other", body).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("invokes")
                    && error.message.contains("parameter 2")),
            "{errors:?}"
        );
    }
}

#[test]
fn a_transition_calls_the_lexical_value_with_its_declared_arguments() {
    let package = PackageKeyIdentity::from_digest([0x61; 32]).unwrap();
    let check = |kind: &str, body: &str| {
        let source = format!(
            "data Read = machine(value: u8) -> u8 reaches none crashes none;
             machine read(value: bool) -> u8 {{ 0u8 }}
             machine probe(read: {kind}) -> u8 {{ {body} }}"
        );
        let (files, root) = captured(package, &source);
        compile(product(files, root, package, &[]).unwrap(), None)
    };
    check("Read", "transition true { true -> read(7u8), _ -> 0u8 }").unwrap();
    for (kind, argument) in [("Read", "true"), ("u8", "7u8")] {
        let call = format!("read({argument})");
        let expression = check(kind, &call).unwrap_err();
        let arm = check(
            kind,
            &format!("transition true {{ true -> {call}, _ -> 0u8 }}"),
        )
        .unwrap_err();
        assert_eq!(arm[0].message, expression[0].message);
    }
}

#[test]
fn selected_members_keep_the_callable_type_they_share() {
    let package = PackageKeyIdentity::from_digest([0x61; 32]).unwrap();
    let (files, root) = captured(
        package,
        include_str!("../../../../../samples/cli/facts/dynamic_trait_dispatch/local/main.omg"),
    );
    compile(product(files, root, package, &[]).unwrap(), None).unwrap();
}

const OMEGA_REQUEST: &str =
    include_str!("../../../../../source/subsystems/compiler/request/request.omg");
const OMEGA_OUTCOME: &str =
    include_str!("../../../../../source/subsystems/compiler/request/outcome.omg");

#[test]
fn omega_compiler_storage_and_outcomes_publish_psi() {
    // Compile the product declarations directly so a broken hosted Build
    // cannot hide a regression in their field types or accessors.
    let sources = include_str!(
        "../../../../../source/subsystems/compiler/source_to_psi/representations/source_files/source.omg"
    );
    let (storage, _) = sources
        .split_once("public machine source_set_add(")
        .unwrap();

    let package = PackageKeyIdentity::from_digest([0x61; 32]).unwrap();
    for (name, text) in [
        ("request staging", OMEGA_REQUEST),
        ("source storage", storage),
        ("outcomes", OMEGA_OUTCOME),
    ] {
        let (files, root) = captured(package, text);
        compile(product(files, root, package, &[]).unwrap(), None)
            .unwrap_or_else(|errors| panic!("{name} could not publish Psi: {errors:?}"));
    }
}

#[test]
#[ignore = "checks the complete request decoder and encoder together, which takes several minutes"]
fn complete_omega_request_protocol_publishes_psi() {
    let package = PackageKeyIdentity::from_digest([0x61; 32]).unwrap();
    let wire_package = PackageKeyIdentity::from_digest([0x62; 32]).unwrap();
    let mut files = SourceMap::default();
    let request = capture_file(
        &mut files,
        "compiler_request",
        package,
        "request.omg",
        OMEGA_REQUEST,
    );
    let outcome = capture_file(
        &mut files,
        "compiler_request",
        package,
        "outcome.omg",
        OMEGA_OUTCOME,
    );
    let protocol = capture_file(
        &mut files,
        "compiler_request",
        package,
        "protocol.omg",
        include_str!("../../../../../source/subsystems/compiler/request/protocol.omg"),
    );
    let wire = capture_file(
        &mut files,
        "wire",
        wire_package,
        "codec.omg",
        include_str!("../../../../../source/foundation/wire/codec.omg"),
    );
    let mut stage = product(files, request, package, &[]).unwrap();
    let scope = ProductDeclarationScope::from_direct_dependencies(
        &[package, wire_package],
        [(package, wire_package)],
    );
    stage.product.declaration_scope = Some(Arc::new(scope));
    stage.parsed.roots.extend([outcome, protocol, wire]);
    stage
        .parsed
        .imports
        .push(SourceScopedTopLevelBinding::logical_module_import(
            protocol,
            wire,
            "wire::u32_from_le_bytes",
            1,
        ));
    let program = compile(stage, None).unwrap();
    assert!(!program.to_bytes().is_empty());
}

#[test]
fn omega_frontend_imports_a_member_of_the_lowered_program_representation() {
    let customer = PackageKeyIdentity::from_digest([0x61; 32]).unwrap();
    let psi = PackageKeyIdentity::from_digest([0x62; 32]).unwrap();
    let import = "psi::representations::lowered_program::ScalarParameter";
    let check = |body: &str| {
        let (mut files, root) = captured(customer, &format!("use {import};\n{body}"));
        let scalar = capture_file(
            &mut files,
            "psi",
            psi,
            "representations/lowered_program/scalar.omg",
            include_str!(
                "../../../../../source/subsystems/compiler/source_to_psi/representations/lowered_program/scalar.omg"
            ),
        );
        let layouts = capture_file(
            &mut files,
            "psi",
            psi,
            "representations/lowered_program/layouts.omg",
            include_str!(
                "../../../../../source/subsystems/compiler/source_to_psi/representations/lowered_program/layouts.omg"
            ),
        );
        let mut stage = product(files, root, customer, &[]).unwrap();
        let scope =
            ProductDeclarationScope::from_direct_dependencies(&[customer, psi], [(customer, psi)]);
        stage.product.declaration_scope = Some(Arc::new(scope));
        stage.parsed.roots.extend([scalar, layouts]);
        stage
            .parsed
            .imports
            .push(SourceScopedTopLevelBinding::logical_module_import(
                root, scalar, import, 1,
            ));
        compile(stage, None)
    };
    let artifact =
        check("machine handle(p: ScalarParameter) -> u64 in Trapping { p.type_handle }").unwrap();
    assert!(!artifact.to_bytes().is_empty());
    let errors =
        check("machine handle(p: ScalarLayout) -> u64 in Trapping { p.size }").unwrap_err();
    assert!(
        errors.iter().any(|d| d
            .message
            .contains("an unknown type name is an error wherever it appears: `ScalarLayout`")),
        "{errors:?}"
    );
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
    let executed = ProductEntryBinding {
        source_span: SourceSpan::new(root, span),
        slot: slot.into(),
        implementation: RootBindingSelection::Described(DescribedProductEntry {
            declaration,
            machine_name: "main".into(),
            slot: slot.into(),
        }),
    };
    let diagnostics = compile(product(files, root, package, &[executed]).unwrap(), None).unwrap_err();
    assert!(
        diagnostics[0]
            .message
            .contains("retained declaration is absent or changed"),
        "{diagnostics:?}"
    );
}

#[test]
fn build_reports_each_unbound_import_at_its_authored_span_before_typing() {
    let text = "use first;\nuse second;\nmachine build() -> u8 { true }";
    let package = PackageKeyIdentity::from_digest([0x61; 32]).unwrap();
    let mut files = SourceMap::default();
    let root = files
        .add_checked_instance(
            "project/build.omg".into(),
            text,
            "project".into(),
            Some(package),
            SourceOrigin::User,
            SourceResolutionStratum::Base,
            DependencyScope::Build,
        )
        .source_id;
    let span = |part: &str| {
        let start = text.find(part).unwrap();
        Span::new(start, start + part.len())
    };
    let missing = [
        ("first", span("use first;")),
        ("second", span("use second;")),
    ];
    let stage = source_compiler::BuildStageSource {
        parsed: source_compiler::ParsedStageSources::parse(Arc::new(files), vec![root], vec![])
            .unwrap(),
        build_sources: [root].into(),
        pending_imports: missing
            .iter()
            .map(|(name, span)| source_compiler::PendingBuildImport {
                source: root,
                span: *span,
                target: source_compiler::SourceImportTarget::Namespace(vec![(*name).into()]),
            })
            .collect(),
        entry: SourceSpan::new(root, span("build")),
    };
    let Err(errors) = stage.prepare(&Default::default(), "linux_x86_64", None) else {
        panic!("a Build with unbound imports was prepared");
    };
    assert_eq!(errors.len(), missing.len());
    for (error, (name, span)) in errors.iter().zip(missing) {
        assert_eq!(
            error.message,
            format!("Build import `{name}` has no supplied source")
        );
        assert_eq!(error.source_span, Some(SourceSpan::new(root, span)));
    }
}

fn typed_build(
    files: SourceMap,
    entry: SourceSpan,
) -> Result<Arc<typed_trees::TypedTrees>, Vec<diagnostics::Diagnostic>> {
    let stage = source_compiler::BuildStageSource {
        parsed: source_compiler::ParsedStageSources::parse(
            Arc::new(files),
            vec![entry.source_id],
            vec![],
        )?,
        build_sources: [entry.source_id].into(),
        pending_imports: vec![],
        entry,
    };
    Ok(stage
        .prepare(&Default::default(), "linux_x86_64", None)?
        .program)
}

#[test]
fn stored_complete_parses_reopen_and_partial_build_parses_are_refused() {
    let id = SourceId(5);
    let entry = SourceSpan::new(id, Span::new(8, 13));
    let limits = record_storage::RecordLimits::default();
    let old_parse = |entry: Option<SourceSpan>, can_return: bool| {
        record_storage::encode(
            &(
                syntax_trees::SyntaxTrees::new(id),
                entry,
                can_return,
                Vec::<()>::new(),
            ),
            limits,
        )
        .unwrap()
    };
    let clean = old_parse(None, false);
    let reopened = record_storage::decode::<source_compiler::SourceSyntax>(&clean, limits).unwrap();
    assert_eq!(reopened, source_compiler::SourceSyntax::new(id));
    assert_eq!(record_storage::encode(&reopened, limits).unwrap(), clean);
    for partial in [old_parse(Some(entry), false), old_parse(None, true)] {
        let error =
            record_storage::decode::<source_compiler::SourceSyntax>(&partial, limits).unwrap_err();
        assert!(error.reason.contains("only part of its source"), "{error}");
    }
}

#[test]
fn a_projected_snapshot_cannot_supply_a_complete_build_invocation() {
    let text = "machine build() {}";
    let package = PackageKeyIdentity::from_digest([0x61; 32]).unwrap();
    let mut files = SourceMap::default();
    let build = files
        .add_checked_instance(
            "project/build.omg".into(),
            text,
            "project".into(),
            Some(package),
            SourceOrigin::User,
            SourceResolutionStratum::Base,
            DependencyScope::Build,
        )
        .source_id;
    let start = text.find("build").unwrap();
    let entry = SourceSpan::new(build, Span::new(start, start + "build".len()));
    let mut program = Arc::unwrap_or_clone(typed_build(files, entry).unwrap());
    let machine = program.machines().at(0).symbol;
    program.roots.partial_build_entry = Some(entry);
    let Err(failure) = source_compiler::CheckedBuildInvocation::judge(
        &program,
        machine,
        &[Some(package)],
        None,
        None,
    ) else {
        panic!("a partial snapshot supplied a complete Build invocation");
    };
    assert_eq!(failure.diagnostics.len(), 1);
    assert!(
        failure.diagnostics[0]
            .message
            .contains("older Build snapshot")
    );
}

fn publish_build(
    files: SourceMap,
    entry: SourceSpan,
    package: PackageKeyIdentity,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let typed = typed_build(files, entry)?;
    let machine = typed
        .machines()
        .iter()
        .find(|machine| typed.symbols.symbol_source_span(machine.symbol) == Some(entry))
        .expect("the parsed Build contains the selected machine");
    let judged = source_compiler::CheckedBuildInvocation::judge(
        &typed,
        machine.symbol,
        &[Some(package)],
        None,
        None,
    )
    .map_err(|failure| failure.diagnostics)?;
    judged
        .invocation
        .prepare_publication()?
        .publish()
        .map(|_| ())
}

#[test]
fn build_and_product_check_the_return_value_before_publication() {
    let package = PackageKeyIdentity::from_digest([0x61; 32]).unwrap();
    for (returned, satisfies_contract) in [("7u32", true), ("8u32", false)] {
        let text = format!("machine build() -> u32 ensures result == 7u32 {{ {returned} }}");
        let (mut files, root) = captured(package, &text);
        let build = files
            .add_checked_instance(
                "project/build.omg".into(),
                text.as_str(),
                "project".into(),
                Some(package),
                SourceOrigin::User,
                SourceResolutionStratum::Base,
                DependencyScope::Build,
            )
            .source_id;
        let start = text.find("build").unwrap();
        let entry = SourceSpan::new(build, Span::new(start, start + "build".len()));
        let product = compile(product(files.clone(), root, package, &[]).unwrap(), None);
        let invocation = publish_build(files, entry, package);

        for (source, result) in [(root, product.map(|_| ())), (build, invocation)] {
            if satisfies_contract {
                result.unwrap();
                continue;
            }
            let errors = result.expect_err("returning 8 breaks the promise to return 7");
            let diagnostic = errors
                .iter()
                .find(|row| row.message.contains("ensures"))
                .unwrap_or_else(|| panic!("expected an unproved ensures clause: {errors:?}"));
            assert_eq!(
                diagnostic.stage,
                Some(diagnostics::DiagnosticStage::Checking)
            );
            let start = text.rfind(returned).unwrap();
            let span = Span::new(start, start + returned.len());
            assert_eq!(diagnostic.source_span, Some(SourceSpan::new(source, span)));
        }
    }
}

#[test]
fn a_build_activation_rejoins_its_authored_machine_in_the_selected_package() {
    let package = PackageKeyIdentity::from_digest([0x61; 32]).unwrap();
    let (mut files, root) = captured(package, "machine main() {}");
    let build = files
        .add_checked_instance(
            "product/build.omg".into(),
            "machine build() {}",
            "product".into(),
            Some(package),
            SourceOrigin::User,
            SourceResolutionStratum::Base,
            DependencyScope::Build,
        )
        .source_id;
    let declaration = RetainedDeclaration::from_retained_parts(
        files.get(build).unwrap().clone(),
        Span::new(8, 13),
        symbols::SymbolKind::Machine,
    )
    .unwrap();
    let mut stage = product(files.clone(), root, package, &[]).unwrap();
    stage.product.policy.activation = Some(&declaration);
    compile(stage, None).unwrap();

    let mut changed = declaration.source().clone();
    changed.package_identity = Some(PackageKeyIdentity::from_digest([0x62; 32]).unwrap());
    let foreign =
        RetainedDeclaration::from_retained_parts(changed, declaration.span(), declaration.kind())
            .unwrap();
    let mut stage = product(files, root, package, &[]).unwrap();
    stage.product.policy.activation = Some(&foreign);
    let diagnostics = compile(stage, None).unwrap_err();
    assert!(
        diagnostics[0]
            .message
            .contains("retained declaration is absent or changed"),
        "{diagnostics:?}"
    );
}

#[test]
fn publication_checks_the_same_grammar_in_root_and_imported_files() {
    let main = "use kit\\answer;
        machine main() -> u32 {
            transition true { true -> kit\\answer(), false -> 0u32 }
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
        let mut stage = product(files, root, package, &[])?;
        stage.parsed.roots.push(dependency);
        compile(stage, None)
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

#[test]
fn an_erased_array_argument_still_requires_an_integer_extent() {
    let package = PackageKeyIdentity::from_digest([0x45; 32]).unwrap();
    let compile_source = |declarations: &str, formals: &str, extent: &str| {
        let text = format!(
            "data Word {{ bits: u32; }} data Plain<T> = Word;
            {declarations}
            machine f{formals}(v: Plain<[u8; {extent}]>) -> Word {{ v }}"
        );
        let (files, root) = captured(package, &text);
        compile(product(files, root, package, &[]).unwrap(), None)
    };

    for (declarations, formals, extent) in [
        ("", "", "3"),
        ("const N: u8 = 3u8;", "", "N"),
        ("machine size() -> u8 crashes none { 3u8 }", "", "size()"),
        ("", "<const N: u8>", "N"),
        ("data Width = u8;", "<const N: Width>", "N"),
    ] {
        compile_source(declarations, formals, extent).unwrap();
    }
    for (declarations, formals, extent) in [
        ("", "", "false"),
        (
            "machine size() -> bool crashes none { false }",
            "",
            "size()",
        ),
        ("", "<const N: bool>", "N"),
        ("data Flag = bool;", "<const N: Flag>", "N"),
    ] {
        let diagnostics = compile_source(declarations, formals, extent).unwrap_err();
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("a fixed-array extent requires an integer, found `bool`")),
            "{diagnostics:?}"
        );
    }
}

/// A constant computed by a call gives an array its length, and the public
/// compiler checks that length where another machine requires a different
/// one. `|east|::N` and `|west|::N` share a last name, so each case also fails if
/// an extent took the other module's value.
///
/// `wiki/spec/language/constants.md`, "Evaluation": "A constant position
/// requests semantic evaluation." The handoffs this program crosses are
/// tested one stage at a time in
/// `pipeline/04_typed-trees-to-prepared-typed-trees/tests/computed_array_extents.rs`
/// and `pipeline/05_prepared-typed-trees-to-lowered-program/tests/prepared_array_extents.rs`.
#[test]
fn a_computed_array_extent_is_refused_where_another_length_is_required() {
    let main = r"use east/value.omg as east;
use west/value.omg as west;
machine pair(values: @[u8; 2]) -> u8 { values[0u64] }
machine triple(values: @[u8; 3]) -> u8 { values[0u64] }
machine choose(values: @[u8; |east|::N]) -> u8 { pair(values) }
machine other(values: @[u8; |west|::N]) -> u8 { triple(values) }";
    let package = PackageKeyIdentity::from_digest([0x58; 32]).unwrap();
    let publish = |east: u64, west: u64| {
        let (mut files, root) = captured(package, main);
        let mut modules = Vec::new();
        for (path, value) in [
            ("product/east/value.omg", east),
            ("product/west/value.omg", west),
        ] {
            let text = format!(
                "public const N: u64 = extent();
                machine extent() -> u64 crashes none {{ {value}u64 }}"
            );
            modules.push(
                files
                    .add_checked_instance(
                        path.into(),
                        text.as_str(),
                        "product".into(),
                        Some(package),
                        SourceOrigin::User,
                        SourceResolutionStratum::Base,
                        DependencyScope::Product,
                    )
                    .source_id,
            );
        }
        let mut stage = product(files, root, package, &[]).unwrap();
        stage.parsed.roots.extend(modules);
        (root, compile(stage, None))
    };

    // `choose` passes `|east|::N` bytes where two are required, and `other`
    // passes `|west|::N` bytes where three are required.
    assert!(!publish(2, 3).1.unwrap().to_bytes().is_empty());
    for (east, west, machine, wanted, supplied) in [(3, 3, "choose", 2, 3), (2, 2, "other", 3, 2)] {
        let (root, published) = publish(east, west);
        let diagnostics = published.unwrap_err();
        let [refusal] = diagnostics.as_slice() else {
            panic!("expected one refusal, found {diagnostics:?}")
        };
        assert_eq!(
            refusal.message,
            format!(
                "the parameter it is passed for expects an array with {wanted} elements \
                 but the supplied array has {supplied} while reading the body of `{machine}`"
            )
        );
        // The refusal points at the argument: the `values` passed on.
        let declaration = main.find(&format!("machine {machine}(")).unwrap();
        let argument = declaration + main[declaration..].find("(values) }").unwrap() + 1;
        assert_eq!(
            refusal.source_span,
            Some(SourceSpan {
                source_id: root,
                span: Span {
                    start: argument,
                    end: argument + "values".len(),
                },
            }),
            "{refusal:?}"
        );
    }
}

#[test]
fn parser_output_must_cover_the_exact_captured_files() {
    let package = PackageKeyIdentity::from_digest([0x73; 32]).unwrap();
    let original = "machine main() -> u32 crashes none { 3 }";
    let (files, root) = captured(package, original);

    let mut edited = product(files.clone(), root, package, &[]).unwrap();
    let (replacement, _) = captured(package, "machine main() -> u32 crashes none { 4 }");
    edited.parsed.files = Arc::new(replacement);
    let errors = compile(edited, None).unwrap_err();
    assert!(
        errors.iter().any(|error| error
            .message
            .contains("does not match its selected source occurrence")),
        "{errors:?}"
    );

    let mut missing = product(files.clone(), root, package, &[]).unwrap();
    let mut expanded = files;
    capture_file(
        &mut expanded,
        "product",
        package,
        "extra.omg",
        "data Extra {}",
    );
    missing.parsed.files = Arc::new(expanded);
    let errors = compile(missing, None).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("missing from the parsed stage")),
        "{errors:?}"
    );
}

#[test]
fn a_published_machine_address_follows_its_logical_file_across_checkouts() {
    let package = PackageKeyIdentity::from_digest([0x63; 32]).unwrap();
    let address = |checkout: &str, filename: &str| {
        let mut files = SourceMap::default();
        let root = capture_file(
            &mut files,
            checkout,
            package,
            filename,
            "public machine read() -> u8 reaches none crashes none { 7u8 }",
        );
        let artifact = compile(product(files, root, package, &[]).unwrap(), None).unwrap();
        let reopened =
            terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap();
        let module = reopened.semantic_module();
        let [machine] = module.machines.as_slice() else {
            panic!("expected one public machine");
        };
        let declaration = machine.contract.declaration.as_ref().unwrap();
        module
            .identities
            .text(declaration.machine.identity())
            .unwrap()
            .to_owned()
    };
    let original = address("first-checkout", "io/current.omg");
    assert_eq!(original, address("second-checkout", "io/current.omg"));
    assert_ne!(original, address("first-checkout", "archive/current.omg"));
    assert_ne!(original, address("first-checkout", "io/previous.omg"));
}

#[test]
fn a_file_qualifier_selects_its_own_sources_declarations() {
    let package = PackageKeyIdentity::from_digest([0x63; 32]).unwrap();
    let (mut files, root) = captured(package, "machine value() -> u8 { |cell|::read() }");
    let bytes = capture_file(
        &mut files,
        "product",
        package,
        "source-files/bytes.omg",
        "machine read() -> u8 { 7u8 }",
    );
    let flags = capture_file(
        &mut files,
        "product",
        package,
        "source-files/flags.omg",
        "machine read() -> bool { true }",
    );
    let make = |selected: Option<SourceId>| {
        let mut stage = product(files.clone(), root, package, &[]).unwrap();
        stage.parsed.roots.extend([bytes, flags]);
        stage.parsed.imports.extend(
            selected.map(|source| SourceScopedTopLevelBinding::file_import(root, source, "cell")),
        );
        compile(stage, None)
    };
    make(Some(bytes)).expect("the selected file returns a byte");
    let wrong = make(Some(flags)).unwrap_err();
    assert!(
        wrong.iter().any(|d| d.message.contains("bool")),
        "{wrong:?}"
    );
    let missing = make(None).unwrap_err();
    assert!(
        missing.iter().any(|d| d.message.contains("read")),
        "{missing:?}"
    );
}

#[test]
fn file_imports_neither_bind_bare_names_nor_reexport_imports() {
    let package = PackageKeyIdentity::from_digest([0x63; 32]).unwrap();
    for (call, wants_alias) in [
        ("read()", false),
        ("read()", true),
        ("|middle|::read()", true),
    ] {
        let (mut files, root) = captured(package, &format!("machine value() -> u8 {{ {call} }}"));
        let helper = capture_file(
            &mut files,
            "product",
            package,
            "helper.omg",
            "machine read() -> u8 { 7u8 }",
        );
        let middle = capture_file(
            &mut files,
            "product",
            package,
            "middle.omg",
            "machine relay() -> u8 { |leaf|::read() }",
        );
        let mut stage = product(files, root, package, &[]).unwrap();
        stage.parsed.roots.extend([helper, middle]);
        stage
            .parsed
            .imports
            .push(SourceScopedTopLevelBinding::file_import(
                middle, helper, "leaf",
            ));
        if wants_alias {
            let imported = if call.starts_with('|') {
                middle
            } else {
                helper
            };
            stage
                .parsed
                .imports
                .push(SourceScopedTopLevelBinding::file_import(
                    root, imported, "middle",
                ));
        }
        let errors = compile(stage, None).expect_err("read is outside the selected file");
        assert!(
            errors.iter().any(|d| d.message.contains("read")),
            "{errors:?}"
        );
    }
}

#[test]
fn file_qualifiers_respect_foreign_declaration_visibility() {
    let package = PackageKeyIdentity::from_digest([0x63; 32]).unwrap();
    let dependency = PackageKeyIdentity::from_digest([0x64; 32]).unwrap();
    for (source, direct, refusal) in [
        ("public machine read() -> u8 { 7u8 }", true, None),
        ("public machine read() -> u8 { 7u8 }", false, Some("direct")),
        ("machine read() -> u8 { 7u8 }", true, Some("private")),
    ] {
        let (mut files, root) = captured(package, "machine value() -> u8 { |leaf|::read() }");
        let helper = capture_file(&mut files, "dependency", dependency, "helper.omg", source);
        let mut stage = product(files, root, package, &[]).unwrap();
        stage.product.declaration_scope =
            Some(Arc::new(ProductDeclarationScope::from_direct_dependencies(
                &[package, dependency],
                direct.then_some((package, dependency)),
            )));
        stage.parsed.roots.push(helper);
        stage
            .parsed
            .imports
            .push(SourceScopedTopLevelBinding::file_import(
                root, helper, "leaf",
            ));
        let actual = compile(stage, None);
        if let Some(refusal) = refusal {
            let errors = actual.expect_err("loading a file cannot grant extra authority");
            assert!(
                errors.iter().any(|d| d.message.contains(refusal)),
                "{errors:?}"
            );
        } else {
            actual.expect("the direct dependency exposes its public machine");
        }
    }
}

#[test]
fn a_static_case_in_a_file_uses_the_type_even_when_a_parameter_has_its_name() {
    let package = PackageKeyIdentity::from_digest([0x63; 32]).unwrap();
    for (second, complete) in [(", cell::Supplied -> 9u8", true), ("", false)] {
        let source = format!(
            "data cell {{ case Stored; case Supplied; }}
            machine value(cell: u8, kind: cell) -> u8 {{
                transition kind {{ cell::Stored -> cell{second} }}
            }}"
        );
        let (files, root) = captured(package, &source);
        let result = compile(product(files, root, package, &[]).unwrap(), None);
        if complete {
            result.expect("the declared cases exhaust the type");
        } else {
            let errors = result.expect_err("Supplied remains possible");
            assert!(
                errors.iter().any(|d| d.message.contains("cell::Supplied")),
                "{errors:?}"
            );
        }
    }
}

#[test]
fn a_fixed_carrier_keeps_its_indexed_domain_under_both_local_spellings() {
    let package = PackageKeyIdentity::from_digest([0x63; 32]).unwrap();
    for (domain, refusal) in [
        ("Occupied<Word, Other>", None),
        ("Tag::Occupied<Word, Other>", None),
        (
            "Word::Occupied<Word, Other>",
            Some("no domain of that name is visible"),
        ),
        ("Tag::Occupied<Word>", Some("expected 2, received 1")),
    ] {
        let source = format!(
            "data Tag {{}} data Word {{}} data Other {{}}
             domain<P, T> Tag::Occupied<P, T>;
             machine carry(value: Tag in {domain}) -> Tag in {domain} {{ value }}"
        );
        let (files, root) = captured(package, &source);
        let result = source_compiler::check(
            product(files, root, package, &[]).unwrap(),
            &mut Default::default(),
        );
        if let Some(refusal) = refusal {
            let errors = result.expect_err("the written carrier and indices must match");
            assert!(
                errors.iter().any(|error| error.message.contains(refusal)),
                "{domain}: {errors:?}"
            );
        } else {
            result.unwrap_or_else(|errors| panic!("{domain}: {errors:?}"));
        }
    }
}

#[test]
fn executable_membership_selects_domains_on_an_eligible_operand() {
    let package = PackageKeyIdentity::from_digest([0x73; 32]).unwrap();
    for (parameter, tested, refusal) in [
        ("Word in Placed", "Op", None),
        ("Word in Other", "Op", None),
        ("Word in Placed & Placed::Op", "Op", None),
        ("Word in Placed & Other", "Placed::Op", None),
        ("Word in Placed & Other", "Op", Some("ambiguous domain")),
        ("Word", "Op", Some("no visible domain")),
        ("Word in Other", "Placed::Op", Some("requires membership")),
        ("Parcel", "Word::Placed", Some("requires carrier")),
    ] {
        let source = format!(
            "data Word {{ bits: u32; }}
             data Parcel {{ bits: u32; }}
             domain Word::Placed;
             domain Word::Other;
             data Placed = Word in Word::Placed;
             data Other = Word in Word::Other;
             domain Placed::Op;
             domain Other::Op;
             machine test(x: {parameter}) -> bool {{ x in {tested} }}"
        );
        let (files, root) = captured(package, &source);
        let result = compile(product(files, root, package, &[]).unwrap(), None);
        if let Some(reason) = refusal {
            let errors = result.expect_err("the operand must select one applicable domain");
            assert!(
                errors.iter().any(|error| error.message.contains(reason)),
                "{errors:?}"
            );
        } else {
            result.expect("the selected domain accepts this operand");
        }
    }
}

#[test]
fn a_cast_in_a_membership_operand_must_establish_its_domain() {
    let package = PackageKeyIdentity::from_digest([0x74; 32]).unwrap();
    for (requirement, proved) in [("", true), ("requires self.bits > 0u32", false)] {
        let source = format!(
            "data Word {{ bits: u32; }}
             domain Word::Placed {requirement};
             domain Word::Other;
             data Placed = Word in Word::Placed;
             data Other = Word in Word::Other;
             domain Placed::Op;
             domain Other::Op;
             machine test(x: Word) -> bool {{ (x as +Placed) in Op }}"
        );
        let (files, root) = captured(package, &source);
        let result = compile(product(files, root, package, &[]).unwrap(), None);
        if proved {
            result.expect("select Op on the completed cast");
        } else {
            let errors = result.expect_err("an empty Op predicate cannot erase an unproved cast");
            assert!(
                errors.iter().any(|error| error.message.contains("cannot prove")
                    && error.message.contains("Word::Placed")),
                "{errors:?}"
            );
        }
    }
}

#[test]
fn scalar_predicates_read_self_on_their_own_carrier() {
    let package = PackageKeyIdentity::from_digest([0x75; 32]).unwrap();
    for (body, refusal) in [
        ("x in Positive::Small", None),
        ("x > 0u32", Some("takes unqualified operands only")),
    ] {
        let source = format!(
            "domain u32::Positive requires self > 0u32;
             domain u32::Positive::Small requires self < 10u32;
             domain u32::Tag;
             machine test(x: u32 in Positive & Tag) -> bool {{ {body} }}"
        );
        let (files, root) = captured(package, &source);
        let result = compile(product(files, root, package, &[]).unwrap(), None);
        if let Some(reason) = refusal {
            let errors = result.expect_err("ordinary source still needs its declared operator");
            assert!(errors.iter().any(|error| error.message.contains(reason)), "{errors:?}");
        } else {
            result.expect("each predicate checks the completed integer at its declared carrier");
        }
    }
}

#[test]
fn an_indexed_domain_retains_the_members_of_its_applied_carrier() {
    let package = PackageKeyIdentity::from_digest([0x63; 32]).unwrap();
    for (input_domain, returned_index, domain_index, refusal) in [
        (
            " in Live<Placed, Word>",
            "Word in Word::Placed",
            "Placed",
            None,
        ),
        (
            " in Segment::Live<Placed, Word>",
            "Word in Word::Placed",
            "Placed",
            None,
        ),
        (
            "",
            "Word in Word::Placed",
            "Placed",
            Some("carries no domain"),
        ),
        (
            " in Live<Placed, Word>",
            "Word in Word::Other",
            "Word in Word::Other",
            Some("but this value carries"),
        ),
        (
            " in Live<Placed, Word>",
            "Placed",
            "Word in Word::Other",
            Some("does not qualify carrier"),
        ),
    ] {
        let source = format!(
            "data Word {{ bits: u64; }}
             domain Word::Placed;
             domain Word::Other;
             data Placed = Word in Word::Placed;
             data Segment<P, T> [linear] {{ value: T; }}
             domain<P, T> Segment<P, T>::Live<P, T>;
             data Second<P, T> {{ value: T; }}
             domain<P, T> Second<P, T>::Live<P, T>;
             machine carry(value: Segment<Placed, Word>{input_domain})
                 -> Segment<{returned_index}, Word> in Live<{domain_index}, Word> {{ value }}"
        );
        let (files, root) = captured(package, &source);
        let result = compile(product(files, root, package, &[]).unwrap(), None);
        if let Some(refusal) = refusal {
            let errors = result.expect_err("returning the segment must preserve its domain");
            assert!(errors.iter().any(|error| error.message.contains(refusal)), "{errors:?}");
        } else {
            result.expect("the alias and expanded index describe the same live segment");
        }
    }
}

#[test]
fn a_domain_on_a_qualified_alias_preserves_both_qualifications() {
    let package = PackageKeyIdentity::from_digest([0x63; 32]).unwrap();
    for (input, carries_stored) in [("Kept", true), ("Segment<Placed, Word>", false)] {
        let source = format!(
            "data Word {{ bits: u64; }}
             domain Word::Placed;
             data Placed = Word in Word::Placed;
             data Segment<P, T> [linear] {{ value: T; }}
             domain<P, T> Segment<P, T>::Stored<P, T>;
             domain<P, T> Segment<P, T>::Live<P, T>;
             data Kept = Segment<Placed, Word> in Stored<Placed, Word>;
             machine carry(x: {input} in Live<Placed, Word>)
                 -> Kept in Live<Placed, Word> {{ x }}"
        );
        let (files, root) = captured(package, &source);
        let result = compile(product(files, root, package, &[]).unwrap(), None);
        if carries_stored {
            result.expect("Live qualifies the carrier of the already stored segment");
        } else {
            let errors = result.expect_err("Live alone does not establish Stored");
            assert!(
                errors.iter().any(|error| {
                    error.message.contains("machine's result declares")
                        && error.message.contains("Stored")
                }),
                "{errors:?}"
            );
        }
    }
}

#[test]
fn transparent_aliases_check_their_const_arguments_even_when_expansion_erases_them() {
    let package = PackageKeyIdentity::from_digest([0x63; 32]).unwrap();
    let declarations = "data Buffer<T, const N: u64> { items: [T; N]; }
        data ByteBuffer<const Capacity: u64> = Buffer<u8, Capacity>;
        data Word { bits: u32; }
        data Plain<const N: u8> = Word;";
    for (source, refusal) in [
        (
            "machine f(x: ByteBuffer<2 + 2>) -> Buffer<u8, 4> { x }",
            None,
        ),
        ("machine f(x: Plain<2>) -> Word { x }", None),
        (
            "data Broken<const N: u8> = Buffer<u8, N>; machine f() {}",
            Some("declared carrier differs"),
        ),
        ("machine f(x: Plain<2u16>) {}", Some("must be a `u8`")),
        (
            "machine f(x: Plain<u16>) {}",
            Some("requires a static value"),
        ),
        (
            "machine f(v: u8, x: Plain<v>) {}",
            Some("requires a static value"),
        ),
        (
            "machine missing() -> u8 { crash Trap; } machine f(x: Plain<missing()>) {}",
            Some("const argument typing does not admit"),
        ),
    ] {
        let (files, root) = captured(package, &format!("{declarations} {source}"));
        let result = compile(product(files, root, package, &[]).unwrap(), None);
        if let Some(refusal) = refusal {
            let errors = result.expect_err(source);
            assert!(
                errors.iter().any(|error| error.message.contains(refusal)),
                "{errors:?}"
            );
        } else {
            result.expect(source);
        }
    }
}

#[test]
fn an_indexed_parameter_and_its_entry_contract_use_the_same_constant() {
    let package = PackageKeyIdentity::from_digest([0x63; 32]).unwrap();
    for domain in [
        "domain<const N: u8> Word::Slot<N>;",
        "domain<T, const N: u8> T::Slot<N>;",
    ] {
        let declarations = format!(
            "data Word {{ bits: u32; }} {domain} data Slotted<const N: u8> = Word in Slot<N>;"
        );
        for (body, refusal) in [
            (
                "machine f(v: Slotted<1 + 1>) -> Word in Slot<2> { v }",
                None,
            ),
            ("machine f(v: Word in Slot<2>) -> Slotted<2> { v }", None),
            ("machine f(v: @Slotted<2>) -> u32 { v.bits }", None),
            (
                "machine f(v: Slotted<2>) -> Word in Slot<3> { v }",
                Some("machine's result declares"),
            ),
        ] {
            let (files, root) = captured(package, &format!("{declarations} {body}"));
            let result = compile(product(files, root, package, &[]).unwrap(), None);
            if let Some(refusal) = refusal {
                let errors = result.expect_err(body);
                assert!(
                    errors.iter().any(|error| error.message.contains(refusal)),
                    "{errors:?}"
                );
            } else {
                result.expect(body);
            }
        }
    }
}

#[test]
fn a_data_type_parameter_rejects_a_value_even_if_expansion_discards_it() {
    let package = PackageKeyIdentity::from_digest([0x63; 32]).unwrap();
    let declarations = "data Word { bits: u32; }
        data Pair<A, B> { first: A; second: B; }
        data Phantom<T> { marker: u8; }
        data Plain<T> = Word;";
    for (body, bad_parameter) in [
        ("machine f(v: Plain<Pair<1, u8>>) -> Word { v }", Some("A")),
        ("machine f(v: Plain<Phantom<1>>) -> Word { v }", Some("T")),
        ("machine f(v: Phantom<1>) -> u8 { v.marker }", Some("T")),
        ("machine f(v: Plain<Pair<u8, u8>>) -> Word { v }", None),
        (
            "data Wrap<T> { item: Phantom<T>; } machine f(v: Wrap<u8>) -> u8 { v.item.marker }",
            None,
        ),
    ] {
        let (files, root) = captured(package, &format!("{declarations} {body}"));
        let result = compile(product(files, root, package, &[]).unwrap(), None);
        if let Some(binder) = bad_parameter {
            let errors = result.expect_err(body);
            assert!(
                errors.iter().any(|error| error
                    .message
                    .contains(&format!("data type argument for `{binder}`"))
                    && error.message.contains("supplied a value")),
                "{errors:?}"
            );
        } else {
            result.expect(body);
        }
    }
}

#[test]
fn a_type_argument_supplies_its_own_declarations_full_arity() {
    let package = PackageKeyIdentity::from_digest([0x63; 32]).unwrap();
    let declarations = "data Word { bits: u32; }
        data Pair<A, B> { first: A; second: B; }
        data Loan<'r> { item: @'r u8; }
        data Plain<T> = Word;";
    for (argument, missing) in [
        ("Pair", Some("Pair")),
        ("u8<u16>", Some("u8")),
        ("@Pair", Some("Pair")),
        ("Pair<u8, Pair>", Some("Pair")),
        ("[Pair<u8<u16>, u8>]", Some("u8")),
        ("Loan", Some("Loan")),
        ("Loan<'r>", None),
        ("Pair<u8, Pair<u8, u16>>", None),
    ] {
        let source = format!("{declarations} machine f<'r>(v: Plain<{argument}>) -> Word {{ v }}");
        let (files, root) = captured(package, &source);
        let result = compile(product(files, root, package, &[]).unwrap(), None);
        if let Some(name) = missing {
            let errors = result.expect_err(&source);
            let expected = format!("type argument `{name}` has the wrong arity");
            assert!(
                errors.iter().any(|error| error.message.contains(&expected)),
                "{errors:?}"
            );
        } else {
            result.expect(&source);
        }
    }
}

#[test]
fn an_erased_type_argument_keeps_its_supplying_lifetime_scope() {
    let package = PackageKeyIdentity::from_digest([0x69; 32]).unwrap();
    let declarations = "data Word { bits: u32; }
        data Loan<'r> { item: @'r u8; }
        data Pair<A, B> { first: A; second: B; }
        data Plain<T> = Word;
        data Forward<'element> = Plain<Loan<'element>>;
        machine unrelated<'other>(v: @'other u8) -> u8 { 1 }";
    for (argument, valid) in [
        ("Loan<'caller>", true),
        ("Forward<'caller>", true),
        ("Pair<@'caller u8, Loan<'caller>>", true),
        ("@u8", true),
        ("Loan<'other>", false),
        ("@'other mutable u8", false),
        ("[@'other u8; 2]", false),
        ("Pair<u8, Loan<'other>>", false),
    ] {
        let source =
            format!("{declarations} machine f<'caller>(v: Plain<{argument}>) -> Word {{ v }}");
        let (files, root) = captured(package, &source);
        let result = compile(product(files, root, package, &[]).unwrap(), None);
        if valid {
            result.expect(&source);
        } else {
            let diagnostics = result.expect_err(&source);
            assert!(
                diagnostics.iter().any(|error| error.message.contains(
                    "lifetime argument `'other` must name a lifetime in its supplying declaration"
                )),
                "{diagnostics:?}",
            );
        }
    }
}

#[test]
fn competing_domain_families_use_the_qualifications_on_each_side() {
    let package = PackageKeyIdentity::from_digest([0x63; 32]).unwrap();
    let declarations = "data Word { bits: u32; }
        domain<T> Word::Mark<T>;
        data Left = Word in Mark<u16>;
        data Right = Word in Mark<u32>;
        domain<T> Left::Op<T>;
        domain<T> Right::Op<T>;
        domain<const N: u8> Word::Slot<N>;
        data Input = Word in Slot<2>;
        data Output = Word in Slot<3>;
        domain Input::Ready;
        domain Output::Ready;";
    for (machine, refusal) in [
        (
            "machine f(x: Word in Op<u8> & Mark<u16>) -> Left in Left::Op<u8> { x }",
            None,
        ),
        (
            "machine f(x: Word in Mark<u32> & Op<u8>) -> Right in Right::Op<u8> { x }",
            None,
        ),
        (
            "machine f(x: Left in Left::Op<u8>) -> Word in Mark<u16> & Mark<u32> & Left::Op<u8> {
                x as =Mark<u16> +Mark<u32> =Op<u8>
             }",
            None,
        ),
        (
            "machine f(x: Word) -> Input in Input::Ready { x as +Ready +Slot<1 + 1> }",
            None,
        ),
        (
            "machine f(x: Word) -> Output in Output::Ready { x as +Slot<3u8> +Ready }",
            None,
        ),
        (
            "machine f(x: Word) -> Input in Input::Ready { x as +Slot<3> +Ready }",
            Some("this value's type carries"),
        ),
        (
            "machine f(x: Word) -> Input in Input::Ready { x as +Slot<2u16> +Ready }",
            Some("must be a `u8`"),
        ),
        ("machine f(x: Left) requires x in Op<u8> {}", None),
        ("machine f(x: Right) requires x in Op<u8> {}", None),
        (
            "machine f(x: Word in Mark<u16> & Mark<u32>) requires x in Op<u8> {}",
            Some("ambiguous domain `Op`"),
        ),
        (
            "machine f(x: Word in Mark<u16> & Mark<u32> & Op<u8>) {}",
            Some("ambiguous domain `Op`"),
        ),
        (
            "machine f(x: Word in Mark<u64> & Op<u8>) {}",
            Some("no visible domain `Op` accepts the qualifications"),
        ),
    ] {
        let (files, root) = captured(package, &format!("{declarations} {machine}"));
        let result = compile(product(files, root, package, &[]).unwrap(), None);
        if let Some(refusal) = refusal {
            let errors = result.expect_err(machine);
            assert!(
                errors.iter().any(|error| error.message.contains(refusal)),
                "{errors:?}"
            );
        } else {
            result.expect(machine);
        }
    }
}

#[test]
fn an_applied_carrier_domain_is_exposed_by_its_declaring_file() {
    let package = PackageKeyIdentity::from_digest([0x63; 32]).unwrap();
    for (name, visible) in [
        ("|parts|::Segment::Live", true),
        ("|parts|::Word::Live", false),
        ("Live", false),
    ] {
        let qualified = format!("|parts|::Segment<Word> in {name}<Word>");
        let source =
            format!("data Word {{}} machine carry(value: {qualified}) -> {qualified} {{ value }}");
        let (mut files, root) = captured(package, &source);
        let parts = capture_file(
            &mut files,
            "product",
            package,
            "parts.omg",
            "public data Segment<T> { value: T; } public domain<T> Segment<T>::Live<T>;",
        );
        let mut stage = product(files, root, package, &[]).unwrap();
        stage.parsed.roots.push(parts);
        stage
            .parsed
            .imports
            .push(SourceScopedTopLevelBinding::file_import(
                root, parts, "parts",
            ));
        let result = compile(stage, None);
        if visible {
            result.expect("the attachment belongs to Segment in the selected file");
        } else {
            let errors = result.expect_err("an attachment needs its own file and carrier");
            assert!(
                errors
                    .iter()
                    .any(|error| error.message.contains("no domain of that name")),
                "{errors:?}"
            );
        }
    }
}

#[test]
fn a_file_qualified_domain_still_requires_its_declared_carrier() {
    let package = PackageKeyIdentity::from_digest([0x63; 32]).unwrap();
    for (domains, accepts_byte) in [
        ("domain u16::Tag; domain u8::Tag;", true),
        ("domain u16::Tag;", false),
    ] {
        let (mut files, root) = captured(
            package,
            "machine value() -> u8 in |units|::Tag { 7u8 as + |units|::Tag }",
        );
        let units = capture_file(
            &mut files, "product", package, "units.omg", domains,
        );
        let mut stage = product(files, root, package, &[]).unwrap();
        stage.parsed.roots.push(units);
        stage.parsed.imports.push(SourceScopedTopLevelBinding::file_import(root, units, "units"));
        let result = compile(stage, None);
        if accepts_byte {
            result.expect("the selected file declares Tag on the operand type");
        } else {
            let errors = result.expect_err("a domain on u16 cannot qualify a u8");
            assert!(errors.iter().any(|error| error.message.contains("Tag")), "{errors:?}");
        }
    }
}

/// The same three files met in another order publish the same Psi.
/// `main.omg` uses a record and a machine from each of `a.omg` and
/// `b.omg`. One compilation captures and imports `a` before `b`, the other
/// `b` before `a`; nothing else differs, so the published bytes must not.
///
/// `wiki/spec/terminal-psi/encoding.md`, "Canonical form": "Unordered sets
/// are strictly sorted by stable identity or canonical bytes".
#[test]
fn the_order_files_are_met_in_does_not_change_the_published_psi() {
    let package = PackageKeyIdentity::from_digest([0x63; 32]).unwrap();
    let main = "machine main() -> u16 crashes none {
            let low: u8 = |a|::alpha(|a|::Alpha { v: 3u8 });
            |b|::beta(|b|::Beta { w: 4u16 })
        }";
    let a = "public data Alpha { v: u8; }
        public machine alpha(x: Alpha) -> u8 crashes none { x.v }";
    let b = "public data Beta { w: u16; }
        public machine beta(y: Beta) -> u16 crashes none { y.w }";
    let publish = |order: [(&str, &str, &str); 2]| {
        let (mut files, root) = captured(package, main);
        let mut imported = Vec::new();
        for (alias, file, text) in order {
            imported.push((
                alias,
                capture_file(&mut files, "product", package, file, text),
            ));
        }
        let mut stage = product(files, root, package, &[]).unwrap();
        for (alias, source) in imported {
            stage.parsed.roots.push(source);
            stage
                .parsed
                .imports
                .push(SourceScopedTopLevelBinding::file_import(
                    root, source, alias,
                ));
        }
        compile(stage, None).unwrap().to_bytes()
    };
    let forward = publish([("a", "a.omg", a), ("b", "b.omg", b)]);
    let backward = publish([("b", "b.omg", b), ("a", "a.omg", a)]);
    let differing = forward
        .iter()
        .zip(&backward)
        .filter(|(x, y)| x != y)
        .count();
    assert!(
        forward == backward,
        "{differing} of {} bytes differ",
        forward.len()
    );
}

#[test]
fn identity_checks_the_complete_applied_data_type_of_both_subjects() {
    let package = PackageKeyIdentity::from_digest([0x79; 32]).unwrap();
    let check = |source: &str| {
        let (files, root) = captured(package, source);
        source_compiler::check(
            product(files, root, package, &[]).unwrap(),
            &mut Default::default(),
        )
    };
    let source = r#"data Holder<T> { field: T; }
machine carry(left: Holder<Holder<u32>>, right: Holder<Holder<u32>>)
requires core\Identity::<Holder<Holder<u32>>>(left, right)
ensures core\Identity::<Holder<Holder<u32>>>(left, right)
{}
machine main() {}
"#;
    check(source).expect("the same typed equality is available in the entry assumptions");

    let changed = source.replace(
        "Identity::<Holder<Holder<u32>>>",
        "Identity::<Holder<Holder<u16>>>",
    );
    let wrong_type = check(&changed).expect_err("the innermost type differs from both subjects");
    assert!(
        wrong_type
            .iter()
            .any(|error| error.message.contains("Identity operand has no exact input observation")),
        "{wrong_type:?}"
    );

    let unproved = source.replace(
        "requires core\\Identity::<Holder<Holder<u32>>>(left, right)\n",
        "",
    );
    let missing = check(&unproved).expect_err("two independent values supply no identity proof");
    assert!(
        missing
            .iter()
            .any(|error| error.message.contains("cannot prove ensures")),
        "{missing:?}"
    );
}

#[test]
fn routed_membership_tests_require_evidence_for_the_completed_operand() {
    let package = PackageKeyIdentity::from_digest([0x76; 32]).unwrap();
    for (declaration, carrier) in [("", "u32"), ("data Word { bits: u32; }", "Word")] {
        let definitions = format!(
            "{declaration}
             domain {carrier}::Stamp established by issue;
             machine issue(x: {carrier}) -> {carrier} ensures result in Stamp {{
                 let held: {carrier} in Stamp = x as +Stamp;
                 held as -Stamp
             }}"
        );
        for (qualifier, body, accepts) in [
            (" in Stamp", "x in Stamp".to_string(), true),
            (
                "",
                format!("let plain: {carrier} = issue(x); plain in Stamp"),
                true,
            ),
            ("", "x in Stamp".to_string(), false),
            (
                "",
                format!("let proved: {carrier} = issue(x); y in Stamp"),
                false,
            ),
            (
                "",
                format!("let mutable proved: {carrier} = issue(x); proved = y; proved in Stamp"),
                false,
            ),
        ] {
            let source = format!(
                "{definitions}
                machine test(x: {carrier}{qualifier}, y: {carrier}) -> bool {{ {body} }}"
            );
            let (files, root) = captured(package, &source);
            let result = compile(product(files, root, package, &[]).unwrap(), None);
            if accepts {
                result.expect("the operand carries authorized provenance");
            } else {
                let errors = result.expect_err("a routed test needs evidence for its own operand");
                assert!(
                    errors
                        .iter()
                        .any(|error| error.message.contains("authorized provenance")),
                    "{errors:?}"
                );
            }
        }
    }
}

#[test]
fn routed_membership_tests_preserve_the_domain_index() {
    let package = PackageKeyIdentity::from_digest([0x77; 32]).unwrap();
    for (declaration, carrier) in [("", "u64"), ("data Word { bits: u64; }", "Word")] {
        for index in [7, 8] {
            let source = format!(
                "{declaration}
                 domain<const N: u64> {carrier}::Stamp<N> established by issue;
                 machine issue(x: {carrier}) -> {carrier} in Stamp<8> {{ x as +Stamp<8> }}
                 machine test(x: {carrier} in Stamp<8>) -> bool {{ x in Stamp::<{index}> }}"
            );
            let (files, root) = captured(package, &source);
            let result = compile(product(files, root, package, &[]).unwrap(), None);
            if index == 8 {
                result.expect("the qualified formal and the test use the same application");
            } else {
                let errors = result.expect_err("the other index needs its own issuance");
                assert!(
                    errors
                        .iter()
                        .any(|error| error.message.contains("authorized provenance")),
                    "{errors:?}"
                );
            }
        }
    }
}
