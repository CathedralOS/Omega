//! Whole quotient transport preserves opacity across paths and operations.
use super::*;
/// A path can select a whole quotient inside an aggregate, but can never
/// continue through its representative's retained physical shape.
fn quotient_path_tip(
    module: &TerminalModule,
    ty: StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
) -> Result<Option<StructuralTypeId>, &'static str> {
    use terminal_psi::{StructuralPathSegment as P, StructuralTypeShape as S};
    let opaque = |ty| {
        module
            .structural_types
            .iter()
            .any(|row| row.id == ty && row.quotient_carrier.is_some())
    };
    // Byte windows end a storage path rather than selecting an owned leaf.
    // Their ordinary bounds/access verifier still owns the window obligation.
    let (path, window) = match path.split_last() {
        Some((P::FixedByteRange { .. }, prefix)) => (prefix, true),
        _ => (path, false),
    };
    let tip = crate::primitive_place::projection_tip_with_policy(
        module.structural_types.iter(),
        ty,
        path,
        |source, segment| {
            !source.is_some_and(opaque)
                && match segment {
                    P::RepresentationView {
                        structural_type, ..
                    } => {
                        !source.is_some_and(|source| contains_quotient_storage(module, source))
                            && !contains_quotient_storage(module, *structural_type)
                    }
                    _ => true,
                }
        },
    )
    .ok_or("structural projection is malformed or crosses an opaque quotient")?;
    if window {
        let selected = tip
            .and_then(|id| module.structural_types.iter().find(|row| row.id == id))
            .ok_or("quotient window has no declared backing")?;
        if opaque(selected.id) || !matches!(selected.shape, S::FixedArray { .. }) {
            return Err("quotient window does not select ordinary array backing");
        }
    }
    Ok(tip)
}

/// A view can rename an entire inline subtree before a path reaches its Q
/// member. Such a rename must not erase or manufacture nested opacity.
fn contains_quotient_storage(module: &TerminalModule, root: StructuralTypeId) -> bool {
    use terminal_psi::{StructuralFieldType as F, StructuralTypeShape as S};
    let children = |fields: &[terminal_psi::StructuralFieldDeclaration]| {
        fields
            .iter()
            .filter(|field| !field.relevance.is_erased())
            .filter_map(|field| {
                if let F::Structural(child) = field.field_type {
                    Some(child)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    };
    let mut pending = vec![root];
    let mut seen = Vec::new();
    while let Some(id) = pending.pop() {
        if seen.contains(&id) {
            continue;
        }
        seen.push(id);
        let Some(declaration) = module.structural_types.iter().find(|row| row.id == id) else {
            return true; // An unresolved view cannot establish transparent storage.
        };
        if declaration.quotient_carrier.is_some() {
            return true;
        }
        match &declaration.shape {
            S::Record { fields, .. } => pending.extend(children(fields)),
            S::Sum { cases } => {
                for case in cases {
                    pending.extend(children(&case.fields));
                }
            }
            S::Mixed { fields, cases } => {
                pending.extend(children(fields));
                for case in cases {
                    pending.extend(children(&case.fields));
                }
            }
            S::FixedArray { element, .. } => pending.push(*element),
            // References and views carry descriptors; their backing is not
            // reinterpreted by a view of the descriptor's inline storage.
            S::Reference { .. }
            | S::ElementView { .. }
            | S::ByteSequence(_)
            | S::BindingCarrier
            | S::PrimitiveScalar(_) => {}
        }
    }
    false
}

pub(super) fn validate_quotient_opacity(module: &TerminalModule) -> Result<(), &'static str> {
    if module.quotients.representations.is_empty() {
        return Ok(());
    }
    for machine in &module.machines {
        for operation in machine.operations() {
            if let Some(result) = operation.result.structural()
                && module
                    .structural_types
                    .iter()
                    .any(|ty| ty.id == result.structural_type && ty.quotient_carrier.is_some())
            {
                let transport = matches!(
                    &operation.kind,
                    OperationKind::StructuralLeafCopy { .. }
                        | OperationKind::StructuralCaseLeafCopy { .. }
                        | OperationKind::CallStructural { .. }
                        | OperationKind::CallStructuralWithScalarArguments { .. }
                );
                if !transport {
                    return Err("only licensed minting or whole transport can produce a quotient");
                }
            }
            if let OperationKind::StructuralLeafCopy { source, path } = &operation.kind {
                let root = quotient_place_type(machine, *source)
                    .ok_or("quotient copy source is absent")?;
                let tip = quotient_path_tip(module, root, path)?;
                let result = operation
                    .result
                    .structural()
                    .ok_or("quotient copy result is absent")?;
                if tip != Some(result.structural_type)
                    && module.structural_types.iter().any(|ty| {
                        ty.quotient_carrier.is_some()
                            && (Some(ty.id) == tip || ty.id == result.structural_type)
                    })
                    && quotient_copy_adaptation(module, machine, operation).is_none()
                {
                    return Err("unlicensed nominal quotient copy");
                }
            }
            if let OperationKind::StructuralCaseLeafCopy { source, path } = &operation.kind {
                let root = quotient_place_type(machine, *source)
                    .ok_or("quotient case-copy source is absent")?;
                let path = crate::canonical_structural_storage_path(
                    module.structural_types.iter(),
                    root,
                    path,
                )
                .ok_or("quotient case-copy path is absent")?;
                quotient_path_tip(module, root, &path)?;
            }

            for projection in operation.kind.structural_projections() {
                let root = quotient_place_type(machine, projection.root)
                    .ok_or("quotient projection has no declared source")?;
                let tip = quotient_path_tip(module, root, projection.path)?;
                if !module
                    .quotients
                    .representations
                    .iter()
                    .any(|row| Some(row.quotient) == tip)
                {
                    continue;
                }
                let transport = matches!(
                    &operation.kind,
                    OperationKind::StructuralLeafCopy { .. }
                        | OperationKind::EstablishRecord { .. }
                        | OperationKind::EstablishStructuralCase { .. }
                        | OperationKind::EstablishStructuralArray { .. }
                        | OperationKind::CallStructuralScalar { .. }
                        | OperationKind::CallStructural { .. }
                        | OperationKind::CallStructuralWithScalarArguments { .. }
                        | OperationKind::CallUnit { .. }
                );
                if !transport {
                    return Err("operation observes an opaque quotient value");
                }
            }
        }
        for block in &machine.blocks {
            if let terminal_psi::Terminator::StructuralCase { source, path, .. } = &block.terminator
            {
                let root = quotient_place_type(machine, *source)
                    .ok_or("quotient case source is absent")?;
                let tip = quotient_path_tip(module, root, path)?;
                if module
                    .structural_types
                    .iter()
                    .any(|ty| Some(ty.id) == tip && ty.quotient_carrier.is_some())
                {
                    return Err("case terminator observes an opaque quotient");
                }
            }
        }
    }
    Ok(())
}
