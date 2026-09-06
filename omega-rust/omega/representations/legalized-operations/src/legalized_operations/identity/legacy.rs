//! Historical fingerprints do not grant current legalization custody.

use super::schema::{IdentitySchema, identity};
use super::shared::*;

#[doc(hidden)]
pub fn legalized_operation_plan_identity_v22_legacy(
    plan: &LegalizedOperationPlan,
) -> LegalizedOperationPlanIdentity {
    identity(plan, IdentitySchema::V22)
}

#[doc(hidden)]
pub fn legalized_operation_plan_identity_v21_legacy(
    plan: &LegalizedOperationPlan,
) -> LegalizedOperationPlanIdentity {
    identity(plan, IdentitySchema::V21)
}

#[doc(hidden)]
pub fn legalized_operation_plan_identity_v20_legacy(
    plan: &LegalizedOperationPlan,
) -> LegalizedOperationPlanIdentity {
    identity(plan, IdentitySchema::V20)
}

#[doc(hidden)]
pub fn legalized_operation_plan_identity_v19_legacy(
    plan: &LegalizedOperationPlan,
) -> LegalizedOperationPlanIdentity {
    identity(plan, IdentitySchema::V19)
}

#[doc(hidden)]
pub fn legalized_operation_plan_identity_v18_legacy(
    plan: &LegalizedOperationPlan,
) -> LegalizedOperationPlanIdentity {
    identity(plan, IdentitySchema::V18)
}

#[doc(hidden)]
pub fn legalized_operation_plan_identity_v17_legacy(
    plan: &LegalizedOperationPlan,
) -> LegalizedOperationPlanIdentity {
    identity(plan, IdentitySchema::V17)
}

#[doc(hidden)]
pub fn legalized_operation_plan_identity_v16_legacy(
    plan: &LegalizedOperationPlan,
) -> LegalizedOperationPlanIdentity {
    identity(plan, IdentitySchema::V16)
}

#[doc(hidden)]
pub fn legalized_operation_plan_identity_v15_legacy(
    plan: &LegalizedOperationPlan,
) -> LegalizedOperationPlanIdentity {
    identity(plan, IdentitySchema::V15)
}

#[doc(hidden)]
pub fn legalized_operation_plan_identity_v12_legacy(
    plan: &LegalizedOperationPlan,
) -> LegalizedOperationPlanIdentity {
    identity(plan, IdentitySchema::V12)
}

#[doc(hidden)]
pub fn legalized_operation_plan_identity_v13_legacy(
    plan: &LegalizedOperationPlan,
) -> LegalizedOperationPlanIdentity {
    identity(plan, IdentitySchema::V13)
}

#[doc(hidden)]
pub fn legalized_operation_plan_identity_v14_legacy(
    plan: &LegalizedOperationPlan,
) -> LegalizedOperationPlanIdentity {
    identity(plan, IdentitySchema::V14)
}

#[doc(hidden)]
pub fn legalized_operation_plan_identity_v9_legacy(
    plan: &LegalizedOperationPlan,
) -> LegalizedOperationPlanIdentity {
    identity(plan, IdentitySchema::V9)
}
