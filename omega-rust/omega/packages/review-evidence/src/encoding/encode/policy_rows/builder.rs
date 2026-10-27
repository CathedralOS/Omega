use crate::encoding::PackageReviewEncodingError;
use crate::encoding::encode::policy_rows::Encoder;
use crate::encoding::encode::policy_rows::rejected;
use crate::record::PACKAGE_POLICY_ROW_VERSION;
use crate::record::PackagePolicyRow;
use crate::record::PackagePolicyRowKind;
use crate::record::PackagePolicyRowLimits;
use crate::record::PackagePolicyRowUsage;
use semantic_vocabulary::PackageKeyIdentity;

pub(super) struct Builder {
    package: PackageKeyIdentity,
    target: target::TargetProfile,
    limits: PackagePolicyRowLimits,
    usage: PackagePolicyRowUsage,
    rows: Vec<PackagePolicyRow>,
    expected_count: usize,
    acceptance_only: bool,
}

impl Builder {
    pub(super) fn new(
        package: PackageKeyIdentity,
        target: target::TargetProfile,
        count: usize,
        limits: PackagePolicyRowLimits,
        acceptance_only: bool,
    ) -> Result<Self, PackageReviewEncodingError> {
        if count > limits.maximum_rows {
            return Err(rejected("package policy exceeds its row count ceiling"));
        }
        let bytes = count
            .checked_mul(std::mem::size_of::<PackagePolicyRow>())
            .ok_or_else(|| rejected("package policy row table size overflows"))?;
        let mut builder = Self {
            package,
            target,
            limits,
            usage: PackagePolicyRowUsage::default(),
            rows: Vec::new(),
            expected_count: count,
            acceptance_only,
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
        if self.acceptance_only && !initial {
            return Ok(());
        }
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
        frame_row(
            &mut measure,
            self.package,
            self.target,
            kind,
            initial,
            audit,
            &value,
        )?;
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
        frame_row(
            &mut output,
            self.package,
            self.target,
            kind,
            initial,
            audit,
            &value,
        )?;
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

/// One row's acceptance frame: the versioning envelope, the occurrence's
/// package identity and target, the row kind, its decision flags, and the
/// carried value. Shared by whole-baseline projection and single-request
/// rendering so both produce identical canonical text.
pub(super) fn frame_row(
    encoder: &mut Encoder,
    package: PackageKeyIdentity,
    target: target::TargetProfile,
    kind: PackagePolicyRowKind,
    initial: bool,
    audit: bool,
    value: &impl Fn(&mut Encoder) -> Result<(), PackageReviewEncodingError>,
) -> Result<(), PackageReviewEncodingError> {
    encoder.fixed_bytes_field("binary_format", b"OMEGA-PACKAGE-POLICY-ROW\0")?;
    encoder.u16_field("row_schema", PACKAGE_POLICY_ROW_VERSION)?;
    encoder.u16_field(
        "baseline_schema",
        crate::encoding::PACKAGE_POLICY_BASELINE_VERSION,
    )?;
    encoder.package_identity_field("package", package)?;
    encoder.field("target", |encoder| {
        encoder.string(target.identity().as_str())
    })?;
    encoder.tag_field("kind", kind.as_str(), kind.canonical_tag())?;
    encoder.boolean_field("initial_requires_decision", initial)?;
    encoder.boolean_field("update_requires_decision", initial)?;
    encoder.boolean_field("audit_recommended_when_present", audit)?;
    encoder.boolean_field(
        "audit_recommended_on_change",
        kind.row_change_audit(initial, audit),
    )?;
    encoder.field("value", value)
}
