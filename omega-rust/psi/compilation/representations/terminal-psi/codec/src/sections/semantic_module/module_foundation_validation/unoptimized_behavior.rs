//! Operation relationships remain source-free after optional transformations.

use crate::codec_error::{CodecError, malformed};
use semantic_vocabulary::InvocationSite;
use terminal_psi::{TerminalModule, UnoptimizedBehaviorOperation as Operation};

pub(super) fn validate(module: &TerminalModule) -> Result<(), CodecError> {
    super::super::wire::require_increasing(
        module.unoptimized_behavior.iter().map(|body| body.machine),
        "unoptimized semantic bodies",
    )?;
    for body in &module.unoptimized_behavior {
        let machine =
            module
                .machine(body.machine)
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "unoptimized behavior names an unknown machine",
                ))?;
        for operation in &body.operations {
            match operation {
                Operation::Invocation { site, callee, .. } => {
                    if module.machine(*callee).is_none()
                        || matches!(site, InvocationSite::ExternalCompletion(_))
                    {
                        return malformed(
                            "unoptimized invocation has an unknown callee or wrong site role",
                        );
                    }
                }
                Operation::NominalCleanup { callee, .. } if module.machine(*callee).is_none() => {
                    return malformed("unoptimized cleanup has an unknown callee");
                }
                Operation::BoundaryInvocation { site, boundary, .. } => {
                    if module.boundary_machine(*boundary).is_none()
                        || matches!(site, InvocationSite::InvokeNoNormal(_))
                    {
                        return malformed(
                            "unoptimized boundary invocation has an unknown declaration or wrong site role",
                        );
                    }
                }
                Operation::InstructionService { service, .. }
                    if module.service(*service).is_none() =>
                {
                    return malformed("unoptimized instruction names an unknown service");
                }
                _ => {}
            }
        }
        // Rewrites may remove or move sites, but cannot introduce a semantic
        // contribution the retained source operation roster did not cover.
        // Missing rows supply no policy evidence; a supplied empty row cannot
        // hide an executable call, service instruction or crash.
        for current in terminal_psi::UnoptimizedMachineBehavior::capture(machine).operations {
            if !body
                .operations
                .iter()
                .any(|retained| same_contribution(*retained, current))
            {
                return malformed("unoptimized behavior omits an executable semantic contribution");
            }
        }
    }
    Ok(())
}

fn same_contribution(left: Operation, right: Operation) -> bool {
    match (left, right) {
        (
            Operation::Invocation { callee: left, .. },
            Operation::Invocation { callee: right, .. },
        )
        | (
            Operation::NominalCleanup { callee: left, .. },
            Operation::NominalCleanup { callee: right, .. },
        ) => left == right,
        (
            Operation::BoundaryInvocation { boundary: left, .. },
            Operation::BoundaryInvocation {
                boundary: right, ..
            },
        ) => left == right,
        (
            Operation::InstructionService { service: left, .. },
            Operation::InstructionService { service: right, .. },
        ) => left == right,
        (Operation::Crash { cause: left, .. }, Operation::Crash { cause: right, .. }) => {
            left == right
        }
        _ => false,
    }
}
