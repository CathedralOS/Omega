//! One recursive record level's per-row retained direct-child custody.
//!
//! A record level carries every classified child on the report's single
//! `children` channel, so its custody mirrors that channel: one authored-order
//! vector whose each row keeps the value-sensitive custody matching the
//! row's own hop and interior vocabulary. `Sum` and `Record` rows retain
//! their child reports inside the row's own custody while `SumArray` and
//! `RecordArray` rows keep only the compact per-element selections — the
//! shared element report lives once in the level's retained `path_layout`
//! row, so replay compares hash-free layout facts before any fingerprint
//! coordinate.

use super::{
    ValidatedConstRecordArrayFieldMaterialization,
    ValidatedConstRecordSumArrayFieldMaterialization, ValidatedConstRecordSumFieldMaterialization,
    ValidatedConstRecursiveNestedSumOccurrenceMaterialization,
};

/// Value-sensitive custody for one classified child row of a recursive record
/// level — the same path-segment vocabulary the report row spells, carried
/// in authored order on the level's `children` channel.
#[derive(Debug)]
pub enum ValidatedConstRecordSumChildMaterialization {
    /// A direct conventional sum field's custody — a pure sum or a mixed
    /// common-field/case shape reached by a field hop.
    Sum(ValidatedConstRecordSumFieldMaterialization),
    /// A direct `[S; N]` sum-array field's compact per-element custody,
    /// reached by a literal index hop.
    SumArray(ValidatedConstRecordSumArrayFieldMaterialization),
    /// A record field's own recursive custody, reached by a field hop.
    Record(ValidatedConstRecursiveNestedSumOccurrenceMaterialization),
    /// A direct `[R; N]` record-array field's compact per-element custody,
    /// reached by a literal index hop; the shared element record's report
    /// lives once in the level's retained `path_layout` row.
    RecordArray(ValidatedConstRecordArrayFieldMaterialization),
}

impl ValidatedConstRecordSumChildMaterialization {
    /// The field name the row's path segment spells — diagnostic
    /// presentation; `member_identity` stays the stable coordinate.
    pub fn field(&self) -> &str {
        match self {
            Self::Sum(custody) => custody.field(),
            Self::SumArray(custody) => custody.field(),
            Self::Record(occurrence) => occurrence.outer_field(),
            Self::RecordArray(custody) => custody.field(),
        }
    }

    /// The stable member identity the row's path segment spells.
    pub const fn member_identity(&self) -> Option<u64> {
        match self {
            Self::Sum(custody) => custody.field_identity(),
            Self::SumArray(custody) => custody.field_identity(),
            Self::Record(occurrence) => occurrence.outer_member_identity(),
            Self::RecordArray(custody) => custody.field_identity(),
        }
    }
}
