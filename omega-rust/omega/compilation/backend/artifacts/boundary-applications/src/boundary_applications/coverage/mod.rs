//! Reconstructible coverage identities joining demands to their realizations.
//!
//! The retained [`super::TerminalBoundaryApplicationCoverage`] carrier lives at
//! the representation root; this owner derives and replays the per-operation
//! identity references it stores.

use super::demands::{
    BoundaryApplication, BoundaryApplicationArgument, TerminalBoundaryApplicationDemand,
    TerminalBoundaryApplicationDemands,
};
use super::realization::{
    BoundaryApplicationRealization, BoundaryApplicationRealizationCompanion,
    TerminalBoundaryApplicationRealizations,
};
use semantic_vocabulary::OperationId;

const COVERAGE_IDENTITY_DOMAIN: &[u8] = b"omega.d29-operator-application-coverage.sha256.v2\0";

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
    terminal: terminal_psi::TerminalPsiIdentity,
    terminal_operation: OperationId,
    coverage: BoundaryApplicationCoverageIdentity,
}

impl OperatorApplicationCoverageRef {
    pub const fn terminal(&self) -> terminal_psi::TerminalPsiIdentity {
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
    terminal: terminal_psi::TerminalPsiIdentity,
    demand: &TerminalBoundaryApplicationDemand,
    companion: &BoundaryApplicationRealizationCompanion,
) -> BoundaryApplicationCoverageIdentity {
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(COVERAGE_IDENTITY_DOMAIN);
    digest.u16(terminal.vocabulary_marker.get());
    digest.raw(terminal.program_fingerprint.as_bytes());
    digest.u64(demand.terminal_operation().get());
    hash_bytes(
        &mut digest,
        &demand
            .requirement()
            .declaration()
            .identity()
            .0
            .to_le_bytes(),
    );
    hash_bytes(&mut digest, demand.requirement().overload().as_bytes());
    encode_application(&mut digest, demand.application());
    digest.raw(companion.selected_plan_digest());
    encode_realization(&mut digest, companion.realization());
    BoundaryApplicationCoverageIdentity(digest.finish().into())
}

fn encode_application(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    application: &BoundaryApplication,
) {
    match application {
        BoundaryApplication::Empty => digest.byte(0),
        BoundaryApplication::Exact {
            arguments,
            lifetimes,
        } => {
            digest.byte(1);
            digest.u64(arguments.len() as u64);
            for argument in arguments {
                match argument {
                    BoundaryApplicationArgument::Type {
                        binder_ordinal,
                        type_identity,
                    } => {
                        digest.byte(1);
                        digest.raw(&binder_ordinal.to_le_bytes());
                        hash_bytes(digest, &type_identity.identity().0.to_le_bytes());
                    }
                    BoundaryApplicationArgument::Const {
                        binder_ordinal,
                        declared_carrier,
                        value_type,
                        value_encoding,
                    }
                    | BoundaryApplicationArgument::Value {
                        binder_ordinal,
                        declared_carrier,
                        value_type,
                        value_encoding,
                    } => {
                        digest.byte(
                            if matches!(argument, BoundaryApplicationArgument::Const { .. }) {
                                2
                            } else {
                                3
                            },
                        );
                        digest.raw(&binder_ordinal.to_le_bytes());
                        hash_bytes(digest, &declared_carrier.identity().0.to_le_bytes());
                        hash_bytes(digest, value_type.as_bytes());
                        hash_bytes(digest, value_encoding.as_bytes());
                    }
                    BoundaryApplicationArgument::Declaration {
                        binder_ordinal,
                        kind,
                        declaration,
                        application,
                    } => {
                        digest.byte(4);
                        digest.raw(&binder_ordinal.to_le_bytes());
                        digest.byte(match kind {
                            terminal_psi::BoundaryDeclarationArgumentKind::Machine => 1,
                            terminal_psi::BoundaryDeclarationArgumentKind::Conformance => 2,
                            terminal_psi::BoundaryDeclarationArgumentKind::Proposition => 3,
                        });
                        hash_bytes(digest, &declaration.identity().0.to_le_bytes());
                        encode_application(digest, application);
                    }
                }
            }
            digest.u64(lifetimes.len() as u64);
            for binding in lifetimes {
                digest.raw(&binding.binder_ordinal.to_le_bytes());
                encode_lifetime_parameter(digest, &binding.parameter);
                match &binding.argument {
                    terminal_psi::BoundaryLifetimeArgument::Named(argument) => {
                        encode_lifetime_parameter(digest, argument);
                    }
                    terminal_psi::BoundaryLifetimeArgument::AnonymousInvocation => {
                        // Preserve named commitments and encode the explicit
                        // anonymous variant with the reserved zero coordinate.
                        hash_bytes(digest, &0u32.to_le_bytes());
                        digest.raw(&0u32.to_le_bytes());
                    }
                }
            }
        }
    }
}

fn encode_lifetime_parameter(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    parameter: &terminal_psi::BoundaryLifetimeParameter,
) {
    hash_bytes(digest, &parameter.declaration.identity().0.to_le_bytes());
    digest.raw(&parameter.parameter_ordinal.to_le_bytes());
}

fn encode_realization(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    realization: &BoundaryApplicationRealization,
) {
    match realization {
        BoundaryApplicationRealization::NongenericCheckedBody {
            realization_machine,
            realization_state,
            realization_contract_commitment,
        } => {
            digest.byte(0);
            hash_bytes(digest, &realization_machine.identity().0.to_le_bytes());
            hash_bytes(digest, &realization_state.identity().0.to_le_bytes());
            digest.raw(realization_contract_commitment);
        }
        BoundaryApplicationRealization::SpecializedCheckedBody {
            realization_template,
            realization_machine,
            realization_state,
            specialization_commitment,
            realization_contract_commitment,
        } => {
            digest.byte(1);
            hash_bytes(digest, &realization_template.identity().0.to_le_bytes());
            hash_bytes(digest, &realization_machine.identity().0.to_le_bytes());
            hash_bytes(digest, &realization_state.identity().0.to_le_bytes());
            digest.raw(specialization_commitment);
            digest.raw(realization_contract_commitment);
        }
        BoundaryApplicationRealization::ExactCompilerIntrinsic { execution } => {
            digest.byte(2);
            digest.raw(&effects::compiler_intrinsic_execution_identity_bytes(
                *execution,
            ));
        }
    }
}

fn hash_bytes(digest: &mut crate::digest_encoder::DomainDigestEncoder, bytes: &[u8]) {
    digest.bytes(bytes);
}

#[cfg(test)]
mod tests {
    use crate::{
        BoundaryApplication, BoundaryApplicationRealization,
        BoundaryApplicationRealizationCompanion, BoundaryOperatorRequirement,
        NominalDeclarationIdentity, TerminalBoundaryApplicationCoverage,
        TerminalBoundaryApplicationDemand, TerminalBoundaryApplicationDemands,
        TerminalBoundaryApplicationRealizations,
    };
    use semantic_vocabulary::OperationId;

    fn terminal() -> terminal_psi::TerminalPsiIdentity {
        terminal_psi::TerminalPsiIdentity {
            vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
            program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([7; 32]),
        }
    }

    #[test]
    fn coverage_identity_binds_operation_plan_and_role() {
        let operation = OperationId::new(1).unwrap();
        let demand = TerminalBoundaryApplicationDemand::new(
            operation,
            BoundaryOperatorRequirement::new(
                NominalDeclarationIdentity::new(terminal_psi::TerminalIdentityIndex(1)),
                "operator::call()->unit".to_owned(),
                None,
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
                    realization_machine: NominalDeclarationIdentity::new(
                        terminal_psi::TerminalIdentityIndex(2),
                    ),
                    realization_state: NominalDeclarationIdentity::new(
                        terminal_psi::TerminalIdentityIndex(3),
                    ),
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

    #[test]
    fn complete_coverage_distinguishes_exact_empty_from_external_absence() {
        let demands = TerminalBoundaryApplicationDemands::new(terminal(), Vec::new()).unwrap();
        let realizations =
            TerminalBoundaryApplicationRealizations::new(&demands, Vec::new()).unwrap();
        let coverage = TerminalBoundaryApplicationCoverage::new(demands, realizations).unwrap();
        coverage.validate_for_terminal(terminal()).unwrap();
        assert!(coverage.references().is_empty());
    }
}
