//! Source declaration roles for exact immutable boundary catalogues.
//!
//! Formation permission comes from fresh package role admission. These rows
//! grant no provider selection, execution, target or terminal authority.

use crate::TypedTrees;
use symbols::SymbolHandle;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum BoundaryCatalogueRole {
    #[default]
    None,
    HostedConsole,
    HostedProcessExit,
    UefiHandoff,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum BoundaryCatalogueIssuer {
    #[default]
    None,
    Proposed,
    Accepted,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct BoundaryCatalogueDeclaration {
    pub role: BoundaryCatalogueRole,
    pub issuer: BoundaryCatalogueIssuer,
    pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
    /// Opaque exact accepted source-role schema. Proposed nomination has no
    /// accepted receipt and retains zero; neither value grants backing.
    pub accepted_schema: [u8; 32],
    pub realization: SymbolHandle,
    pub requirement: SymbolHandle,
    pub source: source::SourceSpan,
}

impl TypedTrees {
    /// Rejoin a freshly supplied source role to this exact source occurrence.
    /// The separate catalogue shape query still validates every signature and
    /// public edge. Neither this receipt nor immutable bytes grant execution.
    pub fn boundary_catalogue_is_admitted(
        &self,
        role: BoundaryCatalogueRole,
        realization: SymbolHandle,
        requirement: SymbolHandle,
    ) -> bool {
        if role == BoundaryCatalogueRole::None || !realization.is_valid() || !requirement.is_valid()
        {
            return false;
        }
        let Some(span) = self.symbols.symbol_source_span(realization) else {
            return false;
        };
        let Some(source) = self.symbols.source_file(span) else {
            return false;
        };
        // A real bundled toolchain source keeps its existing automatic role.
        // Ordinary packages, even byte-for-byte copies, cannot author this
        // provenance and need the explicit package declaration-role input.
        if source.origin == source::SourceOrigin::Toolchain && source.package_identity.is_none() {
            return true;
        }
        let Some(package) = source.package_identity else {
            return false;
        };
        let mut rows = self
            .boundary_catalogue_declarations
            .iter()
            .map(|(_, row)| row)
            .filter(|row| {
                row.role == role
                    && row.issuer != BoundaryCatalogueIssuer::None
                    && (row.issuer != BoundaryCatalogueIssuer::Accepted
                        || row.accepted_schema != [0; 32])
                    && row.package == Some(package)
                    && row.realization == realization
                    && row.requirement == requirement
                    && row.source == span
            });
        rows.next().is_some() && rows.next().is_none()
    }
}
