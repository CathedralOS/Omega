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
