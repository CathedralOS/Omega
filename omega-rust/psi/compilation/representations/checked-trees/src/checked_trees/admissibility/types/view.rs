use super::{AcceptanceCheck, AcceptanceDimension, AcceptanceSummary, AcceptanceVerdict};

pub trait AcceptanceView {
    fn summary(&self) -> AcceptanceSummary;

    fn check(&self, dimension: AcceptanceDimension) -> AcceptanceCheck {
        self.summary().check(dimension)
    }

    fn verdict(&self) -> AcceptanceVerdict {
        self.summary().verdict
    }

    fn is_accepted(&self) -> bool {
        self.summary().is_accepted()
    }

    fn is_dimension_satisfied(&self, dimension: AcceptanceDimension) -> bool {
        self.check(dimension).is_satisfied()
    }

    fn evidence_count(&self) -> usize {
        self.summary().evidence_count()
    }

    fn diagnostic_count(&self) -> usize {
        self.summary().diagnostic_count()
    }

    fn rejected_check_count(&self) -> usize {
        self.summary().rejected_check_count()
    }

    fn has_diagnostics(&self) -> bool {
        self.summary().has_diagnostics()
    }
}
