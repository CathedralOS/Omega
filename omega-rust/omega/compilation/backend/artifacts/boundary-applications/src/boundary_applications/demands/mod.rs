//! Artifact-bound view of Terminal's canonical boundary application demands.

pub use terminal_psi::{
    BoundaryApplication, BoundaryApplicationArgument, BoundaryOperatorRequirement,
    BoundaryTypeIdentity, NominalDeclarationIdentity, TerminalBoundaryApplicationDemand,
};

/// Complete source-free D29 demand projection for one Terminal artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalBoundaryApplicationDemands {
    terminal_psi: terminal_psi::TerminalPsiIdentity,
    rows: Vec<TerminalBoundaryApplicationDemand>,
}

impl TerminalBoundaryApplicationDemands {
    pub fn new(
        terminal_psi: terminal_psi::TerminalPsiIdentity,
        mut rows: Vec<TerminalBoundaryApplicationDemand>,
    ) -> Result<Self, &'static str> {
        rows.sort_by_key(|row| row.terminal_operation().get());
        if rows
            .windows(2)
            .any(|pair| pair[0].terminal_operation() == pair[1].terminal_operation())
        {
            return Err("boundary application demands repeat a Terminal operation");
        }
        Ok(Self { terminal_psi, rows })
    }

    pub fn validate_for_terminal(
        &self,
        terminal_psi: terminal_psi::TerminalPsiIdentity,
    ) -> Result<(), &'static str> {
        if self.terminal_psi != terminal_psi {
            return Err("boundary application demands belong to different Terminal semantics");
        }
        Ok(())
    }

    pub const fn terminal_psi(&self) -> terminal_psi::TerminalPsiIdentity {
        self.terminal_psi
    }

    pub fn rows(&self) -> &[TerminalBoundaryApplicationDemand] {
        &self.rows
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::{BoundaryApplication, BoundaryApplicationArgument, BoundaryTypeIdentity};

    #[test]
    fn exact_application_requires_nonempty_contiguous_tagged_arguments() {
        assert!(BoundaryApplication::exact(Vec::new()).is_err());
        assert!(
            BoundaryApplication::exact(vec![BoundaryApplicationArgument::type_argument(
                1,
                BoundaryTypeIdentity::new("type".to_owned()).unwrap(),
            )])
            .is_err()
        );
        let application = BoundaryApplication::exact(vec![
            BoundaryApplicationArgument::type_argument(
                0,
                BoundaryTypeIdentity::new("type".to_owned()).unwrap(),
            ),
            BoundaryApplicationArgument::const_argument(
                1,
                BoundaryTypeIdentity::new("u64".to_owned()).unwrap(),
                "u64".to_owned(),
                "integer(u64,4)".to_owned(),
            )
            .unwrap(),
        ])
        .unwrap();
        assert_eq!(application.arguments().len(), 2);
    }
}
