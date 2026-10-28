//! Deterministic native Build receipt recipes shared by production and recovery.
//! Source admission supplies the immutable choice commitment. The measured code
//! identity remains explicit, so recovery can check its association to the
//! producer closure rather than trusting unrelated opaque inner digests.

use effects::provider_plan::{
    EvaluatedBindingEvaluationDigest, EvaluatedBindingMaterializationDigest,
    EvaluatedBindingProducerClosureDigest, EvaluatedBindingReceipt, EvaluatedBindingUsage,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeBindingProducerRecipe {
    pub source_choices: [u8; 32],
    pub terminal_code: terminal_psi::TerminalPsiIdentity,
}

impl NativeBindingProducerRecipe {
    pub fn validate(&self) -> Result<(), String> {
        if self.source_choices == [0; 32]
            || self.terminal_code.program_fingerprint.as_bytes() == &[0; 32]
        {
            return Err(
                "native producer recipe has no exact source choices or Terminal code".into(),
            );
        }
        Ok(())
    }

    pub fn closure_digest(&self) -> [u8; 32] {
        let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
            b"omega.build-native-binding.producer-recipe.sha256.v1\0",
        );
        digest.raw(&self.source_choices);
        digest.raw(&self.terminal_code.vocabulary_marker.get().to_le_bytes());
        digest.raw(self.terminal_code.program_fingerprint.as_bytes());
        digest.finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeBindingReceiptRecipe {
    producer_package: Option<semantic_vocabulary::PackageKeyIdentity>,
    producer: String,
    producer_recipe: NativeBindingProducerRecipe,
    locator: target::ForeignLocatorIdentityDigest,
}

impl NativeBindingReceiptRecipe {
    pub fn new(
        producer_package: Option<semantic_vocabulary::PackageKeyIdentity>,
        producer: String,
        producer_recipe: NativeBindingProducerRecipe,
        locator: target::ForeignLocatorIdentityDigest,
    ) -> Self {
        Self {
            producer_package,
            producer,
            producer_recipe,
            locator,
        }
    }

    pub fn materialize(
        &self,
        observation: &crate::BuildObservationSummary,
        usage: crate::BuildEvaluationUsage,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    ) -> Result<EvaluatedBindingReceipt, String> {
        self.materialize_digest(observation.identity().digest(), usage, identities)
    }

    fn materialize_digest(
        &self,
        observation: [u8; 32],
        usage: crate::BuildEvaluationUsage,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    ) -> Result<EvaluatedBindingReceipt, String> {
        self.producer_recipe.validate()?;
        usage.validate().map_err(|error| {
            format!("native binding Build accounting is inconsistent: {error:?}")
        })?;
        let mut invocation = crate::digest_encoder::DomainDigestEncoder::new(
            b"omega.build-native-binding.invocation.sha256.v6\0",
        );
        invocation.raw(&self.producer_recipe.closure_digest());
        invocation.raw(&observation);
        let mut measured = Vec::new();
        crate::build_execution::build_record_codec::encode_build_evaluation_usage(
            &usage,
            &mut measured,
        );
        invocation.bytes(&measured);
        let invocation = invocation.finish();
        let mut materialization = crate::digest_encoder::DomainDigestEncoder::new(
            b"omega.build-native-binding.materialization.sha256.v6\0",
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
            EvaluatedBindingProducerClosureDigest::from_bytes(
                self.producer_recipe.closure_digest(),
            )?,
            5,
            retained_usage,
            EvaluatedBindingEvaluationDigest::from_bytes(invocation)?,
            6,
            EvaluatedBindingMaterializationDigest::from_bytes(materialization.finish())?,
            self.locator,
        )
    }
    pub fn producer_recipe(&self) -> NativeBindingProducerRecipe {
        self.producer_recipe
    }

    /// Recompute every inner relation using the same owner as live production.
    /// An externally replaced inner digest and rehashed outer receipt is not a
    /// completed invocation of this source recipe.
    pub fn validate_receipt(
        &self,
        receipt: &EvaluatedBindingReceipt,
        observation: [u8; 32],
        usage: crate::BuildEvaluationUsage,
    ) -> Result<(), String> {
        let expected = self.materialize_digest(
            observation,
            usage,
            &mut terminal_psi::TerminalIdentityTableBuilder::default(),
        )?;
        if expected.identity_digest() != receipt.identity_digest() {
            return Err("native receipt does not replay its producer, observation, accounting and locator recipe".into());
        }
        Ok(())
    }
}
