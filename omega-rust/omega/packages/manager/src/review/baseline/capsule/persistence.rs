//! Canonical persistence of one validated review-only baseline capsule.

use std::collections::BTreeMap;

use super::ReviewOnlyBaselineCapsule;
use crate::declarations::BuildDeclarationKind;
use crate::review::baseline::encoding::{
    Encoder, capsule_checksum, encode_package_key, encode_replay_record_option, encode_resolution,
    ensure_bounded_string, validate_recovery_row,
};
use crate::review::baseline::validation::row_limits;
use crate::review::baseline::{
    CHECKSUM_BYTES, MAGIC, REVIEW_ONLY_ARTIFACT_CLASS, ReviewOnlyBaselineError,
    ReviewOnlyBaselineLimits, VERSION,
};

impl ReviewOnlyBaselineCapsule {
    pub fn encode(
        &self,
        limits: ReviewOnlyBaselineLimits,
    ) -> Result<Vec<u8>, ReviewOnlyBaselineError> {
        self.validate(limits)?;
        let mut encoder = Encoder::bounded(
            limits
                .maximum_capsule_bytes
                .checked_sub(CHECKSUM_BYTES)
                .ok_or_else(|| ReviewOnlyBaselineError::new("capsule ceiling is too small"))?,
        );
        encoder.fixed(MAGIC);
        encoder.u16(VERSION);
        encoder.byte(REVIEW_ONLY_ARTIFACT_CLASS);
        encoder.byte(0);
        let first = self
            .packages
            .first()
            .ok_or_else(|| ReviewOnlyBaselineError::new("review baseline is empty"))?;
        ensure_bounded_string(
            &first.target,
            limits.maximum_target_bytes,
            "review baseline target violates its byte bounds",
        )?;
        encoder.string(&first.target)?;
        encoder.usize(self.packages.len())?;
        let indices = self
            .packages
            .iter()
            .enumerate()
            .map(|(index, package)| (package.key.clone(), index))
            .collect::<BTreeMap<_, _>>();
        encoder.u32(
            u32::try_from(*indices.get(self.graph.root()).ok_or_else(|| {
                ReviewOnlyBaselineError::new("review baseline root has no package record")
            })?)
            .map_err(|_| ReviewOnlyBaselineError::new("baseline root index exceeds u32"))?,
        );
        encode_root_role(&mut encoder, self.graph.root_role());
        for package in &self.packages {
            let node = self.graph.package(&package.key).ok_or_else(|| {
                ReviewOnlyBaselineError::new("review baseline package has no graph node")
            })?;
            let mut record = Encoder::bounded(limits.maximum_capsule_bytes);
            encode_package_key(&mut record, &package.key, limits.maximum_identity_bytes)?;
            encode_resolution(&mut record, &package.resolution)?;
            record.fixed(&package.source_consumption_commitment.digest());
            record.fixed(&package.whole_review_commitment);
            match package.build_observation_commitment {
                None => record.byte(0),
                Some(commitment) => {
                    record.byte(1);
                    record.fixed(&commitment);
                }
            }
            encode_replay_record_option(&mut record, package.filesystem_replay_record.as_ref())?;
            if let Some(binding) = package.replay_record_parent_binding {
                record.fixed(&binding);
            }
            record.usize(node.dependencies().len())?;
            for dependency in node.dependencies() {
                ensure_bounded_string(
                    dependency.alias().as_str(),
                    limits.maximum_identity_bytes,
                    "review baseline dependency alias violates its byte bounds",
                )?;
                record.string(dependency.alias().as_str())?;
                record.u32(
                    u32::try_from(*indices.get(dependency.target()).ok_or_else(|| {
                        ReviewOnlyBaselineError::new(
                            "review baseline dependency has no package record",
                        )
                    })?)
                    .map_err(|_| {
                        ReviewOnlyBaselineError::new("baseline dependency index exceeds u32")
                    })?,
                );
            }
            record.usize(package.canonical_rows.len())?;
            for row in &package.canonical_rows {
                let recovery_bytes =
                    validate_recovery_row(row, &package.key, &package.target, row_limits(limits))?;
                record.bytes(recovery_bytes)?;
            }
            encoder.bytes(&record.finish()?)?;
        }
        let mut bytes = encoder.finish()?;
        let checksum = capsule_checksum(&bytes);
        bytes
            .try_reserve_exact(CHECKSUM_BYTES)
            .map_err(|_| ReviewOnlyBaselineError::new("capsule checksum allocation failed"))?;
        bytes.extend_from_slice(&checksum);
        Ok(bytes)
    }
}

fn encode_root_role(encoder: &mut Encoder, role: BuildDeclarationKind) {
    encoder.byte(match role {
        BuildDeclarationKind::Package => 0,
        BuildDeclarationKind::Application => 1,
        BuildDeclarationKind::Workspace => 2,
    });
}
