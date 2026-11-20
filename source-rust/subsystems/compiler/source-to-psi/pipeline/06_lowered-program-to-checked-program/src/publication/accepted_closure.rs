//! The checker owns construction of the checked program and its evidence.
use crate::checking::Discharge;
use terminal_check_tables::program::Elaboration;
use terminal_codec::ObligationEvidenceSeal;

/// A closure the route accepted.
#[derive(Debug)]
pub struct AcceptedClosure {
    pub(crate) elaboration: Elaboration,
    pub(crate) discharge: Discharge,
    pub(crate) seals: Vec<ObligationEvidenceSeal>,
}

impl AcceptedClosure {
    pub fn elaboration(&self) -> &Elaboration {
        &self.elaboration
    }

    pub fn discharge(&self) -> &Discharge {
        &self.discharge
    }

    /// Scalar comparisons established on borrowed storage at every normal return of the entry.
    pub fn returned_scalars(&self) -> &[crate::checking::ReturnedScalarFact] {
        let Some(semantic_vocabulary::TargetMachineCallable::Machine(id)) =
            self.elaboration.module.entry
        else {
            return &[];
        };
        self.discharge
            .machines
            .get(id)
            .map_or(&[], |row| &row.returned_scalars)
    }

    pub fn seals(&self) -> &[ObligationEvidenceSeal] {
        &self.seals
    }
}
