//! Normalized layout plan reports and their replay fingerprints.
//!
//! Reports describe validated geometry: field entries, placements, and
//! conventional sum layouts. Fingerprints and replay matching compare two
//! reports by canonical member identity, not by declaration order.

use crate::materialization::field_identities::validate_materialization_field_identities;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IntegerInterpretation {
    Signed,
    Unsigned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutPlacementReport {
    At {
        offset: u64,
    },
    IntegerAt {
        offset: u64,
        stored_width: u64,
        interpretation: IntegerInterpretation,
    },
    Bits {
        container: u64,
        container_width: u64,
        destination_lsb: u64,
        source_lsb: u64,
        width: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutFieldEntryReport {
    /// Normalized field name. Compiler-issued keys do not escape into artifact
    /// reports.
    pub field: String,
    /// Authored stable schema identity when this scope is numbered. Canonical
    /// plan identity uses this instead of the source-facing name, so a rename
    /// preserves identity.
    pub member_identity: Option<u64>,
    pub placement: LayoutPlacementReport,
}

/// A validated layout plan, ready for consumers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutPlanReport {
    /// Compact FNV report coordinate for the complete reflected schema.
    ///
    /// This is never authority. Typed consumers retain the exact schema and
    /// replay its members and physical requirements; later consumers retain
    /// the complete validated layout. Stable member identities below are exact
    /// authored semantic values rather than hashes.
    pub schema_report_fingerprint: u64,
    pub entries: Vec<LayoutFieldEntryReport>,
    /// Declaration-order offsets when every field has one fixed `At`
    /// placement. Fragmented plans deliberately have no such projection.
    pub offsets: Option<Vec<u64>>,
    pub size: Option<u64>,
    pub align: u64,
}

/// One compiler-owned conventional payload field in a case-bearing runtime
/// layout. Unlike [`LayoutPlanReport`], this is not source-programmable
/// placement vocabulary: it reports the language implementation's fixed
/// tag-prefixed overlay representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalSumPayloadFieldLayoutReport {
    pub field: String,
    pub member_identity: Option<u64>,
    /// Absolute byte offset within the complete sum value.
    pub offset: u64,
    pub size: u64,
    pub align: u64,
}

/// One authored-order case and its relevant payload geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalSumCaseLayoutReport {
    pub case: String,
    pub member_identity: Option<u64>,
    /// Runtime discriminant, fixed by authored case order rather than stable
    /// schema identity.
    pub ordinal: u32,
    pub payload_fields: Vec<ConventionalSumPayloadFieldLayoutReport>,
}

/// Exact compiler-owned conventional layout for one closed pure sum or one
/// closed common-field/case mixed shape.
///
/// This report does not extend programmable `Layout` policies with tag/case
/// placement. It is a target-closed observation of the existing fixed runtime
/// representation and grants no storage or materialization authority alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalSumLayoutReport {
    /// Compact schema report coordinate only. Exact case/member rows and their
    /// compiler-owned conventional geometry govern replay.
    pub schema_report_fingerprint: u64,
    pub tag_offset: u64,
    pub tag_size: u64,
    pub tag_align: u64,
    /// A mixed shape's leading common fields in authored order: absolute byte
    /// offsets inside the complete value, packed between the tag and the
    /// shared payload overlay. Empty for a pure sum — the field's presence on
    /// the report does not change pure-sum geometry or fingerprints.
    pub common_fields: Vec<ConventionalSumPayloadFieldLayoutReport>,
    pub cases: Vec<ConventionalSumCaseLayoutReport>,
    pub size: u64,
    pub align: u64,
}

/// One direct runtime-relevant pure-sum occurrence inside a conventional
/// record materialization layout.
///
/// The outer field identity is retained per occurrence, rather than deducing
/// rows from the nested schema, because the same sum type may appear more than
/// once and each occurrence may select a different case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalSumFieldLayoutReport {
    pub field: String,
    pub member_identity: Option<u64>,
    pub layout: ConventionalSumLayoutReport,
}

/// One direct runtime-relevant fixed-array field whose elements all use the
/// same compiler-owned conventional pure-sum layout.
///
/// The report is deliberately compact in the literal element count. Selected
/// cases and bytes remain value-sensitive facts retained once per index by the
/// validated materialization carrier, rather than duplicating this complete
/// all-case layout report once per array element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalSumArrayFieldLayoutReport {
    pub field: String,
    pub member_identity: Option<u64>,
    pub element_count: u64,
    pub element_stride: u64,
    pub element_layout: ConventionalSumLayoutReport,
}

/// One bounded two-segment path from an outer record field to the complete
/// direct conventional pure-sum occurrences of the record stored there.
///
/// Both record layouts and every child sum row are projected from one target
/// runtime plan. The outer occurrence remains explicit so consumers never
/// flatten the child rows into the outer schema or infer custody from names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalNestedRecordSumPathLayoutReport {
    pub outer_layout: LayoutPlanReport,
    pub outer_field: String,
    pub outer_member_identity: Option<u64>,
    pub inner_layout: LayoutPlanReport,
    pub child_sum_layouts: Vec<ConventionalSumFieldLayoutReport>,
}

/// One exact direct outer-field occurrence and the complete inner-record
/// layout facts reachable through that single segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalNestedRecordSumOccurrenceLayoutReport {
    pub outer_field: String,
    pub outer_member_identity: Option<u64>,
    pub inner_layout: LayoutPlanReport,
    pub child_sum_layouts: Vec<ConventionalSumFieldLayoutReport>,
}

/// Compact complete authored-order set of qualifying one-level record paths.
///
/// The outer layout is retained once. Each occurrence owns exactly one inner
/// layout and its complete direct-sum rows, so repeated uses of the same inner
/// type remain distinct without multiplying layouts by selected values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalNestedRecordSumPathsLayoutReport {
    pub outer_layout: LayoutPlanReport,
    pub paths: Vec<ConventionalNestedRecordSumOccurrenceLayoutReport>,
}

/// One exact outer-field occurrence in a recursively nested record path.
///
/// `inner` retains the complete path report for the next record boundary.
/// The recursive report uses this carrier for each record boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalRecordSumOccurrenceLayoutReport {
    pub outer_field: String,
    pub outer_member_identity: Option<u64>,
    pub inner: ConventionalRecursiveRecordSumPathsLayoutReport,
}

/// One exact direct outer-field occurrence of a nonzero literal `[R; N]`
/// fixed array whose record element still reaches conventional sums.
///
/// `inner` retains the complete recursive report every element shares — the
/// element type's own record/sum geometry, retained once rather than per
/// index. `element_count` is the literal declared length and
/// `element_stride` the constant byte distance between consecutive elements,
/// so the field's whole extent stays one `At` placement in the enclosing
/// `outer_layout` while the exact element index stays semantic data on the
/// path. This is the record counterpart of
/// [`ConventionalSumArrayFieldLayoutReport`]: the same compact row shape, but
/// each element crosses one record boundary before reaching sums, so the row
/// carries the element's recursive report instead of one sum overlay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalRecordArrayFieldLayoutReport {
    pub field: String,
    pub member_identity: Option<u64>,
    /// The literal element count the field's extent covers.
    pub element_count: u64,
    /// The constant byte distance between consecutive elements; it covers the
    /// complete `inner` outer extent so repeated elements cannot overlap.
    pub element_stride: u64,
    /// The element record's complete recursive record/sum report, shared by
    /// every index the field spells.
    pub inner: ConventionalRecursiveRecordSumPathsLayoutReport,
}

/// Complete authored-order path reports below one enclosing record layout.
///
/// The child report retains exact geometry and semantic occurrence identity.
/// `child_sum_layouts` retains the record level's own direct conventional
/// pure-sum fields beside its deeper record paths, and
/// `child_sum_array_layouts` retains the level's direct fixed arrays of
/// conventional pure sums: a record that contains a direct sum, a direct
/// sum array, and reaches sums through a record field spells one `Branch`
/// carrying all three, rather than rejecting the direct children the `Leaf`
/// level already retains. `child_record_array_layouts` adds the level's
/// direct fixed arrays of records still reaching sums — the fourth child
/// kind the same general rule admits, one literal element hop away.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalRecordSumPathsLayoutReport {
    pub outer_layout: LayoutPlanReport,
    pub paths: Vec<ConventionalRecordSumOccurrenceLayoutReport>,
    /// The enclosing record level's own direct conventional pure-sum fields,
    /// in authored order — the same channel `Leaf` carries, retained beside
    /// the deeper record paths so the two child kinds coexist at one level.
    pub child_sum_layouts: Vec<ConventionalSumFieldLayoutReport>,
    /// The enclosing record level's own direct fixed arrays of conventional
    /// pure sums, in authored order — one compact row per occurrence, kept
    /// beside `child_sum_layouts` and the deeper record paths under the same
    /// general recursive rule rather than fenced to a top-level-only rung.
    pub child_sum_array_layouts: Vec<ConventionalSumArrayFieldLayoutReport>,
    /// The enclosing record level's own direct fixed arrays of records still
    /// reaching sums, in authored order — one compact row per occurrence,
    /// each carrying the element record's complete recursive report so an
    /// indexed path composes one element hop before crossing the record
    /// boundary inside it.
    pub child_record_array_layouts: Vec<ConventionalRecordArrayFieldLayoutReport>,
}

/// Recursive record-path geometry. Each occurrence retains its own exact record
/// boundary; nesting depth is data rather than a family of Rust interfaces.
/// This report is not custody; consumers independently validate it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConventionalRecursiveRecordSumPathsLayoutReport {
    Leaf {
        outer_layout: LayoutPlanReport,
        child_sum_layouts: Vec<ConventionalSumFieldLayoutReport>,
        /// The leaf level's direct fixed arrays of conventional pure sums, in
        /// authored order — a level that ends the record recursion still
        /// carries both direct child kinds.
        child_sum_array_layouts: Vec<ConventionalSumArrayFieldLayoutReport>,
        /// The leaf level's direct fixed arrays of records still reaching
        /// sums, in authored order — each row's `inner` carries the element
        /// record's own recursive report, so the record recursion continues
        /// inside the repeated element rather than ending at the field.
        child_record_array_layouts: Vec<ConventionalRecordArrayFieldLayoutReport>,
    },
    Branch(ConventionalRecordSumPathsLayoutReport),
}

/// Compiler resource limit shared by recursive projection and materialization.
/// This is not a language limit or a distinct layout judgment per depth.
pub const CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT: usize = 64;

impl ConventionalRecursiveRecordSumPathsLayoutReport {
    pub fn outer_layout(&self) -> &LayoutPlanReport {
        match self {
            Self::Leaf { outer_layout, .. } => outer_layout,
            Self::Branch(report) => &report.outer_layout,
        }
    }

    /// The total conventional-sum leaf occurrences the report reaches: a
    /// `Leaf` level's own direct sums and sum arrays plus every record-array
    /// row's element-level leaf occurrences, or a `Branch` level's direct
    /// children plus the leaf occurrences of every deeper record path.
    pub fn leaf_occurrence_count(&self) -> Option<usize> {
        match self {
            Self::Leaf {
                child_sum_layouts,
                child_sum_array_layouts,
                child_record_array_layouts,
                ..
            } => child_record_array_layouts.iter().try_fold(
                child_sum_layouts
                    .len()
                    .checked_add(child_sum_array_layouts.len())?,
                |total, row| total.checked_add(row.inner.leaf_occurrence_count()?),
            ),
            Self::Branch(report) => {
                let level = report.child_record_array_layouts.iter().try_fold(
                    report
                        .child_sum_layouts
                        .len()
                        .checked_add(report.child_sum_array_layouts.len())?,
                    |total, row| total.checked_add(row.inner.leaf_occurrence_count()?),
                )?;
                report.paths.iter().try_fold(level, |total, path| {
                    total.checked_add(path.inner.leaf_occurrence_count()?)
                })
            }
        }
    }
}

/// One normalized semantic-field-free callback destination in a native
/// layout. Canonical strings remain report coordinates; the retained slot
/// application carries the producer's exact named selection. The payload is
/// generic so this foundation does not depend on a later representation.
/// The authoritative layout policy owns
/// `offset`, but callback-address size/alignment close later with the selected
/// target calling plan and are deliberately absent here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateCallbackLayoutDemandReport<SlotApplication> {
    pub slot_application: SlotApplication,
    pub slot_identity: String,
    pub layout_subject_identity: String,
    pub callback_requirement_identity: String,
    pub offset: u64,
}

/// One validated native layout and its compiler-private demand catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeLayoutPlanReport<SlotApplication> {
    pub layout: LayoutPlanReport,
    pub private_callback_demands: Vec<PrivateCallbackLayoutDemandReport<SlotApplication>>,
}

/// Deterministic compact report coordinate for one validated layout plan.
///
/// This value is never authority: exact layout replay and the strong access
/// layout commitment govern admission.
///
/// Compiler-issued field keys, numbered-member source names, and authored entry
/// order are deliberately absent. Repeated fragments are sorted by stable
/// member identity (or by name for positional schemas) and complete normalized
/// placement, while the schema report coordinate, size, and alignment remain
/// identity-bearing. The derived `offsets` convenience projection is excluded
/// because it contains no fact beyond the entries.
pub fn normalized_layout_plan_report_fingerprint(layout: &LayoutPlanReport) -> u64 {
    let mut entries = layout.entries.iter().collect::<Vec<_>>();
    entries.sort_unstable_by(|left, right| {
        member_sort_key(left)
            .cmp(&member_sort_key(right))
            .then_with(|| {
                placement_sort_key(&left.placement).cmp(&placement_sort_key(&right.placement))
            })
    });

    let mut hash = 0xcbf29ce484222325u64;
    hash_fingerprint_bytes(&mut hash, b"omega.layout-plan.v3");
    hash_fingerprint_u64(&mut hash, layout.schema_report_fingerprint);
    hash_fingerprint_byte(&mut hash, u8::from(layout.size.is_some()));
    if let Some(size) = layout.size {
        hash_fingerprint_u64(&mut hash, size);
    }
    hash_fingerprint_u64(&mut hash, layout.align);
    hash_fingerprint_u64(&mut hash, entries.len() as u64);
    for entry in entries {
        match entry.member_identity {
            Some(identity) => {
                hash_fingerprint_byte(&mut hash, 1);
                hash_fingerprint_u64(&mut hash, identity);
            }
            None => {
                hash_fingerprint_byte(&mut hash, 0);
                hash_fingerprint_u64(&mut hash, entry.field.len() as u64);
                hash_fingerprint_bytes(&mut hash, entry.field.as_bytes());
            }
        }
        match entry.placement {
            LayoutPlacementReport::At { offset } => {
                hash_fingerprint_byte(&mut hash, 0);
                hash_fingerprint_u64(&mut hash, offset);
            }
            LayoutPlacementReport::IntegerAt {
                offset,
                stored_width,
                interpretation,
            } => {
                hash_fingerprint_byte(&mut hash, 2);
                hash_fingerprint_u64(&mut hash, offset);
                hash_fingerprint_u64(&mut hash, stored_width);
                hash_fingerprint_byte(
                    &mut hash,
                    match interpretation {
                        IntegerInterpretation::Signed => 0,
                        IntegerInterpretation::Unsigned => 1,
                    },
                );
            }
            LayoutPlacementReport::Bits {
                container,
                container_width,
                destination_lsb,
                source_lsb,
                width,
            } => {
                hash_fingerprint_byte(&mut hash, 1);
                for value in [
                    container,
                    container_width,
                    destination_lsb,
                    source_lsb,
                    width,
                ] {
                    hash_fingerprint_u64(&mut hash, value);
                }
            }
        }
    }
    if hash == 0 { 1 } else { hash }
}

/// Deterministic compact report coordinate for an exact conventional sum report.
/// Case ordinal remains identity-bearing even for numbered schemas because it
/// controls the runtime tag. Numbered source names are presentation-only.
pub fn normalized_conventional_sum_layout_report_fingerprint(
    layout: &ConventionalSumLayoutReport,
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_fingerprint_bytes(&mut hash, b"omega.conventional-sum-layout.v1");
    for value in [
        layout.schema_report_fingerprint,
        layout.tag_offset,
        layout.tag_size,
        layout.tag_align,
        layout.size,
        layout.align,
        layout.cases.len() as u64,
    ] {
        hash_fingerprint_u64(&mut hash, value);
    }
    // A pure sum's empty common-field list contributes nothing, so an
    // existing pure-sum report keeps its fingerprint byte for byte; a mixed
    // report's common rows hash in between the case count and the case rows.
    if !layout.common_fields.is_empty() {
        hash_fingerprint_u64(&mut hash, layout.common_fields.len() as u64);
        for field in &layout.common_fields {
            hash_optional_member_identity(&mut hash, field.member_identity, &field.field);
            for value in [field.offset, field.size, field.align] {
                hash_fingerprint_u64(&mut hash, value);
            }
        }
    }
    for case in &layout.cases {
        hash_fingerprint_u64(&mut hash, u64::from(case.ordinal));
        hash_optional_member_identity(&mut hash, case.member_identity, &case.case);
        hash_fingerprint_u64(&mut hash, case.payload_fields.len() as u64);
        for field in &case.payload_fields {
            hash_optional_member_identity(&mut hash, field.member_identity, &field.field);
            for value in [field.offset, field.size, field.align] {
                hash_fingerprint_u64(&mut hash, value);
            }
        }
    }
    if hash == 0 { 1 } else { hash }
}

/// Exact, hash-free equality for replaying one retained conventional sum
/// report. Numbered case and payload names are presentation-only; authored
/// case ordinals and every geometry field remain identity-bearing.
pub fn conventional_sum_layout_reports_match_for_replay(
    current: &ConventionalSumLayoutReport,
    retained: &ConventionalSumLayoutReport,
) -> bool {
    if !conventional_sum_member_identities_are_unambiguous(current)
        || !conventional_sum_member_identities_are_unambiguous(retained)
        || current.schema_report_fingerprint != retained.schema_report_fingerprint
        || current.tag_offset != retained.tag_offset
        || current.tag_size != retained.tag_size
        || current.tag_align != retained.tag_align
        || current.size != retained.size
        || current.align != retained.align
        || current.cases.len() != retained.cases.len()
        || current.common_fields.len() != retained.common_fields.len()
    {
        return false;
    }

    let common_fields_match = current
        .common_fields
        .iter()
        .zip(&retained.common_fields)
        .all(|(current_field, retained_field)| {
            current_field.member_identity == retained_field.member_identity
                && (current_field.member_identity.is_some()
                    || current_field.field == retained_field.field)
                && current_field.offset == retained_field.offset
                && current_field.size == retained_field.size
                && current_field.align == retained_field.align
        });
    if !common_fields_match {
        return false;
    }

    current
        .cases
        .iter()
        .zip(&retained.cases)
        .all(|(current_case, retained_case)| {
            current_case.member_identity == retained_case.member_identity
                && (current_case.member_identity.is_some()
                    || current_case.case == retained_case.case)
                && current_case.ordinal == retained_case.ordinal
                && current_case.payload_fields.len() == retained_case.payload_fields.len()
                && current_case
                    .payload_fields
                    .iter()
                    .zip(&retained_case.payload_fields)
                    .all(|(current_field, retained_field)| {
                        current_field.member_identity == retained_field.member_identity
                            && (current_field.member_identity.is_some()
                                || current_field.field == retained_field.field)
                            && current_field.offset == retained_field.offset
                            && current_field.size == retained_field.size
                            && current_field.align == retained_field.align
                    })
        })
}

fn conventional_sum_member_identities_are_unambiguous(
    layout: &ConventionalSumLayoutReport,
) -> bool {
    for (index, field) in layout.common_fields.iter().enumerate() {
        if layout.common_fields[..index]
            .iter()
            .any(|prior| match field.member_identity {
                Some(identity) => prior.member_identity == Some(identity),
                None => prior.member_identity.is_none() && prior.field == field.field,
            })
        {
            return false;
        }
    }
    for (index, case) in layout.cases.iter().enumerate() {
        if layout.cases[..index]
            .iter()
            .any(|prior| match case.member_identity {
                Some(identity) => prior.member_identity == Some(identity),
                None => prior.member_identity.is_none() && prior.case == case.case,
            })
        {
            return false;
        }
        for (field_index, field) in case.payload_fields.iter().enumerate() {
            if case.payload_fields[..field_index]
                .iter()
                .any(|prior| match field.member_identity {
                    Some(identity) => prior.member_identity == Some(identity),
                    None => prior.member_identity.is_none() && prior.field == field.field,
                })
            {
                return false;
            }
        }
    }
    true
}

fn hash_optional_member_identity(hash: &mut u64, identity: Option<u64>, name: &str) {
    match identity {
        Some(identity) => {
            hash_fingerprint_byte(hash, 1);
            hash_fingerprint_u64(hash, identity);
        }
        None => {
            hash_fingerprint_byte(hash, 0);
            hash_fingerprint_u64(hash, name.len() as u64);
            hash_fingerprint_bytes(hash, name.as_bytes());
        }
    }
}

/// Compact report coordinate for a native layout including its private demands. The
/// base layout remains independently reusable by semantic projection; private
/// placement participates only in native-layout identity. Retained application
/// custody is deliberately excluded from this existing compatibility report;
/// consumers must join that exact payload independently of the compact value.
pub fn normalized_native_layout_plan_report_fingerprint<SlotApplication>(
    layout: &NativeLayoutPlanReport<SlotApplication>,
) -> u64 {
    let mut demands = layout.private_callback_demands.iter().collect::<Vec<_>>();
    demands.sort_unstable_by(|left, right| {
        left.slot_identity
            .cmp(&right.slot_identity)
            .then_with(|| {
                left.callback_requirement_identity
                    .cmp(&right.callback_requirement_identity)
            })
            .then_with(|| left.offset.cmp(&right.offset))
    });
    let mut hash = 0xcbf29ce484222325u64;
    hash_fingerprint_bytes(&mut hash, b"omega.native-layout-plan.v1");
    hash_fingerprint_u64(
        &mut hash,
        normalized_layout_plan_report_fingerprint(&layout.layout),
    );
    hash_fingerprint_u64(&mut hash, demands.len() as u64);
    for demand in demands {
        for identity in [
            demand.slot_identity.as_bytes(),
            demand.layout_subject_identity.as_bytes(),
            demand.callback_requirement_identity.as_bytes(),
        ] {
            hash_fingerprint_u64(&mut hash, identity.len() as u64);
            hash_fingerprint_bytes(&mut hash, identity);
        }
        hash_fingerprint_u64(&mut hash, demand.offset);
    }
    if hash == 0 { 1 } else { hash }
}

/// Exact, hash-free equality for replaying one retained validated layout.
///
/// Numbered member names are presentation and may change. Every semantic
/// identity, placement, fixed/dynamic size, alignment, and derived offsets
/// projection must otherwise agree. Callers use this relation for acceptance;
/// the compact fingerprint remains report/cache identity only.
pub fn layout_plan_reports_match_for_replay(
    current: &LayoutPlanReport,
    retained: &LayoutPlanReport,
) -> bool {
    if validate_materialization_field_identities(current).is_err()
        || validate_materialization_field_identities(retained).is_err()
        || current.schema_report_fingerprint != retained.schema_report_fingerprint
        || current.offsets != retained.offsets
        || current.size != retained.size
        || current.align != retained.align
        || current.entries.len() != retained.entries.len()
    {
        return false;
    }

    let mut current_entries = current.entries.iter().collect::<Vec<_>>();
    current_entries.sort_unstable_by(|left, right| {
        member_sort_key(left)
            .cmp(&member_sort_key(right))
            .then_with(|| {
                placement_sort_key(&left.placement).cmp(&placement_sort_key(&right.placement))
            })
    });
    let mut retained_entries = retained.entries.iter().collect::<Vec<_>>();
    retained_entries.sort_unstable_by(|left, right| {
        member_sort_key(left)
            .cmp(&member_sort_key(right))
            .then_with(|| {
                placement_sort_key(&left.placement).cmp(&placement_sort_key(&right.placement))
            })
    });

    current_entries
        .into_iter()
        .zip(retained_entries)
        .all(|(current, retained)| {
            current.member_identity == retained.member_identity
                && (current.member_identity.is_some() || current.field == retained.field)
                && current.placement == retained.placement
        })
}

fn member_sort_key(entry: &LayoutFieldEntryReport) -> (u8, u64, &str) {
    match entry.member_identity {
        Some(identity) => (0, identity, ""),
        None => (1, 0, entry.field.as_str()),
    }
}

fn placement_sort_key(placement: &LayoutPlacementReport) -> (u8, u64, u64, u64, u64, u64) {
    match *placement {
        LayoutPlacementReport::At { offset } => (0, offset, 0, 0, 0, 0),
        LayoutPlacementReport::IntegerAt {
            offset,
            stored_width,
            interpretation,
        } => (
            2,
            offset,
            stored_width,
            match interpretation {
                IntegerInterpretation::Signed => 0,
                IntegerInterpretation::Unsigned => 1,
            },
            0,
            0,
        ),
        LayoutPlacementReport::Bits {
            container,
            container_width,
            destination_lsb,
            source_lsb,
            width,
        } => (
            1,
            container,
            container_width,
            destination_lsb,
            source_lsb,
            width,
        ),
    }
}

pub(crate) fn hash_fingerprint_u64(hash: &mut u64, value: u64) {
    hash_fingerprint_bytes(hash, &value.to_le_bytes());
}

pub(crate) fn hash_fingerprint_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        hash_fingerprint_byte(hash, *byte);
    }
}

pub(crate) fn hash_fingerprint_byte(hash: &mut u64, byte: u8) {
    *hash ^= u64::from(byte);
    *hash = hash.wrapping_mul(0x100000001b3);
}
