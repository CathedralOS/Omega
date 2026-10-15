//! Owned aggregate actuals retain exact type, custody, and current value backing.
//! Arrays, records, completed sums, and bounded byte owners share transport,
//! not semantic identity. A byte owner transports its live-length word and
//! backing together; presenting it as a borrowed view would change that ABI.
//! An owned receiver uses that same transport: `is_self` identifies its source
//! role, not a different ABI or an exemption from the home custody checks.
use super::LiveDefinitions;
use crate::LoweringError;
use semantic_vocabulary::StructuralTypeId;
use std::collections::{BTreeMap, BTreeSet};
use target_operations::{TargetStructuralArgument, TargetStructuralParameter};
use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralTypeDeclaration};

pub(super) fn is_owned_parameter(
    parameter: &terminal_psi::StructuralParameterDeclaration,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> bool {
    parameter.access == StructuralAccess::Owned
        && super::aggregate_results::value_home_layout(parameter.structural_type, types).is_ok()
}

pub(super) fn argument(
    argument: &terminal_psi::StructuralArgument,
    declaration: &terminal_psi::StructuralParameterDeclaration,
    destination: &TargetStructuralParameter,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    live: &LiveDefinitions,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<TargetStructuralArgument, LoweringError> {
    let invalid = |reason| LoweringError::InvalidOwnedStructuralArgument {
        source: argument.place,
        parameter: declaration.place,
        structural_type: declaration.structural_type,
        reason,
    };
    let expected_layout =
        super::aggregate_results::value_home_layout(declaration.structural_type, types)?;
    let expected_shape = expected_layout.shape();
    if !is_owned_parameter(declaration, types)
        || argument.access != StructuralAccess::Owned
        || destination.shape != expected_shape
    {
        return Err(invalid(
            "argument access or destination layout differs from its owned formal",
        ));
    }
    let (source, root_structural_type, source_byte_offset) =
        if let Some(home) = live.structural_homes.get(&argument.place) {
            // The product entry validates the optimized plan's call custody before
            // invoking this lowering core. Target storage carries bytes and live
            // length, not that predicate proof; geometry must not turn valid
            // predicate weakening into native qualification equality. Direct uses
            // of the unvalidated core remain responsible for supplying that proof.
            if home.access() != StructuralAccess::Owned {
                return Err(invalid("source home is not owned"));
            }
            // Partial owned transfer selects bytes from the completed root home;
            // the verified call and residual cleanup retain the root's custody.
            // A root qualification cannot silently become a payload qualification.
            let offset = if argument.path.is_empty() {
                if home.structural_type() != declaration.structural_type
                    || home.layout != expected_layout
                {
                    return Err(invalid(
                        "source home type or layout differs from its formal",
                    ));
                }
                if home.multiplicity() != declaration.multiplicity {
                    return Err(invalid("source home multiplicity differs from its formal"));
                }
                0
            } else {
                let (selected, shape, offset) =
                    crate::lowering::structural_layout::resolve_structural_projection_path(
                        home.structural_type(),
                        &argument.path,
                        types,
                        &mut BTreeMap::new(),
                        &mut BTreeSet::new(),
                    )?;
                if selected != declaration.structural_type || shape != expected_shape {
                    return Err(invalid(
                        "source home projection type or layout differs from its formal",
                    ));
                }
                offset
            };
            // A state arrival owns its destination home, not an incoming
            // operation's home. Preserve that identity through calls just as
            // through reads and loans; independent source replay checks both
            // coordinates against the caller's block declaration.
            let source = home.argument_source();
            (source, home.structural_type(), offset)
        } else if let Some((establishment, structural_type)) =
            live.trivial_affine_locals.get(&argument.place)
        {
            // An empty-record local has no bytes to transport; its establishment
            // is the producer the call replays. Membership adds no payload bytes
            // and is independent of its affine transfer.
            if !argument.path.is_empty()
                || *structural_type != declaration.structural_type
                || declaration.multiplicity != StructuralMultiplicity::Affine
                || expected_shape.byte_size != 0
            {
                return Err(invalid("empty affine local does not match its formal"));
            }
            (
                target_operations::TargetStructuralArgumentSource::StructuralHome {
                    psi_operation: *establishment,
                },
                *structural_type,
                0,
            )
        } else {
            let parameter = prepared
                .parameter(argument.place)
                .ok_or_else(|| invalid("source has no home or ingress parameter"))?;
            if parameter.access != StructuralAccess::Owned {
                return Err(invalid("source parameter is not owned"));
            }
            let offset = if argument.path.is_empty() {
                if parameter.multiplicity != declaration.multiplicity
                    || parameter.structural_type != declaration.structural_type
                    || parameter.shape != expected_shape
                {
                    return Err(invalid(
                        "source parameter custody, type or layout differs from its formal",
                    ));
                }
                0
            } else {
                let (selected, shape, offset) =
                    crate::lowering::structural_layout::resolve_structural_projection_path(
                        parameter.structural_type,
                        &argument.path,
                        types,
                        &mut BTreeMap::new(),
                        &mut BTreeSet::new(),
                    )?;
                if selected != declaration.structural_type || shape != expected_shape {
                    return Err(invalid(
                        "source parameter projection type or layout differs from its formal",
                    ));
                }
                offset
            };
            (
                parameter.placement.clone().into(),
                parameter.structural_type,
                offset,
            )
        };
    Ok(TargetStructuralArgument {
        place: argument.place,
        access: argument.access,
        path: argument.path.clone(),
        root_structural_type,
        structural_type: declaration.structural_type,
        shape: expected_shape,
        source_byte_offset,
        fixed_array_length: None,
        element_stride: None,
        indices: Vec::new(),
        source,
        destination: destination.placement.clone(),
    })
}
