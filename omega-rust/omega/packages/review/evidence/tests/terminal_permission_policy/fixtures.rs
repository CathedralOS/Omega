use super::*;
use effects::provider_plan::ServiceSchema;
use package_compilation::AcceptedSemanticBinding;

pub(super) const FILESYSTEM: &str = r#"pub boundary trait FilesystemHost {
    machine read(descriptor: i32) -> i64;
    machine stat(descriptor: i32) -> i64;
}
"#;

pub(super) struct Fixture {
    pub candidate: CheckedCompilation,
    pub target: TargetProfile,
    pub owner: PackageKeyIdentity,
    pub accepted: AcceptedSemanticBinding,
    pub requirement: String,
    root: TempPackage,
    _dependency: Option<TempPackage>,
    inputs: PackageCompilationInputs,
}

impl Fixture {
    pub fn filesystem(source: &str, foreign: bool, service: &str, method: &str) -> Self {
        Self::new(source, foreign, service, method, false)
    }

    pub fn console() -> Self {
        Self::new(
            r#"pub boundary trait Console {
    machine exit_process(return_code: i32) reaches Console;
}
pub data ConsoleNativeProvider {}
linux_x86_64 boundary machine ConsoleNativeProvider::exit_process(return_code: i32)
    satisfies Console::exit_process;
"#,
            true,
            "Console",
            "exit_process",
            true,
        )
    }

    fn new(source: &str, foreign: bool, service: &str, method: &str, console: bool) -> Self {
        let root = TempPackage::new();
        let dependency = foreign.then(TempPackage::new);
        let owner = if foreign {
            PackageKeyIdentity::from_digest([42; 32]).unwrap()
        } else {
            package_identity()
        };
        let mut sources = vec![PackageSourceBinding::new(
            package_identity(),
            "review-fixture",
            root.0.clone(),
        )];
        let mut dependencies = Vec::new();
        if let Some(dependency) = &dependency {
            dependency.write("service.omg", source);
            root.write("main.omg", "use accepted_service::service;\n");
            sources.push(PackageSourceBinding::new(
                owner,
                "service-package",
                dependency.0.clone(),
            ));
            dependencies.push(PackageDependencyBinding::new(
                package_identity(),
                "accepted_service",
                owner,
            ));
        } else {
            root.write("main.omg", source);
        }
        if source.contains("use calling;") {
            let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .nth(5)
                .unwrap();
            let calling = fs::read_to_string(repository.join("source/library/std/calling.omg"))
                .unwrap()
                .replace("\ndata ", "\npub data ")
                .replace("\ntrait ", "\npub trait ")
                .replace("\ndomain ", "\npub domain ");
            dependency
                .as_ref()
                .unwrap_or(&root)
                .write("calling.omg", &calling);
        }
        root.write(
            "build.omg",
            if console {
                r#"use accepted_service::service;
machine build(builder: &mut Build) {
    builder.package("review-fixture");
    builder.select_provider<Console, ConsoleNativeProvider>();
}
"#
            } else {
                "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n"
            },
        );
        let target = TargetProfile::LinuxX64;
        let inputs =
            PackageCompilationInputs::new_package(package_identity(), sources, dependencies)
                .unwrap();
        let candidate = compile_to_checked_with_packages(
            &root.0.join("main.omg"),
            Some(target.target_name()),
            inputs.clone(),
        )
        .unwrap_or_else(|diagnostics| {
            panic!("terminal permission candidate should check: {diagnostics:#?}")
        });
        let declaration = candidate
            .traits()
            .iter()
            .find(|declaration| {
                declaration.name.as_str() == service
                    && candidate
                        .symbols
                        .symbol_package_identity(declaration.symbol)
                        == Some(owner)
            })
            .unwrap();
        let schema = ServiceSchema::from_typed(&candidate.typed, declaration).unwrap();
        let requirement = schema
            .methods
            .iter()
            .find(|candidate| candidate.name == method)
            .unwrap()
            .requirement_identity
            .clone();
        let accepted = if console {
            let plan = candidate
                .selected_provider_plans()
                .plans()
                .iter()
                .find(|plan| plan.schema.trait_name == service)
                .unwrap();
            AcceptedSemanticBinding::new(
                AcceptedSemanticBindingRole::ConsoleExitProcessI32,
                owner,
                service,
                plan.schema.identity_digest(),
                plan.identity_digest(),
            )
            .unwrap()
        } else {
            candidate
                .candidate_service_binding(
                    AcceptedSemanticBindingRole::FilesystemHostService,
                    owner,
                    service,
                )
                .unwrap()
        };
        Self {
            candidate,
            target,
            owner,
            accepted,
            requirement,
            root,
            _dependency: dependency,
            inputs,
        }
    }

    pub fn check(&self, permitted: Option<TerminalAuthorityDisposition>) -> CheckedCompilation {
        self.check_binding(self.binding(permitted))
            .unwrap_or_else(|diagnostics| {
                panic!("accepted terminal permission source should check: {diagnostics:#?}")
            })
    }

    pub fn binding(
        &self,
        permitted: Option<TerminalAuthorityDisposition>,
    ) -> AcceptedSemanticBinding {
        self.accepted
            .clone()
            .with_terminal_authority_permissions(
                permitted
                    .into_iter()
                    .map(|permitted| {
                        ServiceTerminalAuthorityPermission::new(
                            self.accepted.normalized_schema_digest(),
                            self.requirement.clone(),
                            permitted,
                        )
                    })
                    .collect(),
            )
            .unwrap()
    }

    pub fn check_binding(
        &self,
        accepted: AcceptedSemanticBinding,
    ) -> Result<CheckedCompilation, Vec<diagnostics::Diagnostic>> {
        compile_to_checked_with_packages(
            &self.root.0.join("main.omg"),
            Some(self.target.target_name()),
            self.inputs
                .clone()
                .with_accepted_semantic_bindings(vec![accepted])
                .unwrap(),
        )
    }
}
