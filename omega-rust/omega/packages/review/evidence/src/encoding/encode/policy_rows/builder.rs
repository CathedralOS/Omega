use super::*;

pub(super) struct Builder<'policy> {
    policy: &'policy PackagePolicyBaseline,
    limits: PackagePolicyRowLimits,
    usage: PackagePolicyRowUsage,
    rows: Vec<PackagePolicyRow>,
    expected_count: usize,
}

impl<'policy> Builder<'policy> {
    pub(super) fn new(
        policy: &'policy PackagePolicyBaseline,
        count: usize,
        limits: PackagePolicyRowLimits,
    ) -> Result<Self, PackageReviewEncodingError> {
        if count > limits.maximum_rows {
            return Err(rejected("package policy exceeds its row count ceiling"));
        }
        let bytes = count
            .checked_mul(std::mem::size_of::<PackagePolicyRow>())
            .ok_or_else(|| rejected("package policy row table size overflows"))?;
        let mut builder = Self {
            policy,
            limits,
            usage: PackagePolicyRowUsage::default(),
            rows: Vec::new(),
            expected_count: count,
        };
        builder.charge_bytes(bytes)?;
        builder
            .rows
            .try_reserve_exact(count)
            .map_err(|_| rejected("package policy row table allocation failed"))?;
        Ok(builder)
    }

    fn charge_bytes(&mut self, bytes: usize) -> Result<(), PackageReviewEncodingError> {
        self.usage.owned_bytes = self
            .usage
            .owned_bytes
            .checked_add(bytes)
            .filter(|used| *used <= self.limits.maximum_owned_bytes)
            .ok_or_else(|| rejected("package policy rows exceed aggregate owned storage"))?;
        Ok(())
    }

    fn remaining_elements(&self) -> usize {
        self.limits.maximum_sequence_elements - self.usage.sequence_elements
    }

    fn charge_elements(&mut self, remaining: usize) {
        self.usage.sequence_elements = self.limits.maximum_sequence_elements - remaining;
    }

    pub(super) fn push(
        &mut self,
        kind: PackagePolicyRowKind,
        initial: bool,
        audit: bool,
        key: impl Fn(&mut Encoder) -> Result<(), PackageReviewEncodingError>,
        value: impl Fn(&mut Encoder) -> Result<(), PackageReviewEncodingError>,
    ) -> Result<(), PackageReviewEncodingError> {
        self.usage.rows = self
            .usage
            .rows
            .checked_add(1)
            .filter(|rows| *rows <= self.expected_count)
            .ok_or_else(|| rejected("package policy exceeds its row count ceiling"))?;
        let mut measure = Encoder::row_measure(
            self.limits.maximum_key_bytes,
            None,
            self.remaining_elements(),
            self.limits.maximum_depth,
        );
        key(&mut measure)?;
        let (key_length, _, remaining) = measure.row_metrics()?;
        self.charge_elements(remaining);
        self.charge_bytes(key_length)?;
        let mut output = Encoder::row_output(
            key_length,
            None,
            self.remaining_elements(),
            self.limits.maximum_depth,
        )?;
        key(&mut output)?;
        let (length, _, remaining) = output.row_metrics()?;
        self.charge_elements(remaining);
        if length != key_length {
            return Err(rejected(
                "package policy key sizing disagrees with emission",
            ));
        }
        let (key_bytes, _) = output.finish_row()?;

        let mut measure = Encoder::row_measure(
            self.limits.maximum_canonical_bytes,
            Some(self.limits.maximum_text_bytes),
            self.remaining_elements(),
            self.limits.maximum_depth,
        );
        self.frame(&mut measure, kind, initial, audit, &value)?;
        let (binary_length, text_length, remaining) = measure.row_metrics()?;
        self.charge_elements(remaining);
        self.charge_bytes(
            binary_length
                .checked_add(text_length)
                .ok_or_else(|| rejected("package policy row size overflows"))?,
        )?;
        let mut output = Encoder::row_output(
            binary_length,
            Some(text_length),
            self.remaining_elements(),
            self.limits.maximum_depth,
        )?;
        self.frame(&mut output, kind, initial, audit, &value)?;
        let (binary, text, remaining) = output.row_metrics()?;
        self.charge_elements(remaining);
        if (binary, text) != (binary_length, text_length) {
            return Err(rejected(
                "package policy row sizing disagrees with emission",
            ));
        }
        let (canonical_bytes, canonical_text) = output.finish_row()?;
        self.rows.push(PackagePolicyRow {
            kind,
            key_bytes,
            canonical_bytes,
            canonical_text,
            initial_requires_decision: initial,
            audit_recommended_when_present: audit,
        });
        Ok(())
    }

    fn frame(
        &self,
        encoder: &mut Encoder,
        kind: PackagePolicyRowKind,
        initial: bool,
        audit: bool,
        value: &impl Fn(&mut Encoder) -> Result<(), PackageReviewEncodingError>,
    ) -> Result<(), PackageReviewEncodingError> {
        encoder.field("binary_format", |encoder| {
            encoder.fixed_bytes(b"OMEGA-PACKAGE-POLICY-ROW\0");
            Ok(())
        })?;
        encoder.field("row_schema", |encoder| {
            encoder.u16(PACKAGE_POLICY_ROW_VERSION);
            Ok(())
        })?;
        encoder.field("baseline_schema", |encoder| {
            encoder.u16(crate::encoding::PACKAGE_POLICY_BASELINE_VERSION);
            Ok(())
        })?;
        encoder.field("package", |encoder| {
            encoder.package_identity(self.policy.package);
            Ok(())
        })?;
        encoder.field("target", |encoder| {
            encoder.string(self.policy.target.identity().as_str())
        })?;
        encoder.field("kind", |encoder| {
            encoder.tag(kind.as_str(), kind.canonical_tag());
            Ok(())
        })?;
        encoder.field("initial_requires_decision", |encoder| {
            encoder.boolean(initial);
            Ok(())
        })?;
        encoder.field("update_requires_decision", |encoder| {
            encoder.boolean(kind.update_requires_decision());
            Ok(())
        })?;
        encoder.field("audit_recommended_when_present", |encoder| {
            encoder.boolean(audit);
            Ok(())
        })?;
        encoder.field("audit_recommended_on_change", |encoder| {
            encoder.boolean(kind.row_change_audit(initial, audit));
            Ok(())
        })?;
        encoder.field("value", value)
    }

    pub(super) fn finish(
        mut self,
    ) -> Result<(Vec<PackagePolicyRow>, PackagePolicyRowUsage), PackageReviewEncodingError> {
        if self.rows.len() != self.expected_count {
            return Err(rejected(
                "package policy row count differs from its projection",
            ));
        }
        self.rows.sort_unstable_by(|left, right| {
            (left.kind, &left.key_bytes).cmp(&(right.kind, &right.key_bytes))
        });
        if self
            .rows
            .windows(2)
            .any(|rows| rows[0].kind == rows[1].kind && rows[0].key_bytes == rows[1].key_bytes)
        {
            return Err(rejected(
                "package policy contains duplicate semantic row coordinates",
            ));
        }
        Ok((self.rows, self.usage))
    }
}
