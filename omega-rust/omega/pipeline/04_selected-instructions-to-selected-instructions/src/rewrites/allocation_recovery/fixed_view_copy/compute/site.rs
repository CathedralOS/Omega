//! Per-boundary fixed-site copy construction for the leaf-local and
//! immediate-site policies: one copy in the site's block immediately before
//! the fixed use, sourcing any admitted origin.
use super::apply::{apply_copy, is_u64};
use super::preflight::{find_leaf_block, find_site_block};
use super::{
    BTreeSet, FixedViewCopy, FixedViewCopyDestination, FixedViewCopyError,
    RegisterInstructionConstraint, RegisterOperandAccess, SelectedInstructionId,
    VirtualFixedConstraintSite, VirtualRegisterId, VirtualRegisterOrigin,
};
use register_model::RegisterConstraintKey;

/// The virtual register a boundary copies from, under the leaf-local or
/// immediate admission gate: `leaf_local` admits only live-in entry
/// parameters still carrying their entry fixed view; the immediate form
/// admits any origin that still names a scalar source value.
pub(super) fn admitted_source<'a>(
    function_index: usize,
    function: &'a selected_instructions::SelectedFunction,
    boundary: &super::super::evidence::AuthenticatedFixedViewBoundary,
    leaf_local: bool,
) -> Result<
    (
        &'a selected_instructions::VirtualRegister,
        semantic_vocabulary::ValueId,
    ),
    FixedViewCopyError,
> {
    let source_register = function
        .virtual_registers
        .get(usize::try_from(boundary.virtual_register.0).map_err(|_| {
            FixedViewCopyError::UnsupportedSourceRegister {
                function: function_index,
                register: boundary.virtual_register.0,
            }
        })?)
        .filter(|register| {
            register.id == boundary.virtual_register
                && register.class == boundary.class
                && (!leaf_local || register.entry_fixed_view == Some(boundary.from_view))
        })
        .ok_or(FixedViewCopyError::UnsupportedSourceRegister {
            function: function_index,
            register: boundary.virtual_register.0,
        })?;
    // V1 admits only live-in entry parameters; the immediate form
    // admits any origin that still names a scalar source value.
    let source_value = match (leaf_local, source_register.origin) {
        (_, VirtualRegisterOrigin::EntryParameter { source_value, .. })
        | (
            false,
            VirtualRegisterOrigin::InstructionResult { source_value, .. }
            | VirtualRegisterOrigin::BlockParameter { source_value, .. }
            | VirtualRegisterOrigin::ScalarAbiAddress { source_value, .. },
        ) => source_value,
        _ => {
            return Err(FixedViewCopyError::UnsupportedSourceRegister {
                function: function_index,
                register: boundary.virtual_register.0,
            });
        }
    };
    if !is_u64(source_register.scalar_type) {
        return Err(FixedViewCopyError::UnsupportedSourceRegister {
            function: function_index,
            register: boundary.virtual_register.0,
        });
    }
    Ok((source_register, source_value))
}

/// The block a boundary's fixed-use site must still be found in, reading the
/// source register under the destination view — `leaf_local` confines the
/// site to a leaf `Return` terminator while the immediate form admits the
/// site's own block at any operand-Use position.
pub(super) fn destination_block(
    function_index: usize,
    function: &selected_instructions::SelectedFunction,
    boundary: &super::super::evidence::AuthenticatedFixedViewBoundary,
    instruction: SelectedInstructionId,
    operand: u16,
    source: VirtualRegisterId,
    leaf_local: bool,
) -> Result<selected_instructions::SelectedBlockId, FixedViewCopyError> {
    let block = if leaf_local {
        find_leaf_block(
            function_index,
            function,
            instruction,
            operand,
            source,
            boundary.to_view,
        )?
    } else {
        find_site_block(
            function_index,
            function,
            instruction,
            operand,
            source,
            boundary.to_view,
        )?
    };
    if block != boundary.block {
        return Err(FixedViewCopyError::SegmentEvidenceMismatch);
    }
    Ok(block)
}

/// Build and apply one copy per authenticated boundary. `leaf_local` selects
/// the V1 admission gate (live-in entry parameters only, destination confined
/// to a leaf return block); the immediate form admits any origin that still
/// names a scalar source value and places the copy in the site's own block.
#[allow(clippy::too_many_arguments)]
pub(super) fn build_site_copies(
    function_index: usize,
    source_function: &selected_instructions::SelectedFunction,
    boundaries: &[&super::super::evidence::AuthenticatedFixedViewBoundary],
    transformed: &mut selected_instructions::SelectedFunction,
    copy_row: &RegisterInstructionConstraint,
    copy_key: RegisterConstraintKey,
    leaf_local: bool,
    mut next_instruction: u32,
    mut next_register: u32,
) -> Result<Vec<FixedViewCopy>, FixedViewCopyError> {
    let mut copies = Vec::new();
    let mut destinations = BTreeSet::new();
    for boundary in boundaries {
        let VirtualFixedConstraintSite::Operand {
            instruction,
            operand,
            access: RegisterOperandAccess::Use,
            ..
        } = boundary.site
        else {
            return Err(FixedViewCopyError::UnsupportedTransitionSite {
                function: function_index,
                register: boundary.virtual_register.0,
            });
        };
        if !destinations.insert((instruction, operand)) {
            return Err(FixedViewCopyError::NonCanonicalCopies);
        }
        let (source_register, source_value) =
            admitted_source(function_index, source_function, boundary, leaf_local)?;
        if copy_row.operands[0].class != source_register.class
            || copy_row.operands[1].class != source_register.class
        {
            return Err(FixedViewCopyError::UnsupportedSourceRegister {
                function: function_index,
                register: boundary.virtual_register.0,
            });
        }
        let function_u32 =
            u32::try_from(function_index).map_err(|_| FixedViewCopyError::IdentifierOverflow {
                function: function_index,
            })?;
        let copy = FixedViewCopy {
            function: function_u32,
            machine: source_function.machine,
            source_virtual_register: source_register.id,
            source_value,
            source_definition_site: source_register.definition_site.ok_or(
                FixedViewCopyError::UnsupportedSourceRegister {
                    function: function_index,
                    register: source_register.id.0,
                },
            )?,
            from_view: boundary.from_view,
            to_view: boundary.to_view,
            insertion_block: destination_block(
                function_index,
                source_function,
                boundary,
                instruction,
                operand,
                source_register.id,
                leaf_local,
            )?,
            before_instruction: instruction,
            destinations: vec![FixedViewCopyDestination {
                site: boundary.site,
                block: boundary.block,
                view: boundary.to_view,
            }],
            copy_instruction: SelectedInstructionId(next_instruction),
            result_virtual_register: VirtualRegisterId(next_register),
            copy_constraint: copy_key,
        };
        apply_copy(function_index, transformed, &copy, copy_row)?;
        copies.push(copy);
        next_instruction =
            next_instruction
                .checked_add(1)
                .ok_or(FixedViewCopyError::IdentifierOverflow {
                    function: function_index,
                })?;
        next_register =
            next_register
                .checked_add(1)
                .ok_or(FixedViewCopyError::IdentifierOverflow {
                    function: function_index,
                })?;
    }
    Ok(copies)
}
