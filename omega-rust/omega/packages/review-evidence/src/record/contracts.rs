//! Stable callable-contract, expression, proposition, and declaration vocabulary.

use super::identity::PackageReviewNominalIdentity;

mod callable_contracts;
mod declarations;
mod expressions;
mod propositions;

pub use callable_contracts::{
    PackageReviewCallableContract,
    PackageReviewSynchronousInvocation,
};
pub(crate) use callable_contracts::{ PackageReviewContractFact, PackageReviewResultCaseIdentity};
pub(crate) use declarations::{PackageReviewConstShape, PackageReviewOperatorShape};
pub use expressions::{
};
pub(crate) use expressions::{
    PackageReviewArithmeticDomain, PackageReviewAtomicLoadOrdering,
    PackageReviewByteSequencePredicate, PackageReviewCallableRole, PackageReviewCallableSupply,
    PackageReviewCastForm, PackageReviewCollectionViewOperation, PackageReviewConstructorField,
    PackageReviewContractBinaryOperator, PackageReviewContractCallTarget,
    PackageReviewContractEvidenceArgument, PackageReviewContractEvidenceTerm,
    PackageReviewContractExpression, PackageReviewContractKind,
    PackageReviewContractOperatorMeaning, PackageReviewContractStaticArgument,
    PackageReviewContractUnaryOperator, PackageReviewFloatLiteral, PackageReviewOperatorCoordinate,
    PackageReviewOperatorRealization, PackageReviewReferenceAccess};
pub use propositions::{
};
pub(crate) use propositions::{
    PackageReviewEvidenceInterface, PackageReviewEvidenceRequirement,
    PackageReviewPropositionApplication, PackageReviewPropositionBinder,
    PackageReviewPropositionBinderArgument, PackageReviewPropositionBinderArgumentKind,
    PackageReviewPropositionBinderKind, PackageReviewPropositionBinderValue,
    PackageReviewPropositionEvidence, PackageReviewPropositionParameterApplication,
    PackageReviewPropositionShape, PackageReviewPublicPropositionBody};
