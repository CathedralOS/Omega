use super::{
    PackagePolicyCallingPlan, PackagePolicyNativeParameterOrigin, strictly_sorted, validate_nominal,
};
use crate::record::{
    PackageReviewBoundaryShapeClass as Shape, PackageReviewBoundaryShapeGraph,
    PackageReviewOpaqueRepresentationMovementRole as Role,
    PackageReviewOpaqueRepresentationPathElement as Path,
};

pub(super) fn validate(policy: &PackagePolicyCallingPlan) -> Result<(), &'static str> {
    let graph = &policy.shape_graph;
    // The compiler's normalized boundary vocabulary has 256 shape/field slots.
    // Fixed stack scratch also bounds cycle checking for untrusted recovery.
    if graph.shapes.len() > 256 || graph.fields.len() > 256 {
        return Err("calling shape graph exceeds the supported normalized capacity");
    }
    let mut colors = [0u8; 256];
    for index in 0..graph.shapes.len() {
        visit(graph, index, &mut colors)?;
    }
    if graph
        .parameters
        .iter()
        .chain(graph.result.iter())
        .any(|root| usize::from(*root) >= graph.shapes.len())
    {
        return Err("calling shape root is out of bounds");
    }
    Ok(())
}

fn visit(
    graph: &PackageReviewBoundaryShapeGraph,
    index: usize,
    colors: &mut [u8; 256],
) -> Result<(), &'static str> {
    let shape = graph
        .shapes
        .get(index)
        .ok_or("calling child shape is out of bounds")?;
    if colors[index] == 2 {
        return Ok(());
    }
    if colors[index] == 1 {
        return Err("calling shape graph contains a cycle");
    }
    if !shape.alignment.is_power_of_two() {
        return Err("calling shape alignment is invalid");
    }
    colors[index] = 1;
    match shape.class {
        Shape::FixedArray { element, length } => {
            visit(graph, usize::from(element), colors)?;
            let child = &graph.shapes[usize::from(element)];
            if u32::from(child.byte_size) * u32::from(length) != u32::from(shape.byte_size)
                || child.alignment != shape.alignment
            {
                return Err("calling array geometry differs from its element");
            }
        }
        Shape::Record {
            first_field,
            field_count,
        } => {
            let fields = graph
                .fields
                .get(usize::from(first_field)..usize::from(first_field) + usize::from(field_count))
                .ok_or("calling record field span is out of bounds")?;
            for field in fields {
                visit(graph, usize::from(field.shape), colors)?;
                let child = &graph.shapes[usize::from(field.shape)];
                if u32::from(field.byte_offset) + u32::from(child.byte_size)
                    > u32::from(shape.byte_size)
                {
                    return Err("calling record field exceeds its parent extent");
                }
            }
        }
        Shape::Integer | Shape::Float | Shape::Reference => {}
    }
    colors[index] = 2;
    Ok(())
}

pub(super) fn validate_opaque(policy: &PackagePolicyCallingPlan) -> Result<(), &'static str> {
    // Every retained opaque use originates at a materialized shape node.
    // The bound also keeps cross-owner occurrence scans allocation-free.
    let mut occurrence_count = 0usize;
    for use_ in &policy.opaque_uses {
        occurrence_count = occurrence_count
            .checked_add(use_.occurrences.len())
            .ok_or("calling opaque occurrence count overflows")?;
    }
    if policy.opaque_uses.len() > 256 || occurrence_count > 256 {
        return Err("calling opaque catalog exceeds normalized shape capacity");
    }
    if !strictly_sorted(&policy.opaque_uses) {
        return Err("calling opaque uses are not canonical");
    }
    for (index, use_) in policy.opaque_uses.iter().enumerate() {
        // Opaque selections belong to the nongeneric authoritative build, not
        // to the boundary requirement's lifetime telescope. Callback slots
        // retain their separate calling-relative lifetime validation.
        if !use_.application.lifetime_arguments.is_empty()
            || !use_.application.trait_lifetime_arguments.is_empty()
        {
            return Err("calling opaque selection has no build lifetime telescope");
        }
        validate_nominal(&use_.opaque)?;
        validate_nominal(&use_.carrier)?;
        validate_nominal(&use_.application.declaration)?;
        validate_nominal(&use_.application.trait_identity)?;
        if use_.selection_owner == crate::record::PackageReviewNominalOwner::Unresolved {
            return Err("calling opaque selection owner is unresolved");
        }
        if use_.occurrences.is_empty()
            || !strictly_sorted(&use_.occurrences)
            || policy.opaque_uses[..index]
                .iter()
                .any(|prior| prior.opaque == use_.opaque)
        {
            return Err("calling opaque selection or occurrences are repeated or absent");
        }
        for occurrence in &use_.occurrences {
            if policy.opaque_uses[..index].iter().any(|prior| {
                prior
                    .occurrences
                    .iter()
                    .any(|prior| prior.role == occurrence.role && prior.path == occurrence.path)
            }) {
                return Err("calling opaque occurrence has more than one nominal owner");
            }
            let (mut root, placement) = match occurrence.role {
                Role::Parameter {
                    formal_ordinal,
                    native_ordinal,
                } => {
                    let formal = policy
                        .semantic_parameters
                        .get(formal_ordinal as usize)
                        .ok_or("calling opaque formal is out of bounds")?;
                    let native = policy
                        .native_parameters
                        .get(native_ordinal as usize)
                        .ok_or("calling opaque native parameter is out of bounds")?;
                    if !matches!(native.origin, PackagePolicyNativeParameterOrigin::SemanticFormal { formal_ordinal: candidate, .. } if candidate == formal_ordinal)
                    {
                        return Err("calling opaque native parameter has another semantic origin");
                    }
                    (
                        formal.shape_root,
                        &policy.physical.parameters[native_ordinal as usize],
                    )
                }
                Role::Result => (
                    policy
                        .shape_graph
                        .result
                        .ok_or("calling opaque result is absent")?,
                    policy
                        .physical
                        .result
                        .as_ref()
                        .ok_or("calling opaque result placement is absent")?,
                ),
            };
            if placement != &occurrence.placement {
                return Err("calling opaque occurrence changed its complete placement");
            }
            for element in &occurrence.path {
                root = match (element, policy.shape_graph.shapes[usize::from(root)].class) {
                    (Path::FixedArrayElement, Shape::FixedArray { element, .. }) => element,
                    (
                        Path::RecordField { ordinal },
                        Shape::Record {
                            first_field,
                            field_count,
                        },
                    ) if *ordinal < field_count => {
                        policy.shape_graph.fields[usize::from(first_field) + usize::from(*ordinal)]
                            .shape
                    }
                    _ => return Err("calling opaque occurrence has an invalid shape path"),
                };
            }
            if root != occurrence.carrier_shape_root {
                return Err("calling opaque occurrence changed its carrier root");
            }
        }
    }
    Ok(())
}
