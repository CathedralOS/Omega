// Stable callable-contract, expression, proposition, and declaration vocabulary.

mod callable_contracts;
mod declarations;
mod expressions;
mod propositions;

pub use callable_contracts::{PackageReviewCallableContract, PackageReviewSynchronousInvocation};
pub use callable_contracts::{PackageReviewContractFact, PackageReviewResultCaseIdentity};
pub use declarations::{PackageReviewConstMeaning, PackageReviewConstShape};

pub use expressions::PackageReviewLayoutQueryKind;
pub use expressions::{
    PackageReviewArithmeticDomain, PackageReviewCastForm, PackageReviewFloatLiteral,
    PackageReviewReferenceAccess,
};
pub(crate) use expressions::{
    PackageReviewAtomicLoadOrdering, PackageReviewConstructorField,
    PackageReviewContractEvidenceArgument, PackageReviewContractEvidenceTerm,
    PackageReviewContractStaticArgument, PackageReviewContractUnaryOperator, PackageReviewMatchArm,
    PackageReviewOperatorRealization,
};
pub use expressions::{
    PackageReviewByteSequencePredicate, PackageReviewCallableSupply,
    PackageReviewContractBinaryOperator, PackageReviewContractCallTarget,
    PackageReviewContractExpression, PackageReviewContractKind,
    PackageReviewContractOperatorMeaning, PackageReviewOperatorCoordinate,
};

pub use propositions::{PackageReviewEvidenceInterface, PackageReviewEvidenceRequirement};
pub(crate) use propositions::{
    PackageReviewPropositionApplication, PackageReviewPropositionBinder,
    PackageReviewPropositionBinderArgument, PackageReviewPropositionBinderArgumentKind,
    PackageReviewPropositionBinderKind, PackageReviewPropositionBinderValue,
    PackageReviewPropositionEvidence, PackageReviewPropositionParameterApplication,
    PackageReviewPropositionShape, PackageReviewPublicPropositionBody,
};
