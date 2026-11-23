//! Recognize execution calls of the exact canonical process-exit requirement.
//!
//! The core package owns the requirement's identity. A call qualifies only when
//! its selected declaration and owning boundary trait are those exact core
//! declarations; a local trait or machine with the same path or spelling is an
//! ordinary call. Recognition records the terminal outcome kind. It grants no
//! authority and proves nothing about reach, progress or crossings.
use crate::type_error::SourceTypeError;
use crate::{CallUse, ExternalCompletion, ExternalCompletionKind, TypeError, TypedCall};

pub(crate) fn external_completion(call: &TypedCall) -> Result<Option<ExternalCompletion>, TypeError> {
    if call.use_site != CallUse::Execution || call.machine.package != omega_toolchain::TOOLCHAIN_PACKAGE {
        return Ok(None);
    }
    let (owner, requirement) = omega_toolchain::core_interface().process_exit_requirement();
    if &call.machine != requirement {
        return Ok(None);
    }
    let exact_owner = matches!(call.enclosing_arguments.as_slice(),[(trait_id,arguments)] if trait_id==owner && arguments.is_empty());
    if !exact_owner || !call.type_arguments.is_empty() || call.evidence.is_some() || call.layout_query.is_some() || call.operands.len() != 1 {
        return Err(TypeError::for_site(&call.site, "canonical process exit requires its exact boundary invocation"));
    }
    Ok(Some(ExternalCompletion {
        node: call.node,
        site: call.site.clone(),
        kind: ExternalCompletionKind::ProcessExit,
        owner: owner.clone(),
        requirement: requirement.clone(),
    }))
}
