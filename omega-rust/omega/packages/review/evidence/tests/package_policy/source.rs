use super::*;

pub(super) struct Fixture {
    pub checked: CheckedCompilation,
    pub target: TargetProfile,
    root: TempPackage,
    dependency: Option<TempPackage>,
}

impl Fixture {
    pub fn local(source: &str) -> Self {
        Self::new(source, None)
    }

    pub fn with_build(source: &str, build: &str) -> Self {
        Self::with_build_and_foreign(source, build, None)
    }

    pub fn foreign(source: &str, dependency: &str, owner: PackageKeyIdentity) -> Self {
        Self::new(source, Some((dependency, owner)))
    }

    fn new(source: &str, foreign: Option<(&str, PackageKeyIdentity)>) -> Self {
        Self::with_build_and_foreign(
            source,
            "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
            foreign,
        )
    }

    fn with_build_and_foreign(
        source: &str,
        build: &str,
        foreign: Option<(&str, PackageKeyIdentity)>,
    ) -> Self {
        let root = TempPackage::new();
        root.write("main.omg", source);
        root.write("build.omg", build);
        let mut sources = vec![PackageSourceBinding::new(
            package_identity(),
            "review-fixture",
            root.0.clone(),
        )];
        let mut edges = Vec::new();
        let dependency = foreign.map(|(source, owner)| {
            let dependency = TempPackage::new();
            dependency.write("api.omg", source);
            sources.push(PackageSourceBinding::new(
                owner,
                "dependency-package",
                dependency.0.clone(),
            ));
            edges.push(PackageDependencyBinding::new(
                package_identity(),
                "dependency",
                owner,
            ));
            dependency
        });
        let inputs =
            PackageCompilationInputs::new_package(package_identity(), sources, edges).unwrap();
        let target = TargetProfile::WindowsX64;
        let checked = compile_to_checked_with_packages(
            &root.0.join("main.omg"),
            Some(target.target_name()),
            inputs,
        )
        .unwrap_or_else(|diagnostics| panic!("baseline source checks: {diagnostics:#?}"));
        Self {
            checked,
            target,
            root,
            dependency,
        }
    }

    pub fn grant_filesystem_read(&mut self) {
        use omega_effects::{
            ServiceTerminalAuthorityPermission, TerminalAuthorityClass,
            TerminalAuthorityDisposition,
        };
        assert!(self.dependency.is_none(), "permission helper is root-local");
        let accepted = self
            .checked
            .candidate_service_binding(
                AcceptedSemanticBindingRole::FilesystemHostService,
                package_identity(),
                "FilesystemHost",
            )
            .unwrap();
        let declaration = self
            .checked
            .traits()
            .iter()
            .find(|value| value.name.as_str() == "FilesystemHost")
            .unwrap();
        let schema = omega_effects::provider_plan::ServiceSchema::from_typed(
            &self.checked.typed,
            declaration,
        )
        .unwrap();
        let requirement = &schema
            .methods
            .iter()
            .find(|method| method.name == "read")
            .unwrap()
            .requirement_identity;
        let permission = ServiceTerminalAuthorityPermission::new(
            accepted.normalized_schema_digest(),
            requirement.clone(),
            TerminalAuthorityDisposition::from_classes([
                TerminalAuthorityClass::FilesystemContentRead,
            ]),
        );
        let accepted = accepted
            .with_terminal_authority_permissions(vec![permission])
            .unwrap();
        self.checked = compile_to_checked_with_packages(
            &self.root.0.join("main.omg"),
            Some(self.target.target_name()),
            package_inputs(&self.root.0)
                .with_accepted_semantic_bindings(vec![accepted])
                .unwrap(),
        )
        .expect("filesystem permission source checks");
    }
}
