//! Current structural aliases and descriptor births. Both local loan replay and
//! ordinary-call effects use this same source-chain reconstruction.
use super::{accesses, flow};
use semantic_vocabulary::{OperationId, PlaceId};
use std::collections::BTreeMap;
use terminal_psi::{
    OperationKind, StructuralAccess, StructuralArgument, TerminalMachine, TerminalModule,
    Terminator,
};
use terminal_semantics::borrowed_storage::{AliasSources, Position, Projection};
use terminal_semantics::descriptor_lifetimes::DescriptorLocation;
use terminal_semantics::descriptor_results::DescriptorAnalysis;

pub(super) struct Origins {
    pub aliases: AliasSources,
    pub positions: BTreeMap<OperationId, Position>,
    pub births: Vec<(
        Position,
        DescriptorLocation,
        PlaceId,
        Projection,
        StructuralAccess,
    )>,
}

pub(super) fn reconstruct(module: &TerminalModule, machine: &TerminalMachine) -> Origins {
    let graph = terminal_semantics::descriptor_results::terminal_graph(module);
    let mut analysis = DescriptorAnalysis::new(&module.structural_types, &graph);
    reconstruct_in(module, machine, &mut analysis)
}

pub(super) fn reconstruct_in(
    module: &TerminalModule,
    machine: &TerminalMachine,
    analysis: &mut DescriptorAnalysis<'_>,
) -> Origins {
    let mut aliases = AliasSources::new();
    let mut positions = BTreeMap::new();
    let mut births = Vec::new();
    for block in &machine.blocks {
        for (index, operation) in block.operations.iter().enumerate() {
            let position = (
                block.id,
                u32::try_from(index).expect("operation index fits u32"),
            );
            positions.insert(operation.id, position);
            // A store/call installs a new slot value even when its source is
            // an incoming descriptor with no local establishment. Capture its
            // current backing at this position; never alias the whole owner.
            if let OperationKind::StructuralByteViewFieldStore {
                destination,
                path,
                field,
                source,
                ..
            } = &operation.kind
                && let Some(destination) =
                    flow::location(module, machine, *destination, path, Some(*field))
                        .and_then(|location| analysis.location(machine.id, position, &location))
            {
                for origin in analysis
                    .source_origins(machine.id, position, &DescriptorLocation::whole(*source))
                    .into_iter()
                    .flatten()
                {
                    if let terminal_semantics::descriptor_results::DescriptorCallOrigin::Argument { source, projection } = origin {
                            births.push((position, destination.clone(), source, projection, StructuralAccess::SharedBorrow));
                        }
                }
            }
            for effect in analysis
                .call_stored_effects(machine.id, position)
                .into_iter()
                .flatten()
            {
                for origin in effect.origins {
                    if let terminal_semantics::descriptor_results::DescriptorCallOrigin::Argument { source, projection } = origin {
                        births.push((position, effect.destination.clone(), source, projection, StructuralAccess::SharedBorrow));
                    }
                }
            }
            let Some(result) = operation.result.structural() else {
                continue;
            };
            if matches!(
                operation.kind,
                OperationKind::CallStructural { .. }
                    | OperationKind::CallStructuralWithScalarArguments { .. }
            ) && result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                && terminal_semantics::descriptor_results::is_shared_descriptor(
                    module.structural_types.iter(),
                    result.structural_type,
                )
                && let Some(origins) = analysis.source_origins(
                    machine.id,
                    (position.0, position.1 + 1),
                    &DescriptorLocation::whole(result.place),
                )
            {
                for origin in origins {
                    if let terminal_semantics::descriptor_results::DescriptorCallOrigin::Argument { source, projection } = origin {
                        aliases.entry(result.place).or_default().push((source, projection.clone()));
                        births.push((position, DescriptorLocation::whole(result.place), source, projection, StructuralAccess::SharedBorrow));
                    }
                }
            }
            let descriptor = match &operation.kind {
                OperationKind::EstablishElementView {
                    source,
                    element,
                    region,
                    ..
                }
                | OperationKind::EstablishMutableElementView {
                    source,
                    element,
                    region,
                    ..
                } => {
                    let mut projection = accesses::projection(
                        module,
                        machine,
                        source.place,
                        &source.path,
                        None,
                        position,
                    );
                    // Reinterpreted element fields may have different source identities.
                    let preserves_coordinates =
                        crate::validation::element_view::establishment::source_collection_type(
                            module, machine, source,
                        )
                        .is_some_and(|collection| {
                            terminal_semantics::collection_view_preserves_coordinates(
                                module.structural_types.iter(),
                                collection,
                                *element,
                                *region,
                            )
                        });
                    if !preserves_coordinates {
                        projection.exact = false;
                    }
                    Some((source.place, projection))
                }
                OperationKind::EstablishReference { source } => Some((
                    source.place,
                    accesses::projection(
                        module,
                        machine,
                        source.place,
                        &source.path,
                        None,
                        position,
                    ),
                )),
                OperationKind::StructuralLeafCopy { source, path }
                    if terminal_semantics::descriptor_results::is_shared_descriptor(
                        module.structural_types.iter(),
                        result.structural_type,
                    ) =>
                {
                    let mut projection =
                        accesses::projection(module, machine, *source, path, None, position);
                    projection.indirect |= !path.is_empty();
                    Some((*source, projection))
                }
                OperationKind::ElementViewSubslice {
                    source, start, end, ..
                }
                | OperationKind::ByteSequenceSubslice {
                    source, start, end, ..
                } => Some((*source, Projection::window(*start, *end, position))),
                _ => None,
            };
            if let Some((source, projection)) = descriptor {
                let current = analysis.source_origins(
                    machine.id,
                    (position.0, position.1 + 1),
                    &DescriptorLocation::whole(result.place),
                );
                let sources = current.map(|origins| origins.into_iter().filter_map(|origin| match origin {
                    terminal_semantics::descriptor_results::DescriptorCallOrigin::Argument { source, projection } => Some((source, projection)),
                    terminal_semantics::descriptor_results::DescriptorCallOrigin::Literal { .. } | terminal_semantics::descriptor_results::DescriptorCallOrigin::Empty => None,
                }).collect::<Vec<_>>()).unwrap_or_else(|| vec![(source, projection.clone())]);
                aliases
                    .entry(result.place)
                    .or_default()
                    .extend(sources.clone());
                // A temporary exclusive whole-array descriptor supplies the
                // parent capability; its narrowed child owns the live loan.
                // Shared establishment itself must keep its backing stable.
                let is_birth = match &operation.kind {
                    OperationKind::EstablishElementView {
                        source,
                        element,
                        region,
                        ..
                    }
                    | OperationKind::EstablishMutableElementView {
                        source,
                        element,
                        region,
                        ..
                    } => crate::validation::element_view::establishment::source_collection_type(
                        module, machine, source,
                    )
                    .is_none_or(|collection| {
                        !terminal_semantics::collection_view_preserves_coordinates(
                            module.structural_types.iter(),
                            collection,
                            *element,
                            *region,
                        )
                    }),
                    _ => false,
                } || matches!(
                    operation.kind,
                    OperationKind::ElementViewSubslice { .. }
                        | OperationKind::ByteSequenceSubslice { .. }
                        | OperationKind::StructuralLeafCopy { .. }
                ) || matches!(&operation.kind, OperationKind::EstablishElementView { source, .. }
                            if source.access == StructuralAccess::SharedBorrow);
                if is_birth
                    && let Some((_, access)) =
                        crate::validation::element_view::view_loan_result_in_module(
                            module,
                            machine,
                            result.place,
                        )
                {
                    for (source, projection) in sources {
                        births.push((
                            position,
                            DescriptorLocation::whole(result.place),
                            source,
                            projection,
                            access,
                        ));
                    }
                }
            }
        }
        let position = (
            block.id,
            u32::try_from(block.operations.len()).expect("operation index fits u32"),
        );
        let mut bind = |target, arguments: &[StructuralArgument]| {
            if let Some(destination) = machine
                .blocks
                .iter()
                .find(|candidate| candidate.id == target)
            {
                for (parameter, argument) in destination.structural_parameters.iter().zip(arguments)
                {
                    aliases.entry(parameter.place).or_default().push((
                        argument.place,
                        accesses::projection(
                            module,
                            machine,
                            argument.place,
                            &argument.path,
                            None,
                            position,
                        ),
                    ));
                }
            }
        };
        match &block.terminator {
            Terminator::Jump {
                target,
                structural_arguments,
                ..
            } => bind(*target, structural_arguments),
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                bind(when_true.target, &when_true.structural_arguments);
                bind(when_false.target, &when_false.structural_arguments);
            }
            Terminator::StructuralCase { source, cases } => {
                // Payload extraction may project through captured storage;
                // without an exact field adapter retain the entire subject.
                for case in cases {
                    if let Some(destination) = machine
                        .blocks
                        .iter()
                        .find(|candidate| candidate.id == case.target)
                    {
                        for parameter in &destination.structural_parameters {
                            let mut projection = Projection::new(&[]);
                            projection.exact = false;
                            aliases
                                .entry(parameter.place)
                                .or_default()
                                .push((*source, projection));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Origins {
        aliases,
        positions,
        births,
    }
}
