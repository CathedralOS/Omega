pub mod field_identities {
    use crate::layout_reports::{LayoutFieldEntryReport, LayoutPlanReport};
    use crate::materialization_records::MaterializationDiagnostic;

    use std::collections::{BTreeMap, BTreeSet};

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub enum MaterializationFieldKey {
        Numbered(u64),
        Positional(String),
    }

    pub fn materialization_field_key(
        field: &str,
        member_identity: Option<u64>,
    ) -> MaterializationFieldKey {
        match member_identity {
            Some(identity) => MaterializationFieldKey::Numbered(identity),
            None => MaterializationFieldKey::Positional(field.to_owned()),
        }
    }

    pub fn planned_entries_by_key(
        layout: &LayoutPlanReport,
    ) -> BTreeMap<MaterializationFieldKey, Vec<&LayoutFieldEntryReport>> {
        let mut planned: BTreeMap<MaterializationFieldKey, Vec<&LayoutFieldEntryReport>> =
            BTreeMap::new();
        for entry in &layout.entries {
            planned
                .entry(materialization_field_key(
                    &entry.field,
                    entry.member_identity,
                ))
                .or_default()
                .push(entry);
        }
        planned
    }

    struct SuppliedField<'fields, Field> {
        value: &'fields Field,
        name: &'fields str,
        member_identity: Option<u64>,
    }

    pub struct SuppliedFields<'fields, Field> {
        by_key: BTreeMap<MaterializationFieldKey, SuppliedField<'fields, Field>>,
        by_name: BTreeMap<&'fields str, &'fields Field>,
    }

    impl<'fields, Field> SuppliedFields<'fields, Field> {
        pub fn value(&self, key: &MaterializationFieldKey) -> Option<&'fields Field> {
            self.by_key.get(key).map(|supplied| supplied.value)
        }

        pub fn named(&self, name: &str) -> Option<&'fields Field> {
            self.by_name.get(name).copied()
        }

        pub fn fields_by_key(
            &self,
        ) -> impl Iterator<Item = (&MaterializationFieldKey, &'fields Field)> {
            self.by_key
                .iter()
                .map(|(key, supplied)| (key, supplied.value))
        }

        pub fn names(&self) -> impl Iterator<Item = &'fields str> {
            self.by_name.keys().copied()
        }
    }

    pub fn index_supplied_fields<'fields, Field>(
        fields: &'fields [Field],
        kind: &str,
        duplicate_verb: &str,
        supplied: impl Fn(&'fields Field) -> (&'fields str, Option<u64>),
    ) -> Result<SuppliedFields<'fields, Field>, MaterializationDiagnostic> {
        let mut indexed = SuppliedFields {
            by_key: BTreeMap::new(),
            by_name: BTreeMap::new(),
        };
        for field in fields {
            let (name, member_identity) = supplied(field);
            if indexed.by_name.insert(name, field).is_some() {
                return Err(MaterializationDiagnostic(format!(
                    "{kind} field `{name}` is {duplicate_verb} more than once"
                )));
            }
            let key = materialization_field_key(name, member_identity);
            let indexed_field = SuppliedField {
                value: field,
                name,
                member_identity,
            };
            if indexed.by_key.insert(key, indexed_field).is_some() {
                return Err(MaterializationDiagnostic(format!(
                    "{kind} field `{name}` repeats stable member identity #{}",
                    member_identity
                        .expect("only numbered fields can collide after name validation")
                )));
            }
        }
        Ok(indexed)
    }

    pub fn validate_supplied_field_coverage<Field>(
        layout: &LayoutPlanReport,
        supplied: &SuppliedFields<'_, Field>,
        missing_noun: &str,
        extra_noun: &str,
    ) -> Result<(), MaterializationDiagnostic> {
        if let Some(entry) = layout.entries.iter().find(|entry| {
            !supplied.by_key.contains_key(&materialization_field_key(
                &entry.field,
                entry.member_identity,
            ))
        }) {
            let suffix = stable_identity_suffix(entry.member_identity);
            return Err(MaterializationDiagnostic(format!(
                "layout field `{}` has no {missing_noun}{suffix}",
                entry.field
            )));
        }
        let planned = layout
            .entries
            .iter()
            .map(|entry| materialization_field_key(&entry.field, entry.member_identity))
            .collect::<BTreeSet<_>>();
        if let Some(field) = supplied
            .by_key
            .iter()
            .find_map(|(key, field)| (!planned.contains(key)).then_some(field))
        {
            let suffix = stable_identity_suffix(field.member_identity);
            return Err(MaterializationDiagnostic(format!(
                "{extra_noun} `{}` has no entry in the validated layout plan{suffix}",
                field.name
            )));
        }
        Ok(())
    }

    pub fn validate_materialization_field_identities(
        layout: &LayoutPlanReport,
    ) -> Result<(), MaterializationDiagnostic> {
        validate_member_identities(
            layout
                .entries
                .iter()
                .map(|entry| (entry.field.as_str(), entry.member_identity)),
            |prior, field| format!("layout field identity names both `{prior}` and `{field}`"),
            |field| {
                format!("layout field `{field}` fragments do not retain the same stable identity")
            },
        )
    }

    /// Each stable identity names one member, and each member name keeps one
    /// stable identity across all of its fragments.
    fn validate_member_identities<'a>(
        members: impl Iterator<Item = (&'a str, Option<u64>)>,
        renamed: impl Fn(&str, &str) -> String,
        refragmented: impl Fn(&str) -> String,
    ) -> Result<(), MaterializationDiagnostic> {
        let mut identity_names = std::collections::BTreeMap::new();
        let mut name_identities = std::collections::BTreeMap::new();
        for (field, member_identity) in members {
            let key = materialization_field_key(field, member_identity);
            if let Some(prior_name) = identity_names.insert(key.clone(), field)
                && prior_name != field
            {
                return Err(MaterializationDiagnostic(renamed(prior_name, field)));
            }
            if let Some(prior_identity) = name_identities.insert(field, key.clone())
                && prior_identity != key
            {
                return Err(MaterializationDiagnostic(refragmented(field)));
            }
        }
        Ok(())
    }

    pub const fn stable_identity_suffix(member_identity: Option<u64>) -> &'static str {
        if member_identity.is_some() {
            " with the same stable identity"
        } else {
            ""
        }
    }
}

pub mod field_values {
    use crate::materialization_records::MaterializationDiagnostic;
    use crate::materialization_records::stored_integer_checks::low_mask;

    pub fn validate_authored_width(
        kind: &str,
        width_bits: u16,
    ) -> Result<(), MaterializationDiagnostic> {
        if width_bits == 0 || width_bits > 64 {
            return Err(MaterializationDiagnostic(format!(
                "{kind} field width {width_bits} is outside 1..=64 bits"
            )));
        }
        Ok(())
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ScalarFieldValue {
        pub field: String,
        pub member_identity: Option<u64>,
        pub width_bits: u16,
        pub value: u64,
    }

    impl ScalarFieldValue {
        pub fn new(
            field: impl Into<String>,
            width_bits: u16,
            value: u64,
        ) -> Result<Self, MaterializationDiagnostic> {
            validate_authored_width("scalar", width_bits)?;
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

        numbered_constructor!(new_numbered(field, width_bits: u16, value: u64) -> new);
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct AggregateFieldValue<Bytes = Vec<u8>> {
        pub field: String,
        pub bytes: Bytes,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct AggregateFieldSchema {
        pub field: String,
        pub member_identity: Option<u64>,
        pub byte_size: u64,
        pub shape: AggregateFieldShape,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum AggregateFieldShape {
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

        numbered_constructor!(new_numbered(field, byte_size: u64) -> new);

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

        numbered_constructor!(
            new_repeated_numbered(
                field,
                element_byte_size: u64,
                element_align: u64,
                element_count: u64,
            ) -> new_repeated
        );
    }

    impl AggregateFieldValue {
        pub fn new(
            field: impl Into<String>,
            bytes: impl Into<Vec<u8>>,
        ) -> Result<Self, MaterializationDiagnostic> {
            Self::from_storage(field, bytes.into())
        }
    }

    impl<Bytes: AsRef<[u8]>> AggregateFieldValue<Bytes> {
        pub fn from_storage(
            field: impl Into<String>,
            bytes: Bytes,
        ) -> Result<Self, MaterializationDiagnostic> {
            if bytes.as_ref().is_empty() {
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

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ScalarFieldSchema {
        pub field: String,
        pub member_identity: Option<u64>,
        pub width_bits: u16,
    }

    impl ScalarFieldSchema {
        pub fn new(
            field: impl Into<String>,
            width_bits: u16,
        ) -> Result<Self, MaterializationDiagnostic> {
            validate_authored_width("scalar", width_bits)?;
            Ok(Self {
                field: field.into(),
                member_identity: None,
                width_bits,
            })
        }

        numbered_constructor!(new_numbered(field, width_bits: u16) -> new);
    }
}

pub mod stored_integer_checks {
    use crate::layout_reports::{
        IntegerInterpretation, LayoutFieldEntryReport, LayoutPlacementReport,
    };
    use crate::materialization_records::MaterializationDiagnostic;
    use crate::materialization_records::field_values::ScalarFieldValue;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ScalarFragment {
        pub container_byte_offset: u64,
        pub container_width_bits: u16,
        pub destination_lsb: u16,
        pub source_lsb: u16,
        pub width: u16,
    }

    pub fn scalar_fragment(
        entry: &LayoutFieldEntryReport,
        source_width_bits: u16,
    ) -> Result<ScalarFragment, MaterializationDiagnostic> {
        let (container, container_width, destination_lsb, source_lsb, width) =
            materializer_scalar_window(entry.placement, source_width_bits, "scalar", &entry.field)?;
        Ok(ScalarFragment {
            container_byte_offset: container,
            container_width_bits: container_width,
            destination_lsb,
            source_lsb,
            width,
        })
    }

    pub fn materializer_scalar_window(
        placement: LayoutPlacementReport,
        access_width_bits: u16,
        field_kind: &str,
        field_display: &str,
    ) -> Result<(u64, u16, u16, u16, u16), MaterializationDiagnostic> {
        let (container, container_width, destination_lsb, source_lsb, width) = match placement {
            LayoutPlacementReport::At { offset } => (
                offset,
                u64::from(access_width_bits),
                0,
                0,
                u64::from(access_width_bits),
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
                "{field_kind} field `{field_display}` uses a materializer-incompatible placement"
            )));
        }
        let source_end = source_lsb.checked_add(width).ok_or_else(|| {
            MaterializationDiagnostic(format!("{field_kind} source bit range overflows"))
        })?;
        if source_end > u64::from(access_width_bits) {
            return Err(MaterializationDiagnostic(format!(
                "{field_kind} field `{field_display}` placement reads through bit {source_end}, past its {access_width_bits}-bit source"
            )));
        }
        let destination_end = destination_lsb.checked_add(width).ok_or_else(|| {
            MaterializationDiagnostic(format!("{field_kind} destination bit range overflows"))
        })?;
        if destination_end > container_width {
            return Err(MaterializationDiagnostic(format!(
                "{field_kind} field `{field_display}` placement writes through bit {destination_end}, past its {container_width}-bit container"
            )));
        }
        Ok((
            container,
            u16::try_from(container_width).expect("validated materializer container width"),
            u16::try_from(destination_lsb).expect("validated destination bit index"),
            u16::try_from(source_lsb).expect("validated source bit index"),
            u16::try_from(width).map_err(|_| {
                MaterializationDiagnostic(format!(
                    "{field_kind} field `{field_display}` fragment width {width} is too large"
                ))
            })?,
        ))
    }

    pub fn validate_stored_integer_value(
        entry: &LayoutFieldEntryReport,
        value: &ScalarFieldValue,
    ) -> Result<(), MaterializationDiagnostic> {
        validate_stored_integer_bits(entry, value.width_bits, value.value, "scalar")
    }

    pub const fn byte_sized_width(width_bits: u16) -> bool {
        width_bits != 0 && width_bits <= 64 && width_bits.is_multiple_of(8)
    }

    pub fn stored_value_fits(
        interpretation: IntegerInterpretation,
        stored_width_bits: u16,
        source_width_bits: u16,
        source_value: u64,
    ) -> bool {
        let (minimum, maximum) = interpretation.stored_bounds(u64::from(stored_width_bits));
        let semantic = match interpretation {
            IntegerInterpretation::Signed => signed_bits(source_value, source_width_bits),
            IntegerInterpretation::Unsigned => i128::from(source_value),
        };
        semantic >= minimum && semantic <= maximum
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
        if !byte_sized_width(stored_width) {
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

        if !stored_value_fits(
            interpretation,
            stored_width,
            source_width_bits,
            source_value,
        ) {
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

    fn signed_bits(value: u64, width: u16) -> i128 {
        let value = value & low_mask(width);
        if value & (1_u64 << (width - 1)) == 0 {
            i128::from(value)
        } else {
            i128::from(value) - (1_i128 << width)
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn validate_fragment(
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

    pub const fn low_mask(width: u16) -> u64 {
        if width == 64 {
            u64::MAX
        } else {
            (1_u64 << width) - 1
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializationDiagnostic(pub String);

impl std::fmt::Display for MaterializationDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for MaterializationDiagnostic {}
