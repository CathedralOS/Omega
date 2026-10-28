//! Immutable native producer input, materialized under one measured invocation.
//!
//! Source checking can share the producer and locator. A consuming activation
//! still owns its observation and accounting; dependency receipts stay original.

use effects::provider_plan::{
    EvaluatedBindingEvaluationDigest, EvaluatedBindingMaterializationDigest,
    EvaluatedBindingProducerClosureDigest, EvaluatedBindingReceipt, EvaluatedBindingUsage,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReceiptSource {
    producer_package: Option<semantic_vocabulary::PackageKeyIdentity>,
    producer: String,
    source: [u8; 32],
    locator: target::ForeignLocatorIdentityDigest,
}

impl ReceiptSource {
    pub(super) fn new(
        producer_package: Option<semantic_vocabulary::PackageKeyIdentity>,
        producer: String,
        source: [u8; 32],
        locator: target::ForeignLocatorIdentityDigest,
    ) -> Self {
        Self {
            producer_package,
            producer,
            source,
            locator,
        }
    }

    pub(super) fn materialize(
        &self,
        observation: &package_compilation::BuildObservationSummary,
        usage: package_compilation::BuildEvaluationUsage,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    ) -> Result<EvaluatedBindingReceipt, String> {
        usage.validate().map_err(|error| {
            format!("native binding Build accounting is inconsistent: {error:?}")
        })?;
        let mut invocation = crate::digest_encoder::DomainDigestEncoder::new(
            b"omega.build-native-binding.invocation.sha256.v4\0",
        );
        invocation.raw(&self.source);
        invocation.raw(observation.identity().as_bytes());
        let mut measured = Vec::new();
        package_compilation::build_execution::build_record_codec::encode_build_evaluation_usage(
            &usage,
            &mut measured,
        );
        invocation.bytes(&measured);
        let invocation = invocation.finish();
        let mut materialization = crate::digest_encoder::DomainDigestEncoder::new(
            b"omega.build-native-binding.materialization.sha256.v4\0",
        );
        materialization.raw(&invocation);
        materialization.raw(&self.locator.as_bytes());
        let retained_usage = EvaluatedBindingUsage::from_evaluator(
            usage.usage_schema_version,
            usage.step_schedule_marker,
            usage.fuel_units,
            usage.invocation_fuel_ceiling,
            usage.build_log_bytes,
            usage.filesystem_operation_attempts,
            usage.peak_live_storage_cells,
            usage.peak_live_payload_bytes,
            usage.result_storage_cells,
            usage.result_payload_bytes,
        )?;
        EvaluatedBindingReceipt::from_evaluation(
            self.producer_package,
            &self.producer,
            identities,
            EvaluatedBindingProducerClosureDigest::from_bytes(self.source)?,
            3,
            retained_usage,
            EvaluatedBindingEvaluationDigest::from_bytes(invocation)?,
            4,
            EvaluatedBindingMaterializationDigest::from_bytes(materialization.finish())?,
            self.locator,
        )
    }
}
