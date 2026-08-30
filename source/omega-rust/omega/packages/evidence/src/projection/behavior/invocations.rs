use super::super::semantics::declarations::nominal_identity;
use crate::evidence::PackageReviewSynchronousInvocation;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;

pub(crate) fn canonical_checked_invocation_targets(
    compilation: &CheckedCompilation,
    targets: &[psi_effects::InvocationTarget],
) -> Result<Vec<String>, Vec<Diagnostic>> {
    let mut canonical = targets
        .iter()
        .map(|target| match target {
            psi_effects::InvocationTarget::Parameter(index) => Ok(format!("parameter:{index}")),
            psi_effects::InvocationTarget::Service(symbol) => {
                let matching = compilation
                    .traits()
                    .iter()
                    .filter(|definition| definition.symbol == *symbol)
                    .collect::<Vec<_>>();
                let [definition] = matching.as_slice() else {
                    return Err(vec![Diagnostic::error(format!(
                        "reviewed synchronous invocation resolves service symbol {} to {} declarations; expected exactly one",
                        symbol.arena_index(),
                        matching.len(),
                    ))]);
                };
                if !definition.is_boundary {
                    return Err(vec![Diagnostic::error(format!(
                        "reviewed synchronous invocation resolves `{}` to a non-boundary trait",
                        definition.name,
                    ))]);
                }
                Ok(format!("service:{}", definition.name))
            }
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    canonical.sort();
    canonical.dedup();
    Ok(canonical)
}

pub(crate) fn project_synchronous_invocations(
    compilation: &CheckedCompilation,
    invocations: &[psi_effects::InvocationTarget],
) -> Result<Vec<PackageReviewSynchronousInvocation>, Vec<Diagnostic>> {
    let mut projected = invocations
        .iter()
        .copied()
        .map(|invocation| match invocation {
            psi_effects::InvocationTarget::Parameter(position) => {
                Ok(PackageReviewSynchronousInvocation::Parameter(position))
            }
            psi_effects::InvocationTarget::Service(symbol) => Ok(
                PackageReviewSynchronousInvocation::Service(nominal_identity(compilation, symbol)?),
            ),
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    projected.sort();
    projected.dedup();
    Ok(projected)
}
