/*
We define normalized layout plan reports (`LayoutPlanReport`,
`ConventionalSumLayoutReport`, and nested record-sum path reports), deterministic
placement sort keys, and replay-equivalence fingerprints used to compare and
cache layout geometry across compilation stages.
*/

use crate::materialization::field_identities::validate_materialization_field_identities;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, compiler_storage::Record)]
pub enum IntegerInterpretation {
    Signed,
    Unsigned,
}

impl IntegerInterpretation {
    pub fn stored_bounds(self, stored_width: u64) -> (i128, i128) {
        match self {
            Self::Signed => (
                -(1i128 << (stored_width - 1)),
                (1i128 << (stored_width - 1)) - 1,
            ),
            Self::Unsigned => (0, (1i128 << stored_width) - 1),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, compiler_storage::Record)]
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

impl Default for LayoutPlacementReport {
    fn default() -> Self {
        Self::At { offset: 0 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct LayoutFieldEntryReport {
    pub field: String,
    pub member_identity: Option<u64>,
    pub placement: LayoutPlacementReport,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct LayoutPlanReport {
    pub schema_report_fingerprint: u64,
    pub entries: Vec<LayoutFieldEntryReport>,
    pub offsets: Option<Vec<u64>>,
    pub size: Option<u64>,
    pub align: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalSumPayloadFieldLayoutReport {
    pub field: String,
    pub member_identity: Option<u64>,
    pub offset: u64,
    pub size: u64,
    pub align: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalSumCaseLayoutReport {
    pub case: String,
    pub member_identity: Option<u64>,
    pub ordinal: u32,
    pub payload_fields: Vec<ConventionalSumPayloadFieldLayoutReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalSumLayoutReport {
    pub schema_report_fingerprint: u64,
    pub tag_offset: u64,
    pub tag_size: u64,
    pub tag_align: u64,
    pub common_fields: Vec<ConventionalSumPayloadFieldLayoutReport>,
    pub cases: Vec<ConventionalSumCaseLayoutReport>,
    pub size: u64,
    pub align: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalSumFieldLayoutReport {
    pub field: String,
    pub member_identity: Option<u64>,
    pub layout: ConventionalSumLayoutReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalSumArrayFieldLayoutReport {
    pub field: String,
    pub member_identity: Option<u64>,
    pub element_count: u64,
    pub element_stride: u64,
    pub element_layout: ConventionalSumLayoutReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalNestedRecordSumPathLayoutReport {
    pub outer_layout: LayoutPlanReport,
    pub outer_field: String,
    pub outer_member_identity: Option<u64>,
    pub inner_layout: LayoutPlanReport,
    pub child_sum_layouts: Vec<ConventionalSumFieldLayoutReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalNestedRecordSumOccurrenceLayoutReport {
    pub outer_field: String,
    pub outer_member_identity: Option<u64>,
    pub inner_layout: LayoutPlanReport,
    pub child_sum_layouts: Vec<ConventionalSumFieldLayoutReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalNestedRecordSumPathsLayoutReport {
    pub outer_layout: LayoutPlanReport,
    pub paths: Vec<ConventionalNestedRecordSumOccurrenceLayoutReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConventionalRecordSumChildHop {
    Field,
    Index {
        element_count: u64,
        element_stride: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConventionalRecordSumChildInterior {
    Sum(ConventionalSumLayoutReport),
    Record(ConventionalRecursiveRecordSumPathsLayoutReport),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalRecordSumChildLayoutReport {
    pub field: String,
    pub member_identity: Option<u64>,
    pub hop: ConventionalRecordSumChildHop,
    pub interior: ConventionalRecordSumChildInterior,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalRecursiveRecordSumPathsLayoutReport {
    pub outer_layout: LayoutPlanReport,
    pub children: Vec<ConventionalRecordSumChildLayoutReport>,
}

pub const CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT: usize = 64;

impl ConventionalRecursiveRecordSumPathsLayoutReport {
    pub fn leaf_occurrence_count(&self) -> Option<usize> {
        self.children.iter().try_fold(0usize, |total, child| {
            total.checked_add(match &child.interior {
                ConventionalRecordSumChildInterior::Sum(_) => 1,
                ConventionalRecordSumChildInterior::Record(inner) => {
                    inner.leaf_occurrence_count()?
                }
            })
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct PrivateCallbackLayoutDemandReport<SlotApplication> {
    pub slot_application: SlotApplication,
    pub slot_identity: String,
    pub layout_subject_identity: String,
    pub callback_requirement_identity: String,
    pub placements: Vec<LayoutPlacementReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeLayoutPlanReport<SlotApplication> {
    pub layout: LayoutPlanReport,
    pub private_callback_demands: Vec<PrivateCallbackLayoutDemandReport<SlotApplication>>,
}

pub fn normalized_layout_plan_report_fingerprint(layout: &LayoutPlanReport) -> u64 {
    let mut entries = layout.entries.iter().collect::<Vec<_>>();
    entries.sort_unstable_by(|left, right| {
        member_sort_key(left)
            .cmp(&member_sort_key(right))
            .then_with(|| {
                layout_placement_sort_key(&left.placement)
                    .cmp(&layout_placement_sort_key(&right.placement))
            })
    });

    let mut hash = semantic_vocabulary::Fnv1a::new();
    hash.bytes(b"omega.layout-plan.v3");
    hash.u64(layout.schema_report_fingerprint);
    hash.byte(u8::from(layout.size.is_some()));
    if let Some(size) = layout.size {
        hash.u64(size);
    }
    hash.u64(layout.align);
    hash.u64(entries.len() as u64);
    for entry in entries {
        match entry.member_identity {
            Some(identity) => {
                hash.byte(1);
                hash.u64(identity);
            }
            None => {
                hash.byte(0);
                hash.u64(entry.field.len() as u64);
                hash.bytes(entry.field.as_bytes());
            }
        }
        match entry.placement {
            LayoutPlacementReport::At { offset } => {
                hash.byte(0);
                hash.u64(offset);
            }
            LayoutPlacementReport::IntegerAt {
                offset,
                stored_width,
                interpretation,
            } => {
                hash.byte(2);
                hash.u64(offset);
                hash.u64(stored_width);
                hash.byte(match interpretation {
                    IntegerInterpretation::Signed => 0,
                    IntegerInterpretation::Unsigned => 1,
                });
            }
            LayoutPlacementReport::Bits {
                container,
                container_width,
                destination_lsb,
                source_lsb,
                width,
            } => {
                hash.byte(1);
                for value in [
                    container,
                    container_width,
                    destination_lsb,
                    source_lsb,
                    width,
                ] {
                    hash.u64(value);
                }
            }
        }
    }
    hash.finish().max(1)
}

pub fn normalized_conventional_sum_layout_report_fingerprint(
    layout: &ConventionalSumLayoutReport,
) -> u64 {
    let mut hash = semantic_vocabulary::Fnv1a::new();
    hash.bytes(b"omega.conventional-sum-layout.v1");
    for value in [
        layout.schema_report_fingerprint,
        layout.tag_offset,
        layout.tag_size,
        layout.tag_align,
        layout.size,
        layout.align,
        layout.cases.len() as u64,
    ] {
        hash.u64(value);
    }
    if !layout.common_fields.is_empty() {
        hash.u64(layout.common_fields.len() as u64);
        for field in &layout.common_fields {
            hash_optional_member_identity(&mut hash, field.member_identity, &field.field);
            for value in [field.offset, field.size, field.align] {
                hash.u64(value);
            }
        }
    }
    for case in &layout.cases {
        hash.u64(u64::from(case.ordinal));
        hash_optional_member_identity(&mut hash, case.member_identity, &case.case);
        hash.u64(case.payload_fields.len() as u64);
        for field in &case.payload_fields {
            hash_optional_member_identity(&mut hash, field.member_identity, &field.field);
            for value in [field.offset, field.size, field.align] {
                hash.u64(value);
            }
        }
    }
    hash.finish().max(1)
}

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

fn hash_optional_member_identity(
    hash: &mut semantic_vocabulary::Fnv1a,
    identity: Option<u64>,
    name: &str,
) {
    match identity {
        Some(identity) => {
            hash.byte(1);
            hash.u64(identity);
        }
        None => {
            hash.byte(0);
            hash.u64(name.len() as u64);
            hash.bytes(name.as_bytes());
        }
    }
}

pub fn normalized_native_layout_plan_report_fingerprint<SlotApplication>(
    layout: &NativeLayoutPlanReport<SlotApplication>,
) -> u64 {
    let mut demands = layout.private_callback_demands.iter().collect::<Vec<_>>();
    demands.sort_unstable_by(|left, right| {
        left.slot_identity.cmp(&right.slot_identity).then_with(|| {
            left.callback_requirement_identity
                .cmp(&right.callback_requirement_identity)
        })
    });
    let mut hash = semantic_vocabulary::Fnv1a::new();
    hash.bytes(b"omega.native-layout-plan.v2");
    hash.u64(normalized_layout_plan_report_fingerprint(&layout.layout));
    hash.u64(demands.len() as u64);
    for demand in demands {
        for identity in [
            demand.slot_identity.as_bytes(),
            demand.layout_subject_identity.as_bytes(),
            demand.callback_requirement_identity.as_bytes(),
        ] {
            hash.u64(identity.len() as u64);
            hash.bytes(identity);
        }
        let mut placements = demand.placements.iter().collect::<Vec<_>>();
        placements.sort_unstable_by_key(|placement| layout_placement_sort_key(placement));
        hash.u64(placements.len() as u64);
        for placement in placements {
            let (tag, first, second, third, fourth, fifth) = layout_placement_sort_key(placement);
            hash.u64(u64::from(tag));
            for value in [first, second, third, fourth, fifth] {
                hash.u64(value);
            }
        }
    }
    hash.finish().max(1)
}

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
                layout_placement_sort_key(&left.placement)
                    .cmp(&layout_placement_sort_key(&right.placement))
            })
    });
    let mut retained_entries = retained.entries.iter().collect::<Vec<_>>();
    retained_entries.sort_unstable_by(|left, right| {
        member_sort_key(left)
            .cmp(&member_sort_key(right))
            .then_with(|| {
                layout_placement_sort_key(&left.placement)
                    .cmp(&layout_placement_sort_key(&right.placement))
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

pub fn layout_placement_sort_key(
    placement: &LayoutPlacementReport,
) -> (u8, u64, u64, u64, u64, u64) {
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
