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

/// A whole owned edge move re-homes the moved record's loan leaves onto the
/// bound name: the custody key is the current carrier location and follows
/// the value across the edge. Formation identity, result row, root, parent
/// and referent are provenance and stay put.
pub(super) fn re_home_bound_references(
    bindings: &[abstract_operations::AbstractStructuralBinding],
    parameters: &[terminal_psi::StructuralParameterDeclaration],
    live: &mut LiveDefinitions,
) {
    for (binding, parameter) in bindings.iter().zip(parameters) {
        if parameter.access != terminal_psi::StructuralAccess::Owned
            || binding.argument.access != terminal_psi::StructuralAccess::Owned
            || !binding.argument.path.is_empty()
            || binding.argument.place == parameter.place
        {
            continue;
        }
        let moved: Vec<(
            semantic_vocabulary::PlaceId,
            Vec<terminal_psi::StructuralPathSegment>,
        )> = live
            .references
            .keys()
            .filter(|(carrier, _)| *carrier == binding.argument.place)
            .cloned()
            .collect();
        for (carrier, path) in moved {
            if let Some(custody) = live.references.remove(&(carrier, path.clone())) {
                live.references.insert((parameter.place, path), custody);
            }
        }
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
    // A bounded owner loan is forwarded whole from an identically qualified
    // signature or live join parameter. Forwarding retains the precise access
    // and predicate roster; a mutable loan does not promise frozen contents.
    if abstract_operations::control_flow::owner_loans::is_bounded_owner_loan(
        parameter,
        |identity| structural_types.get(&identity).copied(),
    ) {
        return argument.path.is_empty()
            && !live.structural_homes.contains_key(&argument.place)
            && function
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
                        && !source.is_self
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
            && home.qualifications().is_empty()
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
    use terminal_psi::StructuralAccess::{MutableBorrow, SharedBorrow, WriteOnlyBorrow};
    matches!(
        (source, destination),
        (
            MutableBorrow,
            SharedBorrow | MutableBorrow | WriteOnlyBorrow
        ) | (SharedBorrow, SharedBorrow)
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
                // Binding a reference-bearing subtree would need partial-move
                // custody across an edge; the verified contract admits only a
                // whole owned move — every stored loan travels with the record.
                // A projected path or a borrow lane still rejects here, and the
                // exact-contract checks below keep the same boundary.
                if references::contains_reference(structural_types, parameter.structural_type)
                    && (parameter.access != terminal_psi::StructuralAccess::Owned
                        || binding.argument.access != terminal_psi::StructuralAccess::Owned
                        || !binding.argument.path.is_empty())
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
                let exact_source_owned_contract = live
                    .structural_homes
                    .get(&place)
                    .map(|home| {
                        home.access() == parameter.access
                            && home.multiplicity() == parameter.multiplicity
                            && home.qualifications() == parameter.qualifications.as_slice()
                            && home.projected_qualifications()
                                == parameter.projected_qualifications.as_slice()
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
                                    && source.multiplicity == parameter.multiplicity
                                    && source.qualifications == parameter.qualifications
                                    && source.projected_qualifications
                                        == parameter.projected_qualifications
                            })
                    });
                let argument_type = source_type.and_then(|root| {
                    if binding.argument.path.is_empty() {
                        Some(root)
                    } else {
                        structural_types.subtree(root, &binding.argument.path)
                    }
                });
                // A projected owned argument moves a plain subtree across the
                // edge. No loan leaf may overlap the moved boundary — loan
                // custody has no projected carrier on an edge.
                let moved_loan_leaf = live.references.keys().any(|(carrier, leaf_path)| {
                    *carrier == place
                        && !binding.argument.path.is_empty()
                        && (leaf_path.starts_with(&binding.argument.path)
                            || binding.argument.path.starts_with(leaf_path))
                });
                if binding.parameter != parameter.place
                    || (parameter.access == terminal_psi::StructuralAccess::Owned
                        && exact_source_owned_contract != Some(true))
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
                    || moved_loan_leaf
                    || (!binding.argument.path.is_empty()
                        && (parameter.access != terminal_psi::StructuralAccess::Owned
                            || references::is_suspended_root(live, place)))
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
        | AbstractOperation::Crash { .. }
        | AbstractOperation::ReturnStructural { .. }
        | AbstractOperation::ReturnUnit { .. }
        | AbstractOperation::StructuralCase { .. } => Ok(()),
        _ => Err(LoweringError::unsupported_control_flow(function.machine)),
    }
}
