//! Decision-log custody, corpus admission, and artifact publication.

use std::fs;

use omega_optimization_policy_offline::admit_external_decision_logs;

use crate::{
    arguments::CaptureRequest, error::OfflinePolicyCommandError, publication::publish_new,
};

pub(super) fn capture(request: CaptureRequest) -> Result<(), OfflinePolicyCommandError> {
    let encoded_logs = request
        .logs
        .iter()
        .map(|path| {
            fs::read(path).map_err(|source| OfflinePolicyCommandError::ReadLog {
                path: path.clone(),
                source,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let corpus = admit_external_decision_logs(encoded_logs)
        .map_err(OfflinePolicyCommandError::InvalidCorpus)?;
    publish_new(&request.output, &corpus.encode())
}
