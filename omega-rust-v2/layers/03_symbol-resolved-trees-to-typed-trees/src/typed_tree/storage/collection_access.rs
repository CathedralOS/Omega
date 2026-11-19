use crate::{CollectionSelectors, Site, ValueType};
use omega_resolver::DeclarationId;

#[derive(Debug, Eq, PartialEq)]
pub struct CollectionAccess {
    pub site: Site,
    pub collection: Site,
    pub machine: DeclarationId,
    pub element: ValueType,
    pub result: ValueType,
    pub selectors: CollectionSelectors,
}
