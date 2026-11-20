use source_text::Span;

/// One part of the selected Build entry. The parser supplies the ordered byte
/// spans of its top-level preparation calls in the captured source file.
/// A fresh assembly can assign new file IDs, while these source coordinates
/// must agree before the activation continues.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildStage {
    index: usize,
    preparations: Vec<Span>,
}

impl BuildStage {
    pub const WHOLE: Self = Self {
        index: 0,
        preparations: Vec::new(),
    };

    pub fn at(index: usize, preparations: Vec<Span>) -> Option<Self> {
        (index <= preparations.len()).then_some(Self {
            index,
            preparations,
        })
    }

    pub fn next_index(&self) -> Option<usize> {
        (self.index < self.preparations.len()).then(|| self.index + 1)
    }

    pub fn preparations(&self) -> &[Span] {
        &self.preparations
    }
}
