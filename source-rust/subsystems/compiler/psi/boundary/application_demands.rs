//! A native execution scope can select a subset of the published operation demands.
//! The collection keeps their Terminal identity and rejects two demands for one operation.

use crate::{TerminalBoundaryApplicationDemand, TerminalPsiIdentity};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalBoundaryApplicationDemands {
    terminal_psi: TerminalPsiIdentity,
    rows: std::sync::Arc<[TerminalBoundaryApplicationDemand]>,
}

impl TerminalBoundaryApplicationDemands {
    pub fn new(
        terminal_psi: TerminalPsiIdentity,
        mut rows: Vec<TerminalBoundaryApplicationDemand>,
    ) -> Result<Self, &'static str> {
        rows.sort_by_key(|row| row.terminal_operation().get());
        if rows
            .windows(2)
            .any(|pair| pair[0].terminal_operation() == pair[1].terminal_operation())
        {
            return Err("boundary application demands repeat a Terminal operation");
        }
        Ok(Self {
            terminal_psi,
            rows: rows.into(),
        })
    }

    pub fn validate_for_terminal(
        &self,
        terminal_psi: TerminalPsiIdentity,
    ) -> Result<(), &'static str> {
        if self.terminal_psi != terminal_psi {
            return Err("boundary application demands belong to different Terminal semantics");
        }
        Ok(())
    }

    pub const fn terminal_psi(&self) -> TerminalPsiIdentity {
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
    use crate::{BoundaryApplication, BoundaryApplicationArgument, BoundaryTypeIdentity};

    #[test]
    fn exact_application_requires_nonempty_contiguous_tagged_arguments() {
        assert!(BoundaryApplication::exact(Vec::new()).is_err());
        assert!(
            BoundaryApplication::exact(vec![BoundaryApplicationArgument::type_argument(
                1,
                BoundaryTypeIdentity::new(crate::TerminalIdentityIndex(4)),
            )])
            .is_err()
        );
        let application = BoundaryApplication::exact(vec![
            BoundaryApplicationArgument::type_argument(
                0,
                BoundaryTypeIdentity::new(crate::TerminalIdentityIndex(4)),
            ),
            BoundaryApplicationArgument::const_argument(
                1,
                BoundaryTypeIdentity::new(crate::TerminalIdentityIndex(5)),
                "u64".to_owned(),
                "integer(u64,4)".to_owned(),
            )
            .unwrap(),
        ])
        .unwrap();
        assert_eq!(application.arguments().len(), 2);
    }

    #[test]
    fn scoped_demands_sort_unique_operations_and_bind_their_terminal_identity() {
        use crate::{
            BoundaryOperatorRequirement, NominalDeclarationIdentity, SemanticFingerprint,
            TerminalBoundaryApplicationDemand, TerminalBoundaryApplicationDemands,
            TerminalIdentityIndex, TerminalPsiIdentity, VocabularyMarker,
        };
        use semantic_vocabulary::OperationId;
        let identity = |byte| TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([byte; 32]),
        };
        let demand = |index| {
            TerminalBoundaryApplicationDemand::new(
                OperationId::new(index).unwrap(),
                BoundaryOperatorRequirement::new(
                    NominalDeclarationIdentity::new(TerminalIdentityIndex(1)),
                    "operator::call()->unit".to_owned(),
                    None,
                )
                .unwrap(),
                BoundaryApplication::Empty,
            )
        };
        let demands =
            TerminalBoundaryApplicationDemands::new(identity(1), vec![demand(2), demand(1)])
                .unwrap();
        assert_eq!(demands.rows(), &[demand(1), demand(2)]);
        assert!(demands.validate_for_terminal(identity(1)).is_ok());
        assert!(demands.validate_for_terminal(identity(2)).is_err());
        assert!(
            TerminalBoundaryApplicationDemands::new(
                identity(1),
                vec![demand(1), demand(2), demand(1)]
            )
            .is_err()
        );
    }
}
