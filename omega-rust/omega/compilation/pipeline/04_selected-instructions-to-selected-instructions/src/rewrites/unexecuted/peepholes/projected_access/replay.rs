//! Independent replay of a projected-access fold.
//!
//! The validator never consults
//! [`ProjectedAccessRule`](super::pair::ProjectedAccessRule) or the declared
//! table: it re-derives the whole grammar from the instruction records —
//! the consumer kind and its two-operand shape, the pointer register's last
//! in-block definition as an `AddressOffset` producer, the base register's
//! freedom from intervening redefinition, the access width and the
//! combined-displacement bound, and the catalog surface the access row and
//! the isolated projection row must carry — then rebuilds the expected
//! folded record, requires the proposal to equal it, and restores the
//! complete source by content.
