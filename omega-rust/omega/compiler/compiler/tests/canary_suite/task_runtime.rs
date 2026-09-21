use super::{
    BuildDeclaration, PackageDependencyBinding, PackageKeyIdentity, PackageSourceBinding,
    compile_to_checked, extract_build_declaration, fixture_package_identity,
};
use super::{
    Diagnostic, check_canary, compile_canary_without_output, compile_reviewed_repository_fixture,
    fail_canary, pass_canary, repo_root,
};
use build_declarations::{DependencyPurpose, project_build_entry_syntax, project_dependency_rows};
use compiler::CheckedCompileRequest;
use package_compilation::PackageCompilationInputs;
use source_files_to_tokens::Lexer;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use tokens_to_syntax_trees::parse_syntax_trees;

#[path = "../fixture_rosters/task_runtime.rs"]
pub(super) mod fixture_roster;

fn render(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn lifecycle_operations_conserve_the_linear_claim() {
    let pass = pass_canary(fixture_roster::CORE_TASK_LIFECYCLE_OPERATIONS);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &pass.join("main.omg"),
        None,
    ))
    .unwrap_or_else(|diagnostics| {
        panic!(
            "task lifecycle operations should conserve the claim:\n{}",
            render(&diagnostics)
        )
    });
    let snapshot = checked.typed.snapshot();
    for (name, suspends, blocks) in [
        ("Task::request_cancel", false, false),
        ("Task::finish", true, true),
    ] {
        let machine = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("checked-in core requirement `{name}`"));
        assert!(machine.is_public, "{name}");
        assert_eq!(
            machine.supply_mode,
            language_semantics::MachineSupplyMode::TopLevelRequirement,
            "{name}"
        );
        assert!(!machine.body_is_present, "{name}");
        let machine_snapshot = snapshot
            .roots
            .machines
            .iter()
            .find(|machine| machine.name == name)
            .unwrap_or_else(|| panic!("snapshot for `{name}`"));
        assert_eq!(machine_snapshot.suspends, suspends, "{name}");
        assert_eq!(machine_snapshot.blocks, blocks, "{name}");
        assert!(machine_snapshot.contracts.is_empty(), "{name}");
    }

    let fail = fail_canary(fixture_roster::CORE_TASK_CORE_SCOPE_LOSS);
    let diagnostics = compile_canary_without_output(&fail)
        .expect_err("request_cancel must not settle the task claim");
    let rendered = render(&diagnostics);
    assert!(
        rendered.contains(
            "linear value `task` reaches scope exit without being consumed or transferred"
        ),
        "request_cancel should preserve the original live claim:\n{rendered}"
    );
}

#[test]
fn blocking_executor_custody_claims_hold_through_the_concrete_pin() {
    // BLOCKEXEC concrete pin: the blocking-executor contract surface
    // (bounded submissions, moved custody, linear completion claims,
    // suspension at the wait substrate, provider selection) instantiated
    // at `Token`, since queue/executor machine bodies sit at the
    // generic-machine frontier and real requirement admission rides the
    // TR3-TR8 provider route.
    let pass = pass_canary(fixture_roster::BLOCKEXEC_BLOCKING_EXECUTOR_CUSTODY_CLAIMS_COMPILE);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &pass.join("main.omg"),
        None,
    ))
    .unwrap_or_else(|diagnostics| {
        panic!(
            "blocking-executor custody pin should reach checked trees:\n{}",
            render(&diagnostics)
        )
    });

    // Linear claims: submissions, completion tickets, and the executor
    // itself are exactly-once values.
    for name in ["Submission", "Ticket", "Executor"] {
        let definition = checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == name)
            .unwrap_or_else(|| panic!("checked-in `{name}` declaration"));
        assert_eq!(
            definition.properties.multiplicity,
            language_semantics::Multiplicity::Linear,
            "{name} must carry the `[linear]` claim"
        );
    }

    // Operational envelopes: settlement parks at the wait substrate, the
    // probe carries both suspension and blocking, and plain helpers carry
    // neither.
    let snapshot = checked.typed.snapshot();
    for (name, suspends, blocks, supply, body) in [
        (
            "Ticket::settle",
            true,
            true,
            language_semantics::MachineSupplyMode::CheckedBody,
            true,
        ),
        (
            "WaitSubstrate::park",
            true,
            true,
            language_semantics::MachineSupplyMode::Boundary,
            false,
        ),
        (
            "WaitSubstrate::wake_one",
            false,
            false,
            language_semantics::MachineSupplyMode::Boundary,
            false,
        ),
        (
            "Submission::release",
            false,
            false,
            language_semantics::MachineSupplyMode::CheckedBody,
            true,
        ),
        (
            "Main::probe",
            true,
            true,
            language_semantics::MachineSupplyMode::CheckedBody,
            true,
        ),
    ] {
        let machine = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("checked-in machine `{name}`"));
        assert_eq!(machine.supply_mode, supply, "{name}");
        assert_eq!(machine.body_is_present, body, "{name}");
        let machine_snapshot = snapshot
            .roots
            .machines
            .iter()
            .find(|machine| machine.name == name)
            .unwrap_or_else(|| panic!("snapshot for `{name}`"));
        assert_eq!(machine_snapshot.suspends, suspends, "{name}");
        assert_eq!(machine_snapshot.blocks, blocks, "{name}");
    }

    // Provider selection: build composition admits the canary worker
    // provider for the executor's `WorkerProvider` schema.
    let plan = checked
        .selected_provider_plans()
        .plans()
        .iter()
        .find(|plan| plan.schema.trait_name == "WorkerProvider")
        .expect("selected WorkerProvider plan");
    assert_eq!(plan.provider_type, "CanaryWorkerProvider");
    assert!(
        plan.rows
            .iter()
            .any(|row| row.requirement_identity.contains("execute")),
        "the selected plan must carry the worker `execute` row"
    );
}

#[test]
fn blocking_executor_package_checks_with_the_closed_service_carrier() {
    // BLOCKEXEC package pin: the bundled `blocking-executor` package itself
    // must reach checked trees — its `Executor.runtime` field holds the
    // plain `Service<WorkerProvider>` closed carrier, which an authored
    // `in Bound` qualification would reject outright.
    let package_root = repo_root().join("source/library/blocking-executor/main.omg");
    let checked =
        compile_reviewed_repository_fixture(CheckedCompileRequest::new(&package_root, None))
            .unwrap_or_else(|diagnostics| {
                panic!(
                    "blocking-executor package should reach checked trees:\n{}",
                    render(&diagnostics)
                )
            });

    let executor = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Executor")
        .expect("checked-in `Executor` declaration");
    assert_eq!(
        executor.properties.multiplicity,
        language_semantics::Multiplicity::Linear,
        "Executor must carry the `[linear]` claim"
    );
}

#[test]
fn parked_continuation_is_not_source_addressable_through_task_claims() {
    for &(operation, name) in fixture_roster::PARKED_CONTINUATION_FAIL_CANARIES {
        let diagnostics =
            check_canary(&fail_canary(name)).expect_err("parked continuation access must reject");
        let rendered = render(&diagnostics);
        assert!(
            rendered.contains("has no field `continuation`"),
            "expected compiler-owned continuation opacity for {operation}, got:\n{rendered}"
        );
    }
}

/// One authored dependency occurrence projected from a `build.omg`: the
/// scope the row authorizes, the explicit `depend_as` alias literal when
/// present, and the `Source::Path` `location` string verbatim.
struct FixtureDependencyRow {
    purpose: DependencyPurpose,
    alias: Option<String>,
    location: String,
}

fn build_string_literal(trees: &SyntaxTrees, handle: ExpressionHandle) -> String {
    let ExpressionNode::String(bytes) = trees.expressions.expression(handle) else {
        panic!("fixture dependency argument must be a direct string literal")
    };
    std::str::from_utf8(bytes)
        .expect("fixture dependency literal is utf8")
        .to_owned()
}

/// Extract the `location` of one `Source::Path` literal, rejecting every
/// other source spelling — the fixture harness wires only repository-local
/// path dependencies.
fn depend_source_path_location(trees: &SyntaxTrees, source: ExpressionHandle) -> String {
    let ExpressionNode::StructLiteral(literal) = trees.expressions.expression(source) else {
        panic!("fixture dependency source must be a `Source::Path` literal")
    };
    let constructor = literal.constructor_name.as_str();
    let (owner, case) = constructor
        .rsplit_once("::")
        .map_or((constructor, None), |(owner, case)| (owner, Some(case)));
    assert_eq!(owner, "Source", "dependency source must name `Source`");
    assert_eq!(
        case,
        Some("Path"),
        "fixture dependency wiring admits only `Source::Path`"
    );
    let fields = trees.expressions.struct_fields(literal.fields);
    let [field] = fields else {
        panic!("`Source::Path` takes exactly a `location` field")
    };
    assert_eq!(field.name.as_str(), "location");
    build_string_literal(trees, field.value)
}

/// Project a package root's unconditional `builder.depend*` rows.
/// Conditional `*_when` vocabulary never projects a row and is rejected
/// here — a fixture cannot wire an edge the harness cannot see.
fn fixture_dependency_rows(project_root: &Path) -> Vec<FixtureDependencyRow> {
    let Ok(source) = fs::read_to_string(project_root.join("build.omg")) else {
        return Vec::new();
    };
    let tokens = Lexer::new(&source).tokenize().expect("fixture build lexes");
    let trees = parse_syntax_trees(&tokens).expect("fixture build parses");
    let build = project_build_entry_syntax(&trees).expect("fixture build entry");
    let rows = project_dependency_rows(&trees, &build)
        .expect("fixture dependency rows project")
        .into_iter()
        .map(|row| {
            let alias = row
                .alias()
                .map(|handle| build_string_literal(&trees, handle));
            FixtureDependencyRow {
                purpose: row.purpose(),
                alias,
                location: depend_source_path_location(&trees, row.source()),
            }
        })
        .collect();
    assert!(
        !trees
            .root_items()
            .any(|item| format!("{item:?}").contains("_when(")),
        "fixture dependency wiring cannot cover conditional depend rows"
    );
    rows
}

/// Wire a fixture's authored `builder.depend*` `Source::Path` rows into
/// real package inputs: each row's location resolves against the
/// declaring root, the dependency's own `build.omg` supplies the package
/// name, and the requester-local alias is the explicit `depend_as`
/// literal or the package name's default alias. The bundled standard
/// library keeps the shared fixture identity so the reviewed-entry and
/// service bindings still apply; every other package receives a fresh
/// marker identity. Rows project transitively so a dependency's own
/// path dependencies join the same inputs.
pub(super) fn depend_edge_package_inputs(root_path: &Path) -> Option<PackageCompilationInputs> {
    let project_root = root_path
        .parent()
        .expect("fixture source has a project root");
    if fixture_dependency_rows(project_root).is_empty() {
        return None;
    }

    let declaration = extract_build_declaration(project_root)
        .unwrap_or_else(|error| panic!("fixture {}: {error}", project_root.display()));
    let root_role = declaration.kind();
    let root_name = match declaration {
        BuildDeclaration::Application(application) => application.name,
        BuildDeclaration::Package(package) => package.name,
        BuildDeclaration::Workspace(_) => {
            panic!(
                "fixture {} cannot be a workspace root",
                project_root.display()
            )
        }
    };
    let root_identity = fixture_package_identity(1);
    let standard_library_root = normalize_fixture_path(&repo_root().join("source/library/std"));

    let mut packages = vec![PackageSourceBinding::new(
        root_identity,
        root_name.into_string(),
        project_root.to_path_buf(),
    )];
    let mut dependencies = Vec::new();
    let mut bound: HashMap<PathBuf, (PackageKeyIdentity, String)> = HashMap::new();
    bound.insert(
        normalize_fixture_path(project_root),
        (root_identity, String::new()),
    );
    let mut pending = vec![(normalize_fixture_path(project_root), root_identity)];
    // Marker 2 is the bundled standard library; others allocate upward.
    let mut next_marker: u8 = 3;
    while let Some((root, requester)) = pending.pop() {
        for row in fixture_dependency_rows(&root) {
            let dependency_root = normalize_fixture_path(&root.join(&row.location));
            let (target, target_name) = match bound.get(&dependency_root) {
                Some(bound_target) => bound_target.clone(),
                None => {
                    let dependency_declaration = extract_build_declaration(&dependency_root)
                        .unwrap_or_else(|error| {
                            panic!("dependency {}: {error}", dependency_root.display())
                        });
                    let BuildDeclaration::Package(package) = dependency_declaration else {
                        panic!(
                            "dependency {} must be a package root",
                            dependency_root.display()
                        )
                    };
                    let name = package.name.into_string();
                    let identity = if dependency_root == standard_library_root {
                        fixture_package_identity(2)
                    } else {
                        let marker = next_marker;
                        next_marker += 1;
                        fixture_package_identity(marker)
                    };
                    packages.push(PackageSourceBinding::new(
                        identity,
                        name.clone(),
                        dependency_root.clone(),
                    ));
                    bound.insert(dependency_root.clone(), (identity, name.clone()));
                    pending.push((dependency_root, identity));
                    (identity, name)
                }
            };
            let alias = row.alias.unwrap_or_else(|| target_name.replace('-', "_"));
            dependencies.push(PackageDependencyBinding::for_purpose(
                requester,
                alias,
                target,
                row.purpose,
            ));
        }
    }

    Some(
        PackageCompilationInputs::new(root_identity, root_role, packages, dependencies)
            .unwrap_or_else(|errors| panic!("fixture {}: {errors:#?}", project_root.display())),
    )
}

fn normalize_fixture_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

#[test]
fn blocking_executor_consumer_compiles_through_the_depend_edge() {
    // BLOCKEXEC depend-edge leg: the consumer fixture's authored
    // `builder.depend` row reaches the bundled blocking-executor package
    // through real package inputs, so `use blocking_executor::executor`
    // resolves the package's own contract declarations.
    let pass = pass_canary(fixture_roster::BLOCKEXEC_BLOCKING_EXECUTOR_CONSUMER_DEPEND_COMPILE);
    let root = pass.join("main.omg");
    let package_inputs =
        depend_edge_package_inputs(&root).expect("the depend edge wires package inputs");
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs),
        ..CheckedCompileRequest::new(&root, None)
    })
    .unwrap_or_else(|diagnostics| {
        panic!(
            "blocking-executor consumer should reach checked trees:\n{}",
            render(&diagnostics)
        )
    });

    // The package's own contract types arrive through the depend edge.
    for name in ["Submission", "Ticket", "Executor", "BoundedQueue"] {
        let definition = checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == name)
            .unwrap_or_else(|| panic!("checked-in `{name}` declaration"));
        assert_eq!(
            definition.properties.multiplicity,
            language_semantics::Multiplicity::Linear,
            "{name} must carry the `[linear]` claim"
        );
    }

    // Provider selection: the consumer's canary provider satisfies the
    // package's `WorkerProvider` trait through ordinary build composition.
    let plan = checked
        .selected_provider_plans()
        .plans()
        .iter()
        .find(|plan| plan.schema.trait_name == "WorkerProvider")
        .expect("selected WorkerProvider plan");
    assert_eq!(plan.provider_type, "CanaryWorkerProvider");
}

#[test]
fn blocking_executor_isolated_provider_conformance_binds_the_inherited_row() {
    // BLOCKEXEC isolated-provider composition leg: the package declares
    // `IsolatedWorkerProvider: WorkerProvider` as the composition a
    // worker-isolating provider satisfies. The marker trait owns no
    // requirements, so its complete conformance surface is the inherited
    // `WorkerProvider::execute` row — the fixture binds it by reference to
    // the satisfies-clause realization, and the incomplete-conformance twin
    // (FAIL_CANARIES) pins the missing-row rejection.
    let pass = pass_canary(
        fixture_roster::BLOCKEXEC_BLOCKING_EXECUTOR_ISOLATED_PROVIDER_CONFORMANCE_COMPILE,
    );
    let root = pass.join("main.omg");
    let package_inputs =
        depend_edge_package_inputs(&root).expect("the depend edge wires package inputs");
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs),
        ..CheckedCompileRequest::new(&root, None)
    })
    .unwrap_or_else(|diagnostics| {
        panic!(
            "blocking-executor isolated-provider consumer should reach checked trees:\n{}",
            render(&diagnostics)
        )
    });

    // The named conformance to the package's marker trait is retained with
    // the canary provider as its carrier.
    let conformance = checked
        .conformances()
        .iter()
        .find(|conformance| conformance.trait_name.as_str() == "IsolatedWorkerProvider")
        .expect("checked-in `IsolatedWorkerProvider` conformance");
    assert_eq!(
        conformance.carrier_name().map(|name| name.as_str()),
        Some("CanaryIsolatedWorkerProvider"),
        "the marker-trait conformance must name the canary provider as carrier"
    );

    // The same provider is still selectable for the inherited `WorkerProvider`
    // slot through ordinary build composition.
    let plan = checked
        .selected_provider_plans()
        .plans()
        .iter()
        .find(|plan| plan.schema.trait_name == "WorkerProvider")
        .expect("selected WorkerProvider plan");
    assert_eq!(plan.provider_type, "CanaryIsolatedWorkerProvider");

    // The fence twin: an `IsolatedWorkerProvider` conformance binding no row
    // rejects as incomplete — ambient machines never fill trait rows.
    let fail = fail_canary(
        fixture_roster::BLOCKEXEC_BLOCKING_EXECUTOR_ISOLATED_PROVIDER_INCOMPLETE_CONFORMANCE,
    );
    let expected = fs::read_to_string(fail.join("expected.txt"))
        .expect("incomplete-conformance fail canary should carry expected.txt");
    let fail_root = fail.join("main.omg");
    let fail_package_inputs =
        depend_edge_package_inputs(&fail_root).expect("the depend edge wires package inputs");
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(fail_package_inputs),
        ..CheckedCompileRequest::new(&fail_root, None)
    })
    .expect_err("an empty IsolatedWorkerProvider conformance must reject");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains(expected.trim()),
        "{} missing expected fragment {:?}:\n{combined}",
        fail.display(),
        expected.trim()
    );
}
