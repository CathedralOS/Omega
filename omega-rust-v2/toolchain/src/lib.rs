//! The production package profile supplies the standard library to the V2 resolver.
#![forbid(unsafe_code)]
pub use omega_program_contracts::CollectionPrimitive;
mod core_interface;
mod default_build;
pub use default_build::{DefaultBuild, DEFAULT_ROOT_PACKAGE};
pub use core_interface::{CoreInterface, DeclarationQuery, LayoutQuery, core_interface};
use omega_lexer::SourceSpan;
use omega_resolver::{Dependency, NameError, NameErrorKind, StageSource, SymbolResolvedTrees};

pub const TOOLCHAIN_PACKAGE: &str = "@omega/toolchain";

/// Trees resolved with the standard package exports and the toolchain's reserved identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourcePackage<'text> {
    resolved: SymbolResolvedTrees<'text>,
    package_keys: std::collections::BTreeMap<String, [u8; 32]>,
    assembly: Option<std::sync::Arc<omega_typed::AssemblyTypes>>,
}
impl SourcePackage<'static> {
    /// The toolchain checks its own definitions in the reserved package. An importing
    /// source package receives only its interface and retains its own obligations.
    pub fn toolchain_definition() -> Self {
        Self { resolved: core_interface().declarations.clone(), assembly: None, package_keys: Default::default() }
    }
}

impl SourcePackage<'_> {
    /// User source cannot claim the reserved package during resolution.
    pub fn defines_toolchain(&self) -> bool {
        self.resolved.trees().next().is_some_and(|tree| tree.file.package == TOOLCHAIN_PACKAGE)
    }

    /// Bind the owner already established by Build for this resolved source package.
    /// Focused source inspection retains just its local label; a captured default Build supplies the owner.
    pub fn bind_package_key(&mut self, package: &str, key: [u8; 32]) -> Result<(), &'static str> {
        if !self.resolved.trees().any(|tree| tree.file.package == package) {
            return Err("package key has no matching source package");
        }
        if self.package_keys.get(package).is_some_and(|previous| previous != &key) {
            return Err("source package already has a different key");
        }
        self.package_keys.insert(package.to_owned(), key);
        Ok(())
    }

    pub fn package_key(&self, package: &str) -> Option<&[u8; 32]> {
        self.package_keys.get(package)
    }

    pub fn assembly(&self) -> Option<&omega_typed::AssemblyTypes> {
        self.assembly.as_deref()
    }
}

impl<'text> std::ops::Deref for SourcePackage<'text> {
    type Target = SymbolResolvedTrees<'text>;

    fn deref(&self) -> &Self::Target {
        &self.resolved
    }
}

pub fn check_source_identity(
    package: &str,
    file: &str,
) -> Result<(), NameError> {
    if package == TOOLCHAIN_PACKAGE {
        return Err(NameError {
            package: package.into(),
            file: file.into(),
            span: SourceSpan::new(0, 0),
            message: "package identity reserved for the toolchain".into(),
            kind: NameErrorKind::ReservedPackageIdentity,
        });
    }
    Ok(())
}

pub fn resolve_source_package<'text>(sources: &[StageSource<'text, '_>]) -> Result<SourcePackage<'text>, NameError> {
    resolve_package(sources, &[], None)
}

/// Resolve source packages with the direct dependency edges that Build admitted for them.
/// Each source package also receives its `omega` edge to the toolchain. The resolver refuses
/// an admitted edge that gives an alias a second package, including a retargeted `omega`.
pub fn resolve_admitted_packages<'text>(
    sources: &[StageSource<'text, '_>],
    admitted: &[Dependency<'_>],
) -> Result<SourcePackage<'text>, NameError> {
    resolve_package(sources, admitted, None)
}

/// Resolve target names from the same compiler-owned catalog used in typing.
/// An unspecified target keeps its empty instruction vocabulary.
pub fn resolve_source_package_with_assembly<'text>(
    sources: &[StageSource<'text, '_>],
    catalog: std::sync::Arc<omega_typed::AssemblyTypes>,
) -> Result<SourcePackage<'text>, NameError> {
    resolve_package(sources, &[], Some(catalog))
}
fn resolve_package<'text>(
    sources: &[StageSource<'text, '_>],
    admitted: &[Dependency<'_>],
    assembly: Option<std::sync::Arc<omega_typed::AssemblyTypes>>,
) -> Result<SourcePackage<'text>, NameError> {
    for source in sources {
        check_source_identity(source.package, source.source.logical_path)?;
    }
    let dependencies: Vec<_> = sources
        .iter()
        .map(|s| s.package)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .map(|requester| Dependency {
            requester,
            alias: "omega",
            package: TOOLCHAIN_PACKAGE,
        })
        .chain(admitted.iter().copied())
        .collect();
    let mut instructions = Vec::new();
    let mut registers = Vec::new();
    if let Some(catalog) = assembly.as_ref() {
        instructions.extend(catalog.instructions.keys().map(String::as_str));
        // Source names registers only; other catalog state has no source spelling.
        registers.extend(catalog.register_names());
    }
    let names = omega_resolver::AssemblyVocabulary {
        instructions: &instructions,
        registers: &registers,
    };
    let resolved = omega_resolver::resolve_stage_with_exports(sources, &dependencies, &names, core_interface().exports())?;
    Ok(SourcePackage { resolved, assembly, package_keys: Default::default() })
}
