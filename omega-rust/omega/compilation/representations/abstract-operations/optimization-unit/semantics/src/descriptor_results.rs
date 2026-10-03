//! Current optimization graphs adapted to the shared origin kernel.
//! No cached producer summary grants backing authority. Each caller reconstructs
//! exact declarations, reachable returns, view derivations and ordinary calls.
use abstract_operations::{AbstractFunctionResult, AbstractOperation};
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{BlockId, MachineId, PlaceId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralTypeDeclaration,
};
use terminal_semantics::borrowed_storage::Projection;
use terminal_semantics::descriptor_results::{
    self as kernel, DescriptorDefinition as Definition, DescriptorMachine, DescriptorPlace,
    DescriptorSource,
};

struct Function<'a> {
    id: MachineId,
    entry: BlockId,
    parameters: &'a [StructuralParameterDeclaration],
    result: &'a AbstractFunctionResult,
    blocks: Vec<(
        BlockId,
        &'a [StructuralParameterDeclaration],
        Vec<&'a AbstractOperation>,
    )>,
}

pub fn optimized_graph(unit: &PsiOptimizationUnit) -> Option<Vec<DescriptorMachine>> {
    optimized_catalog_graph(unit.functions.iter(), &unit.structural_types)
}

/// Prepare the current executable catalog once for ownership and source replay.
/// This is analysis scratch, not a cached lifetime assertion or synthetic unit.
pub fn optimized_catalog_graph<'a>(
    functions: impl IntoIterator<Item = &'a optimization_unit::PsiOptimizationFunction>,
    types: &[StructuralTypeDeclaration],
) -> Option<Vec<DescriptorMachine>> {
    let functions = functions
        .into_iter()
        .map(|function| Function {
            id: function.machine,
            entry: function.entry,
            parameters: &function.structural_parameters,
            result: &function.result,
            blocks: function
                .blocks
                .iter()
                .map(|block| {
                    (
                        block.id,
                        block.structural_parameters.as_slice(),
                        block.nodes.iter().map(|node| &node.operation).collect(),
                    )
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    graph(types, &functions)
}

pub fn call_origins(
    unit: &PsiOptimizationUnit,
    caller: MachineId,
    result: PlaceId,
) -> Option<Vec<kernel::DescriptorCallOrigin>> {
    kernel::reconstruct_call_origins(
        &unit.structural_types,
        &optimized_graph(unit)?,
        caller,
        result,
    )
}

fn graph(
    types: &[StructuralTypeDeclaration],
    functions: &[Function<'_>],
) -> Option<Vec<DescriptorMachine>> {
    let mut identities = BTreeSet::new();
    functions
        .iter()
        .map(|function| {
            if !identities.insert(function.id) {
                return None;
            }
            let mut declarations = BTreeMap::new();
            let mut places = Vec::new();
            for (position, parameter) in function.parameters.iter().enumerate() {
                if declarations
                    .insert(parameter.place, parameter.structural_type)
                    .is_some()
                {
                    return None;
                }
                places.push(DescriptorPlace {
                    place: parameter.place,
                    definition: Definition::Parameter {
                        position,
                        access: parameter.access,
                        structural_type: parameter.structural_type,
                    },
                });
            }
            let operations = function
                .blocks
                .iter()
                .flat_map(|(_, _, operations)| operations.iter().copied())
                .collect::<Vec<_>>();
            for (block, parameters, _) in &function.blocks {
                for parameter in *parameters {
                    if declarations
                        .insert(parameter.place, parameter.structural_type)
                        .is_some()
                    {
                        return None;
                    }
                    let mut incoming = Vec::new();
                    let mut bind =
                        |target,
                         bindings: &[abstract_operations::AbstractStructuralBinding]|
                         -> Option<()> {
                            if target != *block {
                                return Some(());
                            }
                            let mut matching = bindings
                                .iter()
                                .filter(|binding| binding.parameter == parameter.place);
                            let argument = &matching.next()?.argument;
                            if matching.next().is_some() {
                                return None;
                            }
                            incoming.push(argument_source(argument)?);
                            Some(())
                        };
                    for operation in &operations {
                        match operation {
                            AbstractOperation::Jump {
                                target,
                                structural_bindings,
                                ..
                            } => bind(*target, structural_bindings)?,
                            AbstractOperation::Conditional {
                                when_true,
                                when_false,
                                ..
                            } => {
                                bind(when_true.target, &when_true.structural_bindings)?;
                                bind(when_false.target, &when_false.structural_bindings)?;
                            }
                            _ => {}
                        }
                    }
                    places.push(DescriptorPlace {
                        place: parameter.place,
                        definition: Definition::Alias(incoming),
                    });
                }
            }
            for operation in &operations {
                if let AbstractOperation::EstablishByteSequenceLiteral {
                    place,
                    structural_type,
                    ..
                } = operation
                {
                    if declarations.insert(place.id, structural_type.id).is_some() {
                        return None;
                    }
                    places.push(DescriptorPlace {
                        place: place.id,
                        definition: Definition::Literal,
                    });
                    continue;
                }
                let Some((_, result)) = operation.structural_result() else {
                    continue;
                };
                if declarations
                    .insert(result.place, result.structural_type)
                    .is_some()
                {
                    return None;
                }
                let shared = result.multiplicity == StructuralMultiplicity::Unrestricted
                    && result.claims.is_empty()
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.qualification_establishments.is_empty()
                    && kernel::is_shared_descriptor(types.iter(), result.structural_type);
                // Alias geometry belongs to the producing operation. An affine
                // reference or mutable descriptor may be an intermediate in a
                // final shared return; its custody does not make new backing.
                let definition = match operation {
                    AbstractOperation::ByteSequenceSubslice { source, .. }
                    | AbstractOperation::ElementViewSubslice { source, .. } => {
                        let mut projection = Projection::new(&[]);
                        projection.exact = false;
                        Definition::Alias(vec![DescriptorSource {
                            source: *source,
                            projection,
                        }])
                    }
                    AbstractOperation::EstablishElementView { source, .. }
                    | AbstractOperation::EstablishReference { source, .. } => {
                        Definition::Alias(vec![argument_source(source)?])
                    }
                    AbstractOperation::StructuralLeafCopy {
                        source,
                        path: abstract_operations::StructuralCopyPath::Ordinary(path),
                        ..
                    } if shared => {
                        let mut projection = Projection::new(path);
                        projection.indirect = !path.is_empty();
                        Definition::Alias(vec![DescriptorSource {
                            source: *source,
                            projection,
                        }])
                    }
                    AbstractOperation::CallStructural {
                        callee,
                        structural_arguments,
                        ..
                    } if shared => {
                        let called = functions.iter().find(|candidate| candidate.id == *callee)?;
                        let declared = called.result.structural()?;
                        if declared.structural_type != result.structural_type
                            || declared.multiplicity != result.multiplicity
                            || declared.qualifications != result.qualifications
                            || declared.projected_qualifications != result.projected_qualifications
                            || structural_arguments.len() != called.parameters.len()
                        {
                            return None;
                        }
                        Definition::Call {
                            callee: *callee,
                            arguments: structural_arguments.clone(),
                        }
                    }
                    _ => Definition::Local,
                };
                places.push(DescriptorPlace {
                    place: result.place,
                    definition,
                });
            }
            let returns = function
                .result
                .structural()
                .filter(|result| {
                    result.multiplicity == StructuralMultiplicity::Unrestricted
                        && kernel::is_shared_descriptor(types.iter(), result.structural_type)
                })
                .and_then(|result| {
                    let mut pending = vec![function.entry];
                    let mut visited = BTreeSet::new();
                    let mut returns = Vec::new();
                    while let Some(block) = pending.pop() {
                        if !visited.insert(block) {
                            continue;
                        }
                        let (_, _, operations) =
                            function.blocks.iter().find(|(id, _, _)| *id == block)?;
                        match operations.last()? {
                            AbstractOperation::ReturnStructural { source, .. }
                                if declarations.get(source) == Some(&result.structural_type) =>
                            {
                                returns.push(*source)
                            }
                            AbstractOperation::Jump { target, .. } => pending.push(*target),
                            AbstractOperation::Conditional {
                                when_true,
                                when_false,
                                ..
                            } => pending.extend([when_true.target, when_false.target]),
                            AbstractOperation::StructuralCase { cases, .. } => {
                                pending.extend(cases.iter().map(|case| case.target))
                            }
                            AbstractOperation::Crash { .. } => {}
                            _ => return None,
                        }
                    }
                    Some(returns)
                });
            Some(DescriptorMachine {
                id: function.id,
                returns,
                places,
            })
        })
        .collect()
}

fn argument_source(argument: &terminal_psi::StructuralArgument) -> Option<DescriptorSource> {
    (argument.access != StructuralAccess::WriteOnlyBorrow).then(|| DescriptorSource {
        source: argument.place,
        projection: Projection::new(&argument.path),
    })
}
