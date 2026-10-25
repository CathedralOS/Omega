//! Owned completion crossing from one real component instance to its receiver.
//!
//! Inspection results are clonable observations. This carrier instead consumes
//! the completed instance and keeps its import/code custody until the receiving
//! operation acknowledges the transfer. It cannot be constructed by an adapter.

use semantic_vocabulary::MachineId;
use terminal_psi::TerminalPsiIdentity;

use super::{TerminalInstance, TerminalInstanceError};
use crate::errors::TerminalInterpretError;
use crate::results::TerminalExecutionResult;

pub(crate) struct ComponentOwnedReturn<'host, 'artifact> {
    provider: TerminalPsiIdentity,
    candidate: MachineId,
    instance_namespace: u64,
    value: TerminalExecutionResult,
    // Keep the producing program and its host/era custody alive through the
    // receiving operation. A returned payload is not an exit acknowledgement.
    owner: TerminalInstance<'host, 'artifact>,
}

impl<'host, 'artifact> ComponentOwnedReturn<'host, 'artifact> {
    pub(crate) const fn provider(&self) -> TerminalPsiIdentity {
        self.provider
    }
    pub(crate) const fn candidate(&self) -> MachineId {
        self.candidate
    }
    pub(crate) const fn instance_namespace(&self) -> u64 {
        self.instance_namespace
    }
    pub(crate) const fn value(&self) -> &TerminalExecutionResult {
        &self.value
    }

    /// The caller authenticates the receiving slot and its result declaration
    /// before this consuming acknowledgement. The old instance can never run
    /// again, and there is no second result retained in it.
    pub(crate) fn acknowledge(
        self,
    ) -> (TerminalExecutionResult, TerminalInstance<'host, 'artifact>) {
        let Self { value, owner, .. } = self;
        (value, owner)
    }
}

impl<'host, 'artifact> TerminalInstance<'host, 'artifact> {
    pub(crate) fn into_component_return(
        mut self,
        selected: &terminal_installation::AdmittedComponentProvider,
    ) -> Result<ComponentOwnedReturn<'host, 'artifact>, (Self, TerminalInstanceError)> {
        let verified = (|| {
            if self.closed || !self.invoked || self.program.terminal_psi() != selected.provider() {
                return Err("component return belongs to another or inactive producing instance");
            }
            let execution = self
                .execution
                .as_ref()
                .ok_or("component return lost its producing execution")?;
            if self.entry != selected.candidate()
                || !execution.call_stack.is_empty()
                || !execution.live_claims.is_empty()
                || !execution.live_affine_frontier.is_empty()
            {
                return Err("component return has unfinished activation or custody");
            }
            let value = execution
                .result
                .as_ref()
                .ok_or("component entry has not returned normally")?;
            match value {
                TerminalExecutionResult::Unit | TerminalExecutionResult::Scalar(_) => {}
                TerminalExecutionResult::Structural(returned) => {
                    let declaration = execution
                        .program
                        .structural_types()
                        .get(&returned.value.structural_type)
                        .ok_or("component return lost its structural declaration")?;
                    // The returning edge already authenticated and removed
                    // these exact claims from its live table. An opaque root
                    // may transfer its single linear debt, never a nested or
                    // independently manufactured completion receipt.
                    let expected_claims = usize::from(declaration.owns_linear_claim);
                    if returned.claims.len() != expected_claims
                        || execution.returned_claims.len() != expected_claims
                        || execution.returned_claims.iter().any(|binding| {
                            !binding.path.is_empty() || !returned.claims.contains(&binding.claim)
                        })
                        || !returned.value.qualifications.is_empty()
                        || !returned.value.projected_qualifications.is_empty()
                        || !returned.value.path.is_empty()
                        || (!declaration.owns_linear_claim && !declaration.no_code_affine_cleanup)
                    {
                        return Err(
                            "component return requires a checked claim, membership or cleanup transfer carrier",
                        );
                    }
                    if !execution.structural_host_atoms.values().any(|payload| {
                        matches!(payload,
                        crate::structural_storage::HostStructuralPayload::Opaque(identity)
                        if *identity == returned.value.opaque_identity)
                    }) {
                        return Err(
                            "component return requires materialized structural storage transfer",
                        );
                    }
                }
                TerminalExecutionResult::ScalarCase(_)
                | TerminalExecutionResult::ScalarArray(_) => {
                    return Err(
                        "component return requires a selected case or array storage transfer carrier",
                    );
                }
            }
            execution
                .local_structural_identities
                .exposure_namespace
                .ok_or("component return has no live instance identity namespace")
        })();
        let namespace = match verified {
            Ok(namespace) => namespace,
            Err(reason) => {
                return Err((
                    self,
                    TerminalInstanceError::Execution(
                        TerminalInterpretError::UnsupportedSemanticVariant(reason),
                    ),
                ));
            }
        };
        let value = self
            .execution
            .as_mut()
            .and_then(|execution| execution.result.take())
            .expect("the completed instance was authenticated without mutation");
        self.closed = true;
        Ok(ComponentOwnedReturn {
            provider: selected.provider(),
            candidate: selected.candidate(),
            instance_namespace: namespace,
            value,
            owner: self,
        })
    }
}
