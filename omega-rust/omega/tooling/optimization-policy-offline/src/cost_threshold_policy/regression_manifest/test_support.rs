use crate::cost_threshold_policy::OfflinePolicyAlgorithmIdentity;

use super::{identity, model::OfflinePolicyRegressionManifest};

/// One substitutable field of [`OfflinePolicyRegressionManifest`]. The
/// custody matrix substitutes exactly one field per leg so a rejection
/// attributes to that claim alone. Every field is representable on the
/// wire; no field is canonical-encoding-closed.
///
/// The independent checker is [`super::decode`]: checked decode recomputes
/// the regression report and the stored identity from the admitted corpus
/// and model before granting manifest custody.
#[derive(Debug, Clone, Copy)]
pub enum OfflinePolicyRegressionManifestFieldForTest {
    Identity,
    Corpus,
    Model,
    Algorithm,
    RegressionSplit,
    ExpectedReport,
    ExpectedSummary,
}

impl OfflinePolicyRegressionManifest {
    /// Substitute exactly one recorded field with the foreign donor's
    /// authentic value — `algorithm` has a single honest vocabulary value,
    /// so its leg takes a fixed non-identity alternate, and
    /// `expected_summary` mutates its own decision count because an
    /// identically shaped corpus can legitimately produce an identical
    /// summary — then recompute the containing `identity` honestly over the
    /// mutated record. The `identity` leg skips recomputation: its
    /// substitution is the stale checksum the checker must reject on its
    /// own.
    pub fn substitute_field_for_test(
        &mut self,
        field: OfflinePolicyRegressionManifestFieldForTest,
        donor: &Self,
    ) {
        match field {
            OfflinePolicyRegressionManifestFieldForTest::Identity => {
                self.identity = donor.identity;
                return;
            }
            OfflinePolicyRegressionManifestFieldForTest::Corpus => {
                self.corpus = donor.corpus;
            }
            OfflinePolicyRegressionManifestFieldForTest::Model => {
                self.model = donor.model;
            }
            OfflinePolicyRegressionManifestFieldForTest::Algorithm => {
                self.algorithm = OfflinePolicyAlgorithmIdentity::from_bytes([0xf1; 32]);
            }
            OfflinePolicyRegressionManifestFieldForTest::RegressionSplit => {
                self.regression_split = donor.regression_split;
            }
            OfflinePolicyRegressionManifestFieldForTest::ExpectedReport => {
                self.expected_report = donor.expected_report;
            }
            OfflinePolicyRegressionManifestFieldForTest::ExpectedSummary => {
                self.expected_summary.decision_count += 1;
            }
        }
        self.identity = identity::identity(self);
    }
}
