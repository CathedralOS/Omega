use crate::Site;

#[derive(Debug, Eq, PartialEq)]
pub enum CollectionSelectors {
    Index(Site),
    Range { start: Option<Site>, end: Option<Site>, inclusive: bool },
}
