//! Exact-width external scalar transfers; normalization is register-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlacedScalarForm {
    pub write: bool,
    pub bytes: u8,
}
impl PlacedScalarForm {
    pub const COUNT: usize = 8;
    pub const ALL: [Self; Self::COUNT] = [
        Self {
            write: false,
            bytes: 1,
        },
        Self {
            write: false,
            bytes: 2,
        },
        Self {
            write: false,
            bytes: 4,
        },
        Self {
            write: false,
            bytes: 8,
        },
        Self {
            write: true,
            bytes: 1,
        },
        Self {
            write: true,
            bytes: 2,
        },
        Self {
            write: true,
            bytes: 4,
        },
        Self {
            write: true,
            bytes: 8,
        },
    ];
    pub const FAMILY_TAG: u8 = 246;
    pub const fn ordinal(self) -> u8 {
        let width = match self.bytes {
            1 => 0,
            2 => 1,
            4 => 2,
            8 => 3,
            _ => return 255,
        };
        (if self.write { 4 } else { 0 }) + width
    }
    pub const fn from_machine_payload(payload: u8) -> Option<Self> {
        if (payload as usize) < Self::COUNT {
            Some(Self::ALL[payload as usize])
        } else {
            None
        }
    }
    pub const fn machine_tag(self) -> u8 {
        Self::FAMILY_TAG
    }
    pub const fn machine_payload(self) -> u8 {
        self.ordinal()
    }
    pub const fn ordinary_semantic(self) -> Option<super::MachineSemanticKind> {
        use super::MachineSemanticKind as Kind;
        if self.write {
            if matches!(self.bytes, 1 | 2 | 4 | 8) {
                Some(Kind::Store)
            } else {
                None
            }
        } else {
            match self.bytes {
                1 => Some(Kind::Load8),
                2 => Some(Kind::Load16),
                4 => Some(Kind::Load32),
                8 => Some(Kind::Load64),
                _ => None,
            }
        }
    }
    pub const fn ordinary_kind(self, byte_offset: u32) -> Option<super::SelectedInstructionKind> {
        use super::SelectedInstructionKind as Kind;
        if self.write {
            Some(Kind::Store {
                byte_offset,
                byte_size: self.bytes,
            })
        } else {
            match self.bytes {
                1 => Some(Kind::Load8 { byte_offset }),
                2 => Some(Kind::Load16 { byte_offset }),
                4 => Some(Kind::Load32 { byte_offset }),
                8 => Some(Kind::Load64 { byte_offset }),
                _ => None,
            }
        }
    }
}
