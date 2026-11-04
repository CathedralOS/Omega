//! Keep the checked table geometry and exact register operation together.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum DescriptorTableOperation {
    /// Check the loaded table and task descriptor before changing TR.
    ActivateTask { entries: u16 },
    /// Observe the selector currently retained by TR.
    ReadTask,
    /// Select a kernel code or data descriptor from the current borrowed GDT.
    ReloadSegments { entries: u16, kind: SegmentReload },
    /// Observe one exact segment selector.
    ReadSegment { register: SegmentRegister },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum SegmentReload {
    Code,
    Data,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum SegmentRegister {
    Code,
    Data,
    Extra,
    Stack,
    Fs,
    Gs,
}

impl SegmentReload {
    pub const fn code(self) -> u8 {
        match self {
            Self::Code => 0,
            Self::Data => 1,
        }
    }
    pub const fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::Code),
            1 => Some(Self::Data),
            _ => None,
        }
    }
}
impl SegmentRegister {
    pub const fn code(self) -> u8 {
        match self {
            Self::Code => 0,
            Self::Data => 1,
            Self::Extra => 2,
            Self::Stack => 3,
            Self::Fs => 4,
            Self::Gs => 5,
        }
    }
    pub const fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::Code),
            1 => Some(Self::Data),
            2 => Some(Self::Extra),
            3 => Some(Self::Stack),
            4 => Some(Self::Fs),
            5 => Some(Self::Gs),
            _ => None,
        }
    }
}
