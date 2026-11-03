//! Keep every fragment of a private handler address together as it is lowered.
//!
//! The address is one sealed value even when hardware stores it in several
//! pieces. This owner retains those pieces in arena storage; consumers borrow
//! the complete sequence instead of substituting a single byte offset.

use arena::{Arena, HandleSpan};
use layout_plans::LayoutPlacementReport;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateCallbackPlacements {
    storage: Arena<LayoutPlacementReport>,
    placements: HandleSpan<LayoutPlacementReport>,
}

impl PrivateCallbackPlacements {
    pub fn new(placements: &[LayoutPlacementReport]) -> Self {
        let mut storage = Arena::with_capacity(placements.len());
        let mut span = HandleSpan::empty();
        for placement in placements {
            storage.append_to_span(&mut span, *placement);
        }
        Self {
            storage,
            placements: span,
        }
    }

    pub fn as_slice(&self) -> &[LayoutPlacementReport] {
        // Neither the arena nor its span can be changed after construction.
        self.storage
            .span(self.placements)
            .expect("private callback placement storage")
    }
}

impl Default for PrivateCallbackPlacements {
    fn default() -> Self {
        Self::new(&[])
    }
}

/// Encode the complete ordered placement list for native compilation records.
pub fn encode_private_callback_placements(
    bytes: &mut Vec<u8>,
    placements: &PrivateCallbackPlacements,
) {
    bytes.extend_from_slice(&(placements.as_slice().len() as u64).to_le_bytes());
    for placement in placements.as_slice() {
        use layout_plans::{IntegerInterpretation, LayoutPlacementReport};
        match *placement {
            LayoutPlacementReport::At { offset } => {
                bytes.push(0);
                bytes.extend_from_slice(&offset.to_le_bytes());
            }
            LayoutPlacementReport::IntegerAt {
                offset,
                stored_width,
                interpretation,
            } => {
                bytes.push(1);
                bytes.extend_from_slice(&offset.to_le_bytes());
                bytes.extend_from_slice(&stored_width.to_le_bytes());
                bytes.push(match interpretation {
                    IntegerInterpretation::Signed => 0,
                    IntegerInterpretation::Unsigned => 1,
                });
            }
            LayoutPlacementReport::Bits {
                container,
                container_width,
                destination_lsb,
                source_lsb,
                width,
            } => {
                bytes.push(2);
                for value in [
                    container,
                    container_width,
                    destination_lsb,
                    source_lsb,
                    width,
                ] {
                    bytes.extend_from_slice(&value.to_le_bytes());
                }
            }
        }
    }
}
