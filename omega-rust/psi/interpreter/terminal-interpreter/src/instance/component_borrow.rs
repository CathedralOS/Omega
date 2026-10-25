//! A checked child's shared view aliases the caller's actual opaque bindings.
//!
//! These rows are associations with existing host objects, never referent-byte
//! snapshots or owned entry construction. The paused caller keeps its storage,
//! frontier and claims; the child owns only paid alias metadata until return
//! acknowledgement. Original exposure coordinates remain available to address
//! observations after artifact-local field presentation.

use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::runtime_rows::RuntimeRows;
use crate::structural_storage::HostStructuralPayload;
use crate::values::{StructuralRuntimePlace, TerminalStructuralValue};
use std::sync::Arc;
use terminal_installation::ComponentTypePresentation;
use terminal_psi::{StructuralPathSegment, TerminalPsiIdentity};

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ComponentBorrowedOrigin {
    owner: TerminalPsiIdentity,
    namespace: u64,
    original: StructuralRuntimePlace,
    presentations: Vec<Arc<ComponentTypePresentation>>,
}

pub(crate) struct ComponentBorrowedStorage {
    receiver: TerminalPsiIdentity,
    provider: TerminalPsiIdentity,
    candidate: semantic_vocabulary::MachineId,
    origins: Arc<RuntimeRows<StructuralRuntimePlace, ComponentBorrowedOrigin>>,
    atoms: RuntimeRows<StructuralRuntimePlace, HostStructuralPayload>,
}

impl ComponentBorrowedStorage {
    pub(crate) fn new(
        selected: &terminal_installation::AdmittedComponentProvider,
        account: Option<&evaluation_accounting::EvaluationInvocationAccount>,
    ) -> Self {
        Self {
            receiver: selected.receiver(),
            provider: selected.provider(),
            candidate: selected.candidate(),
            origins: Arc::new(RuntimeRows::new(account)),
            atoms: RuntimeRows::new(account),
        }
    }

    pub(crate) fn capture(
        &mut self,
        caller: &TerminalExecution<'_>,
        source: &TerminalStructuralValue,
        presentation: Arc<ComponentTypePresentation>,
    ) -> Result<(), TerminalInterpretError> {
        if caller.program.identity != self.receiver
            || source.structural_type != presentation.source()
        {
            return Err(malformed());
        }
        let reject = || {
            TerminalInterpretError::UnsupportedSemanticVariant(
                "shared component input requires a distinct materialized, case, reference or byte backing bridge",
            )
        };
        let home = StructuralRuntimePlace::from(source);
        let within = |stored: &StructuralRuntimePlace| {
            stored.opaque_identity == home.opaque_identity && stored.path.starts_with(&home.path)
        };
        if caller.structural_cases.keys().any(within)
            || caller.reference_referents.keys().any(within)
            || caller.structural_primitive_storage.keys().any(within)
            || caller.structural_byte_storage.keys().any(within)
            || caller.structural_byte_descriptors.keys().any(within)
            || caller.structural_element_view_fields.keys().any(within)
            || caller
                .structural_scalar_fields
                .keys()
                .any(|field| within(&field.parent))
            || caller
                .structural_byte_sequence_fields
                .keys()
                .any(|field| within(&field.parent))
        {
            return Err(reject());
        }
        let existing = caller.component_borrowed_origin(&home);
        let mut presentations = vec![presentation.clone()];
        let (owner, namespace, original) = if let Some((base, origin)) = existing {
            let mut suffix = home
                .path
                .strip_prefix(base.path.as_slice())
                .ok_or_else(malformed)?
                .to_vec();
            for previous in &origin.presentations {
                previous
                    .original_path(&mut suffix)
                    .map_err(|_| malformed())?;
            }
            let mut original = origin.original.clone();
            original.path.extend(suffix);
            presentations.extend(origin.presentations.iter().cloned());
            (origin.owner, origin.namespace, original)
        } else {
            (
                caller.program.identity,
                caller
                    .local_structural_identities
                    .exposure_namespace
                    .ok_or(TerminalInterpretError::StructuralIdentityExhausted)?,
                home.clone(),
            )
        };
        let origin = ComponentBorrowedOrigin {
            owner,
            namespace,
            original,
            presentations,
        };
        let origins = Arc::get_mut(&mut self.origins).ok_or_else(malformed)?;
        if let Some(previous) = origins.get(&home) {
            if previous != &origin {
                return Err(malformed());
            }
        } else {
            origins.insert(home.clone(), origin)?;
        }
        let mut found = false;
        for (stored, payload) in
            crate::structural_storage::cells(&caller.structural_host_atoms, &home)
        {
            let HostStructuralPayload::Opaque(identity) = *payload else {
                return Err(reject());
            };
            let mut suffix = stored
                .path
                .strip_prefix(home.path.as_slice())
                .ok_or_else(malformed)?
                .to_vec();
            presentation
                .present_path(&mut suffix)
                .map_err(|_| malformed())?;
            let mut destination = home.clone();
            destination.path.extend(suffix);
            if let Some(previous) = self.atoms.get(&destination) {
                if !matches!(previous, HostStructuralPayload::Opaque(previous) if *previous == identity)
                {
                    return Err(malformed());
                }
            } else {
                self.atoms
                    .insert(destination, HostStructuralPayload::Opaque(identity))?;
            }
            found = true;
        }
        if !found {
            return Err(reject());
        }
        Ok(())
    }

    /// Only a newly instantiated, uninvoked checked child may install aliases.
    /// Discard host-entry placeholder tokens, then transfer the already funded
    /// actual associations. No source rows or caller custody are retired.
    pub(crate) fn install(
        &mut self,
        instance: &mut super::TerminalInstance<'_, '_>,
    ) -> Result<(), TerminalInterpretError> {
        if instance.invoked
            || instance.closed
            || instance.program.terminal_psi() != self.provider
            || instance.entry != self.candidate
        {
            return Err(malformed());
        }
        let execution = instance.execution.as_mut().ok_or_else(malformed)?;
        if !execution.component_borrowed_origins.is_empty() {
            return Err(malformed());
        }
        for (home, _) in self.origins.iter() {
            if let Some(payload) = execution.structural_host_atoms.get(home) {
                if !matches!(payload, HostStructuralPayload::Opaque(identity) if *identity == home.opaque_identity)
                {
                    return Err(malformed());
                }
                execution.structural_host_atoms.remove(home);
            }
        }
        if self
            .atoms
            .iter()
            .any(|(home, _)| execution.structural_host_atoms.contains_key(home))
        {
            return Err(malformed());
        }
        execution.structural_host_atoms.append(&mut self.atoms)?;
        execution.component_borrowed_origins = Arc::clone(&self.origins);
        Ok(())
    }
}

impl TerminalExecution<'_> {
    fn component_borrowed_origin(
        &self,
        home: &StructuralRuntimePlace,
    ) -> Option<(&StructuralRuntimePlace, &ComponentBorrowedOrigin)> {
        self.component_borrowed_origins
            .iter()
            .filter(|(base, _)| {
                base.opaque_identity == home.opaque_identity && home.path.starts_with(&base.path)
            })
            .max_by_key(|(base, _)| base.path.len())
    }

    pub(crate) fn component_borrowed_address(
        &self,
        root: u64,
        coordinates: &[(u64, u64)],
    ) -> Result<Option<(u64, u64, Vec<(u64, u64)>)>, TerminalInterpretError> {
        if !self
            .component_borrowed_origins
            .keys()
            .any(|home| home.opaque_identity == root)
        {
            return Ok(None);
        }
        let path = coordinates
            .iter()
            .map(|&(kind, value)| match kind {
                0 => Ok(StructuralPathSegment::FixedIndex(value)),
                1 => Ok(StructuralPathSegment::Field(
                    terminal_psi::TerminalIdentityIndex(
                        u32::try_from(value).map_err(|_| malformed())?,
                    ),
                )),
                2 => Ok(StructuralPathSegment::Case(
                    semantic_vocabulary::StructuralCaseId::new(value).ok_or_else(malformed)?,
                )),
                _ => Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "component borrowed address requires its original field storage coordinate",
                )),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let home = StructuralRuntimePlace {
            opaque_identity: root,
            path,
        };
        let (base, origin) = self
            .component_borrowed_origin(&home)
            .ok_or_else(malformed)?;
        let mut suffix = home
            .path
            .strip_prefix(base.path.as_slice())
            .ok_or_else(malformed)?
            .to_vec();
        for presentation in &origin.presentations {
            presentation
                .original_path(&mut suffix)
                .map_err(|_| malformed())?;
        }
        let mut original = origin.original.path.clone();
        original.extend(suffix);
        Ok(Some((
            origin.namespace,
            origin.original.opaque_identity,
            crate::storage_addresses::path_coordinates(&original)?,
        )))
    }
}
