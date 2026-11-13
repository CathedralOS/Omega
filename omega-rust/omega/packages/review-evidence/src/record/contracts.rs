// Stable callable-contract, expression, proposition, and declaration vocabulary.

mod callable_contracts;
mod declarations;
mod expressions;
mod propositions;

pub use callable_contracts::{PackageReviewCallableContract, PackageReviewSynchronousInvocation};
pub(crate) use callable_contracts::{PackageReviewContractFact, PackageReviewResultCaseIdentity};
pub(crate) use declarations::{
    PackageReviewConstMeaning, PackageReviewConstShape, PackageReviewOperatorShape,
};

pub(crate) use expressions::{
    PackageReviewArithmeticDomain, PackageReviewAtomicLoadOrdering,
    PackageReviewByteSequencePredicate, PackageReviewCallableRole, PackageReviewCallableSupply,
    PackageReviewCastForm, PackageReviewConstructorField, PackageReviewContractBinaryOperator,
    PackageReviewContractCallTarget, PackageReviewContractEvidenceArgument,
    PackageReviewContractEvidenceOwner, PackageReviewContractEvidenceTerm,
    PackageReviewContractExpression, PackageReviewContractKind,
    PackageReviewContractOperatorMeaning, PackageReviewContractStaticArgument,
    PackageReviewContractUnaryOperator, PackageReviewFloatLiteral, PackageReviewLayoutQueryKind,
    PackageReviewMatchArm, PackageReviewOperatorCoordinate, PackageReviewOperatorRealization,
    PackageReviewReferenceAccess,
};

pub(crate) use propositions::{
    PackageReviewEvidenceInterface, PackageReviewEvidenceRequirement,
    PackageReviewPropositionApplication, PackageReviewPropositionBinder,
    PackageReviewPropositionBinderArgument, PackageReviewPropositionBinderArgumentKind,
    PackageReviewPropositionBinderKind, PackageReviewPropositionBinderValue,
    PackageReviewPropositionEvidence, PackageReviewPropositionParameterApplication,
    PackageReviewPropositionShape, PackageReviewPublicPropositionBody,
};
