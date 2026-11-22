//! Prepared arguments to one constrained construction reached by an access policy.
//! These observations do not establish the constructor or its resulting plan.
use crate::{DeclarationId, DeclarationTerm, Site, TypeExpression};
use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicySchemaValue {
    pub record: DeclarationId,
    pub storage: Arc<[u8]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PolicyConstructionField {
    Schema(PolicySchemaValue),
    Issuance {
        data: DeclarationId,
        case: DeclarationId,
        payload: PolicyCasePayload,
    },
    /// The type uses the constructor's sole binder for the invocation region.
    Zero(TypeExpression),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyCasePayload {
    NoFields,
    EmptySlice,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyConstructionInputs {
    pub domain: DeclarationId,
    pub conformance: DeclarationId,
    pub arguments: Vec<DeclarationTerm>,
    pub plan: DeclarationId,
    /// The compiler-supplied borrow region is slot zero at this occurrence.
    pub selection_site: Site,
    pub package_key: [u8; 32],
    pub subject_revision: Vec<u8>,
    pub schema: PolicySchemaValue,
    pub constructor: DeclarationId,
    pub construction_site: Site,
    pub result: TypeExpression,
    pub fields: Vec<(DeclarationId, PolicyConstructionField)>,
}
