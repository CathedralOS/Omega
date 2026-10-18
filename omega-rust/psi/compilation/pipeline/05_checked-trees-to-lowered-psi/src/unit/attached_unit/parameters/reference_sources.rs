//! Reference provenance uses an ending `Referent` marker for the borrowed
//! leaf, including a borrow formed at non-reference storage. Ordinary call
//! projections instead cross an actual reference carrier at that segment.
//! Logical borrowed-local roots already join their published carrier through
//! that owner; an endpoint marker must not cross the same descriptor again.

use super::{
    LoweringError, StructuralArgument, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, lower_structural_arguments, unsupported,
};
use crate::expression_preparation::bindings::StructuralLocalBinding;
use checked_trees::{
    CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralPathSegment,
};

/// Formation's scalar namespace and emission context. Runtime selectors are
/// evaluated once before the reference owns its captured endpoint; catalog
/// provenance lowering has no runtime evaluation environment.
pub(crate) struct ReferenceSourceEvaluation<'scope, 'calls> {
    pub(crate) values: &'scope [terminal_psi::ValueDeclaration],
    pub(crate) selectors: &'scope mut dyn Iterator<Item = semantic_vocabulary::ValueId>,
    pub(crate) calls:
        &'scope mut crate::emission::operation_emission::calls::CallEmissionContext<'calls>,
    pub(crate) next_value: &'scope mut u64,
    pub(crate) operations: &'scope mut crate::emission::operation_emission::buffer::OperationBuffer,
}

/// Resolve the original source home and typed prefix through the ordinary
/// argument owner, then retain the reference-source leaf marker. Formation,
/// access and escape permission remain with reference custody validation.
pub(crate) fn lower_reference_source(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &checked_trees::CheckedTrees,
    source: &CheckedUnitStructuralArgumentPlan,
    structural_types: &[StructuralTypeDeclaration],
    parameters: &[StructuralParameterDeclaration],
    trivial_affine_locals: &[StructuralPlaceDeclaration],
    structural_results: &[(Option<StructuralPlaceDeclaration>, bool)],
    primitive_locals: &[super::super::primitive_locals::PrimitiveLocal],
    structural_locals: &[StructuralLocalBinding],
    evaluation: Option<ReferenceSourceEvaluation<'_, '_>>,
) -> Result<StructuralArgument, LoweringError> {
    if source.access == CheckedStructuralAccess::Owned {
        return unsupported("reference source cannot transfer owned custody");
    }
    let mut prefix = source.clone();
    let has_leaf_marker = matches!(
        prefix.path.last(),
        Some(CheckedUnitStructuralPathSegment::Referent)
    );
    if has_leaf_marker {
        prefix.path.pop();
    }
    let mut empty_selectors = std::iter::empty();
    let (values, selectors, calls, emission) = match evaluation {
        Some(evaluation) => (
            Some(evaluation.values),
            evaluation.selectors,
            Some(evaluation.calls),
            Some((evaluation.next_value, evaluation.operations)),
        ),
        None => (
            None,
            &mut empty_selectors as &mut dyn Iterator<Item = semantic_vocabulary::ValueId>,
            None,
            None,
        ),
    };
    let arguments = lower_structural_arguments(
        identities,
        checked,
        std::slice::from_ref(&prefix),
        structural_types,
        parameters,
        trivial_affine_locals,
        structural_results,
        &[],
        &[],
        primitive_locals,
        structural_locals,
        None,
        values,
        selectors,
        calls,
        emission,
    )?;
    let [mut argument] = <[StructuralArgument; 1]>::try_from(arguments)
        .map_err(|_| LoweringError::Unsupported("reference source is not one argument"))?;
    let retains_leaf_marker = if has_leaf_marker && source.borrowed_root_type_identity().is_some() {
        let mut roots = parameters
            .iter()
            .filter(|parameter| parameter.place == argument.place)
            .map(|parameter| parameter.structural_type)
            .chain(structural_results.iter().filter_map(|(declaration, _)| {
                let declaration = declaration
                    .as_ref()
                    .filter(|row| row.id == argument.place)?;
                match declaration.kind {
                    semantic_vocabulary::StructuralPlaceKind::OperationResult {
                        structural_type,
                        ..
                    }
                    | semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                        structural_type,
                        ..
                    } => Some(structural_type),
                    _ => None,
                }
            }))
            .chain(structural_locals.iter().filter_map(|binding| {
                (binding.argument.place == argument.place && binding.argument.path.is_empty())
                    .then_some(binding.structural_type)
            }));
        let root = roots.next().ok_or(LoweringError::Unsupported(
            "reference source lost its published home type",
        ))?;
        if !roots.all(|candidate| candidate == root) {
            return unsupported("reference source has conflicting published home types");
        }
        let endpoint = terminal_semantics::runtime_structural_path_tip(
            structural_types.iter(),
            root,
            &argument.path,
        )
        .ok_or(LoweringError::Unsupported(
            "reference source has no selected published endpoint",
        ))?;
        if matches!(
            endpoint.shape,
            terminal_psi::StructuralTypeShape::Reference { .. }
        ) {
            true
        } else {
            // The ordinary argument join already crossed the retained carrier
            // to the checked referent. Keeping another marker would instead
            // declare storage beneath that descriptor and lose parent lineage.
            if endpoint.identity
                != identities.intern(
                    &checked
                        .type_identity_text(source.type_identity)
                        .unwrap_or_default(),
                )
            {
                return unsupported("reference source changes its checked endpoint type");
            }
            false
        }
    } else {
        has_leaf_marker
    };
    if retains_leaf_marker {
        argument
            .path
            .push(terminal_psi::StructuralPathSegment::Referent);
    }
    Ok(argument)
}

/// Normalize exact caller-relative origins under each returned reference leaf.
/// Multiple completion alternatives share one output path without losing their
/// distinct source places, projections, or access permissions.
/// Checked custody also includes sequence descriptors. Terminal reconstructs
/// their returned contents through descriptor replay, including stored input
/// snapshots and runtime-selected element families. They must not enter this
/// pointer-only table: a descriptor's backing is not its container's referent.
pub(crate) fn lower_reference_result_sources(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &checked_trees::CheckedTrees,
    references: &[checked_trees::CheckedReturnedReferencePlan],
    result_type: semantic_vocabulary::StructuralTypeId,
    structural_types: &[StructuralTypeDeclaration],
    parameters: &[StructuralParameterDeclaration],
) -> Result<Vec<terminal_psi::StructuralReferenceResultSource>, LoweringError> {
    let mut lowered = Vec::new();
    for reference in references {
        if reference.source.access == CheckedStructuralAccess::Owned {
            return unsupported("returned origin cannot transfer owned storage");
        }
        let result_path =
            crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(
                identities,
                &reference.path,
                result_type,
                structural_types,
            )?;
        let field_carrier = terminal_semantics::runtime_structural_field(
            structural_types.iter(),
            result_type,
            &result_path,
        )
        .and_then(|field| field.field_type.byte_sequence_carrier());
        let leaf = terminal_semantics::runtime_structural_path_tip(
            structural_types.iter(),
            result_type,
            &result_path,
        );
        if matches!(
            field_carrier,
            Some(terminal_psi::byte_sequence_view_carrier_pattern!())
        ) || leaf.is_some_and(|leaf| {
            matches!(
                leaf.shape,
                terminal_psi::StructuralTypeShape::ElementView { .. }
                    | terminal_psi::StructuralTypeShape::ByteSequence(
                        terminal_psi::byte_sequence_view_carrier_pattern!()
                    )
            )
        }) {
            continue;
        }
        if !leaf.is_some_and(|leaf| {
            matches!(
                leaf.shape,
                terminal_psi::StructuralTypeShape::Reference { .. }
            )
        }) {
            return unsupported("returned origin has no borrowed carrier leaf");
        }
        let (place, root_type) = match reference.source.root {
            checked_trees::CheckedReferenceOriginRoot::Parameter { index } => {
                let parameter =
                    parameters
                        .get(index as usize)
                        .ok_or(LoweringError::Unsupported(
                            "returned origin lost its input parameter",
                        ))?;
                (Some(parameter.place), parameter.structural_type)
            }
            checked_trees::CheckedReferenceOriginRoot::Invocation { carrier_type } => {
                let declaration = crate::terminal_identities::structural_declaration_by_identity(
                    structural_types,
                    identities,
                    checked,
                    carrier_type,
                    "returned invocation origin lost its carrier type",
                )?;
                (None, declaration.id)
            }
        };
        let path =
            crate::expression_preparation::bindings::structural_paths::lower_reference_origin_path(
                identities,
                checked,
                &reference.source.path,
                root_type,
                structural_types,
            )?;
        let access = crate::expression_preparation::bindings::structural_paths::terminal_access(
            reference.source.access,
        );
        let source = match place {
            Some(place) => terminal_psi::StructuralReferenceOrigin::Parameter(
                terminal_psi::ReferenceOriginSource {
                    place,
                    extent: reference.source.extent,
                    path,
                    access,
                },
            ),
            None => terminal_psi::StructuralReferenceOrigin::Invocation {
                carrier_type: root_type,
                extent: reference.source.extent,
                path,
                access,
            },
        };
        lowered.push(terminal_psi::StructuralReferenceResultSource {
            path: result_path,
            sources: vec![source],
        });
    }
    lowered.sort_by(|left, right| left.path.cmp(&right.path));
    let mut grouped: Vec<terminal_psi::StructuralReferenceResultSource> =
        Vec::with_capacity(lowered.len());
    for reference in lowered {
        if let Some(group) = grouped.last_mut()
            && group.path == reference.path
        {
            group.sources.extend(reference.sources);
        } else {
            grouped.push(reference);
        }
    }
    for group in &mut grouped {
        group.sources.sort();
        group.sources.dedup();
    }
    Ok(grouped)
}
