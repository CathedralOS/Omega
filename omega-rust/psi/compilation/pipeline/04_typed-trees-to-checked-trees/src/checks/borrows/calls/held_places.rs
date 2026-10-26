//! Simultaneous holds use the complete retained access frontier.
//!
//! The common argument resolver has already authenticated the actual descriptor,
//! its completion cut, every alternative and its loan lineage. That frontier may
//! conservatively cover more storage than the pointer's exact address. Comparing
//! all of it is sufficient for interference; requiring a second exact-address
//! inventory would reject valid dynamic projections. This query supplies no
//! address, narrower permission, pointer load or lifetime extension.
use checked_trees::CapturedPlace;

use super::super::place_access::ArgumentBackings;

pub(super) fn places(backings: &ArgumentBackings) -> impl Iterator<Item = &CapturedPlace> {
    backings.places.iter().map(|backing| &backing.place)
}
