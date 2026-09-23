use super::{
    AcceptedSemanticBindingRole, PackageCompilationInputs, PackageDependencyBinding,
    PackageKeyIdentity, PackageSourceBinding, ReviewFixture, ServiceTerminalAuthorityPermission,
    TargetProfile, TempPackage, TerminalAuthorityDisposition, compile_review_fixture,
    package_identity, standard_library_dependency, standard_library_source,
};
use build_declarations::DependencyPurpose;
use compiler::CheckedCompileRequest;
use package_compilation::AcceptedSemanticBinding;

pub(super) const FILESYSTEM: &str = r#"pub boundary trait FilesystemHost {
    machine read(descriptor: i32) -> i64;
    machine stat(descriptor: i32) -> i64;
}
"#;

pub(super) struct Fixture {
    pub candidate: ReviewFixture,
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
        // The seeded entry contract already declares `omega::language::std::calling`;
        // a package-local copy of the vocabulary duplicates it. Sources spell the
        // ordinary dependency alias instead.
        let calling_dependency = source.contains("use calling;");
        let source = if calling_dependency {
            source.replace("use calling;", "use omega_language_std::calling;")
        } else {
            source.to_owned()
        };
        let source = source.as_str();
        let root = TempPackage::new();
        let dependency = foreign.then(TempPackage::new);
        let owner = if foreign {
            PackageKeyIdentity::from_digest([42; 32]).unwrap()
        } else {
            package_identity()
        };
        let mut sources = vec![PackageSourceBinding::new(
            package_identity(),
            "review_fixture",
            root.0.clone(),
        )];
        let mut dependencies = Vec::new();
        if let Some(dependency) = &dependency {
            dependency.write("service.omg", source);
            root.write("main.omg", "use accepted_service::service;\n");
            sources.push(PackageSourceBinding::new(
                owner,
                "service_package",
                dependency.0.clone(),
            ));
            dependencies.push(PackageDependencyBinding::new(
                package_identity(),
                "accepted_service",
                owner,
            ));
            if console {
                // The build entry imports the service too; build imports select build edges.
                dependencies.push(PackageDependencyBinding::for_purpose(
                    package_identity(),
                    "accepted_service",
                    owner,
                    DependencyPurpose::Build,
                ));
            }
        } else {
            root.write("main.omg", source);
        }
        if calling_dependency {
            sources.push(standard_library_source());
            // The import lives in whichever package received the source.
            dependencies.push(standard_library_dependency(owner));
        }
        root.write(
            "build.omg",
            if console {
                r#"use accepted_service::service;
machine build(builder: &mut Build) {
    builder.package("review_fixture");
    builder.select_provider<accepted_service::Console, accepted_service::ConsoleNativeProvider>();
}
"#
            } else {
                "machine build(builder: &mut Build) { builder.package(\"review_fixture\"); }\n"
            },
        );
        // The canonical FilesystemHost mint requires the toolchain's exact
        // leaf surface on linux_x86_64; these fixtures exercise permission
        // policy on an accepted service binding, which is target-agnostic, so
        // they settle on windows_x86_64 where no canonical table applies. The
        // console provider declaration is linux_x86_64-gated, so the console
        // fixture keeps the linux target.
        let target = if console {
            TargetProfile::LinuxX64
        } else {
            TargetProfile::WindowsX64
        };
        let inputs =
            PackageCompilationInputs::new_package(package_identity(), sources, dependencies)
                .unwrap();
        let candidate = compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(inputs.clone()),
            ..CheckedCompileRequest::new(&root.0.join("main.omg"), Some(target.target_name()))
        })
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
        let schema =
            provider_planning::service_schema::from_typed(&candidate.typed, declaration).unwrap();
        let requirement = schema
            .methods
            .iter()
            .find(|candidate| candidate.name == method)
            .unwrap()
            .requirement_identity
            .clone();
        let accepted = if console {
            let plan = candidate
                .custody
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
                .custody
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

    pub fn check(&self, permitted: Option<TerminalAuthorityDisposition>) -> ReviewFixture {
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
    ) -> Result<ReviewFixture, Vec<diagnostics::Diagnostic>> {
        compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(
                self.inputs
                    .clone()
                    .with_accepted_semantic_bindings(vec![accepted])
                    .unwrap(),
            ),
            ..CheckedCompileRequest::new(
                &self.root.0.join("main.omg"),
                Some(self.target.target_name()),
            )
        })
    }
}
