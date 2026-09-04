//! The three section kinds every lane agrees on.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionPlan {
    pub kind: SectionKind,
    pub size: usize,
    pub alignment: usize,
}

impl Default for SectionPlan {
    fn default() -> Self {
        Self {
            kind: SectionKind::Text,
            size: 0,
            alignment: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionKind {
    Text,
    Data,
    Bss,
}
