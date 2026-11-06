//! Compiler-private x86-64 installation and arrival token layout.
//!
//! These offsets describe admitted storage; matching bytes alone never grants
//! root identity, backing permission, or authority to receive an interrupt.

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

/// Hardware frame shape at an explicitly admitted interrupt entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstalledInterruptFrame {
    NoErrorCode,
    ErrorCode,
    /// A nonreturning handler never inspects or resumes this CPU frame.
    /// Reserve the larger six-word shape without claiming whether an error
    /// word exists for this vector on the selected CPU.
    OpaqueFatal,
}

/// How an arrival obtains its semantic argument. Fatal context borrowing does
/// not construct an acknowledgement or grant permission to complete one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstalledInterruptProtocol {
    Acknowledgement,
    FatalContext {
        anchor_gate_byte_offset: u32,
        entry_gate_byte_offset: u32,
        table_bytes: u32,
    },
}

/// Geometry retained with the installed arrival adapter. Acknowledgement entries
/// use an aligned, exclusively leased IST top. Fatal entries can use that class
/// of backing or the interrupted stack and never inspect or return the CPU frame.
/// The root-slot distance always refers to the designated timer's IST top,
/// including when a fatal entry recovers that root from another stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstalledInterruptEntryPlan {
    pub frame: InstalledInterruptFrame,
    pub protocol: InstalledInterruptProtocol,
    pub root_slot_from_top: u32,
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
        if anchor_gate_byte_offset % 16 != 0
            || entry_gate_byte_offset % 16 != 0
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
