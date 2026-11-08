//! Evaluated binding receipts retain their exact producer-table coordinates.
//! A Build origin's requirement is an index into that same producer table.

use super::super::value::{RecordError, RecordReader, RecordValue, RecordWriter};
use super::record_fields;

struct ReceiptRecord {
    build_origin: Option<effects::provider_plan::BuildNativeBindingOrigin>,
    producer_package: Option<semantic_vocabulary::PackageKeyIdentity>,
    producer_callable_identity: terminal_psi::TerminalIdentityIndex,
    producer_closure_digest: effects::provider_plan::EvaluatedBindingProducerClosureDigest,
    evaluator_semantics_marker: u32,
    evaluation_usage: effects::provider_plan::EvaluatedBindingUsage,
    evaluation_digest: effects::provider_plan::EvaluatedBindingEvaluationDigest,
    materializer_schema_version: u32,
    materialization_digest: effects::provider_plan::EvaluatedBindingMaterializationDigest,
    locator_identity_digest: [u8; 32],
    identity_digest: [u8; 32],
}

record_fields!(ReceiptRecord {
    build_origin,
    producer_package,
    producer_callable_identity,
    producer_closure_digest,
    evaluator_semantics_marker,
    evaluation_usage,
    evaluation_digest,
    materializer_schema_version,
    materialization_digest,
    locator_identity_digest,
    identity_digest,
});

impl RecordValue for effects::provider_plan::EvaluatedBindingReceipt {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        ReceiptRecord {
            build_origin: self.build_origin().cloned(),
            producer_package: self.producer_package(),
            producer_callable_identity: self.producer_callable_identity(),
            producer_closure_digest: self.producer_closure_digest(),
            evaluator_semantics_marker: self.evaluator_semantics_marker(),
            evaluation_usage: self.evaluation_usage(),
            evaluation_digest: self.evaluation_digest(),
            materializer_schema_version: self.materializer_schema_version(),
            materialization_digest: self.materialization_digest(),
            locator_identity_digest: self.locator_identity_digest().as_bytes(),
            identity_digest: self.identity_digest(),
        }
        .write(output)
    }

    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        input.record(|input| {
            let ReceiptRecord {
                build_origin,
                producer_package,
                producer_callable_identity,
                producer_closure_digest,
                evaluator_semantics_marker,
                evaluation_usage,
                evaluation_digest,
                materializer_schema_version,
                materialization_digest,
                locator_identity_digest,
                identity_digest,
            } = ReceiptRecord::read(input)?;
            let identities = input.context.binding_identities.clone().ok_or_else(|| {
                input.error("evaluated binding receipt is missing its producer identity namespace")
            })?;
            let producer = identities
                .text(producer_callable_identity)
                .filter(|identity| !identity.is_empty())
                .ok_or_else(|| {
                    input.error("binding receipt has no exact producer in its identity table")
                })?;

            // The ordinary evaluator factory interns into its supplied builder.
            // Seed the full retained table, not a table containing only this
            // producer, and require that the factory neither remints its index
            // nor changes any other retained entry or invocation coordinate.
            let text_bytes = identities
                .entries()
                .iter()
                .fold(0usize, |total, text| total.saturating_add(text.len()));
            input.allocate(text_bytes.saturating_mul(2).saturating_add(
                identities.entries().len().saturating_mul(
                    2 * std::mem::size_of::<String>() + 8 * std::mem::size_of::<usize>(),
                ),
            ))?;
            let mut builder = identities.as_ref().clone().into_builder();
            let recovered = Self::from_evaluation(
                producer_package,
                producer,
                &mut builder,
                producer_closure_digest,
                evaluator_semantics_marker,
                evaluation_usage,
                evaluation_digest,
                materializer_schema_version,
                materialization_digest,
                target::ForeignLocatorIdentityDigest::from_digest(locator_identity_digest),
            )
            .map_err(|reason| input.error(reason))?;
            let recovered = match build_origin {
                Some(origin) => recovered
                    .with_build_origin(origin, builder.as_table())
                    .map_err(|reason| input.error(reason))?,
                None => recovered,
            };
            if recovered.producer_callable_identity() != producer_callable_identity {
                return Err(input.error("binding receipt recovery changed its producer index"));
            }
            if builder.as_table() != identities.as_ref() {
                return Err(
                    input.error("binding receipt recovery changed its producer identity table")
                );
            }
            if recovered.identity_digest() != identity_digest {
                return Err(input.error(
                    "binding receipt commitment differs from its retained evaluation fields",
                ));
            }
            Ok(recovered)
        })
    }
}

impl RecordValue for effects::provider_plan::BuildNativeBindingOrigin {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.requirement_identity.write(output)?;
        self.source_occurrence_digest.write(output)?;
        self.terminal_code.vocabulary_marker.get().write(output)?;
        self.terminal_code
            .program_fingerprint
            .as_bytes()
            .write(output)?;
        self.fuel_schedule.write(output)?;
        self.storage_schedule.write(output)?;
        self.observation_digest.write(output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        input.record(|input| {
            let requirement_identity = RecordValue::read(input)?;
            let source_occurrence_digest = RecordValue::read(input)?;
            let vocabulary = u16::read(input)?;
            let vocabulary_marker =
                terminal_psi::VocabularyMarker::new(vocabulary).ok_or_else(|| {
                    input.error("native Build receipt has a different Terminal vocabulary")
                })?;
            let program_fingerprint =
                terminal_psi::SemanticFingerprint::from_bytes(RecordValue::read(input)?);
            Ok(Self {
                requirement_identity,
                source_occurrence_digest,
                terminal_code: terminal_psi::TerminalPsiIdentity {
                    vocabulary_marker,
                    program_fingerprint,
                },
                fuel_schedule: RecordValue::read(input)?,
                storage_schedule: RecordValue::read(input)?,
                observation_digest: RecordValue::read(input)?,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::value::{RecordLimits, RecordReader, RecordValue, RecordWriter};
    use effects::provider_plan::{
        BuildNativeBindingOrigin, EvaluatedBindingEvaluationDigest,
        EvaluatedBindingMaterializationDigest, EvaluatedBindingProducerClosureDigest,
        EvaluatedBindingReceipt, EvaluatedBindingUsage,
    };
    use std::sync::Arc;

    const REQUIREMENT: &str = "package::Console::write";
    const PRODUCER: &str = "package::build::console_binding";

    fn build_receipt(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    ) -> EvaluatedBindingReceipt {
        let requirement_identity = identities.intern(REQUIREMENT);
        EvaluatedBindingReceipt::from_evaluation(
            semantic_vocabulary::PackageKeyIdentity::from_digest([7; 32]),
            PRODUCER,
            identities,
            EvaluatedBindingProducerClosureDigest::from_bytes([1; 32]).unwrap(),
            3,
            EvaluatedBindingUsage::from_evaluator(1, 1, 10, 100, 0, 0, 4, 8, 1, 2).unwrap(),
            EvaluatedBindingEvaluationDigest::from_bytes([2; 32]).unwrap(),
            2,
            EvaluatedBindingMaterializationDigest::from_bytes([3; 32]).unwrap(),
            target::ForeignLocatorIdentityDigest::from_digest([4; 32]),
        )
        .unwrap()
        .with_build_origin(
            BuildNativeBindingOrigin {
                requirement_identity,
                source_occurrence_digest: [5; 32],
                terminal_code: terminal_psi::TerminalPsiIdentity {
                    vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
                    program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([6; 32]),
                },
                fuel_schedule: 1,
                storage_schedule: 1,
                observation_digest: [8; 32],
            },
            identities.as_table(),
        )
        .unwrap()
    }

    fn round_trip(
        receipt: &EvaluatedBindingReceipt,
        identities: terminal_psi::TerminalIdentityTable,
    ) -> Result<EvaluatedBindingReceipt, String> {
        let mut output = RecordWriter::new(RecordLimits::default());
        receipt.write(&mut output).unwrap();
        let bytes = output.finish();
        let mut input = RecordReader::new(&bytes, RecordLimits::default()).unwrap();
        input.context.binding_identities = Some(Arc::new(identities));
        let recovered = EvaluatedBindingReceipt::read(&mut input).map_err(|e| e.to_string())?;
        input.finish().unwrap();
        Ok(recovered)
    }

    #[test]
    fn build_origin_receipt_round_trips_through_its_identity_table() {
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        let receipt = build_receipt(&mut identities);
        let table = identities.build();
        let recovered = round_trip(&receipt, table.clone()).unwrap();
        assert_eq!(recovered, receipt);
        let origin = recovered.build_origin().unwrap();
        assert_eq!(table.text(origin.requirement_identity), Some(REQUIREMENT));
    }

    #[test]
    fn build_origin_requirement_absent_from_its_table_is_refused() {
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        identities.intern(PRODUCER);
        let receipt = build_receipt(&mut identities);
        // The producer still resolves, but the origin's requirement index
        // lies past the end of this table.
        let mut producer_only = terminal_psi::TerminalIdentityTableBuilder::default();
        producer_only.intern(PRODUCER);
        let error = round_trip(&receipt, producer_only.build()).unwrap_err();
        assert!(error.contains("no exact requirement identity"), "{error}");
    }

    #[test]
    fn build_origin_requirement_with_substituted_text_is_refused() {
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        let receipt = build_receipt(&mut identities);
        // Same ordinals, different requirement text: the commitment covers
        // the resolved text, so the substitution cannot pass as the original.
        let mut substituted = terminal_psi::TerminalIdentityTableBuilder::default();
        substituted.intern("package::Console::read");
        substituted.intern(PRODUCER);
        let error = round_trip(&receipt, substituted.build()).unwrap_err();
        assert!(error.contains("commitment"), "{error}");
    }

    #[test]
    fn reinterned_build_origin_keeps_its_commitment() {
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        let receipt = build_receipt(&mut identities);
        let source = identities.build();
        let mut destination = terminal_psi::TerminalIdentityTableBuilder::default();
        destination.intern("padding shifts every ordinal");
        let moved = receipt
            .reintern_producer(&source, &mut destination)
            .unwrap();
        let destination = destination.build();
        assert_eq!(moved.identity_digest(), receipt.identity_digest());
        assert_eq!(
            destination.text(moved.build_origin().unwrap().requirement_identity),
            Some(REQUIREMENT)
        );
        assert_eq!(round_trip(&moved, destination).unwrap(), moved);
    }
}
