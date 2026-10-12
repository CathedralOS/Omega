//! Batch result disposition after invocation-owned staging cleanup.
//!
//! This operation consumes a cleanup result; it performs no filesystem work
//! or compilation. One original OS error remains shared by the affected rows,
//! while each row retains its own prior failure text.

use crate::review::CompileResolvedPackageReviewsError;
use std::io;
use std::path::Path;
use std::sync::Arc;

pub(super) fn dispose<Outcome, Value, Error>(
    root: &Path,
    cleanup: io::Result<()>,
    mut outcomes: Vec<Outcome>,
    mut result: impl FnMut(&mut Outcome) -> &mut Result<Value, Error>,
    mut review_error: impl FnMut(CompileResolvedPackageReviewsError) -> Error,
) -> Vec<Outcome>
where
    Error: std::fmt::Display,
{
    let Err(error) = cleanup else {
        return outcomes;
    };
    let error = Arc::new(error);
    for outcome in &mut outcomes {
        let result = result(outcome);
        let prior = result.as_ref().err().map(ToString::to_string);
        *result = Err(review_error(
            CompileResolvedPackageReviewsError::BuildStagingCleanup {
                path: root.to_path_buf(),
                error: Arc::clone(&error),
                prior,
            },
        ));
    }
    outcomes
}

#[cfg(test)]
mod tests {
    use super::dispose;
    use crate::review::CompileResolvedPackageReviewsError;
    use std::fmt;
    use std::io;
    use std::path::Path;

    #[derive(Debug)]
    enum TargetError {
        Original(&'static str),
        Cleanup(CompileResolvedPackageReviewsError),
    }

    impl fmt::Display for TargetError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::Original(message) => formatter.write_str(message),
                Self::Cleanup(error) => fmt::Display::fmt(error, formatter),
            }
        }
    }

    #[test]
    fn batch_disposal_preserves_mixed_outcomes_when_cleanup_succeeds() {
        let outcomes = dispose(
            Path::new("staging"),
            Ok(()),
            vec![Ok(7), Err(TargetError::Original("target failed"))],
            |result| result,
            TargetError::Cleanup,
        );
        assert!(matches!(outcomes[0], Ok(7)));
        assert!(matches!(
            outcomes[1],
            Err(TargetError::Original("target failed"))
        ));
    }

    #[test]
    fn batch_disposal_reports_original_cleanup_error_and_each_prior_failure() {
        let original = io::Error::from_raw_os_error(13);
        let original_message = original.to_string();
        let outcomes = dispose(
            Path::new("staging"),
            Err(original),
            vec![
                Ok(7),
                Err(TargetError::Original("first target failed")),
                Err(TargetError::Original("second target failed")),
            ],
            |result| result,
            TargetError::Cleanup,
        );
        for (outcome, expected_prior) in outcomes.iter().zip([
            None,
            Some("first target failed"),
            Some("second target failed"),
        ]) {
            let Err(TargetError::Cleanup(
                CompileResolvedPackageReviewsError::BuildStagingCleanup { path, error, prior },
            )) = outcome
            else {
                panic!("cleanup failure must prevent a successful target report");
            };
            assert_eq!(path, Path::new("staging"));
            assert_eq!(error.raw_os_error(), Some(13));
            assert_eq!(error.to_string(), original_message);
            assert_eq!(prior.as_deref(), expected_prior);
        }
    }
}
