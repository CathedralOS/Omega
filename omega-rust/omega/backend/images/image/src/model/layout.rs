use crate::model::FinalImageSection;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FinalImageLayout {
    pub text_address: u64,
    pub data_address: u64,
    pub bss_address: u64,
}

impl FinalImageLayout {
    pub const fn section_address(self, section: FinalImageSection) -> Option<u64> {
        match section {
            FinalImageSection::Text => Some(self.text_address),
            FinalImageSection::Data => Some(self.data_address),
            FinalImageSection::Bss => Some(self.bss_address),
            FinalImageSection::None => None,
        }
    }
}
