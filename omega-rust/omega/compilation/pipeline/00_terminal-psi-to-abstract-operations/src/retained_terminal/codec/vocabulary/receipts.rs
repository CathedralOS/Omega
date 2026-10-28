//! Evaluated binding receipts retain their exact producer-table coordinates.

use super::super::value::{RecordError, RecordReader, RecordValue, RecordWriter};
use super::record_fields;

struct ReceiptRecord {
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
