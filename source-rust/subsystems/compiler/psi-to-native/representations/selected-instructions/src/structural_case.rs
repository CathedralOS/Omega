/*
  Exact structural case custody and physical transport of edge-produced case
  payloads.

  `SelectedStructuralCaseEdge` records where a selected case edge reads the
  dispatched sum (`SelectedCaseDispatchSource`), which `StructuralCaseId` and
  discriminant tag it matches, how each case payload field is transported
  (`SelectedCasePayloadBinding`), and any affine cleanup actions settled on the
  branch.
*/

use super::{LocalStorageSlotId, VirtualRegisterId};
use semantic_vocabulary::{PlaceId, StructuralCaseId};
use target_vocabulary::LegalizedStructuralCasePayload;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedStructuralCaseEdge {
    pub source: SelectedCaseDispatchSource,
    pub case: StructuralCaseId,
    pub case_tag: i32,
    pub payloads: Vec<SelectedCasePayloadBinding>,
    pub cleanup_actions: Vec<terminal_psi::TerminalAffineCleanupAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedCaseDispatchSource {
    Local {
        slot: LocalStorageSlotId,
    },
    Borrowed {
        place: PlaceId,
        pointer: VirtualRegisterId,
        byte_size: u32, // The addressed value's full byte size.
    },
}

impl SelectedCaseDispatchSource {
    pub fn place(&self) -> Option<PlaceId> {
        match self {
            Self::Local { slot } => slot.structural_place(),
            Self::Borrowed { place, .. } => Some(*place),
        }
    }

    pub fn byte_size(&self) -> Option<u32> {
        match self {
            Self::Borrowed { byte_size, .. } => Some(*byte_size),
            _ => None,
        }
    }

    pub fn local_slot(&self) -> Option<LocalStorageSlotId> {
        match self {
            Self::Local { slot } => Some(*slot),
            Self::Borrowed { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedCasePayloadBinding {
    pub semantic: LegalizedStructuralCasePayload,
    pub transport: SelectedCasePayloadTransport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedCasePayloadTransport {
    Unmaterialized {
        parameter: VirtualRegisterId,
    },
    Unused,
    Registers {
        argument: VirtualRegisterId,
        parameter: VirtualRegisterId,
    },
}
