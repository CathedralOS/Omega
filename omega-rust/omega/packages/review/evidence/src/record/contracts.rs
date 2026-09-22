//! Stable callable-contract, expression, proposition, and declaration vocabulary.

use super::identity::PackageReviewNominalIdentity;

mod callable_contracts;
mod declarations;
mod expressions;
mod propositions;
mod stand_downs;

pub use callable_contracts::{
    PackageReviewCallableContract, PackageReviewContractFact, PackageReviewResultCaseIdentity,
    PackageReviewSynchronousInvocation,
};
pub use declarations::{PackageReviewConstShape, PackageReviewOperatorShape};
pub use expressions::{
    PackageReviewArithmeticDomain, PackageReviewAtomicLoadOrdering,
    PackageReviewByteSequencePredicate, PackageReviewCallableRole, PackageReviewCallableSupply,
    PackageReviewCastForm, PackageReviewCollectionViewOperation, PackageReviewConstructorField,
    PackageReviewContractBinaryOperator, PackageReviewContractCallTarget,
    PackageReviewContractEvidenceArgument, PackageReviewContractEvidenceTerm,
    PackageReviewContractExpression, PackageReviewContractKind,
    PackageReviewContractOperatorMeaning, PackageReviewContractStaticArgument,
    PackageReviewContractUnaryOperator, PackageReviewFloatLiteral, PackageReviewOperatorCoordinate,
    PackageReviewOperatorRealization, PackageReviewReferenceAccess,
};
pub use propositions::{
    PackageReviewEvidenceInterface, PackageReviewEvidenceRequirement,
    PackageReviewPropositionApplication, PackageReviewPropositionBinder,
    PackageReviewPropositionBinderArgument, PackageReviewPropositionBinderArgumentKind,
    PackageReviewPropositionBinderKind, PackageReviewPropositionBinderValue,
    PackageReviewPropositionEvidence, PackageReviewPropositionParameterApplication,
    PackageReviewPropositionShape, PackageReviewPublicPropositionBody,
};
pub use stand_downs::{
    PackageReviewContractEntailmentAssumptionDischarge,
    PackageReviewContractEntailmentOpenObligation, PackageReviewContractEntailmentOpenReason,
};
