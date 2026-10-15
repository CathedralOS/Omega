//! Exact scalar telescopes and simultaneous edge inputs for ordinary Unit blocks.
use super::{KnownUnitInteger, LiveDefinitions, references};
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractBlockEntry, AbstractFunction, AbstractOperation, ValueBinding};
use semantic_vocabulary::{BlockId, ScalarType, ValueId};
use std::collections::BTreeSet;
use target_operations::TargetScalarBlockValue;

pub(super) fn validate_parameters(
    function: &AbstractFunction,
    definitions: &mut BTreeSet<ValueId>,
) -> Result<(), LoweringError> {
    for block in function
        .block_entries
        .iter()
        .filter(|block| block.block != function.entry)
    {
        for parameter in &block.parameters {
            if !matches!(
                parameter.scalar_type,
                ScalarType::Boolean | ScalarType::IeeeFloat(_)
            ) && !matches!(parameter.scalar_type, ScalarType::Integer(integer)
                    if crate::lowering::scalar_abi::fixed_native_integer_shape(integer).is_some())
            {
                return Err(LoweringError::ValueTypeMismatch(parameter.value));
            }
            if !definitions.insert(parameter.value) {
                return Err(LoweringError::DuplicateValue(parameter.value));
            }
        }
    }
    Ok(())
}

/// Capture every selected descriptor before changing storage keys, including
/// backedge swaps. Possible origins follow the bytes; the receiving parameter
/// supplies the exact carrier type. No parent-loan graph is reconstructed.
pub(super) fn re_home_bound_references(
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    bindings: &[abstract_operations::AbstractStructuralBinding],
    parameters: &[terminal_psi::StructuralParameterDeclaration],
    live: &mut LiveDefinitions,
) -> Result<(), LoweringError> {
    if bindings.len() != parameters.len() {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let mut captured = Vec::new();
    let mut selected_sources = BTreeSet::new();
    let mut destinations = BTreeSet::new();
    for (binding, parameter) in bindings.iter().zip(parameters) {
        if binding.parameter != parameter.place {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        if parameter.access != terminal_psi::StructuralAccess::Owned
            || binding.argument.access != terminal_psi::StructuralAccess::Owned
        {
            continue;
        }
        let copies = parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted;
        if copies {
            let source_contract = live
                .structural_homes
                .get(&binding.argument.place)
                .map(|home| (home.access(), home.multiplicity()))
                .or_else(|| {
                    function
                        .structural_parameters
                        .iter()
                        .chain(function.block_structural_parameters())
                        .find(|source| source.place == binding.argument.place)
                        .map(|source| (source.access, source.multiplicity))
                });
            if source_contract
                != Some((
                    terminal_psi::StructuralAccess::Owned,
                    terminal_psi::StructuralMultiplicity::Unrestricted,
                ))
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
        }
        // Move resident descriptor storage simultaneously. An inactive case
        // contributes no descriptor; declaration geometry checks each present
        // slot without manufacturing the other alternatives.
        for (source, descriptor) in &live.references {
            if source.0 != binding.argument.place {
                continue;
            }
            let Some(relative) = source.1.strip_prefix(binding.argument.path.as_slice()) else {
                continue;
            };
            if relative.contains(&terminal_psi::StructuralPathSegment::Referent) {
                continue;
            }
            let destination = (parameter.place, relative.to_vec());
            if references::leaf_referent(types, parameter.structural_type, relative)
                != Some(descriptor.referent)
                || (!copies && !selected_sources.insert(source.clone()))
                || !destinations.insert(destination.clone())
                || (copies
                    && !types
                        .get(&descriptor.carrier_type)
                        .is_some_and(|declaration| {
                            matches!(
                                declaration.shape,
                                terminal_psi::StructuralTypeShape::Reference {
                                    access: terminal_psi::StructuralAccess::SharedBorrow,
                                    ..
                                }
                            )
                        }))
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            captured.push((
                source.clone(),
                destination,
                descriptor.clone(),
                references::leaf_type(types, parameter.structural_type, relative)
                    .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?,
            ));
        }
    }
    // Inputs were captured simultaneously above. A receiving block parameter
    // begins a new occurrence, including on a backedge: its prior descriptors
    // and inactive-case slots cannot remain resident. Copies from other roots
    // survive; only moved sources and the receiving occurrences retire here.
    live.references.retain(|(root, _), _| {
        !parameters.iter().any(|parameter| {
            parameter.place == *root && parameter.access == terminal_psi::StructuralAccess::Owned
        })
    });
    for source in &selected_sources {
        live.references.remove(source);
    }
    for (_, destination, mut storage, carrier_type) in captured {
        storage.carrier_type = carrier_type;
        live.references.insert(destination, storage);
    }
    Ok(())
}

pub(super) fn enter(block: &AbstractBlockEntry, live: &mut LiveDefinitions) {
    for parameter in &block.parameters {
        match parameter.scalar_type {
            ScalarType::Integer(scalar_type) => {
                live.integers.insert(
                    parameter.value,
                    KnownUnitInteger::BlockParameter {
                        block: block.block,
                        value: parameter.value,
                        scalar_type,
                    },
                );
            }
            ScalarType::Boolean | ScalarType::IeeeFloat(_) => {
                live.scalar_block_parameters.insert(
                    parameter.value,
                    TargetScalarBlockValue {
                        block: block.block,
                        value: parameter.value,
                        scalar_type: parameter.scalar_type,
                    },
                );
            }
        }
    }
}

/// A borrowed block parameter carried as its referent's address. The type rule
/// is shared with every Omega stage; each stage replays its own edge sources.
pub(super) fn is_address_join(
    parameter: &terminal_psi::StructuralParameterDeclaration,
    structural_types: &StructuralTypeLookup<'_>,
) -> bool {
    abstract_operations::control_flow::address_joins::is_address_join(parameter, |identity| {
        structural_types.get(&identity).copied()
    })
}

/// The declared referent behind a live edge-forwarded address. Consumers use
/// this same arrival for reads, copies and restoration; no entry ABI row is
/// fabricated, and a declaration alone cannot revive an unavailable carrier.
pub(super) fn address_parameter<'a>(
    function: &'a AbstractFunction,
    live: &LiveDefinitions,
    place: semantic_vocabulary::PlaceId,
) -> Option<&'a terminal_psi::StructuralParameterDeclaration> {
    if !live.address_joins.contains(&place) {
        return None;
    }
    function.block_structural_parameter(place)
}

/// An address-join edge lends one readable root, or a static projection
/// beneath it, that is live in the predecessor: a plain owned home (an
/// established local or an owned join's storage), a claim-free
/// machine parameter, or an earlier address join. The root keeps its owner
/// and storage; the path must name exactly the join's referent type. A root
/// suspended by an outstanding exclusive loan cannot lend a shared view.
fn address_join_argument(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    structural_types: &StructuralTypeLookup<'_>,
    binding: &abstract_operations::AbstractStructuralBinding,
    parameter: &terminal_psi::StructuralParameterDeclaration,
) -> bool {
    let argument = &binding.argument;
    if argument.access != parameter.access
        || !abstract_operations::control_flow::address_joins::is_address_projection(&argument.path)
    {
        return false;
    }
    // Qualifications carry no target storage. The admitted owner loan uses
    // the same address whether its backing is a parameter or a completed local.
    // The current graph already carries the checked qualification judgment.
    // Requiring an empty or identical predicate roster here confuses storage
    // transport with semantic admission and rejects ordinary qualified arrays.
    // Access, live custody and exact projected type still determine the address.
    let accessible = |root, multiplicity, access| {
        terminal_semantics::reference_source_projection_accessible(
            structural_types.values().copied(),
            root,
            multiplicity,
            access,
            &argument.path,
            argument.access,
        )
    };
    let root_type = if let Some(home) = live.structural_homes.get(&argument.place) {
        ((parameter.access == terminal_psi::StructuralAccess::SharedBorrow
            || home
                .operation_result()
                .is_some_and(|(_, result)| result.place == argument.place))
            && !home.has_claims()
            && home.multiplicity() != terminal_psi::StructuralMultiplicity::Linear
            && accessible(home.structural_type(), home.multiplicity(), home.access()))
        .then(|| home.structural_type())
    } else if let Some(source) = function.structural_parameter(argument.place) {
        (abstract_operations::control_flow::address_joins::loan_access_permitted(
            structural_types.values().copied(),
            source.structural_type,
            &argument.path,
            source.access,
            parameter.access,
        ) && source.multiplicity != terminal_psi::StructuralMultiplicity::Linear
            && accessible(source.structural_type, source.multiplicity, source.access))
        .then_some(source.structural_type)
    } else if live.address_joins.contains(&argument.place) {
        function
            .block_structural_parameters()
            .find(|source| source.place == argument.place)
            .filter(|source| {
                abstract_operations::control_flow::address_joins::loan_access_permitted(
                    structural_types.values().copied(),
                    source.structural_type,
                    &argument.path,
                    source.access,
                    parameter.access,
                ) && accessible(source.structural_type, source.multiplicity, source.access)
            })
            .map(|source| source.structural_type)
    } else {
        // A live view lends a selected element under the permission carried
        // by its actual producer, not the geometry of its descriptor slot.
        (parameter.access == terminal_psi::StructuralAccess::SharedBorrow)
            .then_some(())
            .and_then(|_| {
                let (producer, structural_type) = live.views.get(&argument.place)?;
                let (_, result) = function
                    .operations
                    .iter()
                    .find(|operation| operation.psi_operation() == Some(*producer))?
                    .structural_result()?;
                let access = terminal_semantics::descriptor_results::result_access(
                    structural_types.values().copied(),
                    result,
                )?;
                (result.structural_type == *structural_type
                    && accessible(*structural_type, result.multiplicity, access))
                .then_some(*structural_type)
            })
    };
    root_type.and_then(|root| {
        abstract_operations::control_flow::address_joins::borrowed_referent_type(
            structural_types.values().copied(),
            root,
            &argument.path,
            argument.access,
        )
    }) == Some(parameter.structural_type)
}

pub(super) fn validate_successors(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    live: &LiveDefinitions,
    structural_types: &StructuralTypeLookup<'_>,
) -> Result<(), LoweringError> {
    let validate =
        |target: BlockId,
         bindings: &[ValueBinding],
         structural: &[abstract_operations::AbstractStructuralBinding]| {
            let block = function
                .block_entry(target)
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            if bindings.len() != block.parameters.len() {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            if structural.len() != block.structural_parameters.len() {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            for (binding, parameter) in structural.iter().zip(&block.structural_parameters) {
                if is_address_join(parameter, structural_types) {
                    if binding.parameter != parameter.place
                        || !address_join_argument(
                            function,
                            live,
                            structural_types,
                            binding,
                            parameter,
                        )
                    {
                        return Err(LoweringError::unsupported_control_flow(function.machine));
                    }
                    continue;
                }
                if terminal_semantics::view_descriptor_parameter(
                    parameter,
                    structural_types.values().copied(),
                ) && parameter.qualifications.is_empty()
                {
                    let selected =
                        crate::lowering::scalar::descriptor_operands::descriptor_projection(
                            function,
                            structural_types,
                            live.element_view_backings(),
                            &binding.argument,
                        )?;
                    if binding.parameter != parameter.place
                        || binding.argument.access != parameter.access
                        || (selected.0 != parameter.structural_type
                            && !crate::lowering::scalar::descriptor_operands::storage_backs_view_parameter(
                                selected.0,
                                &binding.argument.path,
                                parameter,
                                structural_types,
                            ))
                    {
                        return Err(LoweringError::unsupported_control_flow(function.machine));
                    }
                    continue;
                }
                // An owned subtree carries its contained reference descriptors,
                // not their referents. Re-homing strips exactly this path from
                // each moved carrier key; siblings remain on the source root.
                if references::contains_reference(structural_types, parameter.structural_type)
                    && (parameter.access != terminal_psi::StructuralAccess::Owned
                        || binding.argument.access != terminal_psi::StructuralAccess::Owned)
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                let place = binding.argument.place;
                let source_type = function
                    .structural_parameter(place)
                    .map(|source| source.structural_type)
                    .or_else(|| {
                        live.structural_homes
                            .get(&place)
                            .map(|home| home.structural_type())
                    })
                    .or_else(|| {
                        live.views
                            .get(&place)
                            .map(|(_, structural_type)| *structural_type)
                    })
                    .or_else(|| {
                        (live.block_views.contains(&place) || live.owned_arrivals.contains(&place))
                            .then(|| {
                                function
                                    .block_structural_parameters()
                                    .find(|source| source.place == place)
                                    .map(|source| source.structural_type)
                            })
                            .flatten()
                    });
                // Psi checks the edge's value memberships, including explicit
                // weakening. Physical transport retains storage access and
                // whole-root multiplicity; predicate rosters change no bytes.
                // A containing record's multiplicity is not its field's.
                let source_storage_contract = live
                    .structural_homes
                    .get(&place)
                    .map(|home| {
                        home.access() == parameter.access
                            && (!binding.argument.path.is_empty()
                                || home.multiplicity() == parameter.multiplicity)
                    })
                    .or_else(|| {
                        function
                            .structural_parameters
                            .iter()
                            .chain(function.block_structural_parameters())
                            .find(|source| source.place == place)
                            .map(|source| {
                                source.access == parameter.access
                                    && (!binding.argument.path.is_empty()
                                        || source.multiplicity == parameter.multiplicity)
                            })
                    });
                let leaf_copy_owned_contract = || {
                    // A whole leaf copy moves the exact carrier: its result
                    // mints the standalone type and clears every roster — the
                    // destination's qualifications were already proven against
                    // the leaf's declared predicates upstream.
                    function
                        .operations
                        .iter()
                        .find_map(|operation| match operation {
                            AbstractOperation::StructuralLeafCopy {
                                source: backing,
                                path: abstract_operations::StructuralCopyPath::Ordinary(path),
                                result,
                                ..
                            } if result.place == place
                                && result.structural_type == parameter.structural_type
                                && result.multiplicity
                                    == terminal_psi::StructuralMultiplicity::Unrestricted
                                && result.qualifications.is_empty()
                                && result.projected_qualifications.is_empty()
                                && result.claims.is_empty() =>
                            {
                                let root = function
                                    .arrival_parameter(*backing)
                                    .map(|root| root.structural_type)
                                    .or_else(|| {
                                        function
                                            .block_structural_parameters()
                                            .find(|source| source.place == *backing)
                                            .map(|source| source.structural_type)
                                    });
                                Some(root.is_some_and(|root| {
                                    structural_types.subtree(root, path)
                                        == Some(parameter.structural_type)
                                }))
                            }
                            _ => None,
                        })
                };
                let argument_type = source_type.and_then(|root| {
                    if binding.argument.path.is_empty() {
                        Some(root)
                    } else {
                        structural_types.subtree(root, &binding.argument.path)
                    }
                });
                // Moving an enclosing subtree transports its loan leaves.
                // Descending through a leaf would instead move borrowed backing.
                let crosses_loan_leaf = live.references.keys().any(|(carrier, leaf_path)| {
                    *carrier == place
                        && binding.argument.path.len() > leaf_path.len()
                        && binding.argument.path.starts_with(leaf_path)
                });
                if binding.parameter != parameter.place
                    || (parameter.access == terminal_psi::StructuralAccess::Owned
                        && source_storage_contract != Some(true)
                        && leaf_copy_owned_contract() != Some(true))
                    || !argument_type.is_some_and(|actual| {
                        if parameter.access == terminal_psi::StructuralAccess::Owned {
                            actual == parameter.structural_type
                        } else {
                            terminal_semantics::borrowed_referent_matches(
                                structural_types.values().copied(),
                                actual,
                                parameter.structural_type,
                            )
                        }
                    })
                    || binding.argument.access != parameter.access
                    || crosses_loan_leaf
                    || (!binding.argument.path.is_empty()
                        && (!abstract_operations::control_flow::address_joins::is_static_projection(
                            &binding.argument.path,
                        )
                            || (parameter.access != terminal_psi::StructuralAccess::Owned
                                && !structural_types
                                    .get(&parameter.structural_type)
                                    .is_some_and(|declaration| {
                                        declaration.shape.is_bounded_owned_byte_sequence()
                                    }))))
                    || !matches!(
                        parameter.access,
                        terminal_psi::StructuralAccess::SharedBorrow
                            | terminal_psi::StructuralAccess::MutableBorrow
                            | terminal_psi::StructuralAccess::Owned
                    )
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
            }
            // Every source is read from the predecessor environment. Destination
            // declarations are installed only when that block becomes available.
            for (binding, parameter) in bindings.iter().zip(&block.parameters) {
                let scalar_type = live
                    .integers
                    .get(&binding.argument)
                    .map(|known| ScalarType::Integer(known.scalar_type()))
                    .or_else(|| {
                        live.scalar_homes
                            .get(&binding.argument)
                            .map(|home| home.scalar_type)
                    })
                    .or_else(|| {
                        live.booleans
                            .contains_key(&binding.argument)
                            .then_some(ScalarType::Boolean)
                    })
                    .or_else(|| {
                        live.scalar_block_parameters
                            .get(&binding.argument)
                            .map(|source| source.scalar_type)
                    })
                    .or_else(|| {
                        live.ieee_float_constants
                            .get(&binding.argument)
                            .map(|(_, value)| ScalarType::IeeeFloat(value.format()))
                    })
                    .or_else(|| {
                        function
                            .parameters
                            .iter()
                            .find(|parameter| parameter.value == binding.argument)
                            .map(|parameter| parameter.scalar_type)
                    });
                if binding.parameter != parameter.value
                    || binding.scalar_type != parameter.scalar_type
                    || scalar_type != Some(parameter.scalar_type)
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
            }
            Ok(())
        };
    match operation {
        AbstractOperation::Jump {
            target,
            bindings,
            structural_bindings,
            ..
        } => validate(*target, bindings, structural_bindings),
        AbstractOperation::Conditional {
            when_true,
            when_false,
            ..
        } => {
            validate(
                when_true.target,
                &when_true.bindings,
                &when_true.structural_bindings,
            )?;
            validate(
                when_false.target,
                &when_false.bindings,
                &when_false.structural_bindings,
            )
        }
        AbstractOperation::Return { .. }
        | AbstractOperation::ExternalCompletion { .. }
        | AbstractOperation::Crash { .. }
        | AbstractOperation::ReturnStructural { .. }
        | AbstractOperation::ReturnUnit { .. }
        | AbstractOperation::StructuralCase { .. } => Ok(()),
        _ => Err(LoweringError::unsupported_control_flow(function.machine)),
    }
}
