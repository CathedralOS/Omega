use crate::record::{
    PackageReviewContractStaticArgument, PackageReviewNominalIdentity, PackageReviewTypeIdentity,
};
use psi_symbols::SymbolHandle;

pub(super) struct ProjectedSelectedConformanceApplication {
    pub(super) declaration: PackageReviewNominalIdentity,
    pub(super) lifetime_arguments: Vec<u32>,
    pub(super) arguments: Vec<PackageReviewContractStaticArgument>,
    pub(super) subject: PackageReviewContractStaticArgument,
    pub(super) trait_symbol: SymbolHandle,
    pub(super) trait_lifetime_arguments: Vec<u32>,
    pub(super) trait_arguments: Vec<PackageReviewTypeIdentity>,
}
