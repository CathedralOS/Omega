use crate::record::{
    PackageReviewContractStaticArgument, PackageReviewNominalIdentity, PackageReviewTypeIdentity,
};
use psi_symbols::SymbolHandle;

pub(crate) struct ProjectedSelectedConformanceApplication {
    pub(crate) declaration: PackageReviewNominalIdentity,
    pub(crate) lifetime_arguments: Vec<u32>,
    pub(crate) arguments: Vec<PackageReviewContractStaticArgument>,
    pub(crate) subject: PackageReviewContractStaticArgument,
    pub(crate) trait_symbol: SymbolHandle,
    pub(crate) trait_lifetime_arguments: Vec<u32>,
    pub(crate) trait_arguments: Vec<PackageReviewTypeIdentity>,
}
