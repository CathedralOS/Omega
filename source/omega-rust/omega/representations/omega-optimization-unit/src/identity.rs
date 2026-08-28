use omega_optimization_core::OptimizationUnitIdentity;
use omega_terminal_abstract_operations::{
    TerminalAbstractFunctionResult, TerminalAbstractOperation, TerminalAbstractSuccessor,
    TerminalCompletionClaimSource, TerminalValueBinding,
};
use psi_core::{
    ByteSequenceStructuralField, CanonicalStructuralPathSegment, ContentAlgebra,
    ContentAlgebraKind, ContentConservation, ContentPlaceSegment, ContentPlaceVersion,
    ContentProjectionExpression, ContentProjectionScalar, ContentStructuralPlace, ContentTerm,
    IeeeFloatComparisonKind, IeeeFloatFormat, IeeeFloatStructuralField, IntegerSign, IntegerType,
    IntegerValue, Proposition, PsiSemanticId, ScalarTerm, ScalarType, StructuralCaseSubject,
    StructuralPlaceKind,
};
use psi_terminal::{
    BindingRelevance, BoundaryMachineDeclaration, ByteSequenceCarrier, ClaimContentProjection,
    ContentConservationGuarantee, CrashCause, CrashPredicateTerm, EntryClaim,
    ProgramLocalRootIntroductionSchema, ProviderCandidateConformance, StructuralAccess,
    StructuralArgument, StructuralDomainDeclaration, StructuralDomainRequirement,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralOperationResult, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralResultDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalAffineCleanupAction,
};

use crate::{
    AcceptedObligationFact, EffectLink, FuelSettlement, OptimizationEdge, OptimizationFact,
    OptimizationNode, OwnershipEvent, OwnershipFrontierFact, OwnershipFrontierSite,
    OwnershipFrontierSnapshot, PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance,
    ValueDefinition, ValueDefinitionSite, ValueUse,
};

const UNIT_IDENTITY_DOMAIN: &[u8] = b"omega.psi-optimization-unit-content.v11\0";
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
    bytes.u16(unit.terminal_psi.vocabulary_marker.get());
    bytes.bytes(unit.terminal_psi.program_fingerprint.as_bytes());
    bytes.u32(unit.fuel_schedule.marker());
    bytes.id(unit.entry);
    bytes.slice(&unit.structural_types, encode_structural_type);
    bytes.slice(unit.structural_domains.as_ref(), encode_structural_domain);
    bytes.slice(&unit.boundary_machines, encode_boundary_machine);
    bytes.slice(&unit.provider_candidates, encode_provider_candidate);
    bytes.slice(&unit.accepted_obligation_facts, encode_accepted_fact);
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

#[derive(Default)]
struct CanonicalBytes(Vec<u8>);

impl CanonicalBytes {
    fn finish(self) -> Vec<u8> {
        self.0
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }

    fn u8(&mut self, value: u8) {
        self.0.push(value);
    }

    fn boolean(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.bytes(&value.to_le_bytes());
    }

    fn id(&mut self, value: impl PsiSemanticId) {
        self.u64(value.get());
    }

    fn len(&mut self, len: usize) {
        self.u64(u64::try_from(len).expect("canonical optimization-unit length fits u64"));
    }

    fn string(&mut self, value: &str) {
        self.len(value.len());
        self.bytes(value.as_bytes());
    }

    fn slice<T>(&mut self, values: &[T], encode: impl Fn(&mut Self, &T)) {
        self.len(values.len());
        for value in values {
            encode(self, value);
        }
    }
}

fn encode_accepted_fact(bytes: &mut CanonicalBytes, fact: &AcceptedObligationFact) {
    bytes.bytes(&fact.identity.bytes());
    bytes.u16(fact.terminal_psi.vocabulary_marker.get());
    bytes.bytes(fact.terminal_psi.program_fingerprint.as_bytes());
    bytes.bytes(&fact.proof_bundle_fingerprint);
    bytes.id(fact.machine);
    bytes.id(fact.operation);
    bytes.id(fact.obligation);
    bytes.len(fact.proposition.len());
    bytes.bytes(&fact.proposition);
}

fn encode_ownership_frontier_fact(bytes: &mut CanonicalBytes, fact: &OwnershipFrontierFact) {
    bytes.bytes(&fact.identity.bytes());
    bytes.u16(fact.terminal_psi.vocabulary_marker.get());
    bytes.bytes(fact.terminal_psi.program_fingerprint.as_bytes());
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

fn encode_function_result(bytes: &mut CanonicalBytes, result: &TerminalAbstractFunctionResult) {
    match result {
        TerminalAbstractFunctionResult::Unit => bytes.u8(1),
        TerminalAbstractFunctionResult::Scalar(result) => {
            bytes.u8(2);
            encode_abstract_result(bytes, *result);
        }
        TerminalAbstractFunctionResult::Structural(result) => {
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

fn encode_operation(bytes: &mut CanonicalBytes, operation: &TerminalAbstractOperation) {
    use TerminalAbstractOperation as O;
    match operation {
        O::EstablishByteSequenceLiteral {
            psi_operation,
            place,
            structural_type,
            bytes: literal,
        } => {
            bytes.u8(1);
            bytes.id(*psi_operation);
            encode_place_declaration(bytes, *place);
            encode_structural_type(bytes, structural_type);
            bytes.len(literal.len());
            bytes.bytes(literal);
        }
        O::EstablishTrivialAffineLocal {
            psi_operation,
            place,
            structural_type,
        } => {
            bytes.u8(2);
            bytes.id(*psi_operation);
            encode_place_declaration(bytes, *place);
            encode_structural_type(bytes, structural_type);
        }
        O::CallUnit {
            psi_operation,
            callee,
            structural_arguments,
            claim_transfers,
        } => {
            bytes.u8(3);
            bytes.id(*psi_operation);
            bytes.id(*callee);
            bytes.slice(structural_arguments, encode_structural_argument);
            bytes.slice(claim_transfers, |bytes, transfer| {
                bytes.id(transfer.claim);
                bytes.u32(transfer.argument_index);
            });
        }
        O::CallStructuralScalar {
            psi_operation,
            result,
            callee,
            structural_arguments,
            claim_transfers,
        } => {
            bytes.u8(4);
            bytes.id(*psi_operation);
            encode_abstract_result(bytes, *result);
            bytes.id(*callee);
            bytes.slice(structural_arguments, encode_structural_argument);
            bytes.slice(claim_transfers, |bytes, transfer| {
                bytes.id(transfer.claim);
                bytes.u32(transfer.argument_index);
            });
        }
        O::CallStructural {
            psi_operation,
            result,
            callee,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
        } => {
            bytes.u8(5);
            bytes.id(*psi_operation);
            encode_structural_operation_result(bytes, result);
            bytes.id(*callee);
            bytes.slice(structural_arguments, encode_structural_argument);
            bytes.slice(claim_transfers, |bytes, transfer| {
                bytes.id(transfer.claim);
                bytes.u32(transfer.argument_index);
            });
            bytes.slice(returned_claim_transfers, |bytes, transfer| {
                bytes.id(transfer.callee_claim);
                bytes.id(transfer.caller_claim);
            });
        }
        O::BoundaryCall {
            psi_operation,
            result,
            boundary,
            arguments,
            structural_arguments,
            completion_claim_sources,
            completion_receipts,
        } => {
            bytes.u8(6);
            bytes.id(*psi_operation);
            encode_optional(bytes, result.as_ref(), |bytes, result| {
                encode_abstract_result(bytes, *result)
            });
            bytes.id(*boundary);
            encode_ids(bytes, arguments);
            bytes.slice(structural_arguments, encode_structural_argument);
            bytes.slice(completion_claim_sources, encode_completion_claim_source);
            bytes.slice(completion_receipts, |bytes, receipt| {
                bytes.id(receipt.claim);
                bytes.u32(receipt.argument_index);
            });
        }
        O::PortWrite {
            psi_operation,
            service,
            port,
            value,
        } => {
            bytes.u8(7);
            bytes.id(*psi_operation);
            bytes.id(*service);
            bytes.u16(*port);
            bytes.u8(*value);
        }
        O::Call {
            psi_operation,
            result,
            scalar_type,
            callee,
            arguments,
        } => {
            bytes.u8(8);
            bytes.id(*psi_operation);
            bytes.id(*result);
            encode_scalar_type(bytes, *scalar_type);
            bytes.id(*callee);
            encode_ids(bytes, arguments);
        }
        O::IntegerConstant {
            psi_operation,
            result,
            scalar_type,
            value,
        } => {
            bytes.u8(9);
            bytes.id(*psi_operation);
            bytes.id(*result);
            encode_scalar_type(bytes, *scalar_type);
            encode_integer_value(bytes, *value);
        }
        O::BooleanConstant {
            psi_operation,
            result,
            value,
        } => {
            bytes.u8(10);
            bytes.id(*psi_operation);
            bytes.id(*result);
            bytes.boolean(*value);
        }
        O::BooleanStructuralField {
            psi_operation,
            result,
            source,
            field,
        } => {
            bytes.u8(11);
            bytes.id(*psi_operation);
            bytes.id(*result);
            bytes.id(*source);
            bytes.id(*field);
        }
        O::BooleanNot {
            psi_operation,
            result,
            operand,
        } => encode_untyped_unary(bytes, 12, *psi_operation, *result, *operand),
        O::BooleanEqual {
            psi_operation,
            result,
            left,
            right,
        } => encode_untyped_binary(bytes, 13, *psi_operation, *result, *left, *right),
        O::IntegerEqual {
            psi_operation,
            result,
            left,
            right,
        } => encode_untyped_binary(bytes, 14, *psi_operation, *result, *left, *right),
        O::IntegerLessThan {
            psi_operation,
            result,
            left,
            right,
        } => encode_untyped_binary(bytes, 15, *psi_operation, *result, *left, *right),
        O::IntegerLessOrEqual {
            psi_operation,
            result,
            left,
            right,
        } => encode_untyped_binary(bytes, 16, *psi_operation, *result, *left, *right),
        O::IntegerBitwiseNot {
            psi_operation,
            result,
            scalar_type,
            operand,
        } => encode_typed_unary(bytes, 17, *psi_operation, *result, *scalar_type, *operand),
        O::IntegerWiden {
            psi_operation,
            result,
            source_type,
            target_type,
            operand,
        } => encode_cast(
            bytes,
            18,
            *psi_operation,
            None,
            *result,
            *source_type,
            *target_type,
            *operand,
        ),
        O::IntegerExactCast {
            psi_operation,
            obligation,
            result,
            source_type,
            target_type,
            operand,
        } => encode_cast(
            bytes,
            19,
            *psi_operation,
            Some(*obligation),
            *result,
            *source_type,
            *target_type,
            *operand,
        ),
        O::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            20,
            *psi_operation,
            None,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::IntegerBitwiseOr {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            21,
            *psi_operation,
            None,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::IntegerBitwiseXor {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            22,
            *psi_operation,
            None,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => encode_shift(
            bytes,
            23,
            *psi_operation,
            None,
            *result,
            *value_type,
            *count_type,
            *value,
            *count,
        ),
        O::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => encode_shift(
            bytes,
            24,
            *psi_operation,
            None,
            *result,
            *value_type,
            *count_type,
            *value,
            *count,
        ),
        O::ExactIntegerShiftLeft {
            psi_operation,
            obligation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => encode_shift(
            bytes,
            25,
            *psi_operation,
            Some(*obligation),
            *result,
            *value_type,
            *count_type,
            *value,
            *count,
        ),
        O::ExactIntegerShiftRight {
            psi_operation,
            obligation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => encode_shift(
            bytes,
            26,
            *psi_operation,
            Some(*obligation),
            *result,
            *value_type,
            *count_type,
            *value,
            *count,
        ),
        O::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            27,
            *psi_operation,
            None,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerAdd {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            28,
            *psi_operation,
            Some(*obligation),
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            29,
            *psi_operation,
            None,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            30,
            *psi_operation,
            None,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerSubtract {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            31,
            *psi_operation,
            Some(*obligation),
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            32,
            *psi_operation,
            None,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            33,
            *psi_operation,
            None,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerMultiply {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            34,
            *psi_operation,
            Some(*obligation),
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            35,
            *psi_operation,
            Some(*obligation),
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            36,
            *psi_operation,
            Some(*obligation),
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            37,
            *psi_operation,
            Some(*obligation),
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            38,
            *psi_operation,
            Some(*obligation),
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            39,
            *psi_operation,
            Some(*obligation),
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            40,
            *psi_operation,
            Some(*obligation),
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            41,
            *psi_operation,
            None,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::Jump {
            psi_edge,
            target,
            bindings,
        } => {
            bytes.u8(42);
            bytes.id(*psi_edge);
            bytes.id(*target);
            bytes.slice(bindings, encode_binding);
        }
        O::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            bytes.u8(43);
            bytes.id(*condition);
            encode_successor(bytes, when_true);
            encode_successor(bytes, when_false);
        }
        O::Return {
            psi_edge,
            result,
            value,
            scalar_type,
            cleanup_actions,
        } => {
            bytes.u8(44);
            bytes.id(*psi_edge);
            bytes.id(*result);
            bytes.id(*value);
            encode_scalar_type(bytes, *scalar_type);
            bytes.slice(cleanup_actions, encode_cleanup);
        }
        O::ReturnUnit {
            psi_edge,
            cleanup_actions,
        } => {
            bytes.u8(45);
            bytes.id(*psi_edge);
            bytes.slice(cleanup_actions, encode_cleanup);
        }
        O::ReturnStructural {
            psi_edge,
            source,
            returned_claims,
            trivial_affine_locals,
            trivial_affine_discards,
        } => {
            bytes.u8(46);
            bytes.id(*psi_edge);
            bytes.id(*source);
            encode_ids(bytes, returned_claims);
            bytes.len(trivial_affine_locals.len());
            for (operation, place, structural_type) in trivial_affine_locals {
                bytes.id(*operation);
                encode_place_declaration(bytes, *place);
                encode_structural_type(bytes, structural_type);
            }
            encode_ids(bytes, trivial_affine_discards);
        }
        O::Crash {
            psi_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        } => {
            bytes.u8(47);
            bytes.id(*psi_edge);
            encode_crash_cause(bytes, *cause);
            bytes.slice(site_guard, encode_crash_predicate);
            encode_ids(bytes, frontier_lower_bound);
        }
    }
}

fn encode_untyped_unary(
    bytes: &mut CanonicalBytes,
    tag: u8,
    operation: psi_core::OperationId,
    result: psi_core::ValueId,
    operand: psi_core::ValueId,
) {
    bytes.u8(tag);
    bytes.id(operation);
    bytes.id(result);
    bytes.id(operand);
}

fn encode_untyped_binary(
    bytes: &mut CanonicalBytes,
    tag: u8,
    operation: psi_core::OperationId,
    result: psi_core::ValueId,
    left: psi_core::ValueId,
    right: psi_core::ValueId,
) {
    bytes.u8(tag);
    bytes.id(operation);
    bytes.id(result);
    bytes.id(left);
    bytes.id(right);
}

fn encode_typed_unary(
    bytes: &mut CanonicalBytes,
    tag: u8,
    operation: psi_core::OperationId,
    result: psi_core::ValueId,
    scalar_type: IntegerType,
    operand: psi_core::ValueId,
) {
    bytes.u8(tag);
    bytes.id(operation);
    bytes.id(result);
    encode_integer_type(bytes, scalar_type);
    bytes.id(operand);
}

#[allow(clippy::too_many_arguments)]
fn encode_cast(
    bytes: &mut CanonicalBytes,
    tag: u8,
    operation: psi_core::OperationId,
    obligation: Option<psi_core::ObligationId>,
    result: psi_core::ValueId,
    source_type: IntegerType,
    target_type: IntegerType,
    operand: psi_core::ValueId,
) {
    bytes.u8(tag);
    bytes.id(operation);
    encode_optional(bytes, obligation.as_ref(), |bytes, value| bytes.id(*value));
    bytes.id(result);
    encode_integer_type(bytes, source_type);
    encode_integer_type(bytes, target_type);
    bytes.id(operand);
}

#[allow(clippy::too_many_arguments)]
fn encode_typed_binary(
    bytes: &mut CanonicalBytes,
    tag: u8,
    operation: psi_core::OperationId,
    obligation: Option<psi_core::ObligationId>,
    result: psi_core::ValueId,
    scalar_type: IntegerType,
    left: psi_core::ValueId,
    right: psi_core::ValueId,
) {
    bytes.u8(tag);
    bytes.id(operation);
    encode_optional(bytes, obligation.as_ref(), |bytes, value| bytes.id(*value));
    bytes.id(result);
    encode_integer_type(bytes, scalar_type);
    bytes.id(left);
    bytes.id(right);
}

#[allow(clippy::too_many_arguments)]
fn encode_shift(
    bytes: &mut CanonicalBytes,
    tag: u8,
    operation: psi_core::OperationId,
    obligation: Option<psi_core::ObligationId>,
    result: psi_core::ValueId,
    value_type: IntegerType,
    count_type: IntegerType,
    value: psi_core::ValueId,
    count: psi_core::ValueId,
) {
    bytes.u8(tag);
    bytes.id(operation);
    encode_optional(bytes, obligation.as_ref(), |bytes, value| bytes.id(*value));
    bytes.id(result);
    encode_integer_type(bytes, value_type);
    encode_integer_type(bytes, count_type);
    bytes.id(value);
    bytes.id(count);
}

fn encode_binding(bytes: &mut CanonicalBytes, binding: &TerminalValueBinding) {
    bytes.id(binding.parameter);
    bytes.id(binding.argument);
    encode_scalar_type(bytes, binding.scalar_type);
}

fn encode_successor(bytes: &mut CanonicalBytes, successor: &TerminalAbstractSuccessor) {
    bytes.id(successor.psi_edge);
    bytes.id(successor.target);
    bytes.slice(&successor.bindings, encode_binding);
}

fn encode_optional<T>(
    bytes: &mut CanonicalBytes,
    value: Option<&T>,
    encode: impl Fn(&mut CanonicalBytes, &T),
) {
    bytes.boolean(value.is_some());
    if let Some(value) = value {
        encode(bytes, value);
    }
}

fn encode_ids<T: PsiSemanticId>(bytes: &mut CanonicalBytes, ids: &[T]) {
    bytes.len(ids.len());
    for id in ids {
        bytes.id(*id);
    }
}

fn encode_abstract_result(
    bytes: &mut CanonicalBytes,
    result: omega_terminal_abstract_operations::TerminalAbstractResult,
) {
    bytes.id(result.value);
    encode_scalar_type(bytes, result.scalar_type);
}

fn encode_scalar_type(bytes: &mut CanonicalBytes, scalar_type: ScalarType) {
    match scalar_type {
        ScalarType::Boolean => bytes.u8(1),
        ScalarType::Integer(integer) => {
            bytes.u8(2);
            encode_integer_type(bytes, integer);
        }
    }
}

fn encode_integer_type(bytes: &mut CanonicalBytes, integer_type: IntegerType) {
    bytes.u8(match integer_type.sign() {
        IntegerSign::Unsigned => 1,
        IntegerSign::Signed => 2,
    });
    bytes.u16(integer_type.bits());
}

fn encode_integer_value(bytes: &mut CanonicalBytes, value: IntegerValue) {
    match value {
        IntegerValue::Unsigned(value) => {
            bytes.u8(1);
            bytes.u128(value);
        }
        IntegerValue::Signed(value) => {
            bytes.u8(2);
            bytes.bytes(&value.to_le_bytes());
        }
    }
}

fn encode_structural_parameter(
    bytes: &mut CanonicalBytes,
    parameter: &StructuralParameterDeclaration,
) {
    bytes.id(parameter.place);
    bytes.u32(parameter.position);
    bytes.boolean(parameter.is_self);
    bytes.id(parameter.structural_type);
    encode_multiplicity(bytes, parameter.multiplicity);
    encode_access(bytes, parameter.access);
    encode_ids(bytes, &parameter.qualifications);
}

fn encode_structural_argument(bytes: &mut CanonicalBytes, argument: &StructuralArgument) {
    bytes.id(argument.place);
    bytes.slice(&argument.path, encode_structural_path_segment);
    encode_access(bytes, argument.access);
}

fn encode_structural_path_segment(bytes: &mut CanonicalBytes, segment: &StructuralPathSegment) {
    match segment {
        StructuralPathSegment::Field(identity) => {
            bytes.u8(1);
            bytes.string(identity);
        }
        StructuralPathSegment::FixedIndex(index) => {
            bytes.u8(2);
            bytes.u64(*index);
        }
    }
}

fn encode_access(bytes: &mut CanonicalBytes, access: StructuralAccess) {
    bytes.u8(match access {
        StructuralAccess::Owned => 1,
        StructuralAccess::SharedBorrow => 2,
        StructuralAccess::MutableBorrow => 3,
        StructuralAccess::WriteOnlyBorrow => 4,
    });
}

fn encode_multiplicity(bytes: &mut CanonicalBytes, multiplicity: StructuralMultiplicity) {
    bytes.u8(match multiplicity {
        StructuralMultiplicity::Unrestricted => 1,
        StructuralMultiplicity::Affine => 2,
        StructuralMultiplicity::Linear => 3,
    });
}

fn encode_place_declaration(bytes: &mut CanonicalBytes, place: StructuralPlaceDeclaration) {
    bytes.id(place.id);
    match place.kind {
        StructuralPlaceKind::Parameter { position, is_self } => {
            bytes.u8(1);
            bytes.u32(position);
            bytes.boolean(is_self);
        }
        StructuralPlaceKind::Result => bytes.u8(2),
        StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } => {
            bytes.u8(3);
            bytes.id(producer);
            bytes.id(structural_type);
        }
        StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal,
            structural_type,
        } => {
            bytes.u8(4);
            bytes.u32(declaration_ordinal);
            bytes.id(structural_type);
        }
        StructuralPlaceKind::ProviderAttachment {
            attachment,
            field,
            boundary,
        } => {
            bytes.u8(5);
            bytes.id(attachment);
            bytes.id(field);
            bytes.id(boundary);
        }
        StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal,
            structural_type,
        } => {
            bytes.u8(6);
            bytes.u32(declaration_ordinal);
            bytes.id(structural_type);
        }
    }
}

fn encode_boundary_machine(bytes: &mut CanonicalBytes, declaration: &BoundaryMachineDeclaration) {
    bytes.id(declaration.id);
    bytes.string(&declaration.identity);
    encode_optional(
        bytes,
        declaration.attachment.as_ref(),
        |bytes, attachment| bytes.id(*attachment),
    );
    bytes.slice(&declaration.scalar_parameters, |bytes, parameter| {
        encode_scalar_type(bytes, *parameter)
    });
    bytes.slice(
        &declaration.structural_parameters,
        encode_structural_parameter,
    );
    encode_optional(bytes, declaration.result.as_ref(), |bytes, result| {
        encode_scalar_type(bytes, *result)
    });
    bytes.slice(&declaration.requires, encode_domain_requirement);
    bytes.slice(
        &declaration.program_local_root_introductions,
        encode_program_local_root_introduction,
    );
    bytes.slice(
        &declaration.content_guarantees,
        encode_content_conservation_guarantee,
    );
    encode_ids(bytes, &declaration.published_service_ceiling);
}

fn encode_domain_requirement(
    bytes: &mut CanonicalBytes,
    requirement: &StructuralDomainRequirement,
) {
    bytes.u32(requirement.argument_index);
    bytes.id(requirement.domain);
}

fn encode_program_local_root_introduction(
    bytes: &mut CanonicalBytes,
    schema: &ProgramLocalRootIntroductionSchema,
) {
    bytes.u32(schema.argument_index);
    bytes.u32(schema.source_parameter_position);
    bytes.id(schema.qualification);
    bytes.id(schema.carrier);
    bytes.id(schema.projection.domain);
    bytes.u64(schema.projection.projection_fingerprint);
    encode_content_algebra(bytes, &schema.algebra);
    encode_content_projection_expression(bytes, &schema.capacity);
    bytes.u64(schema.identity);
}

fn encode_content_projection_expression(
    bytes: &mut CanonicalBytes,
    expression: &ContentProjectionExpression,
) {
    match expression {
        ContentProjectionExpression::IntervalSet(members) => {
            bytes.u8(1);
            bytes.len(members.len());
            for (start, end) in members {
                encode_content_projection_scalar(bytes, start);
                encode_content_projection_scalar(bytes, end);
            }
        }
        ContentProjectionExpression::CountedQuantity(magnitude) => {
            bytes.u8(2);
            encode_content_projection_scalar(bytes, magnitude);
        }
    }
}

fn encode_content_projection_scalar(bytes: &mut CanonicalBytes, scalar: &ContentProjectionScalar) {
    // Content expressions may be intentionally deep. Encode their canonical
    // prefix form iteratively so retaining the verifier-owned domain catalog
    // does not turn semantic nesting depth into native thread-stack usage.
    let mut pending = vec![scalar];
    while let Some(scalar) = pending.pop() {
        match scalar {
            ContentProjectionScalar::SubjectField(path)
            | ContentProjectionScalar::RuntimeScalarEmbedding(path) => {
                bytes.u8(
                    if matches!(scalar, ContentProjectionScalar::SubjectField(_)) {
                        1
                    } else {
                        2
                    },
                );
                bytes.slice(path, |bytes, segment| bytes.string(segment));
            }
            ContentProjectionScalar::Natural(value) => {
                bytes.u8(3);
                bytes.string(value);
            }
            ContentProjectionScalar::Successor(inner) => {
                bytes.u8(4);
                pending.push(inner);
            }
            ContentProjectionScalar::Add(left, right)
            | ContentProjectionScalar::Subtract(left, right)
            | ContentProjectionScalar::Multiply(left, right) => {
                bytes.u8(match scalar {
                    ContentProjectionScalar::Add(_, _) => 5,
                    ContentProjectionScalar::Subtract(_, _) => 6,
                    ContentProjectionScalar::Multiply(_, _) => 7,
                    _ => unreachable!(),
                });
                pending.push(right);
                pending.push(left);
            }
        }
    }
}

fn encode_content_conservation_guarantee(
    bytes: &mut CanonicalBytes,
    guarantee: &ContentConservationGuarantee,
) {
    bytes.u64(guarantee.fingerprint);
    bytes.slice(&guarantee.structural_places, |bytes, place| {
        encode_place_declaration(bytes, *place)
    });
    encode_content_conservation(bytes, &guarantee.conservation);
}

fn encode_content_conservation(bytes: &mut CanonicalBytes, conservation: &ContentConservation) {
    encode_content_algebra(bytes, conservation.algebra());
    encode_content_term(bytes, conservation.left());
    encode_content_term(bytes, conservation.right());
}

fn encode_provider_candidate(bytes: &mut CanonicalBytes, candidate: &ProviderCandidateConformance) {
    bytes.id(candidate.boundary);
    bytes.string(&candidate.requirement_identity);
    bytes.string(&candidate.provider_identity);
    bytes.string(&candidate.candidate_identity);
    bytes.id(candidate.candidate);
    bytes.slice(&candidate.signature.parameters, |bytes, parameter| {
        bytes.u32(parameter.position);
        bytes.boolean(parameter.is_self);
        bytes.id(parameter.structural_type);
        encode_multiplicity(bytes, parameter.multiplicity);
        encode_ids(bytes, &parameter.qualifications);
    });
    bytes.slice(
        &candidate.refinement.positional_parameters,
        |bytes, parameter| {
            bytes.u32(parameter.boundary_index);
            bytes.u32(parameter.candidate_index);
        },
    );
    bytes.slice(
        &candidate.refinement.required_domains,
        encode_domain_requirement,
    );
    encode_ids(bytes, &candidate.refinement.realized_service_ceiling);
}

fn encode_structural_type(bytes: &mut CanonicalBytes, declaration: &StructuralTypeDeclaration) {
    bytes.id(declaration.id);
    bytes.string(&declaration.identity);
    match &declaration.shape {
        StructuralTypeShape::ByteSequence(carrier) => {
            bytes.u8(1);
            encode_byte_carrier(bytes, *carrier);
        }
        StructuralTypeShape::Record { fields } => {
            bytes.u8(2);
            bytes.slice(fields, encode_structural_field);
        }
        StructuralTypeShape::FixedArray { element, length } => {
            bytes.u8(3);
            bytes.id(*element);
            bytes.u64(*length);
        }
        StructuralTypeShape::Sum { cases } => {
            bytes.u8(4);
            bytes.len(cases.len());
            for case in cases {
                bytes.id(case.id);
                bytes.string(&case.identity);
                bytes.slice(&case.fields, encode_structural_field);
            }
        }
        StructuralTypeShape::Mixed { fields, cases } => {
            bytes.u8(5);
            bytes.slice(fields, encode_structural_field);
            bytes.len(cases.len());
            for case in cases {
                bytes.id(case.id);
                bytes.string(&case.identity);
                bytes.slice(&case.fields, encode_structural_field);
            }
        }
    }
}

fn encode_structural_domain(bytes: &mut CanonicalBytes, declaration: &StructuralDomainDeclaration) {
    bytes.id(declaration.id);
    bytes.id(declaration.semantic_domain);
    bytes.string(&declaration.identity);
    bytes.id(declaration.carrier);
    encode_optional(
        bytes,
        declaration.content_projection.as_ref(),
        |bytes, projection| {
            bytes.id(projection.identity.domain);
            bytes.u64(projection.identity.projection_fingerprint);
            encode_content_algebra(bytes, &projection.algebra);
            encode_content_projection_expression(bytes, &projection.expression);
        },
    );
}

fn encode_structural_field(bytes: &mut CanonicalBytes, field: &StructuralFieldDeclaration) {
    bytes.id(field.id);
    bytes.string(&field.identity);
    bytes.u8(match field.relevance {
        BindingRelevance::Relevant => 1,
        BindingRelevance::Erased => 2,
    });
    match &field.field_type {
        StructuralFieldType::Scalar(value) => {
            bytes.u8(1);
            encode_scalar_type(bytes, *value);
        }
        StructuralFieldType::IeeeFloat(value) => {
            bytes.u8(2);
            encode_float_format(bytes, *value);
        }
        StructuralFieldType::ByteSequence(value) => {
            bytes.u8(3);
            encode_byte_carrier(bytes, *value);
        }
        StructuralFieldType::Structural(value) => {
            bytes.u8(4);
            bytes.id(*value);
        }
        StructuralFieldType::Erased { type_identity } => {
            bytes.u8(5);
            bytes.string(type_identity);
        }
    }
}

fn encode_byte_carrier(bytes: &mut CanonicalBytes, carrier: ByteSequenceCarrier) {
    match carrier {
        ByteSequenceCarrier::BorrowedView => bytes.u8(1),
        ByteSequenceCarrier::BoundedOwned { capacity } => {
            bytes.u8(2);
            bytes.u64(capacity);
        }
    }
}

fn encode_structural_operation_result(
    bytes: &mut CanonicalBytes,
    result: &StructuralOperationResult,
) {
    bytes.id(result.place);
    bytes.id(result.structural_type);
    encode_multiplicity(bytes, result.multiplicity);
    encode_ids(bytes, &result.qualifications);
    bytes.len(result.claims.len());
    for claim in &result.claims {
        bytes.id(claim.claim);
        bytes.slice(&claim.path, encode_structural_path_segment);
    }
}

fn encode_completion_claim_source(
    bytes: &mut CanonicalBytes,
    source: &TerminalCompletionClaimSource,
) {
    bytes.id(source.claim);
    encode_optional(bytes, source.entry.as_ref(), encode_entry_claim);
    encode_optional(bytes, source.content.as_ref(), encode_content_entry_claim);
}

fn encode_entry_claim(bytes: &mut CanonicalBytes, claim: &EntryClaim) {
    bytes.id(claim.claim);
    bytes.id(claim.input);
    bytes.slice(&claim.path, encode_structural_path_segment);
}

fn encode_content_entry_claim(bytes: &mut CanonicalBytes, claim: &psi_terminal::ContentEntryClaim) {
    bytes.id(claim.claim);
    encode_content_place(bytes, &claim.input);
    bytes.slice(&claim.projections, encode_claim_projection);
}

fn encode_claim_projection(bytes: &mut CanonicalBytes, projection: &ClaimContentProjection) {
    bytes.id(projection.projection.domain);
    bytes.u64(projection.projection.projection_fingerprint);
    encode_content_algebra(bytes, &projection.algebra);
}

fn encode_content_algebra(bytes: &mut CanonicalBytes, algebra: &ContentAlgebra) {
    bytes.u8(match algebra.kind {
        ContentAlgebraKind::IntervalSet => 1,
        ContentAlgebraKind::CountedQuantity => 2,
    });
    bytes.string(&algebra.parameter);
}

fn encode_content_place(bytes: &mut CanonicalBytes, place: &ContentStructuralPlace) {
    bytes.u8(match place.version {
        ContentPlaceVersion::Entry => 1,
        ContentPlaceVersion::Current => 2,
    });
    bytes.id(place.root);
    bytes.len(place.segments.len());
    for segment in &place.segments {
        match segment {
            ContentPlaceSegment::Field(value) => {
                bytes.u8(1);
                bytes.string(value);
            }
            ContentPlaceSegment::FixedIndex(value) => {
                bytes.u8(2);
                bytes.u64(*value);
            }
            ContentPlaceSegment::Case(value) => {
                bytes.u8(3);
                bytes.string(value);
            }
        }
    }
}

fn encode_cleanup(bytes: &mut CanonicalBytes, action: &TerminalAffineCleanupAction) {
    match action {
        TerminalAffineCleanupAction::DiscardRoot(place) => {
            bytes.u8(1);
            bytes.id(*place);
        }
        TerminalAffineCleanupAction::DiscardResidual(discard) => {
            bytes.u8(2);
            bytes.id(discard.place);
            bytes.slice(&discard.path, encode_structural_path_segment);
            bytes.id(discard.structural_type);
        }
        TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
            bytes.u8(3);
            bytes.id(cleanup.place);
            bytes.id(cleanup.structural_type);
            bytes.id(cleanup.cleanup_machine);
            encode_optional(bytes, cleanup.cleanup_receiver.as_ref(), |bytes, place| {
                bytes.id(*place)
            });
            encode_ids(bytes, &cleanup.requirement_obligations);
        }
    }
}

fn encode_crash_cause(bytes: &mut CanonicalBytes, cause: CrashCause) {
    bytes.u8(match cause {
        CrashCause::Trap => 1,
        CrashCause::Abort => 2,
    });
}

fn encode_crash_predicate(bytes: &mut CanonicalBytes, predicate: &CrashPredicateTerm) {
    encode_proposition(bytes, predicate.proposition());
}

fn encode_proposition(bytes: &mut CanonicalBytes, proposition: &Proposition) {
    match proposition {
        Proposition::Truth => bytes.u8(1),
        Proposition::Falsehood => bytes.u8(2),
        Proposition::Atom(id) => {
            bytes.u8(3);
            bytes.id(*id);
        }
        Proposition::Equal(left, right) => {
            bytes.u8(4);
            encode_scalar_term(bytes, left);
            encode_scalar_term(bytes, right);
        }
        Proposition::LessThan(left, right) => {
            bytes.u8(5);
            encode_scalar_term(bytes, left);
            encode_scalar_term(bytes, right);
        }
        Proposition::LessOrEqual(left, right) => {
            bytes.u8(6);
            encode_scalar_term(bytes, left);
            encode_scalar_term(bytes, right);
        }
        Proposition::Conjunction(values) => {
            bytes.u8(7);
            bytes.slice(values, encode_proposition);
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            bytes.u8(8);
            encode_proposition(bytes, premise);
            encode_proposition(bytes, conclusion);
        }
        Proposition::ContentConservation(value) => {
            bytes.u8(9);
            encode_content_algebra(bytes, value.algebra());
            encode_content_term(bytes, value.left());
            encode_content_term(bytes, value.right());
        }
        Proposition::Disjunction(values) => {
            bytes.u8(10);
            bytes.slice(values, encode_proposition);
        }
        Proposition::IeeeFloatComparison {
            kind,
            format,
            left,
            right,
        } => {
            bytes.u8(11);
            encode_float_comparison(bytes, *kind);
            encode_float_format(bytes, *format);
            encode_float_field(bytes, left);
            encode_float_field(bytes, right);
        }
        Proposition::ByteSequenceEqual { left, right } => {
            bytes.u8(12);
            encode_byte_field(bytes, left);
            encode_byte_field(bytes, right);
        }
        Proposition::StructuralCaseMembership { subject, case } => {
            bytes.u8(13);
            encode_case_subject(bytes, subject);
            bytes.id(*case);
        }
    }
}

fn encode_content_term(bytes: &mut CanonicalBytes, term: &ContentTerm) {
    match term {
        ContentTerm::Projection {
            projection,
            subject,
        } => {
            bytes.u8(1);
            bytes.id(projection.domain);
            bytes.u64(projection.projection_fingerprint);
            encode_content_place(bytes, subject);
        }
        ContentTerm::Separate(terms) => {
            bytes.u8(2);
            bytes.slice(terms, encode_content_term);
        }
    }
}

fn encode_scalar_term(bytes: &mut CanonicalBytes, term: &ScalarTerm) {
    use ScalarTerm as S;
    match term {
        S::Value { id, scalar_type } => {
            bytes.u8(1);
            bytes.id(*id);
            encode_scalar_type(bytes, *scalar_type);
        }
        S::Boolean(value) => {
            bytes.u8(2);
            bytes.boolean(*value);
        }
        S::Integer { scalar_type, value } => {
            bytes.u8(3);
            encode_integer_type(bytes, *scalar_type);
            encode_integer_value(bytes, *value);
        }
        S::BooleanField { root, path } => {
            bytes.u8(4);
            bytes.id(*root);
            encode_canonical_path(bytes, path);
        }
        S::IntegerField {
            root,
            path,
            scalar_type,
        } => {
            bytes.u8(5);
            bytes.id(*root);
            encode_canonical_path(bytes, path);
            encode_integer_type(bytes, *scalar_type);
        }
        S::BooleanNot { operand } => encode_scalar_unary(bytes, 6, None, operand),
        S::BooleanEqual { left, right } => encode_scalar_binary(bytes, 7, None, left, right),
        S::IntegerEqual {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 8, Some(*scalar_type), left, right),
        S::IntegerLessThan {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 9, Some(*scalar_type), left, right),
        S::IntegerLessOrEqual {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 10, Some(*scalar_type), left, right),
        S::IntegerBitwiseNot {
            scalar_type,
            operand,
        } => encode_scalar_unary(bytes, 11, Some(*scalar_type), operand),
        S::IntegerWiden {
            source_type,
            target_type,
            operand,
        } => encode_scalar_cast(bytes, 12, *source_type, *target_type, operand),
        S::IntegerExactCast {
            source_type,
            target_type,
            operand,
        } => encode_scalar_cast(bytes, 13, *source_type, *target_type, operand),
        S::IntegerBitwiseAnd {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 14, Some(*scalar_type), left, right),
        S::IntegerBitwiseOr {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 15, Some(*scalar_type), left, right),
        S::IntegerBitwiseXor {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 16, Some(*scalar_type), left, right),
        S::WrappingIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
        } => encode_scalar_shift(bytes, 17, *value_type, *count_type, value, count),
        S::WrappingIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
        } => encode_scalar_shift(bytes, 18, *value_type, *count_type, value, count),
        S::ExactIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
        } => encode_scalar_shift(bytes, 19, *value_type, *count_type, value, count),
        S::ExactIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
        } => encode_scalar_shift(bytes, 20, *value_type, *count_type, value, count),
        S::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 21, Some(*scalar_type), left, right),
        S::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 22, Some(*scalar_type), left, right),
        S::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 23, Some(*scalar_type), left, right),
        S::ExactIntegerDivide {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 24, Some(*scalar_type), left, right),
        S::ExactIntegerRemainder {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 25, Some(*scalar_type), left, right),
        S::WrappingIntegerDivide {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 26, Some(*scalar_type), left, right),
        S::WrappingIntegerRemainder {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 27, Some(*scalar_type), left, right),
        S::SaturatingIntegerDivide {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 28, Some(*scalar_type), left, right),
        S::SaturatingIntegerRemainder {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 29, Some(*scalar_type), left, right),
        S::WrappingIntegerAdd {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 30, Some(*scalar_type), left, right),
        S::SaturatingIntegerAdd {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 31, Some(*scalar_type), left, right),
        S::WrappingIntegerSubtract {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 32, Some(*scalar_type), left, right),
        S::SaturatingIntegerSubtract {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 33, Some(*scalar_type), left, right),
        S::WrappingIntegerMultiply {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 34, Some(*scalar_type), left, right),
        S::SaturatingIntegerMultiply {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 35, Some(*scalar_type), left, right),
    }
}

fn encode_scalar_unary(
    bytes: &mut CanonicalBytes,
    tag: u8,
    scalar_type: Option<IntegerType>,
    operand: &ScalarTerm,
) {
    bytes.u8(tag);
    encode_optional(bytes, scalar_type.as_ref(), |bytes, value| {
        encode_integer_type(bytes, *value)
    });
    encode_scalar_term(bytes, operand);
}
fn encode_scalar_binary(
    bytes: &mut CanonicalBytes,
    tag: u8,
    scalar_type: Option<IntegerType>,
    left: &ScalarTerm,
    right: &ScalarTerm,
) {
    bytes.u8(tag);
    encode_optional(bytes, scalar_type.as_ref(), |bytes, value| {
        encode_integer_type(bytes, *value)
    });
    encode_scalar_term(bytes, left);
    encode_scalar_term(bytes, right);
}
fn encode_scalar_cast(
    bytes: &mut CanonicalBytes,
    tag: u8,
    source: IntegerType,
    target: IntegerType,
    operand: &ScalarTerm,
) {
    bytes.u8(tag);
    encode_integer_type(bytes, source);
    encode_integer_type(bytes, target);
    encode_scalar_term(bytes, operand);
}
fn encode_scalar_shift(
    bytes: &mut CanonicalBytes,
    tag: u8,
    value_type: IntegerType,
    count_type: IntegerType,
    value: &ScalarTerm,
    count: &ScalarTerm,
) {
    bytes.u8(tag);
    encode_integer_type(bytes, value_type);
    encode_integer_type(bytes, count_type);
    encode_scalar_term(bytes, value);
    encode_scalar_term(bytes, count);
}

fn encode_canonical_path(bytes: &mut CanonicalBytes, path: &[CanonicalStructuralPathSegment]) {
    bytes.len(path.len());
    for segment in path {
        match segment {
            CanonicalStructuralPathSegment::Field(value) => {
                bytes.u8(1);
                bytes.id(*value);
            }
            CanonicalStructuralPathSegment::FixedIndex(value) => {
                bytes.u8(2);
                bytes.u64(*value);
            }
            CanonicalStructuralPathSegment::Case(value) => {
                bytes.u8(3);
                bytes.id(*value);
            }
        }
    }
}
fn encode_float_format(bytes: &mut CanonicalBytes, value: IeeeFloatFormat) {
    bytes.u8(match value {
        IeeeFloatFormat::Binary32 => 1,
        IeeeFloatFormat::Binary64 => 2,
    });
}
fn encode_float_comparison(bytes: &mut CanonicalBytes, value: IeeeFloatComparisonKind) {
    bytes.u8(match value {
        IeeeFloatComparisonKind::Equal => 1,
        IeeeFloatComparisonKind::NotEqual => 2,
    });
}
fn encode_float_field(bytes: &mut CanonicalBytes, value: &IeeeFloatStructuralField) {
    bytes.id(value.root());
    encode_canonical_path(bytes, value.path());
}
fn encode_byte_field(bytes: &mut CanonicalBytes, value: &ByteSequenceStructuralField) {
    bytes.id(value.root());
    encode_canonical_path(bytes, value.path());
}
fn encode_case_subject(bytes: &mut CanonicalBytes, value: &StructuralCaseSubject) {
    bytes.id(value.root());
    encode_canonical_path(bytes, value.path());
}
