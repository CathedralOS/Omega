/*
We pair produced Terminal callback thunk bodies with their checked placements,
portable registrar applications, and target-owned ABI recipes. We validate
that every occurrence selects the declared callback in its caller's canonical
Terminal program before native admission borrows executable and ABI evidence.
*/

use calling_conventions::{
    BoundaryEntryPlan, CallbackMaterializationContext, CallbackRequirementId,
    NativeParameterApplication, NativePlace, StaticMachineBinderId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCallbackPlacement {
    pub application_index: u32,
    pub static_machine_ordinal: u32,
    pub canonical_requirement_overload: String,
    pub contract_commitment: [u8; 32],
    pub boundary_entry_plan: BoundaryEntryPlan,
    pub entry_parameters: Vec<NativeCallbackEntryParameter>, // Ordered nominal native rows of the callback's inbound requirement.
    pub private_materialization: Option<BoundCallbackPrivateMaterialization>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCallbackEntryParameter {
    pub application: NativeParameterApplication,
    pub origin: calling_conventions::BoundaryNativeParameterOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrivateCallbackDestinationApplication {
    Direct {
        application: NativeParameterApplication,
    },
    Field {
        root: NativeParameterApplication,
        placements: calling_conventions::PrivateCallbackPlacements,
    },
}

impl PrivateCallbackDestinationApplication {
    pub fn application(&self) -> &NativeParameterApplication {
        match self {
            Self::Direct { application } => application,
            Self::Field { root, .. } => root,
        }
    }

    pub fn placements(&self) -> Option<&calling_conventions::PrivateCallbackPlacements> {
        match self {
            Self::Direct { .. } => None,
            Self::Field { placements, .. } => Some(placements),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundCallbackPrivateMaterialization {
    pub registrar_invokes_callback: bool, // False denotes no same-stack edge during registration.
    pub binder: StaticMachineBinderId,
    pub destination: NativePlace,
    pub requirement: CallbackRequirementId,
    pub registrar_boundary_entry_plan: BoundaryEntryPlan,
    pub registrar_calling_plan_report_fingerprint: u64,
    pub registrar_application_report_fingerprint: u64,
    pub registrar_application_commitment: [u8; 32],
    pub registrar_application: calling_conventions::BoundaryCallingApplication,
    pub destination_application: PrivateCallbackDestinationApplication,
    pub context: CallbackMaterializationContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCallbackThunkArtifact {
    artifact: std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
    entry: semantic_vocabulary::MachineId,
    parameter_formals: Vec<u32>, // Dense semantic formal ordinals in runtime parameter order.
}

impl TerminalCallbackThunkArtifact {
    pub fn for_application(
        artifact: std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
        application_index: u32,
    ) -> Result<Self, &'static str> {
        let module = artifact.semantic_module();
        let application = module
            .callback_application(application_index)
            .ok_or("Terminal callback thunk has no retained application")?;
        let mut bodies = module.machines.iter().filter(|machine| {
            machine.declaration.as_ref().is_some_and(|declaration| {
                declaration.same_identity_as(
                    &module.identities,
                    &application.body,
                    &module.identities,
                )
            })
        });
        let body = bodies
            .next()
            .ok_or("Terminal callback application has no exact retained body")?;
        if bodies.next().is_some() {
            return Err("Terminal callback application has ambiguous retained bodies");
        }
        let entry = body.id;
        let parameter_formals = body.runtime_parameter_formals()?;
        Ok(Self {
            artifact,
            entry,
            parameter_formals,
        })
    }

    pub fn artifact(&self) -> &terminal_codec::CanonicalTerminalArtifact {
        &self.artifact
    }

    pub fn parameter_formals(&self) -> &[u32] {
        &self.parameter_formals
    }

    pub const fn entry(&self) -> semantic_vocabulary::MachineId {
        self.entry
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCallbackOccurrenceProposal {
    placement_index: usize,
    application_index: u32,
    terminal_operation: semantic_vocabulary::OperationId,
    callback_thunk_identity: function_identity::MachineFunctionIdentity,
    callback_thunk_artifact: TerminalCallbackThunkArtifact,
    boundary_entry_plan: calling_conventions::BoundaryEntryPlan,
    entry_parameters: Vec<crate::NativeCallbackEntryParameter>,
}

impl TerminalCallbackOccurrenceProposal {
    pub fn new(
        placement_index: usize,
        application_index: u32,
        terminal_operation: semantic_vocabulary::OperationId,
        callback_thunk_identity: function_identity::MachineFunctionIdentity,
        callback_thunk_artifact: TerminalCallbackThunkArtifact,
        boundary_entry_plan: calling_conventions::BoundaryEntryPlan,
        entry_parameters: Vec<crate::NativeCallbackEntryParameter>,
    ) -> Self {
        Self {
            placement_index,
            application_index,
            terminal_operation,
            callback_thunk_identity,
            callback_thunk_artifact,
            boundary_entry_plan,
            entry_parameters,
        }
    }

    pub const fn application_index(&self) -> u32 {
        self.application_index
    }

    pub const fn boundary_entry_plan(&self) -> &calling_conventions::BoundaryEntryPlan {
        &self.boundary_entry_plan
    }

    pub fn entry_parameters(&self) -> &[crate::NativeCallbackEntryParameter] {
        &self.entry_parameters
    }

    pub const fn placement_index(&self) -> usize {
        self.placement_index
    }

    pub const fn terminal_operation(&self) -> semantic_vocabulary::OperationId {
        self.terminal_operation
    }

    pub const fn callback_thunk_identity(&self) -> function_identity::MachineFunctionIdentity {
        self.callback_thunk_identity
    }

    pub const fn callback_thunk_artifact(&self) -> &TerminalCallbackThunkArtifact {
        &self.callback_thunk_artifact
    }
}

#[derive(Debug, Clone, Copy)]
pub struct NativeCallbackThunkSettlement<'artifact> {
    pub terminal_operation: semantic_vocabulary::OperationId,
    pub placement_index: usize,
    pub callback_function: function_identity::MachineFunctionIdentity,
    pub artifact: &'artifact terminal_codec::CanonicalTerminalArtifact,
    pub entry: semantic_vocabulary::MachineId,
    pub boundary_entry_plan: &'artifact calling_conventions::BoundaryEntryPlan,
    pub parameter_formals: &'artifact [u32],
    pub entry_parameters: &'artifact [crate::NativeCallbackEntryParameter],
}

pub fn validate_callback_thunk_joins(
    caller: terminal_codec::TerminalArtifactView<'_>,
    occurrences: &[TerminalCallbackOccurrenceProposal],
) -> Result<(), &'static str> {
    let module = caller.module();
    for occurrence in occurrences {
        let thunk = occurrence.callback_thunk_artifact();
        if thunk.artifact().view().identity() != caller.identity() {
            return Err("callback thunk must select a body in its caller's Terminal program");
        }
        let application = module
            .callback_application(occurrence.application_index)
            .ok_or("callback thunk has no portable application")?;
        let body = thunk.artifact().semantic_module();
        let declaration = body
            .machine(thunk.entry())
            .and_then(|machine| machine.declaration.as_ref())
            .ok_or("callback thunk has no entry declaration")?;
        let expected_function = function_identity::MachineFunctionIdentity::callback_thunk(
            caller.identity(),
            occurrence.placement_index(),
        );
        if !declaration.same_identity_as(&body.identities, &application.body, &module.identities)
            || application.registrar != occurrence.terminal_operation()
            || expected_function != Some(occurrence.callback_thunk_identity())
        {
            return Err(
                "callback thunk body or function identity drifted from its retained placement",
            );
        }
    }
    Ok(())
}

pub fn native_callback_thunks(
    occurrences: &[TerminalCallbackOccurrenceProposal],
) -> Vec<NativeCallbackThunkSettlement<'_>> {
    occurrences
        .iter()
        .map(|occurrence| NativeCallbackThunkSettlement {
            terminal_operation: occurrence.terminal_operation(),
            placement_index: occurrence.placement_index(),
            callback_function: occurrence.callback_thunk_identity(),
            artifact: occurrence.callback_thunk_artifact().artifact(),
            entry: occurrence.callback_thunk_artifact().entry(),
            boundary_entry_plan: &occurrence.boundary_entry_plan,
            parameter_formals: occurrence.callback_thunk_artifact().parameter_formals(),
            entry_parameters: &occurrence.entry_parameters,
        })
        .collect()
}
