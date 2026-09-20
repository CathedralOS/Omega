//! Module paths retain distinct nominal provider and requirement identities.

use super::{TempProject, application_build};
#[path = "../support/macos_entry_acceptance.rs"]
mod macos_entry_acceptance;
#[path = "../support/windows_entry_acceptance.rs"]
mod windows_entry_acceptance;
use compiler::{
    CheckedCompileRequest, CompileOptions, CompileRequest, RequestedCompileProduct, compile,
    compile_to_checked,
};

fn checked(project: &TempProject) -> compiler::CheckedCompilation {
    compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("macos_arm64"),
    ))
    .unwrap_or_else(|diagnostics| panic!("qualified nominal selection must check: {diagnostics:?}"))
}

#[test]
fn qualified_trait_and_provider_retain_the_exact_module() {
    let project = TempProject::with_main("use ports;", &application_build(""));
    std::fs::write(
        project.0.join("ports.omg"),
        r#"module ports;
pub boundary trait Reader { machine read() -> i32; }
pub data Provider {}
machine Provider::read() -> i32 satisfies Reader::read { 37 }
"#,
    )
    .unwrap();
    let checked = checked(&project);
    let plans = checked.selected_provider_plans().plans();
    assert!(
        plans
            .iter()
            .any(|plan| plan.schema.trait_name == "ports::Reader"
                && plan.provider_type == "Provider")
    );
}

#[test]
fn same_leaf_traits_keep_distinct_module_schemas() {
    let project = TempProject::with_main("use left; use right;", &application_build(""));
    for module in ["left", "right"] {
        std::fs::write(project.0.join(format!("{module}.omg")), format!(
            "module {module}; pub boundary trait Reader {{ machine read() -> i32; }} pub data Provider {{}} machine Provider::read() -> i32 satisfies Reader::read {{ 37 }}"
        )).unwrap();
    }
    let checked = checked(&project);
    for module in ["left", "right"] {
        assert!(
            checked
                .selected_provider_plans()
                .plans()
                .iter()
                .any(|plan| plan.schema.trait_name == format!("{module}::Reader")
                    && plan.provider_type == "Provider")
        );
    }
    let plan = checked
        .selected_provider_plans()
        .plans()
        .iter()
        .find(|plan| plan.schema.trait_name == "left::Reader")
        .expect("left schema");
    let other = checked
        .selected_provider_plans()
        .plans()
        .iter()
        .find(|plan| plan.schema.trait_name == "right::Reader")
        .expect("right schema");
    assert_ne!(
        plan.schema.methods[0].requirement_identity,
        other.schema.methods[0].requirement_identity
    );
    let mut wrong_provider = plan.clone();
    wrong_provider.provider_type = "right::Provider".into();
    let mut wrong_adapter = plan.clone();
    wrong_adapter.rows[0].binding = other.rows[0].binding.clone();
    let mut wrong_requirement = plan.clone();
    wrong_requirement.schema.methods[0].requirement_owner = "right::Reader".into();
    let mut leaf_only = plan.clone();
    leaf_only.schema.trait_name = "Reader".into();
    for forged in [wrong_provider, wrong_adapter, wrong_requirement, leaf_only] {
        assert!(
            !provider_planning::validate_provider_plan_candidates(&checked.typed, &[forged])
                .is_empty(),
            "same-leaf and shortened identities must not substitute for the exact selected declarations"
        );
    }
    let derived = provider_planning::derive_satisfies_plans(
        &checked.typed,
        provider_planning::ProviderPlanDerivation::unevaluated(Some("macos_arm64")),
    );
    let left = derived
        .iter()
        .find(|candidate| candidate.plan.schema.trait_name == "left::Reader")
        .expect("left provenance");
    let right = derived
        .iter()
        .find(|candidate| candidate.plan.schema.trait_name == "right::Reader")
        .expect("right provenance");
    assert!(
        provider_planning::validate_derived_provider_plan_candidates(
            &checked.typed,
            checked.evaluated_via_bindings(),
            std::slice::from_ref(left)
        )
        .is_empty()
    );
    let mut wrong_provider = left.clone();
    wrong_provider.provenance.provider_type = right.provenance.provider_type;
    let mut wrong_schema = left.clone();
    wrong_schema.provenance.schema = right.provenance.schema;
    for forged in [wrong_provider, wrong_schema] {
        assert!(
            !provider_planning::validate_derived_provider_plan_candidates(
                &checked.typed,
                checked.evaluated_via_bindings(),
                &[forged]
            )
            .is_empty(),
            "substituted exact provenance must reject"
        );
    }
    let mut local_implementation = checked.typed.clone();
    local_implementation.push_conformance(typed_trees::trait_definition::Conformance {
        subject: typed_trees::trait_definition::ConformanceSubject::Carrier(
            typed_trees::name::Identifier::generated("Provider"),
        ),
        carrier_symbol: left
            .provenance
            .provider_type
            .expect("nominal left provider"),
        trait_name: typed_trees::name::Identifier::generated("Reader"),
        trait_symbol: left.provenance.schema.symbol(),
        ..Default::default()
    });
    let registry = effects::build_boundary_provider_approval_registry(&local_implementation);
    assert!(
        !registry
            .authorize_boundary_call(left.provenance.schema.symbol())
            .is_approved(),
        "whole-trait local implementation cannot mint external authority"
    );
    assert!(
        registry
            .authorize_boundary_call(right.provenance.schema.symbol())
            .is_approved(),
        "revocation is exact to the implemented trait, not its shared leaf name"
    );
    let mut duplicate = checked.typed.clone();
    let declaration = duplicate
        .traits()
        .iter()
        .find(|definition| definition.symbol == left.provenance.schema.symbol())
        .expect("left declaration")
        .clone();
    duplicate.push_trait_definition(declaration);
    let registry = effects::build_boundary_provider_approval_registry(&duplicate);
    assert!(
        !registry
            .authorize_boundary_call(left.provenance.schema.symbol())
            .is_approved(),
        "a duplicated exact boundary declaration still fails closed"
    );
}

#[test]
fn same_leaf_providers_do_not_merge_before_selection() {
    let project =
        TempProject::with_main("use contract; use left; use right;", &application_build(""));
    std::fs::write(
        project.0.join("contract.omg"),
        "module contract; pub boundary trait Reader { machine read() -> i32; }",
    )
    .unwrap();
    for (module, result) in [("left", 11), ("right", 37)] {
        std::fs::write(project.0.join(format!("{module}.omg")), format!(
            "module {module}; use contract::Reader; pub data Provider {{}} machine Provider::read() -> i32 satisfies Reader::read {{ {result} }}"
        )).unwrap();
    }
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("macos_arm64"),
    ))
    .map(|_| ())
    .expect_err("two providers need an explicit selection");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("two covering provider plans")),
        "{diagnostics:?}"
    );
}

#[test]
fn same_leaf_attached_scalar_callers_keep_module_local_invocation_custody() {
    // Keep the scalar/multi-state customer alongside the native Unit case.
    // This test pins checking and exact invocation custody, not native
    // scalar-call state crossing or entry-receiver establishment.
    let project = TempProject::with_main("use left; use right;", &application_build(""));
    for (module, result) in [("left", 11), ("right", 37)] {
        std::fs::write(
            project.0.join(format!("{module}.omg")),
            format!(
                r#"module {module};
use omega::language::core::service;
pub boundary trait Reader {{ machine read() -> i32; }}
pub data Provider {{}}
machine Provider::read() -> i32 satisfies Reader::read {{ {result} }}
pub data Runner {{ reader: Service<Reader>; }}
pub machine Runner::run(&mut self) reaches Reader invokes Reader; crashes Trap {{
    let value: i32 = self.reader.read();
    transition value == {result} {{ true -> done() _ -> wrong() }}
    state done(&mut self) {{}}
    state wrong(&mut self) {{ crash Trap; }}
}}
"#
            ),
        )
        .expect("module scalar caller fixture");
    }
    let checked = checked(&project);
    for module in ["left", "right"] {
        let machine = checked
            .typed
            .machines()
            .iter()
            .find(|machine| {
                checked.typed.symbols.display_path(machine.symbol, "::")
                    == format!("{module}::Runner::run")
            })
            .expect("module caller");
        let requirement = checked
            .typed
            .traits()
            .iter()
            .find(|definition| {
                checked.typed.trait_declaration_path(definition) == format!("{module}::Reader")
            })
            .expect("module requirement");
        let [invocation] = checked.typed.machine_invokes(machine) else {
            panic!("one exact declared invocation");
        };
        assert_eq!(
            invocation.target,
            typed_trees::signature::AuthoredInvocationTarget::Service(requirement.symbol)
        );
    }
}

#[test]
fn same_leaf_module_providers_dispatch_to_their_own_native_bodies() {
    let Some(profile) = target::TargetProfile::host_if_supported() else {
        eprintln!("SKIP: provider execution requires a supported hosted target");
        return;
    };
    for (entry_module, expected) in [("left", 11), ("right", 37)] {
        let project = TempProject::with_main(
            &format!(
                r#"use left; use right;
use {entry_module}::Reader;
use omega_language_std::console;
use omega::language::core::service;
data Main {{ reader: Service<Reader>; }}
machine Main::run(&mut self) reaches Reader, Console invokes Reader; {{
    block self.reader.check();
}}
"#
            ),
            &application_build(&format!(
                "builder.roots.bind({}::ProgramEntry, Main::run);",
                profile.root_slot_owner_name()
            )),
        );
        for (module, result) in [("left", 11), ("right", 37)] {
            std::fs::write(
                project.0.join(format!("{module}.omg")),
                format!(
                    r#"module {module};
use omega_language_std::console;
pub boundary trait Reader {{ machine check() reaches Console invokes Console; }}
pub data Provider {{}}
machine Provider::check() satisfies Reader::check reaches Console invokes Console; {{
    Console::exit_process({result});
}}
"#
                ),
            )
            .expect("module fixture");
        }
        let inputs =
            super::package_inputs_with_standard_library(&project.main(), "target-activation");
        let standard_library = super::package_identity(2);
        let library_root = inputs
            .package_root(standard_library)
            .expect("standard library");
        let entry_binding = match profile.target_name() {
            "macos_arm64" => macos_entry_acceptance::candidate_macos_entry_binding(
                library_root,
                standard_library,
            ),
            "linux_x86_64" => super::linux_entry_acceptance::candidate_linux_x86_64_entry_binding(
                library_root,
                standard_library,
            ),
            "linux_arm64" => super::linux_entry_acceptance::candidate_linux_arm64_entry_binding(
                library_root,
                standard_library,
            ),
            "windows_x86_64" => windows_entry_acceptance::candidate_windows_x86_64_entry_binding(
                library_root,
                standard_library,
            ),
            target => panic!("missing fixture entry acceptance for supported host {target}"),
        }
        .expect("accept the exact dependency-owned entry contract");
        let mut bindings = vec![entry_binding];
        let inputs = inputs
            .with_accepted_semantic_bindings(bindings.clone())
            .expect("entry acceptance");
        let preliminary = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(inputs.clone()),
            ..CheckedCompileRequest::new(&project.main(), Some(profile.target_name()))
        })
        .unwrap_or_else(|diagnostics| {
            panic!("module provider application must check: {diagnostics:?}")
        });
        bindings.push(
            super::console_acceptance::candidate_console_exit_binding(
                &preliminary,
                standard_library,
                false,
                false,
            )
            .expect("accept exact std exit provider"),
        );
        let inputs = inputs
            .with_accepted_semantic_bindings(bindings)
            .expect("exit acceptance");
        let permissions = native_realization::terminal_authority_permission_policy_with_rows(
            inputs
                .accepted_semantic_bindings()
                .flat_map(|binding| binding.terminal_authority_permissions().iter().cloned())
                .collect(),
        )
        .expect("exit permission");
        let report = compile(
            CompileRequest::new(CompileOptions {
                root_path: project.main(),
                build_dir: None,
                target_name: Some(profile.target_name().into()),
            })
            .with_requested_product(RequestedCompileProduct::NativeArtifact)
            .with_package_inputs(inputs)
            .with_terminal_authority_permission_policy(permissions),
        )
        .and_then(compiler::CompileOutcomes::into_single_report)
        .unwrap_or_else(|diagnostics| {
            panic!("module providers must compile natively: {diagnostics:?}")
        });
        let published = report
            .publish_retained_native_artifact(&project.0.join("out"))
            .expect("native publication");
        let output = std::process::Command::new(
            published
                .checked_native_executable_path()
                .expect("checked executable"),
        )
        .output()
        .expect("execute module providers");
        assert_eq!(
            output.status.code(),
            Some(expected),
            "{entry_module}: {output:?}"
        );
    }
}

#[test]
fn module_qualified_synchronous_cycles_still_reject() {
    for return_edge in [false, true] {
        let project = TempProject::with_main("use ports;", &application_build(""));
        let body = if return_edge {
            "self.alpha.alpha();"
        } else {
            ""
        };
        std::fs::write(
            project.0.join("ports.omg"),
            format!(
                r#"module ports;
use omega::language::core::service;
pub boundary trait Alpha {{ machine alpha(&mut self) reaches Alpha, Beta invokes Beta; }}
pub boundary trait Beta {{ machine beta(&mut self) reaches Alpha, Beta invokes Alpha; }}
data AlphaProvider {{ beta: Service<Beta>; }}
machine AlphaProvider::alpha_checked(&mut self) satisfies Alpha::alpha reaches Alpha, Beta {{ self.beta.beta(); }}
data BetaProvider {{ alpha: Service<Alpha>; }}
machine BetaProvider::beta_checked(&mut self) satisfies Beta::beta reaches Alpha, Beta {{ {body} }}
"#
            ),
        )
        .expect("module cycle fixture");
        let result = compile_to_checked(CheckedCompileRequest::new(
            &project.main(),
            Some("macos_arm64"),
        ));
        if return_edge {
            let diagnostics = result
                .map(|_| ())
                .expect_err("selected synchronous cycle must reject");
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic
                    .message
                    .contains("cyclic synchronous")
                    && diagnostic.message.contains("ports::Alpha")
                    && diagnostic.message.contains("ports::Beta")),
                "{diagnostics:?}"
            );
        } else {
            result.unwrap_or_else(|diagnostics| {
                panic!("an unrealized return edge is not a cycle: {diagnostics:?}")
            });
        }
    }
}
