//! Keep Build consent requests and the work already charged to their source occurrence.
use diagnostics::Diagnostic;

#[derive(Debug, Clone)]
pub enum CompileFailure {
    Diagnostics(Vec<Diagnostic>),
    /// Actual source invocations ran before preparation, judgment or target
    /// admission failed. Keep their records alongside the diagnostic cause.
    SourceCompletion {
        diagnostics: Vec<Diagnostic>,
        accounting: source_completion_accounting::SourceCompletionAccounting,
    },
    BuildConsentRequired {
        site: build_consent::RestrictedBuildSite,
        requests: Vec<build_consent::RestrictedBuildRequest>,
        accounting: source_completion_accounting::SourceCompletionAccounting,
    },
}

impl CompileFailure {
    pub fn source_completion(
        diagnostics: Vec<Diagnostic>,
        accounting: source_completion_accounting::SourceCompletionAccounting,
    ) -> Self {
        if accounting.receipts().is_empty() {
            Self::Diagnostics(diagnostics)
        } else {
            Self::SourceCompletion {
                diagnostics,
                accounting,
            }
        }
    }

    pub fn source_completion_accounting(
        &self,
    ) -> Option<&source_completion_accounting::SourceCompletionAccounting> {
        match self {
            Self::SourceCompletion { accounting, .. }
            | Self::BuildConsentRequired { accounting, .. } => Some(accounting),
            _ => None,
        }
    }

    /// Retain work already spent without changing a pending consent request
    /// into a diagnostic refusal or an execution grant. Shared invocation
    /// occurrences are joined by the account's existing identity rule.
    pub fn with_source_completion_accounting(
        self,
        mut accounting: source_completion_accounting::SourceCompletionAccounting,
    ) -> Self {
        match self {
            Self::Diagnostics(diagnostics) => Self::source_completion(diagnostics, accounting),
            Self::SourceCompletion {
                diagnostics,
                accounting: attempted,
            } => {
                accounting.append(attempted);
                Self::source_completion(diagnostics, accounting)
            }
            Self::BuildConsentRequired {
                site,
                requests,
                accounting: attempted,
            } => {
                accounting.append(attempted);
                Self::BuildConsentRequired {
                    site,
                    requests,
                    accounting,
                }
            }
        }
    }

    pub fn in_build_context(mut self, context: diagnostics::BuildDiagnosticContext) -> Self {
        if let Self::Diagnostics(rows)
        | Self::SourceCompletion {
            diagnostics: rows, ..
        } = &mut self
        {
            for row in rows {
                row.build_context.get_or_insert(context);
            }
        }
        self
    }

    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        match self {
            Self::Diagnostics(diagnostics) | Self::SourceCompletion { diagnostics, .. } => diagnostics,
            Self::BuildConsentRequired { requests, .. } => requests
                .into_iter()
                .map(|request| {
                    Diagnostic::error(format!(
                        "restricted Build execution requires explicit consent; generated-source and final product findings are unavailable; pending request: {request:?}"
                    ))
                })
                .collect(),
        }
    }
}

impl From<source_completion_accounting::SourcePreparationFailure> for CompileFailure {
    fn from(failure: source_completion_accounting::SourcePreparationFailure) -> Self {
        Self::source_completion(failure.diagnostics, failure.accounting)
    }
}

impl From<Vec<Diagnostic>> for CompileFailure {
    fn from(diagnostics: Vec<Diagnostic>) -> Self {
        Self::Diagnostics(diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use build_consent::{RestrictedBuildBounds, RestrictedBuildOperation, RestrictedBuildRequest};

    #[test]
    fn attaching_source_work_preserves_a_pending_consent_request() {
        let request = RestrictedBuildRequest::from_retained(
            RestrictedBuildOperation::ScopedFilesystemExecution,
            Vec::new(),
            Vec::new(),
            RestrictedBuildBounds::from_retained(None, None, vec![b"release".to_vec()], true),
            None,
            None,
        );
        let failure = CompileFailure::BuildConsentRequired {
            site: build_consent::RestrictedBuildSite {
                subject: None,
                entry: "build".into(),
                program: [7; 32],
            },
            requests: vec![request.clone()],
            accounting: Default::default(),
        }
        .with_source_completion_accounting(Default::default());
        let CompileFailure::BuildConsentRequired {
            requests,
            accounting,
            ..
        } = failure
        else {
            panic!("attaching accounting changed the consent outcome")
        };
        assert_eq!(requests, [request]);
        assert!(accounting.receipts().is_empty());
    }
}
