//! The checker owns construction of the checked program and its evidence.
use crate::Discharge;
use terminal_check_tables::program::Elaboration;
use terminal_codec::ObligationEvidenceSeal;

/// A closure the route accepted.
#[derive(Debug)]
pub struct AcceptedClosure {
    pub(crate) elaboration: Elaboration,
    pub(crate) discharge: Discharge,
    pub(crate) seals: Vec<ObligationEvidenceSeal>,
    pub(crate) optimization: Option<terminal_codec::PsiOptimizationExecutionRecord>,
}

impl AcceptedClosure {
    pub fn elaboration(&self) -> &Elaboration {
        &self.elaboration
    }

    pub fn discharge(&self) -> &Discharge {
        &self.discharge
    }

    pub fn optimization(&self) -> Option<&terminal_codec::PsiOptimizationExecutionRecord> {
        self.optimization.as_ref()
    }

    pub fn seals(&self) -> &[ObligationEvidenceSeal] {
        &self.seals
    }
}
