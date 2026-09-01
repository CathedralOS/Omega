use super::{
    BoundaryApplication, BoundaryApplicationArgument, BoundaryApplicationRealization,
    TerminalBoundaryApplicationDemand, TerminalBoundaryApplicationDemands,
    TerminalBoundaryApplicationRealizations,
};
use psi_core::OperationId;
use sha2::{Digest, Sha256};

const COVERAGE_IDENTITY_DOMAIN: &[u8] = b"omega.d29-operator-application-coverage.sha256.v1\0";

/// Domain-separated strong identity reconstructed from one complete D29
/// demand and its exact selected-plan/realization companion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoundaryApplicationCoverageIdentity([u8; 32]);

impl BoundaryApplicationCoverageIdentity {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Reference to independently reconstructible D29 semantic coverage.
///
/// This is the D29 branch later retained by a native physical child. It does
/// not duplicate the demand or companion and grants no physical realization
/// claim by itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OperatorApplicationCoverageRef {
    terminal: psi_terminal::TerminalPsiIdentity,
    terminal_operation: OperationId,
    coverage: BoundaryApplicationCoverageIdentity,
}

impl OperatorApplicationCoverageRef {
    pub const fn terminal(&self) -> psi_terminal::TerminalPsiIdentity {
        self.terminal
    }

    pub const fn terminal_operation(&self) -> OperationId {
        self.terminal_operation
    }

    pub const fn coverage(&self) -> BoundaryApplicationCoverageIdentity {
        self.coverage
    }
}

impl TerminalBoundaryApplicationRealizations {
    /// Reconstruct the complete canonical coverage-reference set. Demand and
    /// companion correspondence is replayed before any identity is returned.
    pub fn coverage_references(
        &self,
        demands: &TerminalBoundaryApplicationDemands,
    ) -> Result<Vec<OperatorApplicationCoverageRef>, &'static str> {
        self.validate_for_demands(demands)?;
        Ok(demands
            .rows()
            .iter()
            .zip(self.rows())
            .map(|(demand, realization)| OperatorApplicationCoverageRef {
                terminal: demands.terminal_psi(),
                terminal_operation: demand.terminal_operation(),
                coverage: coverage_identity(demands.terminal_psi(), demand, realization),
            })
            .collect())
    }
}

fn coverage_identity(
    terminal: psi_terminal::TerminalPsiIdentity,
    demand: &TerminalBoundaryApplicationDemand,
    companion: &super::BoundaryApplicationRealizationCompanion,
) -> BoundaryApplicationCoverageIdentity {
    let mut digest = Sha256::new();
    digest.update(COVERAGE_IDENTITY_DOMAIN);
    digest.update(terminal.vocabulary_marker.get().to_le_bytes());
    digest.update(terminal.program_fingerprint.as_bytes());
    digest.update(demand.terminal_operation().get().to_le_bytes());
    hash_bytes(
        &mut digest,
        demand.requirement().declaration().canonical().as_bytes(),
    );
    hash_bytes(&mut digest, demand.requirement().overload().as_bytes());
    encode_application(&mut digest, demand.application());
    digest.update(companion.selected_plan_digest());
    encode_realization(&mut digest, companion.realization());
    BoundaryApplicationCoverageIdentity(digest.finalize().into())
}

fn encode_application(digest: &mut Sha256, application: &BoundaryApplication) {
    match application {
        BoundaryApplication::Empty => digest.update([0]),
        BoundaryApplication::Exact(arguments) => {
            digest.update([1]);
            digest.update(canonical_usize(arguments.len()));
            for argument in arguments {
                match argument {
                    BoundaryApplicationArgument::Type {
                        binder_ordinal,
                        type_identity,
                    } => {
                        digest.update([0]);
                        digest.update(binder_ordinal.to_le_bytes());
                        hash_bytes(digest, type_identity.canonical().as_bytes());
                    }
                    BoundaryApplicationArgument::Const {
                        binder_ordinal,
                        declared_carrier,
                        value_type,
                        value_encoding,
                    } => {
                        digest.update([1]);
                        digest.update(binder_ordinal.to_le_bytes());
                        hash_bytes(digest, declared_carrier.canonical().as_bytes());
                        hash_bytes(digest, value_type.as_bytes());
                        hash_bytes(digest, value_encoding.as_bytes());
                    }
                }
            }
        }
    }
}

fn encode_realization(digest: &mut Sha256, realization: &BoundaryApplicationRealization) {
    match realization {
        BoundaryApplicationRealization::NongenericCheckedBody {
            realization_machine,
            realization_state,
            realization_contract_commitment,
        } => {
            digest.update([0]);
            hash_bytes(digest, realization_machine.canonical().as_bytes());
            hash_bytes(digest, realization_state.canonical().as_bytes());
            digest.update(realization_contract_commitment);
        }
        BoundaryApplicationRealization::SpecializedCheckedBody {
            realization_template,
            realization_machine,
            realization_state,
            specialization_commitment,
            realization_contract_commitment,
        } => {
            digest.update([1]);
            hash_bytes(digest, realization_template.canonical().as_bytes());
            hash_bytes(digest, realization_machine.canonical().as_bytes());
            hash_bytes(digest, realization_state.canonical().as_bytes());
            digest.update(specialization_commitment);
            digest.update(realization_contract_commitment);
        }
        BoundaryApplicationRealization::ExactCompilerIntrinsic { execution } => {
            digest.update([2]);
            digest.update(omega_effects::compiler_intrinsic_execution_identity_bytes(
                *execution,
            ));
        }
    }
}

fn hash_bytes(digest: &mut Sha256, bytes: &[u8]) {
    digest.update(canonical_usize(bytes.len()));
    digest.update(bytes);
}

fn canonical_usize(value: usize) -> [u8; 8] {
    u64::try_from(value)
        .expect("canonical D29 identity length fits u64")
        .to_le_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BoundaryApplicationRealizationCompanion, BoundaryNominalIdentity,
        BoundaryOperatorRequirement,
    };

    fn terminal() -> psi_terminal::TerminalPsiIdentity {
        psi_terminal::TerminalPsiIdentity {
            vocabulary_marker: psi_terminal::VocabularyMarker::CURRENT,
            program_fingerprint: psi_terminal::SemanticFingerprint::from_bytes([7; 32]),
        }
    }

    #[test]
    fn coverage_identity_binds_operation_plan_and_role() {
        let operation = OperationId::new(1).unwrap();
        let demand = TerminalBoundaryApplicationDemand::new(
            operation,
            BoundaryOperatorRequirement::new(
                BoundaryNominalIdentity::new("package:operator".to_owned()).unwrap(),
                "operator::call()->unit".to_owned(),
            )
            .unwrap(),
            BoundaryApplication::Empty,
        );
        let demands = TerminalBoundaryApplicationDemands::new(terminal(), vec![demand]).unwrap();
        let checked = |plan| {
            BoundaryApplicationRealizationCompanion::new(
                operation,
                plan,
                BoundaryApplicationRealization::NongenericCheckedBody {
                    realization_machine: BoundaryNominalIdentity::new("machine".to_owned())
                        .unwrap(),
                    realization_state: BoundaryNominalIdentity::new("machine::entry".to_owned())
                        .unwrap(),
                    realization_contract_commitment: [3; 32],
                },
            )
            .unwrap()
        };
        let first = TerminalBoundaryApplicationRealizations::new(&demands, vec![checked([1; 32])])
            .unwrap()
            .coverage_references(&demands)
            .unwrap();
        let changed =
            TerminalBoundaryApplicationRealizations::new(&demands, vec![checked([2; 32])])
                .unwrap()
                .coverage_references(&demands)
                .unwrap();
        assert_ne!(first[0].coverage(), changed[0].coverage());
    }
}
