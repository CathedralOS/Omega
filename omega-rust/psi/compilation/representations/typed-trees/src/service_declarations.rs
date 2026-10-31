//! Source-owned boundary declarations and exact method correspondence.
//!
//! Projection preserves simultaneous type/region scopes and produces no target
//! calling evidence. Omega attaches that evidence to the resulting surface.

pub mod projection;
pub use projection::ProjectedServiceSurface;
mod catalog;
pub use catalog::{
    PreparedServiceCatalog, PreparedServiceDeclaration, PreparedServiceRequirement,
    PreparedServiceSurface, ServiceCatalog,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderSchemaDeclaration {
    BoundaryTrait(symbols::SymbolHandle),
    BoundaryRequirement(symbols::SymbolHandle),
    BoundaryOperator(symbols::SymbolHandle),
    SealedAccessor(symbols::SymbolHandle),
}

impl ProviderSchemaDeclaration {
    pub const fn symbol(self) -> symbols::SymbolHandle {
        match self {
            Self::BoundaryTrait(symbol)
            | Self::BoundaryRequirement(symbol)
            | Self::BoundaryOperator(symbol)
            | Self::SealedAccessor(symbol) => symbol,
        }
    }
}
