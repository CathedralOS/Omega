#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CommentKind {
    Line,
    #[default]
    Block,
}
