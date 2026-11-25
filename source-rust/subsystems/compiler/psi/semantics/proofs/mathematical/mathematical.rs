//! Check explicit mathematical certificates with the selected predicative kernel.
//! Terms, declarations, reductions and typing judgments belong to this package.
#![forbid(unsafe_code)]

mod certificate;
mod certificate_abstraction;
mod certificate_strict;
mod conversion;
mod indexed_data;
mod inductive;
mod plain_data;
mod quotient;
pub mod scheme_dsl;
mod signature;
mod substitution;
mod term;
mod theorems;
mod typing;

#[cfg(test)]
mod tests;

pub use certificate::{
    MathematicalCertificate, MathematicalCertificateBatchError, certificate_assumption_closure,
    certificate_type_signature_prefix, verify_mathematical_certificate,
    verify_mathematical_certificates,
};
pub use certificate_abstraction::{
    AbstractedCertificate, AbstractedType, CertificateAbstractionError, MathematicalJudgmentType,
    abstract_certificate_assumptions, abstract_judgment_type,
};
pub use certificate_strict::{StrictCertificateError, strict_certificate_context};
pub use conversion::Budget;
pub use conversion::{
    DEFAULT_CONVERSION_STEPS, convertible, convertible_retaining, weak_head_normalize,
    weak_head_normalize_retaining,
};
pub use indexed_data::{
    ConstructorArguments, DeclaredIndexedData, IndexedCase, IndexedData, IndexedDataError,
    IndexedField, IndexedInduction, IndexedInstance, IndexedValue,
};
pub use inductive::{declare_inductive_family, declare_mutual_inductive_family};
pub use plain_data::PlainData;
pub use quotient::{
    EquivalenceProofs, QUOTIENT, QUOTIENT_BETA, QUOTIENT_BOX_PROP, QUOTIENT_COVERAGE,
    QUOTIENT_EFFECTIVE, QUOTIENT_ELIM, QUOTIENT_ID_CANCEL, QUOTIENT_ID_SYM, QUOTIENT_ID_TRANS,
    QUOTIENT_IND_PROP, QUOTIENT_IS_SET, QUOTIENT_LIFT, QUOTIENT_PROJECT, QUOTIENT_PROP_IS_SET,
    QUOTIENT_SET, QUOTIENT_SOUND, QUOTIENT_TRANSPORT, QUOTIENT_TRANSPORT_CONST, QUOTIENT_UNIQUE,
    QuotientFamily, quotient_scheme,
};
pub use signature::{
    Declaration, DeclarationKind, Signature, SignatureCheckError, assumption_closure,
    check_signature, check_signature_with_attribution, judgment_assumption_closure,
};
pub use substitution::{
    instantiate, instantiate_levels, relocate_constants, shift, substitute,
    universe_parameter_arity,
};
pub use term::{Level, Sort, Term, TermArena, TermHandle, sorts_equal};
pub use theorems::identity_substitution;
pub use typing::{Context, CoreError, check_type, infer_sort, infer_type};
