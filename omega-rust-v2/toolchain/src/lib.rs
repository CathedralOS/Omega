//! The production package profile supplies the standard library to the V2 resolver.
#![forbid(unsafe_code)]
mod collection;
pub use collection::CollectionPrimitive;
mod core_interface;
pub use core_interface::{CoreInterface, LayoutQuery, core_interface};
use omega_lexer::SourceSpan;
use omega_resolver::{Dependency, NameError, StageSource, SymbolResolvedTrees};

pub const TOOLCHAIN_PACKAGE: &str = "@omega/toolchain";

/// Trees resolved with the standard package exports and the toolchain's reserved identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourcePackage<'text>(SymbolResolvedTrees<'text>);

impl<'text> std::ops::Deref for SourcePackage<'text> {
    type Target = SymbolResolvedTrees<'text>;

    fn deref(&self) -> &Self::Target {
        &self.0
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
        });
    }
    Ok(())
}

pub fn resolve_source_package<'text>(sources: &[StageSource<'text, '_>]) -> Result<SourcePackage<'text>, NameError> {
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
        .collect();
    omega_resolver::resolve_stage_with_exports(sources, &dependencies, &Default::default(), core_interface().exports()).map(SourcePackage)
}
