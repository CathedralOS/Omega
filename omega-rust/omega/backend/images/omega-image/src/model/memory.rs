use crate::model::FinalImageSection;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalImageMemory {
    pub text: Vec<u8>,
    pub data: Vec<u8>,
    pub bss_size: usize,
    pub bss_alignment: usize,
}

impl Default for FinalImageMemory {
    fn default() -> Self {
        Self {
            text: Vec::new(),
            data: Vec::new(),
            bss_size: 0,
            bss_alignment: 1,
        }
    }
}

impl FinalImageMemory {
    pub fn initialized_section_mut(&mut self, section: FinalImageSection) -> Option<&mut Vec<u8>> {
        match section {
            FinalImageSection::Text => Some(&mut self.text),
            FinalImageSection::Data => Some(&mut self.data),
            FinalImageSection::Bss | FinalImageSection::None => None,
        }
    }
}
