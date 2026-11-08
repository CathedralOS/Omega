/*
We define the compiler-private x86-64 static-interrupt token offsets (`root`,
`acknowledgement`, `permit`) and the hardware arrival entry plan
(`InstalledInterruptEntryPlan`, `InstalledInterruptFrame`,
`InstalledInterruptProtocol`).

These offsets and frame sizes describe admitted storage geometry; matching
bytes alone never grants root identity, IST backing permission, or interrupt
delivery authority.
*/

pub const WORD_BYTES: u32 = 8;
pub const ALIGNMENT: u32 = 8;

pub mod root {
    pub const STATE: u32 = 0;
    pub const CONTEXT: u32 = 8;
    pub const INVOCATION: u32 = 16;
    pub const DEBT: u32 = 24;
    pub const COMPLETION: u32 = 32;
    pub const PROVIDER: u32 = 40;
    pub const BYTES: u32 = 48;
    pub const ACTIVE: u64 = 2;
}

pub mod acknowledgement {
    pub const ROOT: u32 = 0;
    pub const PROVIDER: u32 = 8;
    pub const INVOCATION: u32 = 16;
    pub const COMPLETION: u32 = 24;
    pub const DEBT: u32 = 32;
    pub const BYTES: u32 = 40;
}

pub mod permit {
    pub const ROOT: u32 = 0;
    pub const INVOCATION: u32 = 8;
    pub const DEBT: u32 = 16;
    pub const BYTES: u32 = 24;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstalledInterruptFrame {
    NoErrorCode,
    ErrorCode,
    OpaqueFatal, // Nonreturning handler reserves the 6-word shape without inspecting or resuming the CPU frame.
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstalledInterruptProtocol {
    Acknowledgement,
    FatalContext {
        anchor_gate_byte_offset: u32,
        entry_gate_byte_offset: u32,
        table_bytes: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstalledInterruptEntryPlan {
    pub frame: InstalledInterruptFrame,
    pub protocol: InstalledInterruptProtocol,
    pub root_slot_from_top: u32, // Distance to the designated timer's IST top.
    pub return_frame_bytes: u32,
    pub saved_register_bytes: u32,
}

impl InstalledInterruptEntryPlan {
    pub const fn new(frame: InstalledInterruptFrame) -> Self {
        Self {
            frame,
            protocol: InstalledInterruptProtocol::Acknowledgement,
            root_slot_from_top: 56,
            return_frame_bytes: 40,
            saved_register_bytes: 120,
        }
    }

    pub const fn fatal_context(
        anchor_gate_byte_offset: u32,
        entry_gate_byte_offset: u32,
        table_bytes: u32,
    ) -> Option<Self> {
        if !anchor_gate_byte_offset.is_multiple_of(16)
            || !entry_gate_byte_offset.is_multiple_of(16)
            || anchor_gate_byte_offset == entry_gate_byte_offset
            || table_bytes < 16
            || table_bytes > 4096
            || anchor_gate_byte_offset > table_bytes - 16
            || entry_gate_byte_offset > table_bytes - 16
        {
            return None;
        }
        Some(Self {
            frame: InstalledInterruptFrame::OpaqueFatal,
            protocol: InstalledInterruptProtocol::FatalContext {
                anchor_gate_byte_offset,
                entry_gate_byte_offset,
                table_bytes,
            },
            root_slot_from_top: 56,
            return_frame_bytes: 40,
            saved_register_bytes: 120,
        })
    }

    pub fn has_canonical_geometry(self) -> bool {
        match self.protocol {
            InstalledInterruptProtocol::Acknowledgement => {
                self.frame != InstalledInterruptFrame::OpaqueFatal && self == Self::new(self.frame)
            }
            InstalledInterruptProtocol::FatalContext {
                anchor_gate_byte_offset,
                entry_gate_byte_offset,
                table_bytes,
            } => {
                Self::fatal_context(anchor_gate_byte_offset, entry_gate_byte_offset, table_bytes)
                    == Some(self)
            }
        }
    }

    pub const fn hardware_frame_bytes(self) -> u32 {
        self.return_frame_bytes
            + match self.frame {
                InstalledInterruptFrame::NoErrorCode => 0,
                InstalledInterruptFrame::ErrorCode | InstalledInterruptFrame::OpaqueFatal => {
                    WORD_BYTES
                }
            }
    }
}
