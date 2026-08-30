#![forbid(unsafe_code)]

//! Normalized programmable-layout plans and symbolic materialization.
//!
//! Layout policies describe geometry. A materializer consumes validated
//! geometry plus compiler-issued symbolic values; source programs never
//! receive numeric code addresses or an arbitrary byte-patching primitive.

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

/// Exact compiler-owned conventional layout for one closed pure sum.
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

/// One exact fixed-depth chain from an outer record through one middle record
/// to one leaf record containing the complete direct conventional-sum set.
///
/// The middle-to-leaf portion reuses the existing singular one-level report;
/// no child placement is flattened into either parent layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalDepthTwoRecordSumPathLayoutReport {
    pub outer_layout: LayoutPlanReport,
    pub outer_field: String,
    pub outer_member_identity: Option<u64>,
    pub middle_path: ConventionalNestedRecordSumPathLayoutReport,
}

/// One exact direct outer-field occurrence and the complete authored-order
/// middle-to-leaf record paths reachable through that occurrence.
///
/// The nested report retains its middle whole-record layout once and one leaf
/// layout plus complete direct-sum row set per middle occurrence. No child row
/// is flattened into the outer record or duplicated across sibling paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalDepthTwoRecordSumOccurrenceLayoutReport {
    pub outer_field: String,
    pub outer_member_identity: Option<u64>,
    pub middle_paths: ConventionalNestedRecordSumPathsLayoutReport,
}

/// Compact complete authored-order set of qualifying depth-two record chains.
///
/// The outer layout is retained once. Each occurrence owns the unchanged
/// plural one-level report for its exact middle record, preserving both path
/// boundaries instead of flattening their layouts or child sum rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalDepthTwoRecordSumPathsLayoutReport {
    pub outer_layout: LayoutPlanReport,
    pub paths: Vec<ConventionalDepthTwoRecordSumOccurrenceLayoutReport>,
}

/// One exact fixed-depth chain through three enclosing records to one leaf
/// record containing the complete direct conventional-sum set.
///
/// The inner portion reuses the existing singular depth-two report whole. No
/// child placement or selected value is flattened into the new outer record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalDepthThreeRecordSumPathLayoutReport {
    pub outer_layout: LayoutPlanReport,
    pub outer_field: String,
    pub outer_member_identity: Option<u64>,
    pub depth_two_path: ConventionalDepthTwoRecordSumPathLayoutReport,
}

/// One normalized semantic-field-free callback destination in a native
/// layout. Declaration identities are exact canonical strings rather than
/// authored ordinals or arena handles. The authoritative layout policy owns
/// `offset`, but callback-address size/alignment close later with the selected
/// target calling plan and are deliberately absent here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateCallbackLayoutDemandReport {
    pub slot_identity: String,
    pub layout_subject_identity: String,
    pub callback_requirement_identity: String,
    pub offset: u64,
}

/// One validated native layout and its compiler-private demand catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeLayoutPlanReport {
    pub layout: LayoutPlanReport,
    pub private_callback_demands: Vec<PrivateCallbackLayoutDemandReport>,
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
    {
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
/// placement participates only in native-layout identity.
pub fn normalized_native_layout_plan_report_fingerprint(layout: &NativeLayoutPlanReport) -> u64 {
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

fn hash_fingerprint_u64(hash: &mut u64, value: u64) {
    hash_fingerprint_bytes(hash, &value.to_le_bytes());
}

fn hash_fingerprint_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        hash_fingerprint_byte(hash, *byte);
    }
}

fn hash_fingerprint_byte(hash: &mut u64, byte: u8) {
    *hash ^= u64::from(byte);
    *hash = hash.wrapping_mul(0x100000001b3);
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum MaterializationFieldKey {
    Numbered(u64),
    Positional(String),
}

fn materialization_field_key(field: &str, member_identity: Option<u64>) -> MaterializationFieldKey {
    match member_identity {
        Some(identity) => MaterializationFieldKey::Numbered(identity),
        None => MaterializationFieldKey::Positional(field.to_owned()),
    }
}

fn validate_materialization_field_identities(
    layout: &LayoutPlanReport,
) -> Result<(), MaterializationDiagnostic> {
    let mut identity_names = std::collections::BTreeMap::new();
    let mut name_identities = std::collections::BTreeMap::new();
    for entry in &layout.entries {
        let key = materialization_field_key(&entry.field, entry.member_identity);
        if let Some(prior_name) = identity_names.insert(key.clone(), entry.field.as_str())
            && prior_name != entry.field
        {
            return Err(MaterializationDiagnostic(format!(
                "layout field identity names both `{prior_name}` and `{}`",
                entry.field
            )));
        }
        if let Some(prior_identity) = name_identities.insert(entry.field.as_str(), key.clone())
            && prior_identity != key
        {
            return Err(MaterializationDiagnostic(format!(
                "layout field `{}` fragments do not retain the same stable identity",
                entry.field
            )));
        }
    }
    Ok(())
}

const fn stable_identity_suffix(member_identity: Option<u64>) -> &'static str {
    if member_identity.is_some() {
        " with the same stable identity"
    } else {
        ""
    }
}

/// One ordinary scalar supplied to a validated dictated-layout materializer.
/// Positional fields select compiler-validated plan entries by name; numbered
/// fields use their stable member identity. Callers never provide a byte
/// offset or destination bit position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarFieldValue {
    pub field: String,
    member_identity: Option<u64>,
    pub width_bits: u16,
    pub value: u64,
}

impl ScalarFieldValue {
    pub fn new(
        field: impl Into<String>,
        width_bits: u16,
        value: u64,
    ) -> Result<Self, MaterializationDiagnostic> {
        if width_bits == 0 || width_bits > 64 {
            return Err(MaterializationDiagnostic(format!(
                "scalar field width {width_bits} is outside 1..=64 bits"
            )));
        }
        if width_bits < 64 && value > low_mask(width_bits) {
            return Err(MaterializationDiagnostic(format!(
                "scalar value {value:#x} does not fit its {width_bits}-bit field"
            )));
        }
        Ok(Self {
            field: field.into(),
            member_identity: None,
            width_bits,
            value,
        })
    }

    /// Constructs a scalar value carrying its compiler-retained stable member
    /// identity. The field spelling remains diagnostic presentation.
    pub fn new_numbered(
        field: impl Into<String>,
        member_identity: u64,
        width_bits: u16,
        value: u64,
    ) -> Result<Self, MaterializationDiagnostic> {
        let mut value = Self::new(field, width_bits, value)?;
        value.member_identity = Some(member_identity);
        Ok(value)
    }
}

/// One complete aggregate supplied to a validated dictated-layout
/// materializer. The compiler derives `bytes` from an owned typed value; source
/// programs do not gain an arbitrary byte-patching operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregateFieldValue {
    pub field: String,
    pub bytes: Vec<u8>,
}

/// Compiler-derived physical extent of one aggregate schema field. This is
/// kept separate from the value so caller-provided bytes cannot claim their
/// own completeness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregateFieldSchema {
    pub field: String,
    /// Stable schema identity when this field belongs to a numbered record.
    /// The validated layout and compiler-derived schema rejoin through this
    /// identity, so a source rename cannot change placement authority.
    member_identity: Option<u64>,
    pub byte_size: u64,
    shape: AggregateFieldShape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AggregateFieldShape {
    Whole,
    Repeated {
        element_byte_size: u64,
        element_align: u64,
        element_count: u64,
    },
}

impl AggregateFieldSchema {
    pub fn new(
        field: impl Into<String>,
        byte_size: u64,
    ) -> Result<Self, MaterializationDiagnostic> {
        if byte_size == 0 {
            return Err(MaterializationDiagnostic(
                "aggregate field schema requires a nonzero physical extent".into(),
            ));
        }
        Ok(Self {
            field: field.into(),
            member_identity: None,
            byte_size,
            shape: AggregateFieldShape::Whole,
        })
    }

    /// Constructs one whole aggregate field with its compiler-retained stable
    /// member identity. The field spelling remains diagnostic presentation.
    pub fn new_numbered(
        field: impl Into<String>,
        member_identity: u64,
        byte_size: u64,
    ) -> Result<Self, MaterializationDiagnostic> {
        let mut schema = Self::new(field, byte_size)?;
        schema.member_identity = Some(member_identity);
        Ok(schema)
    }

    /// Constructs the compiler-derived shape of one outer fixed array. A
    /// validated layout may retain one whole-field `At`, or use exactly one
    /// `At` per element at a constant destination stride. The policy never
    /// supplies the element extent, alignment, or count.
    pub fn new_repeated(
        field: impl Into<String>,
        element_byte_size: u64,
        element_align: u64,
        element_count: u64,
    ) -> Result<Self, MaterializationDiagnostic> {
        if element_byte_size == 0 || element_count == 0 {
            return Err(MaterializationDiagnostic(
                "repeated aggregate schema requires nonzero element extent and count".into(),
            ));
        }
        if element_align == 0 || !element_align.is_power_of_two() {
            return Err(MaterializationDiagnostic(format!(
                "repeated aggregate element alignment {element_align} is not a positive power of two"
            )));
        }
        let byte_size = element_byte_size
            .checked_mul(element_count)
            .ok_or_else(|| {
                MaterializationDiagnostic(
                    "repeated aggregate compiler-derived physical extent overflows".into(),
                )
            })?;
        Ok(Self {
            field: field.into(),
            member_identity: None,
            byte_size,
            shape: AggregateFieldShape::Repeated {
                element_byte_size,
                element_align,
                element_count,
            },
        })
    }

    /// Constructs one numbered outer fixed array. Element geometry remains
    /// compiler-derived while the stable identity rejoins renamed layout rows.
    pub fn new_repeated_numbered(
        field: impl Into<String>,
        member_identity: u64,
        element_byte_size: u64,
        element_align: u64,
        element_count: u64,
    ) -> Result<Self, MaterializationDiagnostic> {
        let mut schema =
            Self::new_repeated(field, element_byte_size, element_align, element_count)?;
        schema.member_identity = Some(member_identity);
        Ok(schema)
    }
}

impl AggregateFieldValue {
    pub fn new(
        field: impl Into<String>,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<Self, MaterializationDiagnostic> {
        let bytes = bytes.into();
        if bytes.is_empty() {
            return Err(MaterializationDiagnostic(
                "aggregate field materialization requires a nonempty complete value".into(),
            ));
        }
        Ok(Self {
            field: field.into(),
            bytes,
        })
    }
}

/// Declared scalar shape used when decoding bytes through a validated layout.
/// The width comes from the compiler-materialized schema, not from the bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarFieldSchema {
    pub field: String,
    member_identity: Option<u64>,
    pub width_bits: u16,
}

impl ScalarFieldSchema {
    pub fn new(
        field: impl Into<String>,
        width_bits: u16,
    ) -> Result<Self, MaterializationDiagnostic> {
        if width_bits == 0 || width_bits > 64 {
            return Err(MaterializationDiagnostic(format!(
                "scalar field width {width_bits} is outside 1..=64 bits"
            )));
        }
        Ok(Self {
            field: field.into(),
            member_identity: None,
            width_bits,
        })
    }

    /// Constructs a decode schema carrying its compiler-retained stable member
    /// identity. Decoded values use the current schema spelling.
    pub fn new_numbered(
        field: impl Into<String>,
        member_identity: u64,
        width_bits: u16,
    ) -> Result<Self, MaterializationDiagnostic> {
        let mut schema = Self::new(field, width_bits)?;
        schema.member_identity = Some(member_identity);
        Ok(schema)
    }
}

/// Compiler-issued identity of an inbound entry stub. The numeric identity is
/// never a callable address and cannot be used for arithmetic or control flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntryStubId(u64);

impl EntryStubId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, MaterializationDiagnostic> {
        nonzero_identity("entry stub", identity).map(Self)
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

/// Compiler-issued identity of statically placed data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DataSymbolId(u64);

impl DataSymbolId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, MaterializationDiagnostic> {
        nonzero_identity("data symbol", identity).map(Self)
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

/// Closed source vocabulary for a toolchain-resolved value. Runtime-created
/// addresses remain ordinary `addr` data and do not enter this plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RelocationTarget {
    Data(DataSymbolId),
    Entry(EntryStubId),
}

impl RelocationTarget {
    pub const fn normalized_identity(self) -> u64 {
        match self {
            Self::Data(identity) => identity.normalized_identity(),
            Self::Entry(identity) => identity.normalized_identity(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolicFieldValue {
    pub field: String,
    member_identity: Option<u64>,
    pub width_bits: u16,
    pub target: RelocationTarget,
}

impl SymbolicFieldValue {
    pub fn new(
        field: impl Into<String>,
        width_bits: u16,
        target: RelocationTarget,
    ) -> Result<Self, MaterializationDiagnostic> {
        if width_bits == 0 || width_bits > 64 {
            return Err(MaterializationDiagnostic(format!(
                "symbolic field width {width_bits} is outside 1..=64 bits"
            )));
        }
        Ok(Self {
            field: field.into(),
            member_identity: None,
            width_bits,
            target,
        })
    }

    /// Constructs a symbolic value carrying its compiler-retained stable
    /// member identity. The field spelling remains diagnostic presentation.
    pub fn new_numbered(
        field: impl Into<String>,
        member_identity: u64,
        width_bits: u16,
        target: RelocationTarget,
    ) -> Result<Self, MaterializationDiagnostic> {
        let mut value = Self::new(field, width_bits, target)?;
        value.member_identity = Some(member_identity);
        Ok(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteOrder {
    LittleEndian,
    BigEndian,
}

/// When another party first consumes the materialized structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsumptionInstant {
    /// A loader reads the structure before the first Omega instruction. Only
    /// fixed values or relocations native to that loader are legal.
    BeforeOmegaEntry,
    /// Omega/provider code runs after the final address is known and may apply
    /// a generated writer before handing the structure to hardware/firmware.
    AfterOmegaHandoff,
}

/// Phase in which a materialized object is placed at its final address.
/// Consumption and placement are independent: a build-placed object may be
/// consumed only after handoff, while a loader-placed table may be consumed
/// before the first Omega instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlacementPhase {
    Build,
    Load,
    PostHandoff,
}

/// Closed permitted range for the complete placed object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlacementAddressRange {
    start_inclusive: u64,
    end_exclusive: u64,
}

impl PlacementAddressRange {
    pub fn new(
        start_inclusive: u64,
        end_exclusive: u64,
    ) -> Result<Self, MaterializationDiagnostic> {
        if start_inclusive >= end_exclusive {
            return Err(MaterializationDiagnostic(format!(
                "placement address range {start_inclusive:#x}..{end_exclusive:#x} is empty or reversed"
            )));
        }
        Ok(Self {
            start_inclusive,
            end_exclusive,
        })
    }

    pub const fn start_inclusive(self) -> u64 {
        self.start_inclusive
    }

    pub const fn end_exclusive(self) -> u64 {
        self.end_exclusive
    }

    fn contains(self, base_address: u64, byte_len: usize) -> bool {
        let Ok(byte_len) = u64::try_from(byte_len) else {
            return false;
        };
        base_address >= self.start_inclusive
            && base_address
                .checked_add(byte_len)
                .is_some_and(|end| end <= self.end_exclusive)
    }
}

/// Compiler-issued identity of a machine-state regime (for example, x86
/// long mode). It is a normalized policy identity, not a user-selected name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MachineRegimeId(u64);

impl MachineRegimeId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, MaterializationDiagnostic> {
        nonzero_identity("machine regime", identity).map(Self)
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

/// Compiler-issued identity of the attenuated artifact-installation authority
/// required by a placement. This cites scope; it is not the capability value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArtifactInstallationScopeId(u64);

impl ArtifactInstallationScopeId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, MaterializationDiagnostic> {
        nonzero_identity("artifact installation scope", identity).map(Self)
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

/// Normalized requirements a concrete placement must satisfy. The layout's
/// own alignment is joined into this record during materialization derivation;
/// policy constraints can strengthen it but never weaken it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlacementConstraints {
    permitted_range: Option<PlacementAddressRange>,
    alignment: u64,
    phase: PlacementPhase,
    machine_regime: Option<MachineRegimeId>,
    installation_scope: Option<ArtifactInstallationScopeId>,
}

impl PlacementConstraints {
    pub fn new(
        permitted_range: Option<PlacementAddressRange>,
        alignment: u64,
        phase: PlacementPhase,
        machine_regime: Option<MachineRegimeId>,
        installation_scope: Option<ArtifactInstallationScopeId>,
    ) -> Result<Self, MaterializationDiagnostic> {
        if alignment == 0 {
            return Err(MaterializationDiagnostic(
                "placement alignment must be nonzero".into(),
            ));
        }
        Ok(Self {
            permitted_range,
            alignment,
            phase,
            machine_regime,
            installation_scope,
        })
    }

    pub const fn unconstrained(phase: PlacementPhase) -> Self {
        Self {
            permitted_range: None,
            alignment: 1,
            phase,
            machine_regime: None,
            installation_scope: None,
        }
    }

    pub const fn permitted_range(self) -> Option<PlacementAddressRange> {
        self.permitted_range
    }

    pub const fn alignment(self) -> u64 {
        self.alignment
    }

    pub const fn phase(self) -> PlacementPhase {
        self.phase
    }

    pub const fn machine_regime(self) -> Option<MachineRegimeId> {
        self.machine_regime
    }

    pub const fn installation_scope(self) -> Option<ArtifactInstallationScopeId> {
        self.installation_scope
    }

    fn joined_with_layout(
        mut self,
        layout_alignment: u64,
        byte_len: usize,
    ) -> Result<Self, MaterializationDiagnostic> {
        if layout_alignment == 0 {
            return Err(MaterializationDiagnostic(
                "layout alignment must be nonzero".into(),
            ));
        }
        self.alignment = checked_lcm(self.alignment, layout_alignment).ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "placement alignment {} and layout alignment {layout_alignment} have no representable common multiple",
                self.alignment
            ))
        })?;
        if let Some(range) = self.permitted_range {
            let range_len = range.end_exclusive - range.start_inclusive;
            let byte_len = u64::try_from(byte_len).map_err(|_| {
                MaterializationDiagnostic(
                    "materialization length cannot be represented as an address range".into(),
                )
            })?;
            if byte_len > range_len {
                return Err(MaterializationDiagnostic(format!(
                    "{}-byte materialization cannot fit in permitted range {:#x}..{:#x}",
                    byte_len, range.start_inclusive, range.end_exclusive
                )));
            }
        }
        Ok(self)
    }

    pub fn validate_site(
        self,
        byte_len: usize,
        site: PlacementSite,
    ) -> Result<(), MaterializationDiagnostic> {
        if site.phase != self.phase {
            return Err(MaterializationDiagnostic(format!(
                "placement phase {:?} does not satisfy required phase {:?}",
                site.phase, self.phase
            )));
        }
        if !site.base_address.is_multiple_of(self.alignment) {
            return Err(MaterializationDiagnostic(format!(
                "placement address {:#x} is not aligned to {} bytes",
                site.base_address, self.alignment
            )));
        }
        if let Some(range) = self.permitted_range
            && !range.contains(site.base_address, byte_len)
        {
            return Err(MaterializationDiagnostic(format!(
                "{}-byte placement at {:#x} lies outside permitted range {:#x}..{:#x}",
                byte_len, site.base_address, range.start_inclusive, range.end_exclusive
            )));
        }
        if self.machine_regime.is_some() && site.machine_regime != self.machine_regime {
            return Err(MaterializationDiagnostic(format!(
                "placement machine regime {:?} does not satisfy required regime {:?}",
                site.machine_regime, self.machine_regime
            )));
        }
        if self.installation_scope.is_some() && site.installation_scope != self.installation_scope {
            return Err(MaterializationDiagnostic(format!(
                "placement installation scope {:?} does not satisfy required scope {:?}",
                site.installation_scope, self.installation_scope
            )));
        }
        Ok(())
    }
}

/// Concrete facts known when a linker, loader, or provider chooses a final
/// address. Validation compares these facts to the normalized constraints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlacementSite {
    pub base_address: u64,
    pub phase: PlacementPhase,
    pub machine_regime: Option<MachineRegimeId>,
    pub installation_scope: Option<ArtifactInstallationScopeId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaterializationContext {
    pub consumption: ConsumptionInstant,
    pub byte_order: ByteOrder,
    /// Width accepted by the target container's native absolute relocation.
    /// `None` means no such relocation is available.
    pub native_pointer_relocation_bits: Option<u16>,
    pub placement: PlacementConstraints,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializationWrite {
    pub field: String,
    pub target: RelocationTarget,
    pub container_byte_offset: u64,
    pub container_width_bits: u16,
    pub destination_lsb: u16,
    pub source_lsb: u16,
    pub width: u16,
    pub stored_integer_fit: Option<StoredIntegerFit>,
}

/// Value-domain constraint retained when a symbolic source lands in a
/// narrower stored-integer encoding. Post-handoff resolution must discharge
/// this constraint before any destination byte changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoredIntegerFit {
    pub source_width_bits: u16,
    pub stored_width_bits: u16,
    pub interpretation: IntegerInterpretation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaterializationAction {
    /// Constant-folded when a fixed image or earlier placement pass already
    /// knows the target address.
    ResolvedWrite {
        write: MaterializationWrite,
        source_value: u64,
    },
    /// A loader-native whole-pointer relocation. Fragmented native
    /// relocations are deliberately absent from the vocabulary.
    NativePointerRelocation {
        field: String,
        target: RelocationTarget,
        destination_byte_offset: u64,
        width_bits: u16,
    },
    /// A deriver-generated post-handoff writer step. Providers resolve the
    /// target without exposing its numeric address to ordinary Omega code.
    RuntimeWriter(MaterializationWrite),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostHandoffWriterSource {
    Resolved(u64),
    Resolve(RelocationTarget),
}

/// `PHWRITR1`: target-neutral packed input ABI for a reusable post-handoff
/// fragment. Word zero is the destination address. The remaining words are
/// dense source slots assigned by the fragment plan. Numeric words stay inside
/// provider-owned invocation evidence; source code sees neither this context
/// nor an address-valued writer operation.
pub const POST_HANDOFF_WRITER_CONTEXT_ABI_V1: u64 = 0x5048_5752_4954_5231;
pub const POST_HANDOFF_WRITER_DESTINATION_OFFSET: usize = 0;
pub const POST_HANDOFF_WRITER_SOURCE_SLOTS_OFFSET: usize = 8;
pub const POST_HANDOFF_WRITER_SOURCE_SLOT_WIDTH: usize = 8;

pub fn post_handoff_writer_context_byte_len(source_slot_count: usize) -> Option<usize> {
    source_slot_count
        .checked_mul(POST_HANDOFF_WRITER_SOURCE_SLOT_WIDTH)?
        .checked_add(POST_HANDOFF_WRITER_SOURCE_SLOTS_OFFSET)
}

/// One address-free fragment in a reusable generated writer. `source_slot`
/// indexes the provider-private invocation context. Field names, symbolic
/// identities, resolved values, and concrete placement are deliberately
/// absent: none changes the emitted transfer geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneratedPostHandoffWriterStep {
    pub container_byte_offset: u64,
    pub container_width_bits: u16,
    pub destination_lsb: u16,
    pub source_lsb: u16,
    pub width: u16,
    pub source_slot: usize,
}

/// Static normalized plan for one reusable post-handoff fragment.
///
/// Its report fingerprint covers only facts that can change emitted code. Exact
/// relocation targets, resolved content, placement, resolver authority, and
/// roots belong to `PostHandoffWriterInvocationPlan` instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedPostHandoffWriterFragmentPlan {
    context_abi: u64,
    byte_len: usize,
    byte_order: ByteOrder,
    source_slot_count: usize,
    steps: Vec<GeneratedPostHandoffWriterStep>,
    report_fingerprint: u64,
}

impl GeneratedPostHandoffWriterFragmentPlan {
    pub const fn context_abi(&self) -> u64 {
        self.context_abi
    }

    pub const fn byte_len(&self) -> usize {
        self.byte_len
    }

    pub const fn byte_order(&self) -> ByteOrder {
        self.byte_order
    }

    pub const fn source_slot_count(&self) -> usize {
        self.source_slot_count
    }

    pub fn steps(&self) -> &[GeneratedPostHandoffWriterStep] {
        &self.steps
    }

    pub const fn report_fingerprint(&self) -> u64 {
        self.report_fingerprint
    }
}

/// Exact value bound to one private source slot for one invocation. The target
/// remains present even for a pre-resolved value so fragmented writes group by
/// compiler-issued source identity rather than accidentally by equal numeric
/// content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostHandoffWriterSourceSlot {
    pub target: RelocationTarget,
    pub source: PostHandoffWriterSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostHandoffWriterFitConstraint {
    pub source_slot: usize,
    pub field: String,
    pub fit: StoredIntegerFit,
}

/// Invocation-sensitive half of generated writer lowering. This evidence is
/// intentionally separate from the reusable fragment identity. Only validated
/// writer lowering constructs it; consumers may inspect but cannot substitute
/// targets, placement, or fit evidence.
///
/// ```compile_fail
/// use psi_layout_plans::PostHandoffWriterInvocationPlan;
///
/// fn discard_fit_evidence(invocation: &mut PostHandoffWriterInvocationPlan) {
///     invocation.fit_constraints.clear();
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostHandoffWriterInvocationPlan {
    fragment: GeneratedPostHandoffWriterFragmentPlan,
    placement: PlacementConstraints,
    sources: Vec<PostHandoffWriterSourceSlot>,
    fit_constraints: Vec<PostHandoffWriterFitConstraint>,
}

impl PostHandoffWriterInvocationPlan {
    pub const fn fragment(&self) -> &GeneratedPostHandoffWriterFragmentPlan {
        &self.fragment
    }

    pub const fn placement(&self) -> PlacementConstraints {
        self.placement
    }

    pub const fn source_slot_count(&self) -> usize {
        self.sources.len()
    }

    pub fn sources(&self) -> &[PostHandoffWriterSourceSlot] {
        &self.sources
    }

    pub fn fit_constraints(&self) -> &[PostHandoffWriterFitConstraint] {
        &self.fit_constraints
    }

    /// Independently replay the sealed invocation before a consumer accepts
    /// provider-supplied words. Rejection only borrows this carrier, so the
    /// exact invocation remains available for inspection or corrected retry.
    pub fn validate_structure(&self) -> Result<(), MaterializationDiagnostic> {
        let fragment = &self.fragment;
        if fragment.context_abi != POST_HANDOFF_WRITER_CONTEXT_ABI_V1 {
            return Err(MaterializationDiagnostic(
                "post-handoff writer invocation uses an unsupported context ABI".into(),
            ));
        }
        if fragment.byte_len == 0 || fragment.steps.is_empty() || self.sources.is_empty() {
            return Err(MaterializationDiagnostic(
                "post-handoff writer invocation requires nonempty bytes, sources, and fragments"
                    .into(),
            ));
        }
        if self.placement.alignment == 0 {
            return Err(MaterializationDiagnostic(
                "post-handoff writer invocation placement alignment must be nonzero".into(),
            ));
        }
        if fragment.source_slot_count != self.sources.len()
            || post_handoff_writer_context_byte_len(self.sources.len()).is_none()
        {
            return Err(MaterializationDiagnostic(
                "post-handoff writer invocation source-slot geometry is inconsistent".into(),
            ));
        }

        let mut distinct_targets = std::collections::BTreeSet::new();
        for slot in &self.sources {
            if !distinct_targets.insert(slot.target) {
                return Err(MaterializationDiagnostic(
                    "post-handoff writer invocation repeats one relocation target in multiple source slots"
                        .into(),
                ));
            }
            if let PostHandoffWriterSource::Resolve(target) = slot.source
                && target != slot.target
            {
                return Err(MaterializationDiagnostic(
                    "post-handoff writer invocation resolver source does not match its source-slot target"
                        .into(),
                ));
            }
        }

        let mut used_slots = vec![false; self.sources.len()];
        let mut next_first_slot = 0;
        for step in &fragment.steps {
            validate_fragment(
                fragment.byte_len,
                "generated post-handoff writer fragment",
                step.container_byte_offset,
                step.container_width_bits,
                step.destination_lsb,
                step.source_lsb,
                step.width,
            )?;
            let used = used_slots.get_mut(step.source_slot).ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "post-handoff writer fragment names missing source slot {}",
                    step.source_slot
                ))
            })?;
            if !*used {
                if step.source_slot != next_first_slot {
                    return Err(MaterializationDiagnostic(
                        "post-handoff writer fragment source slots are not in canonical first-occurrence order"
                            .into(),
                    ));
                }
                *used = true;
                next_first_slot += 1;
            }
        }
        if used_slots.iter().any(|used| !used) {
            return Err(MaterializationDiagnostic(
                "post-handoff writer invocation retains an unused source slot".into(),
            ));
        }

        for constraint in &self.fit_constraints {
            validate_stored_integer_fit_shape(
                &constraint.field,
                constraint.fit,
                "post-handoff invocation",
            )?;
            if constraint.source_slot >= self.sources.len()
                || !fragment.steps.iter().any(|step| {
                    step.source_slot == constraint.source_slot
                        && step.container_width_bits == constraint.fit.stored_width_bits
                        && step.width == constraint.fit.stored_width_bits
                        && step.destination_lsb == 0
                        && step.source_lsb == 0
                })
            {
                return Err(MaterializationDiagnostic(format!(
                    "post-handoff stored-integer constraint for `{}` does not bind one exact generated fragment",
                    constraint.field
                )));
            }
        }

        let expected_report_fingerprint = generated_post_handoff_writer_report_fingerprint(
            fragment.byte_len,
            fragment.byte_order,
            fragment.source_slot_count,
            &fragment.steps,
        );
        if fragment.report_fingerprint != expected_report_fingerprint {
            return Err(MaterializationDiagnostic(
                "post-handoff writer fragment fingerprint does not match its exact geometry".into(),
            ));
        }
        Ok(())
    }

    pub fn validate_source_values(
        &self,
        source_values: &[u64],
    ) -> Result<(), MaterializationDiagnostic> {
        self.validate_structure()?;
        if source_values.len() != self.sources.len() {
            return Err(MaterializationDiagnostic(format!(
                "post-handoff writer has {} source values for {} source slots",
                source_values.len(),
                self.sources.len()
            )));
        }
        for (slot_index, (slot, supplied)) in self.sources.iter().zip(source_values).enumerate() {
            if let PostHandoffWriterSource::Resolved(expected) = slot.source
                && *supplied != expected
            {
                return Err(MaterializationDiagnostic(format!(
                    "post-handoff writer source slot {slot_index} for {:?} supplied {supplied:#x}, but its invocation evidence retains {expected:#x}",
                    slot.target
                )));
            }
        }
        for constraint in &self.fit_constraints {
            let value = source_values.get(constraint.source_slot).ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "post-handoff stored-integer constraint for `{}` names missing source slot {}",
                    constraint.field, constraint.source_slot
                ))
            })?;
            validate_stored_integer_fit(
                &constraint.field,
                constraint.fit,
                *value,
                "resolved symbolic",
            )?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostHandoffWriterStep {
    pub write: MaterializationWrite,
    pub source: PostHandoffWriterSource,
}

/// Provider-consumable writer program derived from symbolic materialization
/// actions. It contains no source-callable address operation: only the
/// provider resolver may turn a sealed relocation target into address bits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostHandoffWriterPlan {
    pub byte_len: usize,
    pub byte_order: ByteOrder,
    pub placement: PlacementConstraints,
    pub steps: Vec<PostHandoffWriterStep>,
}

impl PostHandoffWriterPlan {
    /// Derive one reusable address-free fragment plus exact invocation evidence.
    /// Source slots follow first target occurrence and are dense. Repeated
    /// fragments of one symbolic target therefore consume one once-resolved
    /// word even when the concrete address or artifact realization changes.
    pub fn lower_reusable_fragment(
        &self,
    ) -> Result<PostHandoffWriterInvocationPlan, MaterializationDiagnostic> {
        validate_post_handoff_writer_nonempty(&self.steps)?;

        let mut sources = Vec::<PostHandoffWriterSourceSlot>::new();
        let mut fit_constraints = Vec::new();
        let mut steps = Vec::with_capacity(self.steps.len());
        for step in &self.steps {
            validate_post_handoff_writer_step(self.byte_len, step)?;

            let source_slot = if let Some(index) = sources
                .iter()
                .position(|source| source.target == step.write.target)
            {
                if sources[index].source != step.source {
                    return Err(MaterializationDiagnostic(format!(
                        "post-handoff writer target {:?} has inconsistent invocation values across fragments",
                        step.write.target
                    )));
                }
                index
            } else {
                let index = sources.len();
                sources.push(PostHandoffWriterSourceSlot {
                    target: step.write.target,
                    source: step.source,
                });
                index
            };
            if let Some(fit) = step.write.stored_integer_fit {
                fit_constraints.push(PostHandoffWriterFitConstraint {
                    source_slot,
                    field: step.write.field.clone(),
                    fit,
                });
            }
            steps.push(GeneratedPostHandoffWriterStep {
                container_byte_offset: step.write.container_byte_offset,
                container_width_bits: step.write.container_width_bits,
                destination_lsb: step.write.destination_lsb,
                source_lsb: step.write.source_lsb,
                width: step.write.width,
                source_slot,
            });
        }

        let fragment = GeneratedPostHandoffWriterFragmentPlan {
            context_abi: POST_HANDOFF_WRITER_CONTEXT_ABI_V1,
            byte_len: self.byte_len,
            byte_order: self.byte_order,
            source_slot_count: sources.len(),
            report_fingerprint: generated_post_handoff_writer_report_fingerprint(
                self.byte_len,
                self.byte_order,
                sources.len(),
                &steps,
            ),
            steps,
        };
        Ok(PostHandoffWriterInvocationPlan {
            fragment,
            placement: self.placement,
            sources,
            fit_constraints,
        })
    }

    /// Validate the complete direct-destination program without resolving a
    /// symbolic address or mutating the destination. Provider preparation
    /// uses this pass to bind one exact checked writer before any numeric
    /// entry address exists outside the sealed resolver.
    pub fn validate(
        &self,
        destination_len: usize,
        site: PlacementSite,
    ) -> Result<(), MaterializationDiagnostic> {
        validate_post_handoff_writer_nonempty(&self.steps)?;
        self.placement.validate_site(self.byte_len, site)?;
        if destination_len < self.byte_len {
            return Err(MaterializationDiagnostic(format!(
                "post-handoff writer needs {} bytes, destination has {}",
                self.byte_len, destination_len
            )));
        }
        let mut sources = Vec::<PostHandoffWriterSourceSlot>::new();
        for step in &self.steps {
            validate_post_handoff_writer_step(self.byte_len, step)?;
            if let Some(source) = sources
                .iter()
                .find(|source| source.target == step.write.target)
            {
                if source.source != step.source {
                    return Err(MaterializationDiagnostic(format!(
                        "post-handoff writer target {:?} has inconsistent invocation values across fragments",
                        step.write.target
                    )));
                }
            } else {
                sources.push(PostHandoffWriterSourceSlot {
                    target: step.write.target,
                    source: step.source,
                });
            }
        }
        Ok(())
    }

    /// Validates the concrete placement and every write, resolves every target,
    /// then commits one staged image into the unpublished destination. Repeated
    /// fragments of one target resolve once so a provider cannot observe
    /// inconsistent address values within one materialization. Every rejection
    /// leaves the destination bytes unchanged; successful application commits
    /// the complete writer range once. Publication remains a later transition.
    pub fn execute(
        &self,
        destination: &mut [u8],
        site: PlacementSite,
        mut resolve: impl FnMut(RelocationTarget) -> Option<u64>,
    ) -> Result<(), MaterializationDiagnostic> {
        self.validate(destination.len(), site)?;

        let mut resolved_targets = std::collections::BTreeMap::new();
        let mut values = Vec::with_capacity(self.steps.len());
        for step in &self.steps {
            let target = step.write.target;
            let value = if let Some(value) = resolved_targets.get(&target) {
                *value
            } else {
                let value = match step.source {
                    PostHandoffWriterSource::Resolved(value) => value,
                    PostHandoffWriterSource::Resolve(target) => {
                        resolve(target).ok_or_else(|| {
                            MaterializationDiagnostic(format!(
                                "post-handoff writer could not resolve symbolic target {target:?}"
                            ))
                        })?
                    }
                };
                for candidate in self
                    .steps
                    .iter()
                    .filter(|candidate| candidate.write.target == target)
                {
                    validate_write_source_value(&candidate.write, value, "resolved symbolic")?;
                }
                resolved_targets.insert(target, value);
                value
            };
            values.push(value);
        }

        apply_post_handoff_writes_atomically(
            &mut destination[..self.byte_len],
            self.byte_order,
            &self.steps,
            &values,
        )
    }
}

fn apply_post_handoff_writes_atomically(
    destination: &mut [u8],
    byte_order: ByteOrder,
    steps: &[PostHandoffWriterStep],
    values: &[u64],
) -> Result<(), MaterializationDiagnostic> {
    if steps.len() != values.len() {
        return Err(MaterializationDiagnostic(
            "post-handoff writer application requires one resolved value per fragment".into(),
        ));
    }
    let mut staged = destination.to_vec();
    for (step, value) in steps.iter().zip(values) {
        apply_write(&mut staged, byte_order, &step.write, *value)?;
    }
    destination.copy_from_slice(&staged);
    Ok(())
}

fn validate_post_handoff_writer_nonempty(
    steps: &[PostHandoffWriterStep],
) -> Result<(), MaterializationDiagnostic> {
    if steps.is_empty() {
        return Err(MaterializationDiagnostic(
            "post-handoff writer requires at least one fragment".into(),
        ));
    }
    Ok(())
}

fn validate_post_handoff_writer_step(
    byte_len: usize,
    step: &PostHandoffWriterStep,
) -> Result<(), MaterializationDiagnostic> {
    if let PostHandoffWriterSource::Resolve(target) = step.source
        && target != step.write.target
    {
        return Err(MaterializationDiagnostic(format!(
            "post-handoff writer source {target:?} does not match write target {:?}",
            step.write.target
        )));
    }
    validate_write(byte_len, &step.write)?;
    if let PostHandoffWriterSource::Resolved(source_value) = step.source {
        validate_write_source_value(&step.write, source_value, "pre-resolved symbolic")?;
    }
    Ok(())
}

fn generated_post_handoff_writer_report_fingerprint(
    byte_len: usize,
    byte_order: ByteOrder,
    source_slot_count: usize,
    steps: &[GeneratedPostHandoffWriterStep],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_fingerprint_bytes(&mut hash, b"omega.post-handoff-writer.v1");
    hash_fingerprint_u64(&mut hash, POST_HANDOFF_WRITER_CONTEXT_ABI_V1);
    hash_fingerprint_u64(&mut hash, byte_len as u64);
    hash_fingerprint_byte(
        &mut hash,
        match byte_order {
            ByteOrder::LittleEndian => 0,
            ByteOrder::BigEndian => 1,
        },
    );
    hash_fingerprint_u64(&mut hash, source_slot_count as u64);
    hash_fingerprint_u64(&mut hash, steps.len() as u64);
    for step in steps {
        for value in [
            step.container_byte_offset,
            u64::from(step.container_width_bits),
            u64::from(step.destination_lsb),
            u64::from(step.source_lsb),
            u64::from(step.width),
            step.source_slot as u64,
        ] {
            hash_fingerprint_u64(&mut hash, value);
        }
    }
    if hash == 0 { 1 } else { hash }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolicMaterializationPlan {
    pub byte_len: usize,
    pub byte_order: ByteOrder,
    pub placement: PlacementConstraints,
    pub actions: Vec<MaterializationAction>,
}

impl SymbolicMaterializationPlan {
    pub fn derive_post_handoff_writer(
        &self,
    ) -> Result<PostHandoffWriterPlan, MaterializationDiagnostic> {
        if self.actions.is_empty() {
            return Err(MaterializationDiagnostic(
                "post-handoff writer requires at least one fragment".into(),
            ));
        }
        let mut steps = Vec::with_capacity(self.actions.len());
        for action in &self.actions {
            let step = match action {
                MaterializationAction::ResolvedWrite {
                    write,
                    source_value,
                } => PostHandoffWriterStep {
                    write: write.clone(),
                    source: PostHandoffWriterSource::Resolved(*source_value),
                },
                MaterializationAction::RuntimeWriter(write) => PostHandoffWriterStep {
                    write: write.clone(),
                    source: PostHandoffWriterSource::Resolve(write.target),
                },
                MaterializationAction::NativePointerRelocation { .. } => {
                    return Err(MaterializationDiagnostic(
                        "loader-native relocation cannot enter a post-handoff writer program"
                            .into(),
                    ));
                }
            };
            steps.push(step);
        }
        Ok(PostHandoffWriterPlan {
            byte_len: self.byte_len,
            byte_order: self.byte_order,
            placement: self.placement,
            steps,
        })
    }

    /// Applies a fully resolved plan atomically with respect to the destination
    /// slice: unresolved actions reject before any output byte changes.
    pub fn materialize_resolved_into(
        &self,
        destination: &mut [u8],
    ) -> Result<(), MaterializationDiagnostic> {
        if destination.len() < self.byte_len {
            return Err(MaterializationDiagnostic(format!(
                "materialization needs {} bytes, destination has {}",
                self.byte_len,
                destination.len()
            )));
        }
        if let Some(action) = self
            .actions
            .iter()
            .find(|action| !matches!(action, MaterializationAction::ResolvedWrite { .. }))
        {
            return Err(MaterializationDiagnostic(format!(
                "materialization still contains an unresolved action: {action:?}"
            )));
        }

        for action in &self.actions {
            let MaterializationAction::ResolvedWrite {
                write,
                source_value,
            } = action
            else {
                unreachable!("unresolved actions were rejected above")
            };
            validate_write(self.byte_len, write)?;
            validate_write_source_value(write, *source_value, "resolved symbolic")?;
        }

        let mut staged = destination[..self.byte_len].to_vec();
        for action in &self.actions {
            let MaterializationAction::ResolvedWrite {
                write,
                source_value,
            } = action
            else {
                unreachable!("unresolved actions were rejected above")
            };
            apply_write(&mut staged, self.byte_order, write, *source_value)?;
        }
        destination[..self.byte_len].copy_from_slice(&staged);
        Ok(())
    }
}

/// Materializes one complete ordinary scalar value through validated layout
/// entries. Output starts from zero so padding and reserved bits stay
/// deterministic. The destination is changed only after every field and
/// fragment has validated.
///
/// This is the numeric sibling of symbolic materialization. It cannot resolve
/// code/data symbols, install hardware state, or mint authority; it only turns
/// named scalar values into bytes according to an already validated plan.
pub fn materialize_scalar_layout_into(
    layout: &LayoutPlanReport,
    values: &[ScalarFieldValue],
    byte_order: ByteOrder,
    destination: &mut [u8],
) -> Result<(), MaterializationDiagnostic> {
    let byte_len = layout
        .size
        .ok_or_else(|| {
            MaterializationDiagnostic(
                "scalar materialization requires a fixed-size layout plan".into(),
            )
        })
        .and_then(|size| {
            usize::try_from(size).map_err(|_| {
                MaterializationDiagnostic(format!(
                    "fixed layout size {size} cannot be represented on this compiler host"
                ))
            })
        })?;
    if destination.len() < byte_len {
        return Err(MaterializationDiagnostic(format!(
            "scalar materialization needs {byte_len} bytes, destination has {}",
            destination.len()
        )));
    }
    validate_materialization_field_identities(layout)?;

    let mut supplied = std::collections::BTreeMap::new();
    let mut supplied_names = std::collections::BTreeSet::new();
    for value in values {
        if !supplied_names.insert(value.field.as_str()) {
            return Err(MaterializationDiagnostic(format!(
                "scalar field `{}` is supplied more than once",
                value.field
            )));
        }
        let key = materialization_field_key(&value.field, value.member_identity);
        if supplied.insert(key, value).is_some() {
            return Err(MaterializationDiagnostic(format!(
                "scalar field `{}` repeats stable member identity #{}",
                value.field,
                value
                    .member_identity
                    .expect("only numbered values can collide after name validation")
            )));
        }
    }

    let planned = layout
        .entries
        .iter()
        .map(|entry| materialization_field_key(&entry.field, entry.member_identity))
        .collect::<std::collections::BTreeSet<_>>();
    if let Some(entry) = layout.entries.iter().find(|entry| {
        !supplied.contains_key(&materialization_field_key(
            &entry.field,
            entry.member_identity,
        ))
    }) {
        let suffix = stable_identity_suffix(entry.member_identity);
        return Err(MaterializationDiagnostic(format!(
            "layout field `{}` has no supplied scalar value{suffix}",
            entry.field
        )));
    }
    if let Some(value) = supplied
        .iter()
        .find_map(|(key, value)| (!planned.contains(key)).then_some(value))
    {
        let suffix = stable_identity_suffix(value.member_identity);
        return Err(MaterializationDiagnostic(format!(
            "supplied scalar field `{}` has no entry in the validated layout plan{suffix}",
            value.field
        )));
    }

    let mut staged = vec![0_u8; byte_len];
    for entry in &layout.entries {
        let key = materialization_field_key(&entry.field, entry.member_identity);
        let value = supplied
            .get(&key)
            .expect("complete field set validated above");
        apply_scalar_entry(&mut staged, byte_order, entry, value)?;
    }
    destination[..byte_len].copy_from_slice(&staged);
    Ok(())
}

/// Materializes complete aggregate fields through whole-extent `At`
/// placements, or one fixed outer array through compiler-sized element `At`
/// placements at a constant destination stride. All validation and copying
/// happens against a staged zeroed buffer, so rejection leaves `destination`
/// unchanged. Aggregate fields are deliberately not interpreted as scalar
/// fragments or stored integers.
pub fn materialize_aggregate_layout_into(
    layout: &LayoutPlanReport,
    fields: &[AggregateFieldSchema],
    values: &[AggregateFieldValue],
    destination: &mut [u8],
) -> Result<(), MaterializationDiagnostic> {
    let byte_len = layout
        .size
        .ok_or_else(|| {
            MaterializationDiagnostic(
                "aggregate materialization requires a fixed-size layout plan".into(),
            )
        })
        .and_then(|size| {
            usize::try_from(size).map_err(|_| {
                MaterializationDiagnostic(format!(
                    "fixed layout size {size} cannot be represented on this compiler host"
                ))
            })
        })?;
    if destination.len() < byte_len {
        return Err(MaterializationDiagnostic(format!(
            "aggregate materialization needs {byte_len} bytes, destination has {}",
            destination.len()
        )));
    }
    validate_materialization_field_identities(layout)?;

    let mut schemas = std::collections::BTreeMap::new();
    let mut schema_names = std::collections::BTreeSet::new();
    for field in fields {
        if !schema_names.insert(field.field.as_str()) {
            return Err(MaterializationDiagnostic(format!(
                "aggregate field `{}` is declared more than once",
                field.field
            )));
        }
        let key = materialization_field_key(&field.field, field.member_identity);
        if schemas.insert(key, field).is_some() {
            return Err(MaterializationDiagnostic(format!(
                "aggregate field `{}` repeats stable member identity #{}",
                field.field,
                field
                    .member_identity
                    .expect("only numbered fields can collide after name validation")
            )));
        }
    }
    let mut supplied = std::collections::BTreeMap::new();
    for value in values {
        if supplied.insert(value.field.as_str(), value).is_some() {
            return Err(MaterializationDiagnostic(format!(
                "aggregate field `{}` is supplied more than once",
                value.field
            )));
        }
    }
    let mut planned =
        std::collections::BTreeMap::<MaterializationFieldKey, Vec<&LayoutFieldEntryReport>>::new();
    for entry in &layout.entries {
        let key = materialization_field_key(&entry.field, entry.member_identity);
        planned.entry(key).or_default().push(entry);
    }
    if let Some(entries) = planned
        .iter()
        .find_map(|(key, entries)| (!schemas.contains_key(key)).then_some(entries))
    {
        let entry = entries
            .first()
            .expect("planned aggregate key always retains an entry");
        let suffix = stable_identity_suffix(entry.member_identity);
        return Err(MaterializationDiagnostic(format!(
            "layout field `{}` has no aggregate schema extent{suffix}",
            entry.field
        )));
    }
    if let Some(field) = schemas
        .iter()
        .find_map(|(key, field)| (!planned.contains_key(key)).then_some(field))
    {
        let suffix = stable_identity_suffix(field.member_identity);
        return Err(MaterializationDiagnostic(format!(
            "aggregate schema field `{}` has no entry in the validated layout plan{suffix}",
            field.field
        )));
    }
    if let Some(field) = fields
        .iter()
        .find(|field| !supplied.contains_key(field.field.as_str()))
    {
        return Err(MaterializationDiagnostic(format!(
            "layout field `{}` has no supplied aggregate value",
            field.field
        )));
    }
    if let Some(field) = supplied
        .keys()
        .find(|field| !schema_names.contains(**field))
    {
        return Err(MaterializationDiagnostic(format!(
            "supplied aggregate field `{field}` has no compiler-derived aggregate schema"
        )));
    }

    let mut staged = vec![0_u8; byte_len];
    let mut occupied = vec![false; byte_len];
    for (field_key, schema) in schemas {
        let field_name = schema.field.as_str();
        let entries = planned
            .get_mut(&field_key)
            .expect("complete aggregate plan set validated above");
        if entries
            .iter()
            .any(|entry| !matches!(entry.placement, LayoutPlacementReport::At { .. }))
        {
            let requirement = match schema.shape {
                AggregateFieldShape::Whole => "one whole `At` placement",
                AggregateFieldShape::Repeated { .. } => {
                    "whole-value or fixed-element `At` placement"
                }
            };
            return Err(MaterializationDiagnostic(format!(
                "aggregate field `{field_name}` requires {requirement}"
            )));
        }
        let value = supplied
            .get(field_name)
            .expect("complete aggregate field set validated above");
        let expected_size = usize::try_from(schema.byte_size).map_err(|_| {
            MaterializationDiagnostic(format!(
                "aggregate field `{}` extent cannot be represented on this compiler host",
                field_name
            ))
        })?;
        if value.bytes.len() != expected_size {
            return Err(MaterializationDiagnostic(format!(
                "aggregate field `{}` supplies {} bytes, but its compiler-derived extent is {expected_size}",
                field_name,
                value.bytes.len()
            )));
        }

        entries.sort_unstable_by_key(|entry| match entry.placement {
            LayoutPlacementReport::At { offset } => offset,
            _ => unreachable!("non-At aggregate entries rejected above"),
        });
        let (source_chunk_size, required_align) = if entries.len() == 1 {
            (expected_size, None)
        } else {
            let AggregateFieldShape::Repeated {
                element_byte_size,
                element_align,
                element_count,
            } = schema.shape
            else {
                return Err(MaterializationDiagnostic(format!(
                    "aggregate field `{field_name}` has more than one `At` placement but is not an outer fixed array"
                )));
            };
            let actual_count = u64::try_from(entries.len()).map_err(|_| {
                MaterializationDiagnostic(format!(
                    "aggregate field `{field_name}` placement count cannot be represented as u64"
                ))
            })?;
            if actual_count != element_count {
                return Err(MaterializationDiagnostic(format!(
                    "repeated aggregate field `{field_name}` has {actual_count} element placements, expected {element_count}"
                )));
            }
            let element_byte_size = usize::try_from(element_byte_size).map_err(|_| {
                MaterializationDiagnostic(format!(
                    "aggregate field `{field_name}` element extent cannot be represented on this compiler host"
                ))
            })?;
            let offsets = entries
                .iter()
                .map(|entry| match entry.placement {
                    LayoutPlacementReport::At { offset } => offset,
                    _ => unreachable!("non-At aggregate entries rejected above"),
                })
                .collect::<Vec<_>>();
            let stride = offsets[1].checked_sub(offsets[0]).ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "repeated aggregate field `{field_name}` element offsets are not ordered"
                ))
            })?;
            if stride < element_byte_size as u64
                || offsets.windows(2).any(|pair| pair[1] - pair[0] != stride)
            {
                return Err(MaterializationDiagnostic(format!(
                    "repeated aggregate field `{field_name}` element placements do not have one nonoverlapping constant stride"
                )));
            }
            (element_byte_size, Some(element_align))
        };

        let chunks = value.bytes.chunks_exact(source_chunk_size);
        if !chunks.remainder().is_empty() || chunks.len() != entries.len() {
            return Err(MaterializationDiagnostic(format!(
                "aggregate field `{field_name}` bytes do not tile its compiler-derived source elements exactly"
            )));
        }
        for (entry, source) in entries.iter().zip(chunks) {
            let LayoutPlacementReport::At { offset } = entry.placement else {
                unreachable!("non-At aggregate entries rejected above")
            };
            if required_align.is_some_and(|align| !offset.is_multiple_of(align)) {
                return Err(MaterializationDiagnostic(format!(
                    "repeated aggregate field `{field_name}` element offset {offset} violates its compiler-derived alignment {}",
                    required_align.expect("checked as present")
                )));
            }
            let start = usize::try_from(offset).map_err(|_| {
                MaterializationDiagnostic(format!(
                    "aggregate field `{field_name}` offset cannot be represented on this compiler host"
                ))
            })?;
            let end = start.checked_add(source.len()).ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "aggregate field `{field_name}` destination range overflows"
                ))
            })?;
            if end > byte_len {
                return Err(MaterializationDiagnostic(format!(
                    "aggregate field `{field_name}` writes through byte {end}, past the {byte_len}-byte layout"
                )));
            }
            if occupied[start..end].iter().any(|occupied| *occupied) {
                return Err(MaterializationDiagnostic(format!(
                    "aggregate field `{field_name}` overlaps an earlier aggregate placement"
                )));
            }
            staged[start..end].copy_from_slice(source);
            occupied[start..end].fill(true);
        }
    }
    destination[..byte_len].copy_from_slice(&staged);
    Ok(())
}

/// Decodes one complete fixed scalar layout without establishing any semantic
/// domain or authority fact. Callers receive ordinary named values; a separate
/// validator decides whether those values establish an imported-table claim.
pub fn decode_scalar_layout(
    layout: &LayoutPlanReport,
    fields: &[ScalarFieldSchema],
    byte_order: ByteOrder,
    source: &[u8],
) -> Result<Vec<ScalarFieldValue>, MaterializationDiagnostic> {
    let byte_len = layout
        .size
        .ok_or_else(|| {
            MaterializationDiagnostic("scalar decoding requires a fixed-size layout plan".into())
        })
        .and_then(|size| {
            usize::try_from(size).map_err(|_| {
                MaterializationDiagnostic(format!(
                    "fixed layout size {size} cannot be represented on this compiler host"
                ))
            })
        })?;
    if source.len() < byte_len {
        return Err(MaterializationDiagnostic(format!(
            "scalar decoding needs {byte_len} bytes, source has {}",
            source.len()
        )));
    }
    validate_materialization_field_identities(layout)?;

    let mut decoded = std::collections::BTreeMap::new();
    let mut schema_names = std::collections::BTreeSet::new();
    for field in fields {
        if !schema_names.insert(field.field.as_str()) {
            return Err(MaterializationDiagnostic(format!(
                "scalar field `{}` is declared more than once",
                field.field
            )));
        }
        let key = materialization_field_key(&field.field, field.member_identity);
        if decoded.insert(key, (field, 0_u64, 0_u64)).is_some() {
            return Err(MaterializationDiagnostic(format!(
                "scalar field `{}` repeats stable member identity #{}",
                field.field,
                field
                    .member_identity
                    .expect("only numbered schemas can collide after name validation")
            )));
        }
    }
    let planned = layout
        .entries
        .iter()
        .map(|entry| materialization_field_key(&entry.field, entry.member_identity))
        .collect::<std::collections::BTreeSet<_>>();
    if let Some(entry) = layout.entries.iter().find(|entry| {
        !decoded.contains_key(&materialization_field_key(
            &entry.field,
            entry.member_identity,
        ))
    }) {
        let suffix = stable_identity_suffix(entry.member_identity);
        return Err(MaterializationDiagnostic(format!(
            "layout field `{}` has no scalar decode schema{suffix}",
            entry.field
        )));
    }
    if let Some(field) = decoded
        .iter()
        .find_map(|(key, (field, _, _))| (!planned.contains(key)).then_some(field))
    {
        let suffix = stable_identity_suffix(field.member_identity);
        return Err(MaterializationDiagnostic(format!(
            "scalar decode field `{}` has no entry in the validated layout plan{suffix}",
            field.field
        )));
    }

    for entry in &layout.entries {
        let key = materialization_field_key(&entry.field, entry.member_identity);
        let (field, value, covered) = decoded
            .get_mut(&key)
            .expect("complete field set validated above");
        let width_bits = field.width_bits;
        let fragment = scalar_fragment(entry, width_bits)?;
        validate_fragment(
            byte_len,
            &entry.field,
            fragment.container_byte_offset,
            fragment.container_width_bits,
            fragment.destination_lsb,
            fragment.source_lsb,
            fragment.width,
        )?;
        if let LayoutPlacementReport::IntegerAt {
            stored_width,
            interpretation,
            ..
        } = entry.placement
        {
            let stored_width = u16::try_from(stored_width).map_err(|_| {
                MaterializationDiagnostic(format!(
                    "scalar field `{}` has an invalid stored-integer width",
                    entry.field
                ))
            })?;
            if stored_width == 0
                || stored_width > 64
                || !stored_width.is_multiple_of(8)
                || width_bits < stored_width
            {
                return Err(MaterializationDiagnostic(format!(
                    "scalar field `{}` cannot decode {stored_width}-bit stored-integer storage into its {}-bit semantic carrier",
                    entry.field, width_bits
                )));
            }
            if *covered != 0 {
                return Err(MaterializationDiagnostic(format!(
                    "scalar field `{}` has more than one stored-integer decode entry",
                    entry.field
                )));
            }
            let container_bytes = usize::from(fragment.container_width_bits / 8);
            let start = usize::try_from(fragment.container_byte_offset).map_err(|_| {
                MaterializationDiagnostic(
                    "stored-integer offset cannot be represented on this host".into(),
                )
            })?;
            let end = start.checked_add(container_bytes).ok_or_else(|| {
                MaterializationDiagnostic("stored-integer byte range overflows".into())
            })?;
            let stored = read_container(&source[start..end], byte_order) & low_mask(stored_width);
            *value = match interpretation {
                IntegerInterpretation::Unsigned => stored,
                IntegerInterpretation::Signed if stored & (1_u64 << (stored_width - 1)) != 0 => {
                    stored | (low_mask(width_bits) & !low_mask(stored_width))
                }
                IntegerInterpretation::Signed => stored,
            };
            *covered = low_mask(width_bits);
            continue;
        }
        let source_mask = low_mask(fragment.width) << fragment.source_lsb;
        if *covered & source_mask != 0 {
            return Err(MaterializationDiagnostic(format!(
                "scalar field `{}` decode fragments overlap in the logical source",
                entry.field
            )));
        }
        let container_bytes = usize::from(fragment.container_width_bits / 8);
        let start = usize::try_from(fragment.container_byte_offset).map_err(|_| {
            MaterializationDiagnostic("container offset cannot be represented on this host".into())
        })?;
        let end = start
            .checked_add(container_bytes)
            .ok_or_else(|| MaterializationDiagnostic("container byte range overflows".into()))?;
        let container = read_container(&source[start..end], byte_order);
        let value_fragment = (container >> fragment.destination_lsb) & low_mask(fragment.width);
        *value |= value_fragment << fragment.source_lsb;
        *covered |= source_mask;
    }

    decoded
        .into_iter()
        .map(|(_, (field, value, covered))| {
            let width_bits = field.width_bits;
            if covered != low_mask(width_bits) {
                return Err(MaterializationDiagnostic(format!(
                    "scalar field `{}` decode fragments do not tile its complete {width_bits}-bit source",
                    field.field
                )));
            }
            Ok(ScalarFieldValue {
                field: field.field.clone(),
                member_identity: field.member_identity,
                width_bits,
                value,
            })
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializationDiagnostic(pub String);

impl std::fmt::Display for MaterializationDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for MaterializationDiagnostic {}

#[cfg(test)]
mod tests;

/// Derives a phase-aware consumer plan. `resolve` is compiler/provider
/// infrastructure; source code never receives its returned address.
pub fn derive_symbolic_materialization(
    layout: &LayoutPlanReport,
    symbolic_fields: &[SymbolicFieldValue],
    context: MaterializationContext,
    mut resolve: impl FnMut(RelocationTarget) -> Option<u64>,
) -> Result<SymbolicMaterializationPlan, MaterializationDiagnostic> {
    let byte_len = layout
        .size
        .ok_or_else(|| {
            MaterializationDiagnostic(
                "symbolic materialization requires a fixed-size layout plan".into(),
            )
        })
        .and_then(|size| {
            usize::try_from(size).map_err(|_| {
                MaterializationDiagnostic(format!(
                    "fixed layout size {size} cannot be represented on this compiler host"
                ))
            })
        })?;
    let placement = context
        .placement
        .joined_with_layout(layout.align, byte_len)?;
    validate_materialization_field_identities(layout)?;

    let mut supplied = std::collections::BTreeSet::new();
    let mut names = std::collections::BTreeSet::new();
    for symbolic in symbolic_fields {
        if !names.insert(symbolic.field.as_str()) {
            return Err(MaterializationDiagnostic(format!(
                "symbolic field `{}` is supplied more than once",
                symbolic.field
            )));
        }
        let key = materialization_field_key(&symbolic.field, symbolic.member_identity);
        if !supplied.insert(key) {
            return Err(MaterializationDiagnostic(format!(
                "symbolic field `{}` repeats stable member identity #{}",
                symbolic.field,
                symbolic
                    .member_identity
                    .expect("only numbered symbolic values can collide after name validation")
            )));
        }
    }
    let mut planned =
        std::collections::BTreeMap::<MaterializationFieldKey, Vec<&LayoutFieldEntryReport>>::new();
    for entry in &layout.entries {
        planned
            .entry(materialization_field_key(
                &entry.field,
                entry.member_identity,
            ))
            .or_default()
            .push(entry);
    }
    for symbolic in symbolic_fields {
        let key = materialization_field_key(&symbolic.field, symbolic.member_identity);
        let Some(entries) = planned.get(&key) else {
            let suffix = stable_identity_suffix(symbolic.member_identity);
            return Err(MaterializationDiagnostic(format!(
                "symbolic field `{}` has no entry in the validated layout plan{suffix}",
                symbolic.field
            )));
        };
        let entry_names = entries
            .iter()
            .map(|entry| entry.field.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        if let Some(drifted) = layout.entries.iter().find(|entry| {
            entry_names.contains(entry.field.as_str())
                && materialization_field_key(&entry.field, entry.member_identity) != key
        }) {
            return Err(MaterializationDiagnostic(format!(
                "layout field `{}` fragments do not retain one stable member identity",
                drifted.field
            )));
        }
    }

    let prepared_writes = symbolic_fields
        .iter()
        .map(|symbolic| {
            let key = materialization_field_key(&symbolic.field, symbolic.member_identity);
            planned
                .get(&key)
                .expect("symbolic layout membership validated above")
                .iter()
                .map(|entry| {
                    let write = write_from_entry(entry, symbolic)?;
                    validate_write(byte_len, &write)?;
                    Ok((entry.placement, write))
                })
                .collect::<Result<Vec<_>, MaterializationDiagnostic>>()
        })
        .collect::<Result<Vec<_>, MaterializationDiagnostic>>()?;

    let mut resolved_targets = std::collections::BTreeMap::new();
    let mut actions = Vec::new();
    for (symbolic_index, symbolic) in symbolic_fields.iter().enumerate() {
        let writes = &prepared_writes[symbolic_index];
        let resolved = if let Some(resolved) = resolved_targets.get(&symbolic.target) {
            *resolved
        } else {
            let resolved = resolve(symbolic.target);
            if let Some(source_value) = resolved {
                for (_, candidate_writes) in symbolic_fields
                    .iter()
                    .zip(&prepared_writes)
                    .filter(|(candidate, _)| candidate.target == symbolic.target)
                {
                    for (_, write) in candidate_writes {
                        validate_write_source_value(write, source_value, "symbolic")?;
                    }
                }
            }
            resolved_targets.insert(symbolic.target, resolved);
            resolved
        };
        for (placement, write) in writes.iter().cloned() {
            let action = match resolved {
                Some(source_value) => MaterializationAction::ResolvedWrite {
                    write,
                    source_value,
                },
                None if context.consumption == ConsumptionInstant::AfterOmegaHandoff => {
                    MaterializationAction::RuntimeWriter(write)
                }
                None => match placement {
                    LayoutPlacementReport::At { .. }
                        if context.native_pointer_relocation_bits == Some(symbolic.width_bits) =>
                    {
                        MaterializationAction::NativePointerRelocation {
                            field: symbolic.field.clone(),
                            target: symbolic.target,
                            destination_byte_offset: write.container_byte_offset,
                            width_bits: symbolic.width_bits,
                        }
                    }
                    LayoutPlacementReport::At { .. } => {
                        return Err(MaterializationDiagnostic(format!(
                            "loader consumes symbolic field `{}` before Omega entry, but the target has no native {}-bit pointer relocation",
                            symbolic.field, symbolic.width_bits
                        )));
                    }
                    LayoutPlacementReport::IntegerAt { .. } => {
                        return Err(MaterializationDiagnostic(format!(
                            "loader consumes stored-integer field `{}` before Omega entry; symbolic materialization has no integer fit proof",
                            symbolic.field
                        )));
                    }
                    LayoutPlacementReport::Bits { .. } => {
                        return Err(MaterializationDiagnostic(format!(
                            "loader consumes fragmented symbolic field `{}` before Omega entry; unresolved fragments require a fixed address or a post-handoff writer",
                            symbolic.field
                        )));
                    }
                },
            };
            actions.push(action);
        }
    }

    Ok(SymbolicMaterializationPlan {
        byte_len,
        byte_order: context.byte_order,
        placement,
        actions,
    })
}

fn write_from_entry(
    entry: &LayoutFieldEntryReport,
    symbolic: &SymbolicFieldValue,
) -> Result<MaterializationWrite, MaterializationDiagnostic> {
    let (container, container_width, destination_lsb, source_lsb, width) = match entry.placement {
        LayoutPlacementReport::At { offset } => (
            offset,
            u64::from(symbolic.width_bits),
            0,
            0,
            u64::from(symbolic.width_bits),
        ),
        LayoutPlacementReport::IntegerAt {
            offset,
            stored_width,
            ..
        } => (offset, stored_width, 0, 0, stored_width),
        LayoutPlacementReport::Bits {
            container,
            container_width,
            destination_lsb,
            source_lsb,
            width,
        } => (
            container,
            container_width,
            destination_lsb,
            source_lsb,
            width,
        ),
    };
    if container_width == 0 || container_width > 64 || container_width % 8 != 0 || width == 0 {
        return Err(MaterializationDiagnostic(format!(
            "symbolic field `{}` uses a materializer-incompatible placement",
            symbolic.field
        )));
    }
    let source_end = source_lsb
        .checked_add(width)
        .ok_or_else(|| MaterializationDiagnostic("symbolic source bit range overflows".into()))?;
    if source_end > u64::from(symbolic.width_bits) {
        return Err(MaterializationDiagnostic(format!(
            "symbolic field `{}` placement reads through bit {source_end}, past its {}-bit source",
            symbolic.field, symbolic.width_bits
        )));
    }
    let destination_end = destination_lsb.checked_add(width).ok_or_else(|| {
        MaterializationDiagnostic("symbolic destination bit range overflows".into())
    })?;
    if destination_end > container_width {
        return Err(MaterializationDiagnostic(format!(
            "symbolic field `{}` placement writes through bit {destination_end}, past its {container_width}-bit container",
            symbolic.field
        )));
    }
    Ok(MaterializationWrite {
        field: symbolic.field.clone(),
        target: symbolic.target,
        container_byte_offset: container,
        container_width_bits: u16::try_from(container_width)
            .expect("validated materializer container width"),
        destination_lsb: u16::try_from(destination_lsb).expect("validated destination bit index"),
        source_lsb: u16::try_from(source_lsb).expect("validated source bit index"),
        width: u16::try_from(width).map_err(|_| {
            MaterializationDiagnostic(format!(
                "symbolic field `{}` fragment width {width} is too large",
                symbolic.field
            ))
        })?,
        stored_integer_fit: match entry.placement {
            LayoutPlacementReport::IntegerAt {
                stored_width,
                interpretation,
                ..
            } => Some(StoredIntegerFit {
                source_width_bits: symbolic.width_bits,
                stored_width_bits: u16::try_from(stored_width).map_err(|_| {
                    MaterializationDiagnostic(format!(
                        "symbolic field `{}` has an invalid stored-integer width",
                        symbolic.field
                    ))
                })?,
                interpretation,
            }),
            LayoutPlacementReport::At { .. } | LayoutPlacementReport::Bits { .. } => None,
        },
    })
}

fn apply_scalar_entry(
    bytes: &mut [u8],
    byte_order: ByteOrder,
    entry: &LayoutFieldEntryReport,
    value: &ScalarFieldValue,
) -> Result<(), MaterializationDiagnostic> {
    validate_stored_integer_value(entry, value)?;
    let fragment = scalar_fragment(entry, value.width_bits)?;
    validate_fragment(
        bytes.len(),
        &value.field,
        fragment.container_byte_offset,
        fragment.container_width_bits,
        fragment.destination_lsb,
        fragment.source_lsb,
        fragment.width,
    )?;
    apply_fragment(
        bytes,
        byte_order,
        &value.field,
        fragment.container_byte_offset,
        fragment.container_width_bits,
        fragment.destination_lsb,
        fragment.source_lsb,
        fragment.width,
        value.value,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScalarFragment {
    container_byte_offset: u64,
    container_width_bits: u16,
    destination_lsb: u16,
    source_lsb: u16,
    width: u16,
}

fn scalar_fragment(
    entry: &LayoutFieldEntryReport,
    source_width_bits: u16,
) -> Result<ScalarFragment, MaterializationDiagnostic> {
    let (container, container_width, destination_lsb, source_lsb, width) = match entry.placement {
        LayoutPlacementReport::At { offset } => (
            offset,
            u64::from(source_width_bits),
            0,
            0,
            u64::from(source_width_bits),
        ),
        LayoutPlacementReport::IntegerAt {
            offset,
            stored_width,
            ..
        } => (offset, stored_width, 0, 0, stored_width),
        LayoutPlacementReport::Bits {
            container,
            container_width,
            destination_lsb,
            source_lsb,
            width,
        } => (
            container,
            container_width,
            destination_lsb,
            source_lsb,
            width,
        ),
    };
    if container_width == 0 || container_width > 64 || container_width % 8 != 0 || width == 0 {
        return Err(MaterializationDiagnostic(format!(
            "scalar field `{}` uses a materializer-incompatible placement",
            entry.field
        )));
    }
    let source_end = source_lsb
        .checked_add(width)
        .ok_or_else(|| MaterializationDiagnostic("scalar source bit range overflows".into()))?;
    if source_end > u64::from(source_width_bits) {
        return Err(MaterializationDiagnostic(format!(
            "scalar field `{}` placement reads through bit {source_end}, past its {}-bit source",
            entry.field, source_width_bits
        )));
    }
    let destination_end = destination_lsb.checked_add(width).ok_or_else(|| {
        MaterializationDiagnostic("scalar destination bit range overflows".into())
    })?;
    if destination_end > container_width {
        return Err(MaterializationDiagnostic(format!(
            "scalar field `{}` placement writes through bit {destination_end}, past its {container_width}-bit container",
            entry.field
        )));
    }

    Ok(ScalarFragment {
        container_byte_offset: container,
        container_width_bits: u16::try_from(container_width)
            .expect("validated materializer container width"),
        destination_lsb: u16::try_from(destination_lsb).expect("validated destination bit index"),
        source_lsb: u16::try_from(source_lsb).expect("validated source bit index"),
        width: u16::try_from(width).map_err(|_| {
            MaterializationDiagnostic(format!(
                "scalar field `{}` fragment width {width} is too large",
                entry.field
            ))
        })?,
    })
}

fn validate_stored_integer_value(
    entry: &LayoutFieldEntryReport,
    value: &ScalarFieldValue,
) -> Result<(), MaterializationDiagnostic> {
    validate_stored_integer_bits(entry, value.width_bits, value.value, "scalar")
}

fn validate_stored_integer_bits(
    entry: &LayoutFieldEntryReport,
    source_width_bits: u16,
    source_value: u64,
    value_kind: &str,
) -> Result<(), MaterializationDiagnostic> {
    let LayoutPlacementReport::IntegerAt {
        stored_width,
        interpretation,
        ..
    } = entry.placement
    else {
        return Ok(());
    };
    let stored_width = u16::try_from(stored_width).map_err(|_| {
        MaterializationDiagnostic(format!(
            "{value_kind} field `{}` has an invalid stored-integer width",
            entry.field,
        ))
    })?;
    if stored_width == 0 || stored_width > 64 || !stored_width.is_multiple_of(8) {
        return Err(MaterializationDiagnostic(format!(
            "{value_kind} field `{}` has an invalid {stored_width}-bit stored-integer width",
            entry.field
        )));
    }
    if source_width_bits < stored_width {
        return Err(MaterializationDiagnostic(format!(
            "{value_kind} field `{}` has a {source_width_bits}-bit value narrower than its {stored_width}-bit storage",
            entry.field,
        )));
    }

    let fits = match interpretation {
        IntegerInterpretation::Signed => {
            let semantic = signed_bits(source_value, source_width_bits);
            let magnitude = 1_i128 << (stored_width - 1);
            semantic >= -magnitude && semantic < magnitude
        }
        IntegerInterpretation::Unsigned => source_value <= low_mask(stored_width),
    };
    if !fits {
        return Err(MaterializationDiagnostic(format!(
            "{value_kind} field `{}` value {source_value:#x} does not fit its {stored_width}-bit {} storage",
            entry.field,
            match interpretation {
                IntegerInterpretation::Signed => "signed",
                IntegerInterpretation::Unsigned => "unsigned",
            }
        )));
    }
    Ok(())
}

fn validate_write_source_value(
    write: &MaterializationWrite,
    source_value: u64,
    value_kind: &str,
) -> Result<(), MaterializationDiagnostic> {
    let Some(fit) = write.stored_integer_fit else {
        return Ok(());
    };
    validate_stored_integer_fit(&write.field, fit, source_value, value_kind)
}

fn validate_stored_integer_fit(
    field: &str,
    fit: StoredIntegerFit,
    source_value: u64,
    value_kind: &str,
) -> Result<(), MaterializationDiagnostic> {
    validate_stored_integer_fit_shape(field, fit, value_kind)?;
    let StoredIntegerFit {
        source_width_bits,
        stored_width_bits,
        interpretation,
    } = fit;

    let fits = match interpretation {
        IntegerInterpretation::Signed => {
            let semantic = signed_bits(source_value, source_width_bits);
            let magnitude = 1_i128 << (stored_width_bits - 1);
            semantic >= -magnitude && semantic < magnitude
        }
        IntegerInterpretation::Unsigned => source_value <= low_mask(stored_width_bits),
    };
    if !fits {
        return Err(MaterializationDiagnostic(format!(
            "{value_kind} field `{field}` value {source_value:#x} does not fit its {stored_width_bits}-bit {} storage",
            match interpretation {
                IntegerInterpretation::Signed => "signed",
                IntegerInterpretation::Unsigned => "unsigned",
            }
        )));
    }
    Ok(())
}

fn validate_stored_integer_fit_shape(
    field: &str,
    fit: StoredIntegerFit,
    value_kind: &str,
) -> Result<(), MaterializationDiagnostic> {
    let StoredIntegerFit {
        source_width_bits,
        stored_width_bits,
        interpretation: _,
    } = fit;
    if source_width_bits == 0 || source_width_bits > 64 {
        return Err(MaterializationDiagnostic(format!(
            "{value_kind} field `{field}` has an invalid {source_width_bits}-bit source width"
        )));
    }
    if stored_width_bits == 0
        || stored_width_bits > 64
        || !stored_width_bits.is_multiple_of(8)
        || source_width_bits < stored_width_bits
    {
        return Err(MaterializationDiagnostic(format!(
            "{value_kind} field `{field}` has an invalid {stored_width_bits}-bit stored-integer fit constraint"
        )));
    }
    Ok(())
}

fn signed_bits(value: u64, width: u16) -> i128 {
    let value = value & low_mask(width);
    if value & (1_u64 << (width - 1)) == 0 {
        i128::from(value)
    } else {
        i128::from(value) - (1_i128 << width)
    }
}

fn apply_write(
    bytes: &mut [u8],
    byte_order: ByteOrder,
    write: &MaterializationWrite,
    source_value: u64,
) -> Result<(), MaterializationDiagnostic> {
    apply_fragment(
        bytes,
        byte_order,
        &write.field,
        write.container_byte_offset,
        write.container_width_bits,
        write.destination_lsb,
        write.source_lsb,
        write.width,
        source_value,
    )
}

#[allow(clippy::too_many_arguments)]
fn apply_fragment(
    bytes: &mut [u8],
    byte_order: ByteOrder,
    field: &str,
    container_byte_offset: u64,
    container_width_bits: u16,
    destination_lsb: u16,
    source_lsb: u16,
    width: u16,
    source_value: u64,
) -> Result<(), MaterializationDiagnostic> {
    let container_bytes = usize::from(container_width_bits / 8);
    let start = usize::try_from(container_byte_offset).map_err(|_| {
        MaterializationDiagnostic("container offset cannot be represented on this host".into())
    })?;
    let end = start
        .checked_add(container_bytes)
        .ok_or_else(|| MaterializationDiagnostic("container byte range overflows".into()))?;
    let materialization_len = bytes.len();
    let container_slice = bytes.get_mut(start..end).ok_or_else(|| {
        MaterializationDiagnostic(format!(
            "symbolic field `{}` writes outside the {}-byte materialization",
            field, materialization_len
        ))
    })?;
    let mut container_value = read_container(container_slice, byte_order);
    let fragment_mask = low_mask(width);
    let fragment = (source_value >> source_lsb) & fragment_mask;
    let destination_mask = fragment_mask << destination_lsb;
    container_value =
        (container_value & !destination_mask) | ((fragment << destination_lsb) & destination_mask);
    write_container(container_slice, byte_order, container_value);
    Ok(())
}

fn validate_write(
    materialization_len: usize,
    write: &MaterializationWrite,
) -> Result<(), MaterializationDiagnostic> {
    validate_fragment(
        materialization_len,
        &write.field,
        write.container_byte_offset,
        write.container_width_bits,
        write.destination_lsb,
        write.source_lsb,
        write.width,
    )?;
    if let Some(fit) = write.stored_integer_fit
        && (fit.stored_width_bits != write.container_width_bits
            || fit.stored_width_bits != write.width
            || write.destination_lsb != 0
            || write.source_lsb != 0)
    {
        return Err(MaterializationDiagnostic(format!(
            "symbolic field `{}` has stored-integer fit evidence inconsistent with its write geometry",
            write.field
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_fragment(
    materialization_len: usize,
    field: &str,
    container_byte_offset: u64,
    container_width_bits: u16,
    destination_lsb: u16,
    source_lsb: u16,
    width: u16,
) -> Result<(), MaterializationDiagnostic> {
    if container_width_bits == 0
        || container_width_bits > 64
        || !container_width_bits.is_multiple_of(8)
    {
        return Err(MaterializationDiagnostic(format!(
            "symbolic field `{}` has invalid {}-bit container",
            field, container_width_bits
        )));
    }
    if width == 0
        || width > 64
        || source_lsb.checked_add(width).is_none_or(|end| end > 64)
        || destination_lsb
            .checked_add(width)
            .is_none_or(|end| end > container_width_bits)
    {
        return Err(MaterializationDiagnostic(format!(
            "symbolic field `{}` has an invalid source or destination bit range",
            field
        )));
    }
    let start = usize::try_from(container_byte_offset).map_err(|_| {
        MaterializationDiagnostic("container offset cannot be represented on this host".into())
    })?;
    let end = start
        .checked_add(usize::from(container_width_bits / 8))
        .ok_or_else(|| MaterializationDiagnostic("container byte range overflows".into()))?;
    if end > materialization_len {
        return Err(MaterializationDiagnostic(format!(
            "symbolic field `{}` writes outside the {}-byte materialization",
            field, materialization_len
        )));
    }
    Ok(())
}

fn read_container(bytes: &[u8], byte_order: ByteOrder) -> u64 {
    bytes
        .iter()
        .enumerate()
        .fold(0_u64, |value, (index, byte)| {
            let shift = match byte_order {
                ByteOrder::LittleEndian => index * 8,
                ByteOrder::BigEndian => (bytes.len() - 1 - index) * 8,
            };
            value | (u64::from(*byte) << shift)
        })
}

fn write_container(bytes: &mut [u8], byte_order: ByteOrder, value: u64) {
    let byte_len = bytes.len();
    for (index, byte) in bytes.iter_mut().enumerate() {
        let shift = match byte_order {
            ByteOrder::LittleEndian => index * 8,
            ByteOrder::BigEndian => (byte_len - 1 - index) * 8,
        };
        *byte = ((value >> shift) & 0xff) as u8;
    }
}

const fn low_mask(width: u16) -> u64 {
    if width == 64 {
        u64::MAX
    } else {
        (1_u64 << width) - 1
    }
}

const fn checked_lcm(left: u64, right: u64) -> Option<u64> {
    let divisor = gcd(left, right);
    (left / divisor).checked_mul(right)
}

const fn gcd(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

fn nonzero_identity(kind: &str, identity: u64) -> Result<u64, MaterializationDiagnostic> {
    if identity == 0 {
        Err(MaterializationDiagnostic(format!(
            "normalized {kind} identity cannot be zero"
        )))
    } else {
        Ok(identity)
    }
}
