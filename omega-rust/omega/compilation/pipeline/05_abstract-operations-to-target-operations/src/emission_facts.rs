//! Project the remaining source obligations before dropping executable graphs.

use abstract_operations::{AbstractOperation as Operation, AbstractStructuralCallResult};
use verified_optimization_input::publication::{
    DescriptorProducer, DescriptorProducerFact, EmissionFunctionFacts, EmissionProgramFacts,
    OptimizedProgramProjection, PrimitiveLocalFact, RamObligation, RamSource, ScalarCallOrigin,
    ScalarCallResultFact, SemanticEmissionFacts,
};

pub(crate) fn project(
    optimized: &optimization_unit::OptimizedAbstractProgram,
    lowering: &verified_optimization_input::AdmittedLoweringFacts,
) -> SemanticEmissionFacts {
    let plan = optimized.plan();
    SemanticEmissionFacts::new(
        lowering.emission_admission(),
        EmissionProgramFacts {
            psi: plan.psi,
            entry: plan.entry,
            functions: plan.functions.iter().map(function).collect(),
        },
        OptimizedProgramProjection {
            identity: optimized.projection(),
            terminal: optimized.projection_psi(),
            final_unit: optimized.final_unit(),
        },
    )
}

fn function(source: &abstract_operations::AbstractFunction) -> EmissionFunctionFacts {
    let mut facts = EmissionFunctionFacts {
        machine: source.machine,
        attachment: source.attachment,
        parameters: source.parameters.clone(),
        structural_parameters: source.structural_parameters.clone(),
        result: source.result.clone(),
        entry_claims: source.entry_claims.clone(),
        source_occurrences: Vec::new(),
        requires_graph_storage_replay: abstract_operations::requires_graph_storage_replay(
            source.operations(),
        ),
        descriptors: Vec::new(),
        primitive_locals: Vec::new(),
        scalar_call_results: Vec::new(),
        ram_obligations: Vec::new(),
    };
    for operation in source.operations() {
        let identity = operation.psi_operation();
        let boundary = match operation {
            Operation::BoundaryCall {
                boundary,
                psi_operation,
                ..
            } => Some((
                *boundary,
                semantic_vocabulary::InvocationSite::Operation(*psi_operation),
            )),
            Operation::ExternalCompletion {
                boundary, psi_edge, ..
            } => Some((
                *boundary,
                semantic_vocabulary::InvocationSite::ExternalCompletion(*psi_edge),
            )),
            _ => None,
        };
        facts.source_occurrences.push(
            verified_optimization_input::publication::EmissionOperationProvenance {
                operation: identity,
                boundary,
            },
        );
        let descriptor = match operation {
            Operation::ByteSequenceSubslice {
                psi_operation,
                result,
                ..
            } => Some((
                *psi_operation,
                DescriptorProducer::ByteSubslice(result.clone()),
            )),
            Operation::ElementViewSubslice {
                psi_operation,
                result,
                ..
            }
            | Operation::EstablishElementView {
                psi_operation,
                result,
                ..
            } => Some((
                *psi_operation,
                DescriptorProducer::ElementView(result.clone()),
            )),
            Operation::EstablishByteSequenceLiteral {
                psi_operation,
                place,
                structural_type,
                ..
            } => Some((
                *psi_operation,
                DescriptorProducer::ByteLiteral {
                    place: place.id,
                    structural_type: structural_type.id,
                },
            )),
            _ => None,
        };
        if let Some((operation, descriptor)) = descriptor {
            facts.descriptors.push(DescriptorProducerFact {
                operation,
                descriptor,
            });
        }
        if let Operation::EstablishPrimitiveLocal {
            psi_operation,
            result,
            value,
            ..
        } = operation
        {
            facts.primitive_locals.push(PrimitiveLocalFact {
                operation: *psi_operation,
                result: result.clone(),
                initializer: *value,
            });
        }
        let scalar = match operation {
            Operation::CallStructural {
                psi_operation,
                result: AbstractStructuralCallResult::Scalar(result),
                callee,
                ..
            } => Some((*psi_operation, ScalarCallOrigin::Authored(*callee), *result)),
            Operation::BoundaryCall {
                psi_operation,
                result: abstract_operations::AbstractBoundaryResult::Scalar(result),
                boundary,
                ..
            } => Some((
                *psi_operation,
                ScalarCallOrigin::Boundary(*boundary),
                *result,
            )),
            _ => None,
        };
        if let Some((operation, target, result)) = scalar {
            facts.scalar_call_results.push(ScalarCallResultFact {
                operation,
                target,
                result,
            });
        }
        let source = match operation {
            Operation::EstablishAccessReference { .. } => Some(RamSource::PlacedBacking),
            Operation::BoundaryCall { psi_operation, .. } => {
                Some(RamSource::Boundary(*psi_operation))
            }
            Operation::ExternalCompletion { psi_edge, .. } => {
                Some(RamSource::ExternalCompletion(*psi_edge))
            }
            _ => None,
        };
        if let Some(source) = source {
            facts.ram_obligations.push(RamObligation {
                operation: identity,
                source,
            });
        }
    }
    facts
}
