use crate::*;
use semantic_vocabulary::{ServiceId};

impl Psi {
    pub fn callback_application(&self, index: u32) -> Option<&crate::TerminalCallbackApplication> {
        self.interfaces.callback_applications
            .binary_search_by_key(&index, |row| row.application_index)
            .ok()
            .map(|ordinal| &self.interfaces.callback_applications[ordinal])
            .or_else(|| {
                self.interfaces.installed_interrupt_registrations
                    .iter()
                    .find_map(|registration| {
                        registration
                            .callbacks
                            .binary_search_by_key(&index, |row| row.application_index)
                            .ok()
                            .map(|ordinal| &registration.callbacks[ordinal])
                    })
            })
    }

    /// Resolve the declaration instance attached to this boundary, rather than
    /// looking up its template by name. Closed calls can share a template while
    /// retaining different ordered static arguments.
    pub fn boundary_calling_abi(
        &self,
        boundary: semantic_vocabulary::BoundaryMachineId,
    ) -> Result<&crate::CallingAbiDeclaration, String> {
        // Source publication sorts this roster; the receiving foundation check
        // requires increasing boundary IDs. Reuse that order for each demand.
        let missing = "boundary has no unique Terminal calling declaration";
        let position = self
            .interfaces.calling_signatures
            .binary_search_by_key(&boundary, |row| row.boundary)
            .map_err(|_| missing.to_owned())?;
        if (position != 0 && self.interfaces.calling_signatures[position - 1].boundary == boundary)
            || self
                .interfaces.calling_signatures
                .get(position + 1)
                .is_some_and(|row| row.boundary == boundary)
        {
            return Err(missing.into());
        }
        let declaration = &self.interfaces.calling_signatures[position];
        self.calling_abi_signature(*declaration.application.as_ref().map_err(Clone::clone)?)
            .ok_or_else(|| "boundary lost its exact declaration ABI".into())
    }

    pub fn calling_abi_identity(
        &self,
        boundary: semantic_vocabulary::BoundaryMachineId,
    ) -> Result<semantic_vocabulary::CallingAbiIdentity, String> {
        self.boundary_calling_abi(boundary)?
            .identity(&self.identities)
    }

    pub fn calling_abi_signature(
        &self,
        id: semantic_vocabulary::CallingSignatureId,
    ) -> Option<&crate::CallingAbiDeclaration> {
        self.interfaces.calling_abi_signatures
            .binary_search_by_key(&id, |row| row.id)
            .ok()
            .map(|position| &self.interfaces.calling_abi_signatures[position])
    }

    pub fn service(&self, id: ServiceId) -> Option<&ServiceDeclaration> {
        self.interfaces.services.iter().find(|service| service.id == id)
    }

    pub fn boundary_machine(
        &self,
        id: semantic_vocabulary::BoundaryMachineId,
    ) -> Option<&BoundaryMachineDeclaration> {
        find_boundary_machine(&self.interfaces.boundary_machines, id)
    }

    /// Exact requirements whose selected service rows remain to be supplied.
    /// Keep the declaration's bound distinct from an ordinary pinned row.
    pub fn installation_reach_requirements(&self) -> Vec<semantic_vocabulary::BoundaryMachineId> {
        self.interfaces.boundary_machines
            .iter()
            .filter(|boundary| boundary.service_reach_is_installation_bound)
            .map(|boundary| boundary.id)
            .collect()
    }
}
