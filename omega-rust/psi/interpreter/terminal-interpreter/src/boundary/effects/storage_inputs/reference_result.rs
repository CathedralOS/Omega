//! A host returns a selection of this call's captured storage, never a pointer.
//! Validate result regions and staged source contents before either publication.

use super::{BoundaryStorageInputs, structural_buffer::TerminalBoundaryReferenceSelection};
use crate::boundary::errors::{TerminalInterpretError, malformed};
use crate::engine::execution::TerminalExecution;
use crate::storage::reference::PreparedReference;
use crate::storage::structural_storage::contents::StorageLeafKind;
use terminal_psi::{BoundaryMachineDeclaration, StructuralAccess, StructuralPathSegment};

impl BoundaryStorageInputs {
    pub(crate) fn require_result_captures(
        &self,
        declaration: &BoundaryMachineDeclaration,
    ) -> Result<(), TerminalInterpretError> {
        for region in &declaration.result_reference_regions {
            let terminal_psi::TerminalDynamicReferenceOrigin::Parameter { argument_index } =
                region.origin
            else {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "host reference result requires captured input storage",
                ));
            };
            if !region.path.is_empty()
                || !self.reference_inputs.iter().any(|capture| {
                    capture.argument_index == argument_index as usize
                        && capture.access != StructuralAccess::Owned
                        && crate::storage::reference::access_covers(capture.access, region.access)
                })
            {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "host reference result has no admitted borrowed input capture",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn prepare_reference_result(
        &self,
        execution: &TerminalExecution,
        declaration: &BoundaryMachineDeclaration,
        result: &terminal_psi::StructuralOperationResult,
        selection: &TerminalBoundaryReferenceSelection,
        identity_cursor: &mut Option<u64>,
    ) -> Result<PreparedReference, TerminalInterpretError> {
        if !execution.supports_captured_reference_result(declaration, result) {
            return Err(malformed());
        }
        let selected = super::reference_outputs::source(selection, &self.reference_inputs)?;
        let types = execution.structural_types();
        let terminal_psi::StructuralTypeShape::Reference { access, .. } = types
            .get(&result.structural_type)
            .ok_or_else(malformed)?
            .shape
        else {
            return Err(malformed());
        };
        if selection.access != access
            || !crate::storage::reference::access_covers(selected.access, access)
        {
            return Err(malformed());
        }
        let relative = terminal_semantics::canonical_structural_storage_path(
            types.values(),
            selected.referent.structural_type,
            &selection.path,
        )
        .ok_or_else(malformed)?;
        let allowed = declaration.result_reference_regions.iter().any(|region| {
            if !region.path.is_empty()
                || region.access != access
                || region.origin
                    != (terminal_psi::TerminalDynamicReferenceOrigin::Parameter {
                        argument_index: selected.argument_index as u32,
                    })
            {
                return false;
            }
            let prefix = terminal_semantics::reference_source_storage_path(
                types.values(),
                selected.referent.structural_type,
                &region.source_path,
            );
            relative.starts_with(prefix)
        });
        if !allowed {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "host reference result exceeds its public input regions",
            ));
        }
        // A staged retag cannot lend the old payload. As for parameter outputs,
        // validate the captured source's case prefix before its first pointer hop.
        if let Some(binding) = self
            .structural
            .iter()
            .find(|binding| binding.argument_index == selected.argument_index)
        {
            let leaves = binding.completed_leaves.as_deref().ok_or_else(malformed)?;
            let prefix_end = relative
                .iter()
                .position(|step| matches!(step, StructuralPathSegment::Referent))
                .unwrap_or(relative.len());
            let active = crate::boundary::structural_inputs::case_membership::case_projection_with_membership_is_active(
                &types, selected.referent.structural_type, &relative[..prefix_end],
                &|prefix| leaves.iter().find_map(|leaf| match leaf.kind {
                    StorageLeafKind::Case { case, .. } => {
                        let path = terminal_semantics::canonical_structural_storage_path(
                            types.values(), selected.referent.structural_type, &leaf.path)?;
                        (path == prefix).then_some(case)
                    }
                    _ => None,
                }))?;
            if !active {
                return Err(malformed());
            }
        }
        let mut argument = selected.source_argument.clone();
        argument.path.extend(relative);
        argument.access = access;
        let actual = execution.resolve_live_projection(argument.place, &argument.path, access)?;
        if selected.access != StructuralAccess::SharedBorrow {
            let normal = self
                .structural
                .iter()
                .find(|binding| binding.argument_index == selected.argument_index)
                .map_or(&[][..], |binding| binding.normal_qualifications.as_slice());
            if !execution.normal_qualifications_cover(
                &actual,
                &actual.qualifications,
                &actual.projected_qualifications,
                normal,
            )? {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "host reference result lacks exact normal qualification preservation",
                ));
            }
        }
        execution.capture_reference(
            &argument,
            result.structural_type,
            &result.projected_qualifications,
            None,
            identity_cursor,
            &[],
        )
    }
}
