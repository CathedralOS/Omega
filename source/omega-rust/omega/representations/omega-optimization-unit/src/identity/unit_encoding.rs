//! Canonical optimization-unit, fact, ownership, and CFG custody walk.

use super::operation_encoding::*;
use super::structural_encoding::*;
use super::*;

const UNIT_IDENTITY_DOMAIN: &[u8] = b"omega.psi-optimization-unit-content.v20\0";
const STRUCTURAL_DOMAIN_CATALOG_IDENTITY_DOMAIN: &[u8] =
    b"omega.psi-optimization-structural-domain-catalog.v1\0";

pub fn structural_domain_catalog_identity(
    domains: &[StructuralDomainDeclaration],
) -> OptimizationUnitIdentity {
    let mut bytes = CanonicalBytes::default();
    bytes.bytes(STRUCTURAL_DOMAIN_CATALOG_IDENTITY_DOMAIN);
    bytes.slice(domains, encode_structural_domain);
    OptimizationUnitIdentity::from_canonical_bytes(&bytes.finish())
}

pub fn recompute_psi_optimization_unit_identity(
    unit: &PsiOptimizationUnit,
) -> OptimizationUnitIdentity {
    let mut bytes = CanonicalBytes::default();
    bytes.bytes(UNIT_IDENTITY_DOMAIN);
    bytes.u16(unit.psi.vocabulary_marker.get());
    bytes.bytes(unit.psi.program_fingerprint.as_bytes());
    bytes.u32(unit.fuel_schedule.marker());
    bytes.id(unit.entry);
    bytes.slice(&unit.structural_types, encode_structural_type);
    bytes.slice(unit.structural_domains.as_ref(), encode_structural_domain);
    bytes.slice(unit.services.as_ref(), encode_service_declaration);
    encode_root_service_reach(&mut bytes, &unit.root_service_reach);
    bytes.slice(&unit.boundary_machines, encode_boundary_machine);
    bytes.slice(&unit.provider_candidates, encode_provider_candidate);
    bytes.slice(&unit.accepted_obligation_facts, encode_accepted_fact);
    bytes.slice(&unit.proof_questions, encode_proof_question);
    bytes.slice(
        &unit.ownership_frontier_facts,
        encode_ownership_frontier_fact,
    );
    bytes.slice(&unit.pruned_machines, |bytes, custody| {
        bytes.id(custody.machine);
        bytes.u32(custody.source_ordinal);
    });
    bytes.slice(&unit.functions, encode_function);
    OptimizationUnitIdentity::from_canonical_bytes(&bytes.finish())
}

fn encode_service_declaration(bytes: &mut CanonicalBytes, service: &ServiceDeclaration) {
    bytes.id(service.id);
    bytes.string(&service.identity);
    encode_ids(bytes, &service.parents);
}

fn encode_root_service_reach(bytes: &mut CanonicalBytes, reach: &TerminalRootServiceReach) {
    encode_ids(bytes, &reach.concrete);
    bytes.slice(&reach.installation_dependencies, |bytes, dependency| {
        bytes.string(&dependency.requirement_identity);
        encode_ids(bytes, &dependency.upper_bound);
    });
}

#[derive(Default)]
pub(super) struct CanonicalBytes(Vec<u8>);

impl CanonicalBytes {
    pub(super) fn finish(self) -> Vec<u8> {
        self.0
    }

    pub(super) fn bytes(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }

    pub(super) fn u8(&mut self, value: u8) {
        self.0.push(value);
    }

    pub(super) fn boolean(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    pub(super) fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }

    pub(super) fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    pub(super) fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    pub(super) fn u128(&mut self, value: u128) {
        self.bytes(&value.to_le_bytes());
    }

    pub(super) fn id(&mut self, value: impl PsiSemanticId) {
        self.u64(value.get());
    }

    pub(super) fn len(&mut self, len: usize) {
        self.u64(u64::try_from(len).expect("canonical optimization-unit length fits u64"));
    }

    pub(super) fn string(&mut self, value: &str) {
        self.len(value.len());
        self.bytes(value.as_bytes());
    }

    pub(super) fn slice<T>(&mut self, values: &[T], encode: impl Fn(&mut Self, &T)) {
        self.len(values.len());
        for value in values {
            encode(self, value);
        }
    }
}

fn encode_accepted_fact(bytes: &mut CanonicalBytes, fact: &AcceptedObligationFact) {
    bytes.bytes(&fact.identity.bytes());
    bytes.u16(fact.psi.vocabulary_marker.get());
    bytes.bytes(fact.psi.program_fingerprint.as_bytes());
    bytes.bytes(&fact.proof_bundle_fingerprint);
    bytes.id(fact.machine);
    bytes.id(fact.operation);
    bytes.id(fact.obligation);
    bytes.len(fact.proposition.len());
    bytes.bytes(&fact.proposition);
}

fn encode_proof_question(bytes: &mut CanonicalBytes, question: &ProofQuestion) {
    bytes.bytes(&question.identity.bytes());
    bytes.u16(question.terminal_psi.vocabulary_marker.get());
    bytes.bytes(question.terminal_psi.program_fingerprint.as_bytes());
    bytes.bytes(&question.proof_bundle_fingerprint);
    match question.owner {
        ProofQuestionOwner::Operation { machine, operation } => {
            bytes.u8(1);
            bytes.id(machine);
            bytes.id(operation);
        }
        ProofQuestionOwner::CallRequires {
            machine,
            operation,
            requirement_position,
        } => {
            bytes.u8(2);
            bytes.id(machine);
            bytes.id(operation);
            bytes.u32(requirement_position);
        }
        ProofQuestionOwner::NominalCleanupRequires {
            machine,
            edge,
            cleanup_position,
            requirement_position,
        } => {
            bytes.u8(3);
            bytes.id(machine);
            bytes.id(edge);
            bytes.u32(cleanup_position);
            bytes.u32(requirement_position);
        }
        ProofQuestionOwner::ContractEnsures {
            machine,
            contract,
            clause_position,
        } => {
            bytes.u8(4);
            bytes.id(machine);
            bytes.id(contract);
            bytes.u32(clause_position);
        }
    }
    bytes.id(question.obligation);
    match question.class {
        ProofQuestionClass::Derivable => bytes.u8(1),
        ProofQuestionClass::AdmissionAuthorized {
            site,
            kind,
            authority_identity,
        } => {
            bytes.u8(2);
            bytes.id(site);
            bytes.u8(match kind {
                ProofQuestionAdmissionKind::ForeignBoundaryGuarantee => 1,
                ProofQuestionAdmissionKind::ProviderFact => 2,
                ProofQuestionAdmissionKind::CheckedAssemblyClaim => 3,
            });
            bytes.id(authority_identity);
        }
    }
    bytes.len(question.proposition.len());
    bytes.bytes(&question.proposition);
    bytes.slice(&question.requirements, |bytes, proposition| {
        bytes.len(proposition.len());
        bytes.bytes(proposition);
    });
    bytes.slice(&question.semantic_axioms, |bytes, proposition| {
        bytes.len(proposition.len());
        bytes.bytes(proposition);
    });
    bytes.boolean(question.canonical_certificate);
}

fn encode_ownership_frontier_fact(bytes: &mut CanonicalBytes, fact: &OwnershipFrontierFact) {
    bytes.bytes(&fact.identity.bytes());
    bytes.u16(fact.psi.vocabulary_marker.get());
    bytes.bytes(fact.psi.program_fingerprint.as_bytes());
    bytes.id(fact.machine);
    match fact.site {
        OwnershipFrontierSite::BlockEntry(id) => {
            bytes.u8(1);
            bytes.id(id);
        }
        OwnershipFrontierSite::OperationEntry(id) => {
            bytes.u8(2);
            bytes.id(id);
        }
        OwnershipFrontierSite::OperationExit(id) => {
            bytes.u8(3);
            bytes.id(id);
        }
        OwnershipFrontierSite::EdgeEntry(id) => {
            bytes.u8(4);
            bytes.id(id);
        }
        OwnershipFrontierSite::EdgeExit(id) => {
            bytes.u8(5);
            bytes.id(id);
        }
    }
    encode_ownership_frontier_snapshot(bytes, &fact.snapshot);
}

fn encode_ownership_frontier_snapshot(
    bytes: &mut CanonicalBytes,
    snapshot: &OwnershipFrontierSnapshot,
) {
    bytes.slice(&snapshot.claims, |bytes, claim| {
        bytes.id(claim.claim);
        encode_optional(bytes, claim.input.as_ref(), |bytes, input| bytes.id(*input));
        bytes.slice(&claim.path, encode_structural_path_segment);
        encode_optional(bytes, claim.multiplicity.as_ref(), |bytes, multiplicity| {
            encode_multiplicity(bytes, *multiplicity)
        });
    });
    bytes.slice(&snapshot.owned_places, |bytes, place| {
        bytes.id(place.place);
        encode_multiplicity(bytes, place.multiplicity);
    });
    bytes.slice(&snapshot.partial_custody, |bytes, partial| {
        bytes.id(partial.place);
        bytes.slice(&partial.moved_paths, |bytes, path| {
            bytes.slice(path, encode_structural_path_segment)
        });
    });
}

fn encode_function(bytes: &mut CanonicalBytes, function: &PsiOptimizationFunction) {
    bytes.id(function.machine);
    encode_optional(bytes, function.attachment.as_ref(), |bytes, attachment| {
        bytes.id(*attachment)
    });
    bytes.id(function.entry);
    bytes.slice(&function.parameters, encode_definition);
    bytes.slice(&function.structural_parameters, encode_structural_parameter);
    bytes.slice(&function.structural_places, |bytes, place| {
        encode_place_declaration(bytes, *place)
    });
    encode_function_result(bytes, &function.result);
    bytes.len(function.declared_places.len());
    for place in &function.declared_places {
        bytes.id(*place);
    }
    bytes.slice(&function.entry_claim_declarations, encode_entry_claim);
    bytes.slice(&function.content_entry_claims, encode_content_entry_claim);
    encode_optional(
        bytes,
        function.verified_contract.as_ref(),
        |bytes, contract| encode_machine_contract(bytes, contract),
    );
    bytes.slice(
        &function.evidence_contract_lanes,
        encode_evidence_contract_lane,
    );
    bytes.len(function.entry_claims.len());
    for claim in &function.entry_claims {
        bytes.id(*claim);
    }
    encode_ids(bytes, &function.published_service_ceiling);
    bytes.slice(&function.facts, encode_fact);
    bytes.len(function.blocks.len());
    for block in &function.blocks {
        bytes.id(block.id);
        bytes.slice(&block.parameters, encode_definition);
        bytes.slice(&block.nodes, encode_node);
    }
}

fn encode_function_result(bytes: &mut CanonicalBytes, result: &AbstractFunctionResult) {
    match result {
        AbstractFunctionResult::Unit => bytes.u8(1),
        AbstractFunctionResult::Scalar(result) => {
            bytes.u8(2);
            encode_abstract_result(bytes, *result);
        }
        AbstractFunctionResult::Structural(result) => {
            bytes.u8(3);
            encode_structural_result(bytes, result);
        }
    }
}

fn encode_structural_result(bytes: &mut CanonicalBytes, result: &StructuralResultDeclaration) {
    bytes.id(result.place);
    bytes.id(result.structural_type);
    encode_multiplicity(bytes, result.multiplicity);
    encode_ids(bytes, &result.qualifications);
    encode_projected_qualification_roster(bytes, &result.projected_qualifications);
}

fn encode_node(bytes: &mut CanonicalBytes, node: &OptimizationNode) {
    encode_operation(bytes, &node.operation);
    bytes.slice(&node.provenance, |bytes, provenance| {
        encode_provenance(bytes, *provenance)
    });
    bytes.slice(&node.fuel, encode_fuel);
    encode_effect(bytes, node.effect);
    bytes.slice(&node.definitions, encode_definition);
    bytes.slice(&node.uses, encode_use);
    bytes.slice(&node.successors, encode_edge);
    bytes.slice(&node.ownership, encode_ownership);
}

fn encode_provenance(bytes: &mut CanonicalBytes, provenance: PsiProvenance) {
    match provenance {
        PsiProvenance::Operation(operation) => {
            bytes.u8(1);
            bytes.id(operation);
        }
        PsiProvenance::Edge(edge) => {
            bytes.u8(2);
            bytes.id(edge);
        }
    }
}

fn encode_fuel(bytes: &mut CanonicalBytes, fuel: &FuelSettlement) {
    encode_provenance(bytes, fuel.site);
    bytes.u64(fuel.units);
}

fn encode_effect(bytes: &mut CanonicalBytes, effect: EffectLink) {
    bytes.u64(effect.input);
    bytes.u64(effect.output);
}

fn encode_definition(bytes: &mut CanonicalBytes, definition: &ValueDefinition) {
    bytes.id(definition.value);
    encode_scalar_type(bytes, definition.scalar_type);
    match definition.site {
        ValueDefinitionSite::FunctionParameter(position) => {
            bytes.u8(1);
            bytes.u32(position);
        }
        ValueDefinitionSite::BlockParameter { block, position } => {
            bytes.u8(2);
            bytes.id(block);
            bytes.u32(position);
        }
        ValueDefinitionSite::Node { block, node } => {
            bytes.u8(3);
            bytes.id(block);
            bytes.u32(node);
        }
    }
}

fn encode_use(bytes: &mut CanonicalBytes, value_use: &ValueUse) {
    bytes.id(value_use.value);
    bytes.id(value_use.block);
    bytes.u32(value_use.node);
}

fn encode_edge(bytes: &mut CanonicalBytes, edge: &OptimizationEdge) {
    bytes.id(edge.psi_edge);
    bytes.id(edge.target);
    bytes.slice(&edge.bindings, encode_binding);
    encode_ids(bytes, &edge.trivial_affine_discards);
    bytes.slice(&edge.provenance, |bytes, provenance| {
        encode_provenance(bytes, *provenance)
    });
    bytes.slice(&edge.fuel, encode_fuel);
}

fn encode_ownership(bytes: &mut CanonicalBytes, event: &OwnershipEvent) {
    match event {
        OwnershipEvent::ClaimTransfer(claims) => {
            bytes.u8(1);
            encode_ids(bytes, claims);
        }
        OwnershipEvent::ClaimCompletion(claims) => {
            bytes.u8(2);
            encode_ids(bytes, claims);
        }
        OwnershipEvent::Cleanup(actions) => {
            bytes.u8(3);
            bytes.slice(actions, encode_cleanup);
        }
        OwnershipEvent::StructuralReturn(claims) => {
            bytes.u8(4);
            encode_ids(bytes, claims);
        }
        OwnershipEvent::CrashFrontier(claims) => {
            bytes.u8(5);
            encode_ids(bytes, claims);
        }
    }
}

fn encode_fact(bytes: &mut CanonicalBytes, fact: &OptimizationFact) {
    match fact {
        OptimizationFact::OperationObligationReference {
            obligation,
            support,
        } => {
            bytes.u8(1);
            bytes.id(*obligation);
            bytes.id(*support);
        }
        OptimizationFact::BooleanConstant {
            value,
            constant,
            support,
        } => {
            bytes.u8(2);
            bytes.id(*value);
            bytes.boolean(*constant);
            bytes.id(*support);
        }
        OptimizationFact::IntegerConstant {
            value,
            constant,
            support,
        } => {
            bytes.u8(3);
            bytes.id(*value);
            encode_integer_value(bytes, *constant);
            bytes.id(*support);
        }
    }
}
