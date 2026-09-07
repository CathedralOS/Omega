//! Exact identity-domain and retained-roster policy for every supported schema.

use super::{canonical, shared::*};

#[derive(Clone, Copy)]
pub(super) enum IdentitySchema {
    V9,
    V12,
    V13,
    V14,
    V15,
    V16,
    V17,
    V18,
    V19,
    V20,
    V21,
    V22,
    V27,
}

pub(super) fn identity(
    plan: &LegalizedOperationPlan,
    schema: IdentitySchema,
) -> LegalizedOperationPlanIdentity {
    let (domain, retain_call_contract, retain_historical_empty_call_roster) = match schema {
        IdentitySchema::V27 => (
            b"omega.terminal-legalized-operations.v27\0".as_slice(),
            true,
            true,
        ),
        IdentitySchema::V22 => (
            b"omega.terminal-legalized-operations.v22\0".as_slice(),
            true,
            true,
        ),
        IdentitySchema::V9 => (
            b"omega.terminal-legalized-operations.v9\0".as_slice(),
            false,
            false,
        ),
        IdentitySchema::V12 => (
            b"omega.terminal-legalized-operations.v12\0".as_slice(),
            true,
            false,
        ),
        IdentitySchema::V13 => (
            b"omega.terminal-legalized-operations.v13\0".as_slice(),
            true,
            false,
        ),
        IdentitySchema::V14 => (
            b"omega.terminal-legalized-operations.v14\0".as_slice(),
            true,
            false,
        ),
        IdentitySchema::V15 => (
            b"omega.terminal-legalized-operations.v15\0".as_slice(),
            true,
            false,
        ),
        IdentitySchema::V16 => (
            b"omega.terminal-legalized-operations.v16\0".as_slice(),
            true,
            false,
        ),
        IdentitySchema::V17 => (
            b"omega.terminal-legalized-operations.v17\0".as_slice(),
            true,
            true,
        ),
        IdentitySchema::V18 => (
            b"omega.terminal-legalized-operations.v18\0".as_slice(),
            true,
            true,
        ),
        IdentitySchema::V19 => (
            b"omega.terminal-legalized-operations.v19\0".as_slice(),
            true,
            true,
        ),
        IdentitySchema::V20 => (
            b"omega.terminal-legalized-operations.v20\0".as_slice(),
            true,
            true,
        ),
        IdentitySchema::V21 => (
            b"omega.terminal-legalized-operations.v21\0".as_slice(),
            true,
            true,
        ),
    };
    canonical::identity(
        plan,
        domain,
        retain_call_contract,
        retain_historical_empty_call_roster,
        matches!(schema, IdentitySchema::V22 | IdentitySchema::V27),
    )
}
