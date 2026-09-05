use super::*;

pub(super) struct Fixture {
    pub checked: CheckedCompilation,
    pub target: TargetProfile,
    _root: TempPackage,
    _dependency: Option<TempPackage>,
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
        let mut dependencies = Vec::new();
        let dependency = foreign.map(|(source, owner)| {
            let package = TempPackage::new();
            package.write("helpers.omg", source);
            sources.push(PackageSourceBinding::new(
                owner,
                "helper-package",
                package.0.clone(),
            ));
            dependencies.push(PackageDependencyBinding::new(
                package_identity(),
                "dependency",
                owner,
            ));
            package
        });
        let inputs =
            PackageCompilationInputs::new_package(package_identity(), sources, dependencies)
                .unwrap();
        let target = TargetProfile::WindowsX64;
        let checked = compile_to_checked_with_packages(
            &root.0.join("main.omg"),
            Some(target.target_name()),
            inputs,
        )
        .unwrap_or_else(|diagnostics| panic!("callable policy fixture checks: {diagnostics:#?}"));
        Self {
            checked,
            target,
            _root: root,
            _dependency: dependency,
        }
    }
}
