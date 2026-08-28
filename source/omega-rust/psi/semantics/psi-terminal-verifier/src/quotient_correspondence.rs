//! Independent replay of the standalone quotient-correspondence bridge.

use std::collections::BTreeSet;

use psi_language_semantics::quotient_correspondence::{
    CanonicalQuotientCorrespondence, QuotientContractFactCoordinate, QuotientContractOwner,
    QuotientCorrespondenceOperationKind, QuotientPositionalRelation, QuotientTheoremParameter,
    QuotientTheoremParameterRole,
};
use psi_terminal::{QuotientCorrespondenceIdentity, RetainedQuotientCorrespondence};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuotientCorrespondenceReplayError {
    UnsupportedOperationKind,
    NonHermeticIdentity,
    NonEmptyStaticApplication,
    CallableIdentityCollision,
    InvalidRelationIdentity,
    RuntimeArityMismatch,
    NonCanonicalRuntimePosition {
        expected: u32,
    },
    TheoremParameterMismatch,
    TheoremRelationPremiseMismatch,
    NonEmptyLegalityPremises,
    DuplicateTheoremCoordinate,
    TheoremConclusionMismatch,
    InvalidResultFlow,
    IdentityMismatch {
        expected: QuotientCorrespondenceIdentity,
        actual: QuotientCorrespondenceIdentity,
    },
}

pub fn replay_non_executable_quotient_correspondence(
    retained: &RetainedQuotientCorrespondence,
) -> Result<(), QuotientCorrespondenceReplayError> {
    let certificate = &retained.certificate;
    if certificate.operation_kind != QuotientCorrespondenceOperationKind::Define {
        return Err(QuotientCorrespondenceReplayError::UnsupportedOperationKind);
    }
    validate_callable(&certificate.public_operation)?;
    validate_callable(&certificate.representative.callable)?;
    validate_callable(&certificate.selected_theorem.callable)?;
    if !certificate
        .representative
        .static_application
        .bindings
        .is_empty()
        || !certificate
            .selected_theorem
            .static_application
            .bindings
            .is_empty()
    {
        return Err(QuotientCorrespondenceReplayError::NonEmptyStaticApplication);
    }
    if certificate.public_operation == certificate.representative.callable
        || certificate.public_operation == certificate.selected_theorem.callable
        || certificate.representative.callable == certificate.selected_theorem.callable
    {
        return Err(QuotientCorrespondenceReplayError::CallableIdentityCollision);
    }

    for relation in &certificate.input_relations {
        match relation {
            QuotientPositionalRelation::Quotient(relation) => validate_relation(relation)?,
            QuotientPositionalRelation::ExactEquality {
                public_type,
                representative_type,
            } if valid_type(public_type)
                && valid_type(representative_type)
                && public_type == representative_type => {}
            QuotientPositionalRelation::ExactEquality { .. } => {
                return Err(QuotientCorrespondenceReplayError::InvalidRelationIdentity);
            }
        }
    }
    validate_relation(&certificate.result_relation)?;

    if certificate.runtime_positions.len() != certificate.input_relations.len() {
        return Err(QuotientCorrespondenceReplayError::RuntimeArityMismatch);
    }
    for (position, runtime) in certificate.runtime_positions.iter().enumerate() {
        let expected = u32::try_from(position)
            .map_err(|_| QuotientCorrespondenceReplayError::RuntimeArityMismatch)?;
        if runtime.public_position != expected || runtime.representative_position != expected {
            return Err(
                QuotientCorrespondenceReplayError::NonCanonicalRuntimePosition { expected },
            );
        }
    }

    let (parameters, left_arguments, right_arguments, relation_rows) =
        expected_theorem_shape(&certificate.input_relations)?;
    if certificate.theorem.parameters != parameters {
        return Err(QuotientCorrespondenceReplayError::TheoremParameterMismatch);
    }
    if certificate.theorem.relation_premises.len() != relation_rows.len() {
        return Err(QuotientCorrespondenceReplayError::TheoremRelationPremiseMismatch);
    }
    let mut coordinates = BTreeSet::new();
    for (actual, expected) in certificate
        .theorem
        .relation_premises
        .iter()
        .zip(relation_rows)
    {
        if actual.expected_position != expected.0
            || actual.relation != expected.1
            || actual.left_parameter != expected.2
            || actual.right_parameter != expected.3
        {
            return Err(QuotientCorrespondenceReplayError::TheoremRelationPremiseMismatch);
        }
        if !coordinates.insert(actual.actual) {
            return Err(QuotientCorrespondenceReplayError::DuplicateTheoremCoordinate);
        }
    }
    if !certificate.theorem.legality_premises.is_empty() {
        return Err(QuotientCorrespondenceReplayError::NonEmptyLegalityPremises);
    }
    if !coordinates.insert(certificate.theorem.conclusion.actual) {
        return Err(QuotientCorrespondenceReplayError::DuplicateTheoremCoordinate);
    }
    if certificate.theorem.conclusion.relation != certificate.result_relation.relation
        || certificate.theorem.conclusion.left.arguments != left_arguments
        || certificate.theorem.conclusion.right.arguments != right_arguments
    {
        return Err(QuotientCorrespondenceReplayError::TheoremConclusionMismatch);
    }
    if certificate.result_flow.state_position != 0 {
        return Err(QuotientCorrespondenceReplayError::InvalidResultFlow);
    }

    let expected = independent_identity(certificate);
    if retained.identity != expected {
        return Err(QuotientCorrespondenceReplayError::IdentityMismatch {
            expected,
            actual: retained.identity.clone(),
        });
    }
    Ok(())
}

fn expected_theorem_shape(
    relations: &[QuotientPositionalRelation],
) -> Result<
    (
        Vec<QuotientTheoremParameter>,
        Vec<u32>,
        Vec<u32>,
        Vec<(u32, String, u32, u32)>,
    ),
    QuotientCorrespondenceReplayError,
> {
    let mut parameters = Vec::new();
    let mut left = Vec::with_capacity(relations.len());
    let mut right = Vec::with_capacity(relations.len());
    let mut premises = Vec::new();
    for (input, relation) in relations.iter().enumerate() {
        let input_position = u32::try_from(input)
            .map_err(|_| QuotientCorrespondenceReplayError::TheoremParameterMismatch)?;
        match relation {
            QuotientPositionalRelation::Quotient(relation) => {
                let left_parameter = u32::try_from(parameters.len())
                    .map_err(|_| QuotientCorrespondenceReplayError::TheoremParameterMismatch)?;
                parameters.push(QuotientTheoremParameter {
                    theorem_position: left_parameter,
                    role: QuotientTheoremParameterRole::QuotientLeft { input_position },
                });
                let right_parameter = u32::try_from(parameters.len())
                    .map_err(|_| QuotientCorrespondenceReplayError::TheoremParameterMismatch)?;
                parameters.push(QuotientTheoremParameter {
                    theorem_position: right_parameter,
                    role: QuotientTheoremParameterRole::QuotientRight { input_position },
                });
                left.push(left_parameter);
                right.push(right_parameter);
                premises.push((
                    u32::try_from(premises.len()).map_err(|_| {
                        QuotientCorrespondenceReplayError::TheoremRelationPremiseMismatch
                    })?,
                    relation.relation.clone(),
                    left_parameter,
                    right_parameter,
                ));
            }
            QuotientPositionalRelation::ExactEquality { .. } => {
                let shared = u32::try_from(parameters.len())
                    .map_err(|_| QuotientCorrespondenceReplayError::TheoremParameterMismatch)?;
                parameters.push(QuotientTheoremParameter {
                    theorem_position: shared,
                    role: QuotientTheoremParameterRole::Shared { input_position },
                });
                left.push(shared);
                right.push(shared);
            }
        }
    }
    Ok((parameters, left, right, premises))
}

fn validate_callable(
    callable: &psi_language_semantics::quotient_correspondence::QuotientCallableIdentity,
) -> Result<(), QuotientCorrespondenceReplayError> {
    if !hermetic(&callable.declaration) || callable.overload.is_empty() {
        return Err(QuotientCorrespondenceReplayError::NonHermeticIdentity);
    }
    Ok(())
}

fn validate_relation(
    relation: &psi_language_semantics::quotient_correspondence::QuotientRelationIdentity,
) -> Result<(), QuotientCorrespondenceReplayError> {
    if !hermetic(&relation.quotient_declaration)
        || !hermetic(&relation.relation)
        || !valid_type(&relation.quotient_type)
        || !valid_type(&relation.carrier_type)
        || relation.quotient_type == relation.carrier_type
    {
        return Err(QuotientCorrespondenceReplayError::InvalidRelationIdentity);
    }
    Ok(())
}

fn hermetic(identity: &str) -> bool {
    identity.starts_with("package:") || identity.starts_with("toolchain::")
}

fn valid_type(identity: &str) -> bool {
    !identity.is_empty() && !identity.contains("unresolved-owner")
}

fn independent_identity(
    certificate: &CanonicalQuotientCorrespondence,
) -> QuotientCorrespondenceIdentity {
    let mut writer = ReplayIdentityWriter::new();
    writer.string("omega.quotient-correspondence.total-direct-define.v1");
    writer.byte(match certificate.operation_kind {
        QuotientCorrespondenceOperationKind::Lift => 1,
        QuotientCorrespondenceOperationKind::Define => 2,
    });
    writer.callable(&certificate.public_operation);
    writer.application(&certificate.representative);
    writer.application(&certificate.selected_theorem);
    writer.len(certificate.input_relations.len());
    for relation in &certificate.input_relations {
        match relation {
            QuotientPositionalRelation::Quotient(relation) => {
                writer.byte(1);
                writer.relation(relation);
            }
            QuotientPositionalRelation::ExactEquality {
                public_type,
                representative_type,
            } => {
                writer.byte(2);
                writer.string(public_type);
                writer.string(representative_type);
            }
        }
    }
    writer.relation(&certificate.result_relation);
    writer.len(certificate.runtime_positions.len());
    for position in &certificate.runtime_positions {
        writer.u32(position.public_position);
        writer.u32(position.representative_position);
    }
    writer.len(certificate.theorem.parameters.len());
    for parameter in &certificate.theorem.parameters {
        writer.u32(parameter.theorem_position);
        match parameter.role {
            QuotientTheoremParameterRole::QuotientLeft { input_position } => {
                writer.byte(1);
                writer.u32(input_position);
            }
            QuotientTheoremParameterRole::QuotientRight { input_position } => {
                writer.byte(2);
                writer.u32(input_position);
            }
            QuotientTheoremParameterRole::Shared { input_position } => {
                writer.byte(3);
                writer.u32(input_position);
            }
        }
    }
    writer.len(certificate.theorem.relation_premises.len());
    for premise in &certificate.theorem.relation_premises {
        writer.u32(premise.expected_position);
        writer.coordinate(premise.actual);
        writer.string(&premise.relation);
        writer.u32(premise.left_parameter);
        writer.u32(premise.right_parameter);
    }
    writer.len(certificate.theorem.legality_premises.len());
    for premise in &certificate.theorem.legality_premises {
        writer.coordinate(*premise);
    }
    writer.coordinate(certificate.theorem.conclusion.actual);
    writer.string(&certificate.theorem.conclusion.relation);
    writer.len(certificate.theorem.conclusion.left.arguments.len());
    for argument in &certificate.theorem.conclusion.left.arguments {
        writer.u32(*argument);
    }
    writer.len(certificate.theorem.conclusion.right.arguments.len());
    for argument in &certificate.theorem.conclusion.right.arguments {
        writer.u32(*argument);
    }
    writer.byte(1);
    writer.byte(1);
    writer.byte(1);
    writer.byte(1);
    writer.byte(1);
    writer.u32(certificate.result_flow.state_position);
    writer.u32(certificate.result_flow.statement_position);
    QuotientCorrespondenceIdentity(writer.finish())
}

struct ReplayIdentityWriter {
    bytes: Vec<u8>,
}

impl ReplayIdentityWriter {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }
    fn finish(self) -> Vec<u8> {
        self.bytes
    }
    fn bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }
    fn byte(&mut self, value: u8) {
        self.bytes(&[value]);
    }
    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }
    fn len(&mut self, value: usize) {
        self.bytes(&(value as u64).to_le_bytes());
    }
    fn string(&mut self, value: &str) {
        self.len(value.len());
        self.bytes(value.as_bytes());
    }
    fn callable(
        &mut self,
        callable: &psi_language_semantics::quotient_correspondence::QuotientCallableIdentity,
    ) {
        self.string(&callable.declaration);
        self.string(&callable.overload);
    }
    fn application(
        &mut self,
        application: &psi_language_semantics::quotient_correspondence::QuotientMachineApplication,
    ) {
        self.callable(&application.callable);
        self.len(application.static_application.bindings.len());
        for binding in &application.static_application.bindings {
            self.string(binding);
        }
    }
    fn relation(
        &mut self,
        relation: &psi_language_semantics::quotient_correspondence::QuotientRelationIdentity,
    ) {
        self.string(&relation.quotient_declaration);
        self.string(&relation.quotient_type);
        self.string(&relation.carrier_type);
        self.string(&relation.relation);
    }
    fn coordinate(&mut self, coordinate: QuotientContractFactCoordinate) {
        self.byte(match coordinate.owner {
            QuotientContractOwner::Machine => 1,
            QuotientContractOwner::State => 2,
        });
        self.u32(coordinate.contract_position);
        self.u32(coordinate.fact_position);
    }
}

#[cfg(test)]
mod tests {
    use psi_core::{BlockId, ContractId, EdgeId, MachineId};
    use psi_language_semantics::quotient_correspondence::{
        CanonicalQuotientCorrespondence, QuotientCallableIdentity, QuotientContractFactCoordinate,
        QuotientContractOwner, QuotientCorrespondenceOperationKind, QuotientCrashCertificate,
        QuotientDefineRuntimePosition, QuotientDirectResultFlow, QuotientMachineApplication,
        QuotientPositionalRelation, QuotientPurityCertificate, QuotientRelationIdentity,
        QuotientRepresentativeApplication, QuotientRepresentativeEligibility,
        QuotientStaticApplication, QuotientTerminationCertificate, QuotientTheoremConclusion,
        QuotientTheoremCorrespondence, QuotientTheoremEligibility, QuotientTheoremParameter,
        QuotientTheoremParameterRole, QuotientTheoremRelationPremise,
    };
    use psi_terminal::{
        Block, MachineContract, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
        VocabularyMarker, retain_non_executable_quotient_correspondence,
    };

    use super::*;

    fn callable(name: &str) -> QuotientCallableIdentity {
        QuotientCallableIdentity {
            declaration: format!("package:{}::{name}", "01".repeat(32)),
            overload: format!("named-callable:{name}"),
        }
    }

    fn relation(name: &str) -> QuotientRelationIdentity {
        QuotientRelationIdentity {
            quotient_declaration: format!("package:{}::{name}", "02".repeat(32)),
            quotient_type: format!("package:{}::{name}", "02".repeat(32)),
            carrier_type: format!("package:{}::{name}Carrier", "03".repeat(32)),
            relation: format!("package:{}::{name}Relation", "04".repeat(32)),
        }
    }

    fn coordinate(fact_position: u32) -> QuotientContractFactCoordinate {
        QuotientContractFactCoordinate {
            owner: QuotientContractOwner::State,
            contract_position: 0,
            fact_position,
        }
    }

    fn certificate() -> CanonicalQuotientCorrespondence {
        let input = relation("Value");
        let result = relation("Result");
        CanonicalQuotientCorrespondence {
            operation_kind: QuotientCorrespondenceOperationKind::Define,
            public_operation: callable("Public::apply"),
            representative: QuotientMachineApplication {
                callable: callable("Carrier::apply"),
                static_application: QuotientStaticApplication { bindings: vec![] },
            },
            selected_theorem: QuotientMachineApplication {
                callable: callable("apply_respects"),
                static_application: QuotientStaticApplication { bindings: vec![] },
            },
            input_relations: vec![
                QuotientPositionalRelation::Quotient(input.clone()),
                QuotientPositionalRelation::ExactEquality {
                    public_type: "u32@exact".to_owned(),
                    representative_type: "u32@exact".to_owned(),
                },
            ],
            result_relation: result.clone(),
            runtime_positions: vec![
                QuotientDefineRuntimePosition {
                    public_position: 0,
                    representative_position: 0,
                },
                QuotientDefineRuntimePosition {
                    public_position: 1,
                    representative_position: 1,
                },
            ],
            theorem: QuotientTheoremCorrespondence {
                parameters: vec![
                    QuotientTheoremParameter {
                        theorem_position: 0,
                        role: QuotientTheoremParameterRole::QuotientLeft { input_position: 0 },
                    },
                    QuotientTheoremParameter {
                        theorem_position: 1,
                        role: QuotientTheoremParameterRole::QuotientRight { input_position: 0 },
                    },
                    QuotientTheoremParameter {
                        theorem_position: 2,
                        role: QuotientTheoremParameterRole::Shared { input_position: 1 },
                    },
                ],
                relation_premises: vec![QuotientTheoremRelationPremise {
                    expected_position: 0,
                    actual: coordinate(0),
                    relation: input.relation,
                    left_parameter: 0,
                    right_parameter: 1,
                }],
                legality_premises: vec![],
                conclusion: QuotientTheoremConclusion {
                    actual: coordinate(1),
                    relation: result.relation,
                    left: QuotientRepresentativeApplication {
                        arguments: vec![0, 2],
                    },
                    right: QuotientRepresentativeApplication {
                        arguments: vec![1, 2],
                    },
                },
            },
            representative_eligibility: QuotientRepresentativeEligibility {
                purity: QuotientPurityCertificate::PureClosure,
                termination: QuotientTerminationCertificate::Unconditional,
            },
            theorem_eligibility: QuotientTheoremEligibility {
                purity: QuotientPurityCertificate::PureClosure,
                termination: QuotientTerminationCertificate::Unconditional,
                crash: QuotientCrashCertificate::CrashFree,
            },
            result_flow: QuotientDirectResultFlow {
                state_position: 0,
                statement_position: 7,
            },
        }
    }

    fn replayed(
        certificate: CanonicalQuotientCorrespondence,
    ) -> Result<(), QuotientCorrespondenceReplayError> {
        replay_non_executable_quotient_correspondence(
            &retain_non_executable_quotient_correspondence(certificate),
        )
    }

    fn module_with(
        quotient_correspondences: Vec<RetainedQuotientCorrespondence>,
    ) -> TerminalModule {
        let machine = MachineId::new(1).unwrap();
        TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine,
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
            closed_conformance_applications: Vec::new(),
            quotient_correspondences,
            machines: vec![TerminalMachine {
                id: machine,
                attachment: None,
                structural_parameters: Vec::new(),
                ranked_scc: None,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: Vec::new(),
                result: TerminalMachineResult::Unit,
                structural_places: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: BlockId::new(1).unwrap(),
                blocks: vec![Block {
                    id: BlockId::new(1).unwrap(),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: EdgeId::new(1).unwrap(),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: MachineContract {
                    id: ContractId::new(1).unwrap(),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        }
    }

    #[test]
    fn replays_total_direct_define_without_executable_authority() {
        assert_eq!(replayed(certificate()), Ok(()));
    }

    #[test]
    fn rejects_every_structural_drift_independently() {
        let mut changed = certificate();
        changed.operation_kind = QuotientCorrespondenceOperationKind::Lift;
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::UnsupportedOperationKind)
        );

        let mut changed = certificate();
        changed.public_operation.declaration = "local::Public".to_owned();
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::NonHermeticIdentity)
        );

        let mut changed = certificate();
        changed
            .representative
            .static_application
            .bindings
            .push("T=u8".to_owned());
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::NonEmptyStaticApplication)
        );

        let mut changed = certificate();
        changed.selected_theorem.callable = changed.representative.callable.clone();
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::CallableIdentityCollision)
        );

        let mut changed = certificate();
        let QuotientPositionalRelation::ExactEquality {
            representative_type,
            ..
        } = &mut changed.input_relations[1]
        else {
            unreachable!()
        };
        *representative_type = "u64@exact".to_owned();
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::InvalidRelationIdentity)
        );

        let mut changed = certificate();
        changed.runtime_positions.pop();
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::RuntimeArityMismatch)
        );

        let mut changed = certificate();
        changed.runtime_positions[1].representative_position = 0;
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::NonCanonicalRuntimePosition { expected: 1 })
        );

        let mut changed = certificate();
        changed.theorem.parameters.swap(0, 1);
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::TheoremParameterMismatch)
        );

        let mut changed = certificate();
        changed.theorem.relation_premises[0].right_parameter = 2;
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::TheoremRelationPremiseMismatch)
        );

        let mut changed = certificate();
        changed.theorem.legality_premises.push(coordinate(8));
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::NonEmptyLegalityPremises)
        );

        let mut changed = certificate();
        changed.theorem.conclusion.actual = coordinate(0);
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::DuplicateTheoremCoordinate)
        );

        let mut changed = certificate();
        changed.theorem.conclusion.left.arguments.swap(0, 1);
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::TheoremConclusionMismatch)
        );

        let mut changed = certificate();
        changed.result_flow.state_position = 1;
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::InvalidResultFlow)
        );

        let mut retained = retain_non_executable_quotient_correspondence(certificate());
        retained.identity.0.push(1);
        assert!(matches!(
            replay_non_executable_quotient_correspondence(&retained),
            Err(QuotientCorrespondenceReplayError::IdentityMismatch { .. })
        ));
    }

    #[test]
    fn length_delimited_identity_rejects_field_boundary_collisions() {
        let first = retain_non_executable_quotient_correspondence(certificate());
        let mut shifted = certificate();
        shifted.public_operation.declaration.push('x');
        shifted.public_operation.overload.remove(0);
        let shifted = retain_non_executable_quotient_correspondence(shifted);
        assert_ne!(first.identity, shifted.identity);
    }

    #[test]
    fn module_representation_replays_rows_but_execution_rejects_them() {
        let module = module_with(vec![retain_non_executable_quotient_correspondence(
            certificate(),
        )]);
        assert_eq!(crate::validate_module_representation(&module), Ok(()));
        assert_eq!(
            crate::validate_module(&module).unwrap_err(),
            crate::ModuleError::NonExecutableQuotientCorrespondence
        );

        let mut tampered = module;
        tampered.quotient_correspondences[0].identity.0.push(0);
        assert!(matches!(
            crate::validate_module_representation(&tampered),
            Err(crate::ModuleError::InvalidQuotientCorrespondence {
                error: QuotientCorrespondenceReplayError::IdentityMismatch { .. },
                ..
            })
        ));
    }

    #[test]
    fn module_representation_rejects_order_uniqueness_and_owner_collisions() {
        let first = retain_non_executable_quotient_correspondence(certificate());
        let duplicate = module_with(vec![first.clone(), first.clone()]);
        assert_eq!(
            crate::validate_module_representation(&duplicate),
            Err(crate::ModuleError::DuplicateQuotientCorrespondenceIdentity)
        );

        let mut second_certificate = certificate();
        second_certificate.public_operation = callable("Public::other");
        let second = retain_non_executable_quotient_correspondence(second_certificate);
        let mut ordered = vec![first.clone(), second];
        ordered.sort_by(|left, right| left.identity.cmp(&right.identity));
        let mut reversed = ordered;
        reversed.reverse();
        assert_eq!(
            crate::validate_module_representation(&module_with(reversed)),
            Err(crate::ModuleError::NonCanonicalQuotientCorrespondenceOrder)
        );

        let mut collision_certificate = certificate();
        collision_certificate.result_flow.statement_position += 1;
        let collision = retain_non_executable_quotient_correspondence(collision_certificate);
        let mut collided = vec![first, collision];
        collided.sort_by(|left, right| left.identity.cmp(&right.identity));
        assert_eq!(
            crate::validate_module_representation(&module_with(collided)),
            Err(crate::ModuleError::DuplicateQuotientCorrespondenceOwner)
        );
    }
}
