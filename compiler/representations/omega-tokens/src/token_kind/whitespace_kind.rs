#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WhitespaceKind {
    Space,
    Newline,
    #[default]
    Mixed,
}
