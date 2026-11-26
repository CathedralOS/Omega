// Stable callable-contract, expression, proposition, and declaration vocabulary.

mod callable_contracts;
mod expressions;
mod propositions;

use crate::record::accessors::field_projection_accessors;
use crate::record::{PackageReviewNominalIdentity, PackageReviewTypeIdentity};
pub use callable_contracts::{
    PackageReviewCallableContract, PackageReviewContractFact, PackageReviewResultCaseIdentity,
    PackageReviewSynchronousInvocation,
};
pub use expressions::{
    PackageReviewArithmeticDomain, PackageReviewByteSequencePredicate, PackageReviewCallableSupply,
    PackageReviewCastForm, PackageReviewContractBinaryOperator, PackageReviewContractCallTarget,
    PackageReviewContractExpression, PackageReviewContractKind,
    PackageReviewContractOperatorMeaning, PackageReviewFloatLiteral, PackageReviewLayoutQueryKind,
    PackageReviewOperatorCoordinate, PackageReviewReferenceAccess,
};
pub(crate) use expressions::{
    PackageReviewAtomicLoadOrdering, PackageReviewConstructorField,
    PackageReviewContractEvidenceArgument, PackageReviewContractEvidenceTerm,
    PackageReviewContractStaticArgument, PackageReviewContractUnaryOperator, PackageReviewMatchArm,
    PackageReviewOperatorRealization,
};
pub use propositions::{PackageReviewEvidenceInterface, PackageReviewEvidenceRequirement};
pub(crate) use propositions::{
    PackageReviewPropositionApplication, PackageReviewPropositionBinder,
    PackageReviewPropositionBinderArgument, PackageReviewPropositionBinderArgumentKind,
    PackageReviewPropositionBinderKind, PackageReviewPropositionBinderValue,
    PackageReviewPropositionEvidence, PackageReviewPropositionParameterApplication,
    PackageReviewPropositionShape, PackageReviewPublicPropositionBody,
};

// Stable public constant declaration shapes (`PackageReviewConstShape`).
//
// `PackageReviewConstMeaning` distinguishes a completed evaluated value from an
// admitted target-dependent contract expression so a folded scalar never
// replaces its target dependency.

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewConstShape {
    pub identity: PackageReviewNominalIdentity,
    pub declared_type: PackageReviewTypeIdentity,
    pub meaning: PackageReviewConstMeaning,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewConstMeaning {
    Evaluated(String),
    TargetDependent(self::expressions::PackageReviewContractExpression),
}

impl PackageReviewConstShape {
    field_projection_accessors!(borrowed identity: &PackageReviewNominalIdentity);
    field_projection_accessors!(borrowed declared_type: &PackageReviewTypeIdentity);
}
