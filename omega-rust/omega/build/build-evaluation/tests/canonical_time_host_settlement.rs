//! Provider settlement mints the canonical `TimeHost` provider plan for a
//! build that accepts the `TimeHostService` binding.
//!
//! The canonical raw-clock boundary admits no authored `satisfies`
//! conformance or `select_provider` row, so demanded leaves used to reach
//! closure review with zero selected provider rows. Settlement now mints the
//! toolchain's reviewed per-target realization table — requirements whose
//! irreducible kernel realization is one named syscall — and joins it into
//! the selected closure through `with_toolchain_settled_plan`, outside
//! candidate provenance. The per-target clock constants carry no kernel
//! mechanism and stay uncovered; a demanded leaf with no transport still
//! rejects at lowering.

use std::path::PathBuf;

use build_evaluation::settle_checked_providers;
use build_evaluation::target_machines::{
    SelectedTargetMachineDeclarations, filter_target_machines,
};
use effects::provider_plan::ProviderBinding;
use package_compilation::{
    AcceptedSemanticBinding, AcceptedSemanticBindingRole, BuildDeclarationKind,
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use semantic_vocabulary::PackageKeyIdentity;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The canonical `TimeHost` surface: the three syscall-bound requirements
/// plus the per-target constants the linux_x86_64 table deliberately leaves
/// uncovered.
const TIME_SOURCE: &str = r#"
    pub boundary trait TimeHost {
        machine monotonic_ticks() -> u64 reaches TimeHost;
        machine monotonic_ticks_per_second() -> u64 reaches TimeHost;
        machine wall_clock_raw() -> u64 reaches TimeHost;
        machine wall_clock_units_per_second() -> u64 reaches TimeHost;
        machine wall_clock_epoch_offset_seconds() -> u64 reaches TimeHost;
        machine sleep(milliseconds: u32) reaches TimeHost;
    }
"#;

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

fn identity(seed: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([seed; 32]).expect("fixture package identity")
}

const ROOT: u8 = 1;
const HOST: u8 = 2;

struct Fixture {
    root_dir: PathBuf,
    host_dir: PathBuf,
    typed: typed_trees::TypedTrees,
    target_machines: Option<SelectedTargetMachineDeclarations>,
}

impl Fixture {
    /// Type the canonical-trait fixture owned by the `host` package so the
    /// accepted binding can rejoin it by exact package identity.
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!(
            "omega-canonical-time-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
        ));
        let root_dir = base.join("root");
        let host_dir = base.join("host");
        std::fs::create_dir_all(&root_dir).expect("create fixture root package dir");
        std::fs::create_dir_all(&host_dir).expect("create fixture host package dir");
        let host_source = host_dir.join("time_host.omg");
        std::fs::write(&host_source, TIME_SOURCE).expect("write fixture boundary source");
        let mut sources = source::SourceMap::default();
        let source_id = sources
            .add_with_metadata(
                host_source,
                TIME_SOURCE.to_owned(),
                host_dir.clone(),
                Some(identity(HOST)),
                source::SourceOrigin::User,
            )
            .source_id;
        let tokens = source_files_to_tokens::Lexer::new(TIME_SOURCE)
            .tokenize()
            .expect("tokenize fixture");
        let mut syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens)
            .expect("parse fixture");
        let target_machines = filter_target_machines(&mut syntax, None)
            .expect("the fixture declares no target machine");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                syntax: &syntax,
                sources: Some(std::sync::Arc::new(sources)),
                top_level_bindings: Vec::new(),
            },
        )
        .expect("resolve fixture");
        let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type fixture");
        Self {
            root_dir,
            host_dir,
            typed,
            target_machines: Some(target_machines),
        }
    }

    /// The exact package-owned `TimeHost` boundary trait the fixture typed
    /// program declares.
    fn time_host_trait(&self) -> &typed_trees::trait_definition::TraitDefinition {
        let definitions = self
            .typed
            .traits()
            .iter()
            .filter(|definition| {
                definition.is_boundary
                    && self
                        .typed
                        .symbols
                        .symbol_package_identity(definition.symbol)
                        == Some(identity(HOST))
                    && self.typed.symbols.display_path(definition.symbol, "::") == "TimeHost"
            })
            .collect::<Vec<_>>();
        let [definition] = definitions.as_slice() else {
            panic!("the fixture resolves exactly one package-owned TimeHost boundary");
        };
        definition
    }

    /// The accepted `TimeHostService` binding for this fixture's exact
    /// schema, attached beside a `host` package the graph knows.
    fn package_inputs(&self, with_binding: bool) -> PackageCompilationInputs {
        let inputs = PackageCompilationInputs::new(
            identity(ROOT),
            BuildDeclarationKind::Application,
            vec![
                PackageSourceBinding::new(identity(ROOT), "root", self.root_dir.clone()),
                PackageSourceBinding::new(identity(HOST), "host", self.host_dir.clone()),
            ],
            vec![PackageDependencyBinding::new(
                identity(ROOT),
                "host",
                identity(HOST),
            )],
        )
        .unwrap_or_else(|errors| panic!("fixture package inputs: {errors:#?}"));
        if !with_binding {
            return inputs;
        }
        let definition = self.time_host_trait();
        let schema = provider_planning::service_schema::from_typed(&self.typed, definition)
            .expect("the fixture boundary yields a service schema");
        let binding = AcceptedSemanticBinding::new_service(
            AcceptedSemanticBindingRole::TimeHostService,
            identity(HOST),
            "TimeHost",
            package_compilation::accepted_service_schema_digest(
                AcceptedSemanticBindingRole::TimeHostService,
                &schema,
            ),
        )
        .expect("the fixture service binding is well-formed");
        inputs
            .with_accepted_semantic_bindings(vec![binding])
            .unwrap_or_else(|errors| panic!("fixture binding acceptance: {errors:#?}"))
    }

    fn settle(
        &mut self,
        with_binding: bool,
        target_name: Option<&'static str>,
    ) -> build_evaluation::CheckedProviderSelection {
        let inputs = self.package_inputs(with_binding);
        let profile = target_name.map(|name| {
            target::TargetProfile::from_canonical_target_name(name).expect("known target profile")
        });
        settle_checked_providers(
            &mut self.typed,
            self.target_machines.take().expect("settle once"),
            profile,
            Some(&inputs),
            &[],
            &std::collections::BTreeSet::new(),
            &[],
            &[],
        )
        .unwrap_or_else(|diagnostics| panic!("fixture settlement: {diagnostics:#?}"))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(parent) = self.root_dir.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
    }
}

#[test]
fn accepted_binding_mints_the_toolchain_time_host_plan() {
    let mut fixture = Fixture::new();
    let trait_symbol = fixture.time_host_trait().symbol;
    let selection = fixture.settle(true, Some("linux_x86_64"));
    let minted = selection
        .selected_provider_plan_facts
        .plans()
        .iter()
        .filter(|plan| plan.provider_type == "omega::toolchain::time_host")
        .collect::<Vec<_>>();
    let [plan] = minted.as_slice() else {
        panic!("one toolchain-settled TimeHost plan: {minted:#?}");
    };
    assert_eq!(plan.target, "linux_x86_64");
    assert_eq!(plan.schema.trait_name, "TimeHost");
    assert_eq!(
        plan.schema.trait_package_identity,
        Some(identity(HOST)),
        "the minted plan carries the exact bound package-owned schema"
    );
    assert_eq!(
        plan.provider_type_package_identity, None,
        "toolchain plans carry no provider package identity"
    );
    assert_eq!(
        plan.origin_package_identity, None,
        "toolchain plans carry no origin package identity"
    );
    let mut numbers = plan
        .rows
        .iter()
        .map(|row| match row.binding {
            ProviderBinding::Syscall { number } => (row.method.as_str(), number),
            _ => panic!("canonical time host rows bind reviewed syscalls only"),
        })
        .collect::<Vec<_>>();
    numbers.sort();
    assert_eq!(
        numbers,
        vec![
            ("monotonic_ticks", 228),
            ("sleep", 35),
            ("wall_clock_raw", 228),
        ]
    );
    for row in &plan.rows {
        assert!(
            !row.requirement_identity.is_empty(),
            "every minted row carries its method's exact requirement identity"
        );
        let method = plan
            .schema
            .methods
            .iter()
            .find(|method| method.name == row.method)
            .expect("minted rows name schema methods");
        assert_eq!(row.requirement_identity, method.requirement_identity);
    }
    let minted = selection
        .selected_provider_provenance
        .iter()
        .enumerate()
        .filter(|(_, provenance)| provenance.plan.report_fingerprint() == plan.report_fingerprint())
        .collect::<Vec<_>>();
    let [(index, provenance)] = minted.as_slice() else {
        panic!("the minted plan joins review provenance exactly once");
    };
    assert!(
        matches!(
            provenance.selected_by,
            provider_planning::ProviderSelectionProvenance::UniqueCoveringCandidate
        ),
        "the toolchain-settled plan reports unique-covering-candidate selection"
    );
    assert_eq!(
        provenance.provider.schema.symbol(),
        trait_symbol,
        "provenance names the exact boundary trait declaration"
    );
    assert!(
        provenance.provider.provider_type.is_none(),
        "toolchain settlement retains no authored provider type"
    );
    assert!(
        provenance
            .provider
            .row_realizations
            .iter()
            .all(|realization| !realization.is_valid()),
        "toolchain rows retain no authored realization machine"
    );
    assert_eq!(
        selection.selected_provider_plan_facts.plans()[*index],
        provenance.plan,
        "review provenance stays index-aligned with the selected facts"
    );
}

#[test]
fn no_binding_mints_no_plan() {
    let selection = Fixture::new().settle(false, Some("linux_x86_64"));
    assert!(
        selection.selected_provider_plan_facts.plans().is_empty(),
        "without the accepted binding no canonical time host plan is minted"
    );
}

#[test]
fn a_target_with_no_reviewed_table_mints_no_plan() {
    let selection = Fixture::new().settle(true, Some("macos_arm64"));
    assert!(
        selection.selected_provider_plan_facts.plans().is_empty(),
        "a target without a reviewed realization table mints nothing rather than guessing"
    );
}
