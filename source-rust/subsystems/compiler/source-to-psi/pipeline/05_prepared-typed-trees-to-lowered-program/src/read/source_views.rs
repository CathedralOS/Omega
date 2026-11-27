//! Resolve a view once before choosing its byte or typed-element representation.
//! The supplied scope stays with its element; this query grants no loan authority.

use crate::source_state::{SourceSliceApplication, SourceTypeApplication, TypedTrees};
use semantic_vocabulary::MachineId;
use symbols::SymbolHandle;
use terminal_check_tables::Checked;

pub(super) enum SourceViewKind {
    Bytes(super::bytes::SourceByteView),
    Elements(super::arrays::SourceSequenceView),
}

pub(super) struct SourceView {
    pub source: SourceTypeApplication,
    pub slice: SourceSliceApplication,
    pub kind: SourceViewKind,
}

pub(super) fn source_view(
    typed: &TypedTrees,
    source: SourceTypeApplication,
    owner: SymbolHandle,
    machine: MachineId,
) -> Checked<Option<SourceView>> {
    let Some(slice) = source
        .scope
        .slice_application(typed, source.reference)
        .map_err(|reason| crate::read::refuse_at(machine, crate::Unserved::Type(reason)))?
    else {
        return Ok(None);
    };
    let kind = if let Some(bytes) = super::bytes::source_byte_view(typed, &slice, owner, machine)? {
        SourceViewKind::Bytes(bytes)
    } else if let Some(elements) =
        super::arrays::source_sequence_view(typed, &source, &slice, owner, machine)?
    {
        SourceViewKind::Elements(elements)
    } else {
        return Ok(None);
    };
    Ok(Some(SourceView {
        source,
        slice,
        kind,
    }))
}
