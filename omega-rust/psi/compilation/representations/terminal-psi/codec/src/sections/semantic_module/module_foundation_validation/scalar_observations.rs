//! Structural read predicates retain exact declaration geometry. This is
//! representation validation only: it grants neither live access nor bounds.
use crate::codec_error::{CodecError, malformed};
use semantic_vocabulary::{
    PlaceId, Proposition, ScalarTerm, ScalarType, StructuralTypeId, ValueId,
};
use terminal_psi::{BoundaryMachineDeclaration, StructuralAccess, TerminalMachine, TerminalModule};

pub(super) fn machine(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Result<(), CodecError> {
    let root = |place| {
        let parameter = machine
            .structural_parameters
            .iter()
            .chain(machine.blocks.iter().flat_map(|b| &b.structural_parameters))
            .find(|p| p.place == place);
        if parameter.is_some_and(|p| p.access == StructuralAccess::WriteOnlyBorrow) {
            return None;
        }
        super::structural_place_type(machine, place).or_else(|| match &machine.result {
            terminal_psi::TerminalMachineResult::Structural(r) if r.place == place => {
                Some(r.structural_type)
            }
            _ => None,
        })
    };
    let value = |id| {
        super::declared_scalar_type(machine, id).or_else(|| {
            machine
                .contract
                .erased_scalar_formals
                .iter()
                .chain(machine.blocks.iter().flat_map(|b| &b.erased_scalar_formals))
                .find(|v| v.id == id)
                .map(|v| v.scalar_type)
        })
    };
    for proposition in machine
        .contract
        .requires
        .iter()
        .chain(machine.contract.ensures.iter().map(|row| &row.proposition))
        .chain(
            machine
                .contract
                .outcome_specific_ensures
                .iter()
                .map(|row| &row.proposition),
        )
        .chain(
            machine
                .contract
                .crash_routes
                .iter()
                .flat_map(|row| &row.alternatives)
                .filter_map(|guard| match guard {
                    terminal_psi::CrashRouteGuard::Predicate(p) => Some(p.proposition()),
                    _ => None,
                }),
        )
        .chain(
            module
                .scalar_block_invariants
                .iter()
                .filter(|row| row.machine == machine.id)
                .map(|row| &row.predicate),
        )
    {
        validate(module, proposition, &root, &value)?;
    }
    for operation in machine.operations() {
        use terminal_psi::OperationKind;
        let (arguments, routes): (&[ScalarTerm], &[terminal_psi::CrashRouteBucket]) =
            match &operation.kind {
                OperationKind::Call {
                    erased_arguments,
                    crash_continuations,
                    ..
                }
                | OperationKind::CallUnit {
                    erased_arguments,
                    crash_continuations,
                    ..
                }
                | OperationKind::CallDynamic {
                    erased_arguments,
                    crash_continuations,
                    ..
                }
                | OperationKind::CallStructuralScalar {
                    erased_arguments,
                    crash_continuations,
                    ..
                }
                | OperationKind::CallStructuralWithScalarArguments {
                    erased_arguments,
                    crash_continuations,
                    ..
                } => (erased_arguments, crash_continuations),
                OperationKind::CallStructural {
                    crash_continuations,
                    ..
                } => (&[], crash_continuations),
                _ => (&[], &[]),
            };
        validate_terms(
            module,
            arguments.iter().map(|term| (term, false)).collect(),
            &root,
            &value,
        )?;
        for route in routes.iter().flat_map(|row| &row.alternatives) {
            if let terminal_psi::CrashRouteGuard::Predicate(predicate) = route {
                validate(module, predicate.proposition(), &root, &value)?;
            }
        }
    }
    for block in &machine.blocks {
        use terminal_psi::Terminator;
        let arguments = match &block.terminator {
            Terminator::Jump {
                erased_arguments, ..
            } => erased_arguments.iter().collect(),
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => when_true
                .erased_arguments
                .iter()
                .chain(&when_false.erased_arguments)
                .collect(),
            Terminator::Crash { site_guard, .. } => {
                for predicate in site_guard {
                    validate(module, predicate.proposition(), &root, &value)?;
                }
                Vec::new()
            }
            _ => Vec::new(),
        };
        validate_terms(
            module,
            arguments.into_iter().map(|term| (term, false)).collect(),
            &root,
            &value,
        )?;
    }
    Ok(())
}

pub(super) fn boundary(
    module: &TerminalModule,
    boundary: &BoundaryMachineDeclaration,
) -> Result<(), CodecError> {
    let root = |place| {
        boundary
            .structural_parameters
            .iter()
            .find(|p| p.place == place && p.access != StructuralAccess::WriteOnlyBorrow)
            .map(|p| p.structural_type)
    };
    let value = |id: ValueId| {
        usize::try_from(id.get())
            .ok()
            .and_then(|position| position.checked_sub(1))
            .and_then(|position| {
                boundary
                    .scalar_parameters
                    .get(position)
                    .copied()
                    .or_else(|| {
                        (position == boundary.scalar_parameters.len())
                            .then(|| boundary.result.scalar())
                            .flatten()
                    })
            })
    };
    for proposition in boundary
        .scalar_requires
        .iter()
        .chain(&boundary.scalar_ensures)
        .chain(
            boundary
                .crash_routes
                .iter()
                .flat_map(|row| &row.alternatives)
                .filter_map(|guard| match guard {
                    terminal_psi::CrashRouteGuard::Predicate(p) => Some(p.proposition()),
                    _ => None,
                }),
        )
    {
        validate(module, proposition, &root, &value)?;
    }
    Ok(())
}

fn validate(
    module: &TerminalModule,
    proposition: &Proposition,
    root_type: &impl Fn(PlaceId) -> Option<StructuralTypeId>,
    value_type: &impl Fn(ValueId) -> Option<ScalarType>,
) -> Result<(), CodecError> {
    let mut propositions = vec![proposition];
    let mut scalars = Vec::new();
    while let Some(proposition) = propositions.pop() {
        match proposition {
            Proposition::StructuralIdentity { left, right } => {
                if left.structural_type() != right.structural_type() {
                    return malformed("structural identity operands have different declared types");
                }
                for subject in [left, right] {
                    let Some(root) = root_type(subject.root()) else {
                        return malformed("structural identity has no readable declared root");
                    };
                    let Some(endpoint) = terminal_semantics::canonical_structural_path_tip(
                        module.structural_types.iter(),
                        root,
                        subject.path(),
                    ) else {
                        return malformed("structural identity has no exact declared endpoint");
                    };
                    if endpoint.id != subject.structural_type()
                        || !matches!(
                            endpoint.shape,
                            terminal_psi::StructuralTypeShape::Record { .. }
                                | terminal_psi::StructuralTypeShape::Sum { .. }
                                | terminal_psi::StructuralTypeShape::Mixed { .. }
                                | terminal_psi::StructuralTypeShape::FixedArray { .. }
                        )
                    {
                        return malformed(
                            "structural identity disagrees with its exact data carrier",
                        );
                    }
                }
            }
            Proposition::Equal(a, b)
            | Proposition::LessThan(a, b)
            | Proposition::LessOrEqual(a, b)
            | Proposition::ScalarIeeeFloatComparison {
                left: a, right: b, ..
            } => {
                scalars.push((a, false));
                scalars.push((b, false));
            }
            Proposition::Conjunction(items) | Proposition::Disjunction(items) => {
                propositions.extend(items)
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                propositions.push(premise);
                propositions.push(conclusion);
            }
            _ => {}
        }
    }
    validate_terms(module, scalars, root_type, value_type)
}

fn validate_terms(
    module: &TerminalModule,
    mut scalars: Vec<(&ScalarTerm, bool)>,
    root_type: &impl Fn(PlaceId) -> Option<StructuralTypeId>,
    value_type: &impl Fn(ValueId) -> Option<ScalarType>,
) -> Result<(), CodecError> {
    while let Some((term, selector)) = scalars.pop() {
        if let ScalarTerm::StructuralRead {
            root,
            path,
            scalar_type,
        } = term
        {
            let Some(root) = root_type(*root) else {
                return malformed("structural read predicate has no readable declared root");
            };
            if terminal_semantics::structural_read_scalar_type(
                module.structural_types.iter(),
                root,
                path,
            ) != Some(*scalar_type)
            {
                return malformed(
                    "structural read predicate disagrees with its declaration path or scalar carrier",
                );
            }
            scalars.extend(term.operands().into_iter().map(|term| (term, true)));
        } else {
            if selector
                && let ScalarTerm::Value { id, scalar_type } = term
                && value_type(*id) != Some(*scalar_type)
            {
                return malformed("structural read selector has no exact scalar declaration");
            }
            scalars.extend(term.operands().into_iter().map(|term| (term, selector)));
        }
    }
    Ok(())
}
