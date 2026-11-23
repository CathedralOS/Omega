//! A Build publishes immutable source bytes under logical names in its own activation.

use build_output::PackageGeneratedSource;
use evaluation_accounting::{EvaluationInvocationAccount, EvaluationStorageLease};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Debug, Default)]
pub struct BuildSourcePublications {
    sources: BTreeMap<Vec<u8>, Arc<[u8]>>,
    reservation: Option<(EvaluationInvocationAccount, EvaluationStorageLease)>,
}

impl BuildSourcePublications {
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    pub fn publish(
        &mut self,
        account: &EvaluationInvocationAccount,
        logical_name: &[u8],
        bytes: &[u8],
    ) -> Result<(), String> {
        self.check_account(account)?;
        PackageGeneratedSource::validate_publication(logical_name, bytes)?;
        if self.sources.contains_key(logical_name) {
            return Err(format!(
                "generated source filename \"{}\" was already published",
                logical_name.escape_ascii()
            ));
        }
        let size = publication_bytes(logical_name, bytes)?;
        let mut storage = account
            .reserve_storage(1, size)
            .map_err(|error| error.to_string())?;
        if let Some((_, existing)) = &mut self.reservation {
            existing
                .absorb(&mut storage)
                .map_err(|error| error.to_string())?;
        } else {
            self.reservation = Some((account.clone(), storage));
        }
        self.sources.insert(logical_name.to_vec(), Arc::from(bytes));
        Ok(())
    }

    fn check_account(&self, account: &EvaluationInvocationAccount) -> Result<(), String> {
        if self
            .reservation
            .as_ref()
            .is_some_and(|(owner, _)| !owner.same_invocation(account))
        {
            return Err("generated source publications belong to another Build activation".into());
        }
        Ok(())
    }

    pub(super) fn finish(
        self,
        account: &EvaluationInvocationAccount,
    ) -> Result<Vec<PackageGeneratedSource>, String> {
        self.check_account(account)?;
        let bytes = self.sources.iter().try_fold(0u64, |total, (path, bytes)| {
            total
                .checked_add(publication_bytes(path, bytes)?)
                .ok_or("generated source result byte count overflowed")
        })?;
        let count =
            u64::try_from(self.sources.len()).map_err(|_| "generated source count overflowed")?;
        account
            .charge_result_custody(count, bytes)
            .map_err(|error| error.to_string())?;
        Ok(self
            .sources
            .into_iter()
            .map(|(path, bytes)| PackageGeneratedSource::from_retained_parts(path, bytes))
            .collect())
    }
}

fn publication_bytes(path: &[u8], bytes: &[u8]) -> Result<u64, &'static str> {
    path.len()
        .checked_add(bytes.len())
        .and_then(|size| u64::try_from(size).ok())
        .ok_or("generated source byte count overflowed")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_record_codec::{encode_observation_summary, recover_observation_summary};
    use crate::{BuildMachineFilesystemScope, CapturedBuildObservations};
    use filesystem_authority::observation_codec::ObservationCursor;
    use std::path::{Path, PathBuf};

    fn account(live: u64, result: u64) -> EvaluationInvocationAccount {
        let limits = evaluation_contracts::BuildEvaluationSponsorLimits::new(
            100, 1, 1, 1, 2, live, 2, result,
        )
        .unwrap();
        let sponsor = evaluation_accounting::BuildEvaluationSponsor::new(limits);
        sponsor
            .begin_invocation(sponsor.measurement(), 100)
            .unwrap()
    }

    #[test]
    fn published_bytes_survive_a_changed_input_and_a_reopened_review() {
        let scope = BuildMachineFilesystemScope::for_root(
            Path::new("build.omg"),
            PathBuf::from("staged"),
            None,
        );
        let account = account(128, 128);
        let mut bytes = b"first".to_vec();
        let path = b"generated/start.omg";
        let mut publications = BuildSourcePublications::default();
        publications.publish(&account, path, &bytes).unwrap();
        bytes[0] = b'X';
        assert!(publications.publish(&account, path, &bytes).is_err());
        let mut observations = CapturedBuildObservations::new(&scope);
        observations
            .retain_source_publications(publications, &account)
            .unwrap();
        assert!(
            observations
                .retain_source_publications(BuildSourcePublications::default(), &account)
                .is_err()
        );
        let summary = observations.settle(false, "build").unwrap();
        assert!(summary.filesystem_operation_attempts().is_empty());
        let usage = account.snapshot().unwrap();
        assert_eq!(usage.record().result_payload_bytes, (path.len() + 5) as u64);
        let mut encoded = Vec::new();
        encode_observation_summary(&summary, &mut encoded);
        let recover =
            |bytes: &[u8]| recover_observation_summary(&mut ObservationCursor::new(bytes));
        let reopened = recover(&encoded).unwrap();
        assert_eq!(reopened.generated_sources()[0].bytes(), b"first");
        assert_eq!(reopened, summary);
        let pos = encoded.windows(5).position(|row| row == b"first").unwrap();
        encoded[pos] = b'F';
        assert_ne!(recover(&encoded).unwrap().identity(), summary.identity());
        encoded[pos] = 255;
        assert!(recover(&encoded).is_none());
        encoded[pos] = b'f';
        encoded.push(0);
        assert!(recover(&encoded).is_none());
    }

    #[test]
    fn a_publication_requires_a_valid_name_utf8_and_both_storage_budgets() {
        let invocation = account(16, 5);
        let mut publications = BuildSourcePublications::default();
        for path in [
            b"../start.omg".as_slice(),
            b"/x.omg",
            b"a//b.omg",
            b"a\\b.omg",
            b"x.rs",
            b".omg",
            b"\xff.omg",
        ] {
            assert!(
                publications
                    .publish(&invocation, path, b"")
                    .unwrap_err()
                    .contains("invalid generated source filename")
            );
        }
        assert!(
            publications
                .publish(&invocation, b"ok.omg", &[0xff])
                .unwrap_err()
                .contains("UTF-8")
        );
        assert!(
            publications
                .publish(&invocation, b"ok.omg", &[b'x'; 11])
                .is_err()
        );
        assert!(publications.is_empty());
        publications.publish(&invocation, b"ok.omg", b"x").unwrap();
        assert!(
            publications
                .publish(&account(16, 16), b"else.omg", b"")
                .unwrap_err()
                .contains("another Build activation")
        );
        assert!(publications.finish(&invocation).is_err());
        assert_eq!(
            invocation.snapshot().unwrap().record().result_payload_bytes,
            0
        );
    }
}
