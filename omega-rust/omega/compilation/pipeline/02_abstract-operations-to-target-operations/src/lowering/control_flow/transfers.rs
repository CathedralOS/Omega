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

/// Owned edge values re-home only loan leaves beneath their selected paths.
/// Every source is captured before changing keys, including back-edge swaps.
/// Formation identity, result row, root, parent and referent stay put.
pub(super) fn re_home_bound_references(
    bindings: &[abstract_operations::AbstractStructuralBinding],
    parameters: &[terminal_psi::StructuralParameterDeclaration],
    live: &mut LiveDefinitions,
) {
    let mut moved = Vec::new();
    for (binding, parameter) in bindings.iter().zip(parameters) {
        if parameter.access != terminal_psi::StructuralAccess::Owned
            || binding.argument.access != terminal_psi::StructuralAccess::Owned
        {
            continue;
        }
        for ((carrier, path), custody) in &live.references {
            if *carrier == binding.argument.place
                && let Some(relative) = path.strip_prefix(binding.argument.path.as_slice())
            {
                moved.push((
                    (*carrier, path.clone()),
                    (parameter.place, relative.to_vec()),
                    custody.clone(),
                ));
            }
        }
    }
    for (source, _, _) in &moved {
        live.references.remove(source);
    }
    for (_, destination, custody) in moved {
        live.references.insert(destination, custody);
    }
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
    function
        .block_entries
        .iter()
        .flat_map(|block| &block.structural_parameters)
        .find(|parameter| parameter.place == place)
}

/// An address-join edge lends one readable root, or a static projection
/// beneath it, that is live in the predecessor: a plain owned home (an
/// established local or an owned join's storage), a claim-free unqualified
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
        || !abstract_operations::control_flow::address_joins::is_static_projection(&argument.path)
        || references::is_suspended_root(live, argument.place)
    {
        return false;
    }
    // Qualifications carry no target storage. The admitted owner loan uses
    // the same address whether its backing is a parameter or a completed local.
    // Forwarded parameters retain their precise access and predicate roster;
    // local homes retain their authenticated result and independently held custody.
    let owner_loan = abstract_operations::control_flow::owner_loans::is_bounded_owner_loan(
        parameter,
        |identity| structural_types.get(&identity).copied(),
    ) && argument.path.is_empty();
    if owner_loan && !live.structural_homes.contains_key(&argument.place) {
        return function
            .structural_parameters
            .iter()
            .chain(
                live.address_joins
                    .contains(&argument.place)
                    .then(|| {
                        function
                            .block_entries
                            .iter()
                            .flat_map(|entry| &entry.structural_parameters)
                    })
                    .into_iter()
                    .flatten(),
            )
            .any(|source| {
                source.place == argument.place
                    && source.structural_type == parameter.structural_type
                    && source.access == parameter.access
                    && source.multiplicity == parameter.multiplicity
                    && source.qualifications == parameter.qualifications
                    && source.projected_qualifications.is_empty()
            });
    }
    let root_type = if let Some(home) = live.structural_homes.get(&argument.place) {
        ((parameter.access == terminal_psi::StructuralAccess::SharedBorrow
            || home
                .operation_result()
                .is_some_and(|(_, result)| result.place == argument.place))
            && !home.has_claims()
            && home.multiplicity() != terminal_psi::StructuralMultiplicity::Linear
            && (home.qualifications().is_empty() || owner_loan)
            && home.projected_qualifications().is_empty())
        .then(|| home.structural_type())
    } else if let Some(source) = function
        .structural_parameters
        .iter()
        .find(|source| source.place == argument.place)
    {
        (loan_access_permitted(source.access, parameter.access)
            && source.multiplicity != terminal_psi::StructuralMultiplicity::Linear
            && source.qualifications.is_empty()
            && source.projected_qualifications.is_empty())
        .then_some(source.structural_type)
    } else if live.address_joins.contains(&argument.place) {
        function
            .block_entries
            .iter()
            .flat_map(|entry| &entry.structural_parameters)
            .find(|source| source.place == argument.place)
            .filter(|source| loan_access_permitted(source.access, parameter.access))
            .map(|source| source.structural_type)
    } else {
        // An element view is a readable descriptor root: a static index
        // beneath it names one element's address, lent shared only.
        (parameter.access == terminal_psi::StructuralAccess::SharedBorrow)
            .then(|| {
                live.views
                    .get(&argument.place)
                    .map(|(_, structural_type)| *structural_type)
            })
            .flatten()
    };
    root_type.and_then(|root| structural_types.subtree(root, &argument.path))
        == Some(parameter.structural_type)
}

/// Carrying an address cannot amplify the source loan's authority. Signature
/// parameters and prior arrivals obey the same rule; referent shape and live
/// storage remain separate requirements of the transfer.
fn loan_access_permitted(
    source: terminal_psi::StructuralAccess,
    destination: terminal_psi::StructuralAccess,
) -> bool {
    use terminal_psi::StructuralAccess::{MutableBorrow, Owned, SharedBorrow, WriteOnlyBorrow};
    matches!(
        (source, destination),
        (
            MutableBorrow,
            SharedBorrow | MutableBorrow | WriteOnlyBorrow
        ) | (Owned | SharedBorrow, SharedBorrow)
            | (WriteOnlyBorrow, WriteOnlyBorrow)
    )
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
                .block_entries
                .iter()
                .find(|block| block.block == target)
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
                    .structural_parameters
                    .iter()
                    .find(|source| source.place == place)
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
                                    .block_entries
                                    .iter()
                                    .flat_map(|entry| &entry.structural_parameters)
                                    .find(|source| source.place == place)
                                    .map(|source| source.structural_type)
                            })
                            .flatten()
                    });
                // Psi has checked the selected subtree's multiplicity and
                // predicates against the destination. Only whole-root moves
                // share the root's complete contract; a containing record's
                // multiplicity and qualifications are not its field's.
                let exact_source_owned_contract = live
                    .structural_homes
                    .get(&place)
                    .map(|home| {
                        home.access() == parameter.access
                            && (!binding.argument.path.is_empty()
                                || (home.multiplicity() == parameter.multiplicity
                                    && home.qualifications()
                                        == parameter.qualifications.as_slice()
                                    && home.projected_qualifications()
                                        == parameter.projected_qualifications.as_slice()))
                    })
                    .or_else(|| {
                        function
                            .structural_parameters
                            .iter()
                            .chain(
                                function
                                    .block_entries
                                    .iter()
                                    .flat_map(|entry| &entry.structural_parameters),
                            )
                            .find(|source| source.place == place)
                            .map(|source| {
                                source.access == parameter.access
                                    && (!binding.argument.path.is_empty()
                                        || (source.multiplicity == parameter.multiplicity
                                            && source.qualifications == parameter.qualifications
                                            && source.projected_qualifications
                                                == parameter.projected_qualifications))
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
                                            .block_entries
                                            .iter()
                                            .flat_map(|entry| &entry.structural_parameters)
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
                        && exact_source_owned_contract != Some(true)
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
                    || (parameter.access == terminal_psi::StructuralAccess::Owned
                        && references::owned_transfer_is_suspended(
                            function,
                            live,
                            structural_types,
                            &binding.argument,
                        ))
                    || (!binding.argument.path.is_empty()
                        && (!abstract_operations::control_flow::address_joins::is_static_projection(&binding.argument.path)
                            || (parameter.access != terminal_psi::StructuralAccess::Owned
                                && references::is_suspended_root(live, place))
                            || (parameter.access != terminal_psi::StructuralAccess::Owned
                                && !structural_types.get(&parameter.structural_type).is_some_and(
                                    |declaration| {
                                        matches!(
                                            declaration.shape,
                                            terminal_psi::StructuralTypeShape::ByteSequence(
                                                terminal_psi::ByteSequenceCarrier::BoundedOwned {
                                                    ..
                                                },
                                            )
                                        )
                                    },
                                ))))
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
