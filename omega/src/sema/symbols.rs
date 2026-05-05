use crate::sema::names::Name;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub name: Name,
}
