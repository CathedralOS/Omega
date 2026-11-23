/// Projection from the selected data layout, identified by its core declaration.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum LayoutQuery {
    Size,
    Alignment,
}
