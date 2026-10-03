//! Current structural aliases and descriptor births. Both local loan replay and
//! ordinary-call effects use this same source-chain reconstruction.
use super::accesses;
use semantic_vocabulary::{OperationId, PlaceId};
use std::collections::BTreeMap;
use terminal_psi::{
    OperationKind, StructuralAccess, StructuralArgument, TerminalMachine, TerminalModule,
    Terminator,
};
use terminal_semantics::borrowed_storage::{AliasSources, Position, Projection};

pub(super) struct Origins {
    pub aliases: AliasSources,
    pub positions: BTreeMap<OperationId, Position>,
    pub births: Vec<(Position, PlaceId, PlaceId, Projection, StructuralAccess)>,
}

pub(super) fn reconstruct(module: &TerminalModule, machine: &TerminalMachine) -> Origins {
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
            let Some(result) = operation.result.structural() else {
                continue;
            };
            if let Some(origins) =
                terminal_semantics::descriptor_results::call_origins(module, machine, operation)
            {
                for origin in origins {
                    if let terminal_semantics::descriptor_results::DescriptorCallOrigin::Argument { source, projection } = origin {
                        aliases.entry(result.place).or_default().push((source, projection.clone()));
                        births.push((position, result.place, source, projection, StructuralAccess::SharedBorrow));
                    }
                }
            }
            let descriptor = match &operation.kind {
                OperationKind::EstablishElementView { source, .. }
                | OperationKind::EstablishMutableElementView { source, .. }
                | OperationKind::EstablishReference { source } => Some((
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
                aliases
                    .entry(result.place)
                    .or_default()
                    .push((source, projection.clone()));
                // A temporary exclusive whole-array descriptor supplies the
                // parent capability; its narrowed child owns the live loan.
                // Shared establishment itself must keep its backing stable.
                let is_birth = matches!(
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
                    births.push((position, result.place, source, projection, access));
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
