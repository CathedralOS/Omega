//! Join independently executed Build registrations to their checked Product.
use super::{AcquiredBuildSource, DeclaredBuildSource, ResolvedBuildSources, SourceHost};
use std::sync::Arc;

/// A child may be demanded while its parent is running and visited again by its
/// final source join. A live grant identifies exactly one issued registration.
pub(super) struct SettledRegistration {
    pub(super) network: Option<Arc<package_compilation::BuildNetworkInvocation>>,
    pub(super) required_programs: Vec<package_compilation::DelegatedProgram>,
}

impl SettledRegistration {
    pub(super) fn matches(&self, backing: &build_execution::DependencyBacking) -> bool {
        match (&self.network, &backing.network) {
            (Some(prior), Some(current)) => Arc::ptr_eq(prior, current),
            (None, None) => true,
            _ => false,
        }
    }
}

pub(super) struct SettledSource {
    pub(super) registrations: Vec<SettledRegistration>,
    pub(super) source: AcquiredBuildSource,
    pub(super) declaration: DeclaredBuildSource,
    pub(super) build: project_build::SettledDependency,
    pub(super) graph: ResolvedBuildSources,
}

pub(super) fn require_programs(
    used: &[package_compilation::DelegatedProgram],
    granted: &[package_compilation::DelegatedProgram],
) -> Result<(), String> {
    if let Some(missing) = used.iter().find(|used| !used.is_supplied_by(granted)) {
        return Err(format!(
            "this dependency registration lacks the reviewed ProgramLaunch binding for `{}`",
            missing.meaning.declaration
        ));
    }
    Ok(())
}

impl SourceHost<'_, '_> {
    /// Each live registration ran with independent admission and effects. The
    /// source graph may join their Products only when both executions agree.
    pub(super) fn remember_settled(&mut self, mut source: SettledSource) -> Result<usize, String> {
        if let Some((index, earlier)) = self
            .settled
            .iter_mut()
            .enumerate()
            .find(|(_, settled)| settled.source.same_source(&source.source))
        {
            source.source.verify_reused_capture(&earlier.source)?;
            let current = source.build.evaluated();
            let prior = earlier.build.evaluated();
            if source.build.inputs().source_inputs() != earlier.build.inputs().source_inputs()
                || !current.same_product_and_observations(prior)
            {
                return Err(format!(
                    "separate Build executions for package {} produced incompatible results",
                    source.declaration.package_name()
                ));
            }
            earlier.registrations.append(&mut source.registrations);
            return Ok(index);
        }
        let index = self.settled.len();
        self.settled.push(source);
        Ok(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_access_to_one_live_registration_keeps_its_execution() {
        let network = Arc::new(package_compilation::BuildNetworkInvocation::Granted { peer: None });
        let issued = build_execution::DependencyBacking {
            network: Some(Arc::clone(&network)),
            ..Default::default()
        };
        let first = SettledRegistration {
            network: Some(Arc::clone(&network)),
            required_programs: Vec::new(),
        };
        let second = build_execution::DependencyBacking {
            network: Some(Arc::new(*network)),
            ..Default::default()
        };
        assert!(first.matches(&issued.clone()));
        assert!(!first.matches(&second));
        assert!(!first.matches(&Default::default()));
    }
}
