//! Completed owned values supply typed inline cells, not pointee observations.
//! Recorded equations supply projection demands only. Every demanded cell is
//! rejoined to the exact completed value and declared carrier before it enters
//! the current storage roster. Array extents are never enumerated; future or
//! unresolved demands fail closed. Exact stores can preserve whole disjoint
//! cells, while generic/opaque write invalidation stays with the parent owner.

use super::super::super::structural_terms;
use super::super::{StructuralJudge, StructuralTerm};
use super::{
    CapturedStorageCoordinate, CapturedStorageSegment, CapturedStorageValue, project_captured_value,
};
use typed_trees::types::TypeReferenceHandle;

/// A recorded fact can demand a cell of one exact completed inline value.
/// It supplies a path, never evidence for the fact or a substitute cell value.
struct CompletedStorageProjection {
    root: StructuralTerm,
    root_type: TypeReferenceHandle,
    selected_type: TypeReferenceHandle,
    segments: Vec<CapturedStorageSegment>,
}

impl StructuralJudge<'_> {
    pub(super) fn inline_coordinates_disjoint(
        &self,
        left: &CapturedStorageCoordinate,
        right: &CapturedStorageCoordinate,
    ) -> bool {
        if left.root != right.root {
            return false;
        }
        let Some(root) = left.root.as_symbol() else {
            return false;
        };
        let Some(mut reference) = crate::declaration_type_reference(self.program, root) else {
            return false;
        };
        for (left, right) in left.segments.iter().zip(&right.segments) {
            if crate::reference_shell(self.program, reference).is_some() {
                return false;
            }
            match (left, right) {
                (
                    CapturedStorageSegment::Place(facts::PlaceSegment::Field { symbol: left }),
                    CapturedStorageSegment::Place(facts::PlaceSegment::Field { symbol: right }),
                ) => {
                    let Some(carrier) = crate::value_custody::places::unwrapped_type_reference(
                        self.program,
                        reference,
                    ) else {
                        return false;
                    };
                    let Some(left) = self.program.closed_data_member_field(
                        carrier,
                        *left,
                        self.program.symbols.name(*left),
                    ) else {
                        return false;
                    };
                    let Some(right) = self.program.closed_data_member_field(
                        carrier,
                        *right,
                        self.program.symbols.name(*right),
                    ) else {
                        return false;
                    };
                    if left.identity != right.identity {
                        return true;
                    }
                    reference = left.type_reference;
                }
                (CapturedStorageSegment::Index(left), CapturedStorageSegment::Index(right)) => {
                    // A view is a descriptor crossing even without a reference
                    // shell. Only one exact inline fixed array owns these cells.
                    let Some(length) = crate::value_custody::places::fixed_array_type_length(
                        self.program,
                        reference,
                    )
                    .and_then(|length| u64::try_from(length).ok()) else {
                        return false;
                    };
                    let length = numerics::bignum::BigInt::from_u64(length);
                    let left = self.resolve(left.clone()).into_semantic_value();
                    let right = self.resolve(right.clone()).into_semantic_value();
                    if let (StructuralTerm::Integer(left), StructuralTerm::Integer(right)) =
                        (&left, &right)
                    {
                        if left < &numerics::bignum::BigInt::zero()
                            || left >= &length
                            || right < &numerics::bignum::BigInt::zero()
                            || right >= &length
                        {
                            return false;
                        }
                        if left != right {
                            return true;
                        }
                    }
                    if left != right {
                        return false;
                    }
                    let Some(element) =
                        crate::value_custody::places::collection_element_type_reference(
                            self.program,
                            reference,
                        )
                    else {
                        return false;
                    };
                    reference = element;
                }
                _ => return false,
            }
        }
        false
    }

    fn completed_projection_path(
        &self,
        value: &StructuralTerm,
    ) -> Option<CompletedStorageProjection> {
        match value.semantic_value() {
            StructuralTerm::CapturedValue { result_type, .. }
            | StructuralTerm::Application { result_type, .. } => Some(CompletedStorageProjection {
                root: value.clone(),
                root_type: *result_type,
                selected_type: *result_type,
                segments: Vec::new(),
            }),
            StructuralTerm::Projection { subject, path } => {
                let mut selected = self.completed_projection_path(subject)?;
                for name in path.split('.') {
                    if crate::reference_shell(self.program, selected.selected_type).is_some() {
                        return None;
                    }
                    let carrier = crate::value_custody::places::unwrapped_type_reference(
                        self.program,
                        selected.selected_type,
                    )?;
                    let data = self.program.closed_data_definition(carrier)?;
                    let mut fields = self
                        .program
                        .data_member_fields(self.program.data_members(data))
                        .filter(|field| field.name.as_str() == name);
                    let (Some(field), None) = (fields.next(), fields.next()) else {
                        return None;
                    };
                    let field =
                        self.program
                            .closed_data_member_field(carrier, field.symbol, name)?;
                    selected.segments.push(CapturedStorageSegment::Place(
                        facts::PlaceSegment::Field {
                            symbol: field.symbol,
                        },
                    ));
                    selected.selected_type = field.type_reference;
                }
                Some(selected)
            }
            StructuralTerm::IndexedValue {
                collection,
                index,
                element_type,
                element_identity,
            } => {
                let mut selected = self.completed_projection_path(collection)?;
                if crate::reference_shell(self.program, selected.selected_type).is_some() {
                    return None;
                }
                let carrier = crate::value_custody::places::unwrapped_type_reference(
                    self.program,
                    selected.selected_type,
                )?;
                if !matches!(
                    self.program.type_reference_table.type_reference(carrier),
                    typed_trees::types::TypeReferenceNode::FixedArray { .. }
                ) {
                    return None;
                }
                let element = crate::value_custody::places::collection_element_type_reference(
                    self.program,
                    carrier,
                )?;
                if self.program.normalized_type_identity(element) != *element_identity
                    || !crate::declared_carriers_match(self.program, element, *element_type)
                {
                    return None;
                }
                index.observation_identity_key()?;
                if let StructuralTerm::Integer(index) = index.semantic_value()
                    && let Some(length) =
                        crate::value_custody::places::fixed_array_type_length(self.program, carrier)
                            .and_then(|length| u64::try_from(length).ok())
                    && (index < &numerics::bignum::BigInt::zero()
                        || index >= &numerics::bignum::BigInt::from_u64(length))
                {
                    return None;
                }
                selected
                    .segments
                    .push(CapturedStorageSegment::Index((**index).clone()));
                selected.selected_type = element;
                Some(selected)
            }
            _ => None,
        }
    }

    /// Copy known inline cells from a completed binding. The equations are a
    /// demand roster only: each cell comes from the actual bound value through
    /// declared projections, not from a historical equation's other side.
    /// Array extents are never enumerated; only recorded paths are requested.
    pub(super) fn retain_completed_storage_cells(
        &mut self,
        coordinate: CapturedStorageCoordinate,
        reference: TypeReferenceHandle,
        value: StructuralTerm,
    ) {
        let mut demands: std::collections::HashMap<String, Vec<CompletedStorageProjection>> =
            std::collections::HashMap::new();
        let guarantees = std::sync::Arc::clone(&self.normal_guarantees);
        let mut terms = guarantees
            .iter()
            .flat_map(|(left, right)| [left, right])
            .collect::<Vec<_>>();
        // Guards can establish a component fact after initialization. Their
        // retained rewrites supply demands too, never substitute cell contents.
        terms.extend(
            self.rewrites
                .entries()
                .iter()
                .flat_map(|(left, right)| [left, right]),
        );
        while let Some(term) = terms.pop() {
            if matches!(
                term.semantic_value(),
                StructuralTerm::Projection { .. } | StructuralTerm::IndexedValue { .. }
            ) && let Some(projection) = self.completed_projection_path(term)
                && let Some(identity) = projection.root.observation_identity_key()
            {
                demands.entry(identity).or_default().push(projection);
            }
            term.for_each_child(|child| terms.push(child));
        }
        let mut pending = vec![(coordinate, reference, value, Vec::new())];
        let mut cells = Vec::new();
        let mut visited: std::collections::HashMap<String, Vec<CapturedStorageCoordinate>> =
            std::collections::HashMap::new();
        while let Some((coordinate, reference, value, mut active)) = pending.pop() {
            // Recorded demands can include every prefix of one deep value.
            // Expand each actual coordinate once rather than each prefix's
            // entire descendant graph. Equality, not the bucket, owns identity.
            let coordinates = visited.entry(format!("{coordinate:?}")).or_default();
            if coordinates.contains(&coordinate) {
                continue;
            }
            coordinates.push(coordinate.clone());
            if crate::reference_shell(self.program, reference).is_some() {
                continue;
            }
            let Some(carrier) =
                crate::value_custody::places::unwrapped_type_reference(self.program, reference)
            else {
                continue;
            };
            if active.contains(&carrier) {
                continue;
            }
            active.push(carrier);
            if let Some(data) = self.program.closed_data_definition(carrier) {
                for field in self
                    .program
                    .data_member_fields(self.program.data_members(data))
                {
                    let Some(field) = self.program.closed_data_member_field(
                        carrier,
                        field.symbol,
                        field.name.as_str(),
                    ) else {
                        continue;
                    };
                    let segment = CapturedStorageSegment::Place(facts::PlaceSegment::Field {
                        symbol: field.symbol,
                    });
                    let Some((field_type, field_value)) = project_captured_value(
                        self.program,
                        reference,
                        value.clone(),
                        std::slice::from_ref(&segment),
                    ) else {
                        continue;
                    };
                    let mut field_coordinate = coordinate.clone();
                    field_coordinate.segments.push(segment);
                    cells.push((field_coordinate.clone(), field_type, field_value.clone()));
                    pending.push((field_coordinate, field_type, field_value, active.clone()));
                }
            } else if matches!(
                self.program.type_reference_table.type_reference(carrier),
                typed_trees::types::TypeReferenceNode::FixedArray { .. }
            ) && let Some(identity) = value.observation_identity_key()
                && let Some(paths) = demands.get(&identity)
            {
                for path in paths {
                    if path.root != value
                        || !crate::declared_carriers_match(self.program, path.root_type, reference)
                    {
                        continue;
                    }
                    let Some((selected_type, selected_value)) = project_captured_value(
                        self.program,
                        reference,
                        value.clone(),
                        &path.segments,
                    ) else {
                        continue;
                    };
                    if !crate::declared_carriers_match(
                        self.program,
                        selected_type,
                        path.selected_type,
                    ) {
                        continue;
                    }
                    let mut selected_coordinate = coordinate.clone();
                    selected_coordinate
                        .segments
                        .extend_from_slice(&path.segments);
                    cells.push((
                        selected_coordinate.clone(),
                        selected_type,
                        selected_value.clone(),
                    ));
                    pending.push((
                        selected_coordinate,
                        selected_type,
                        selected_value,
                        active.clone(),
                    ));
                }
            }
        }
        self.retain_storage_cells(cells);
    }

    /// The key only buckets candidates; coordinate equality is authoritative.
    /// Batch insertion avoids a full retained-roster scan for every array cell.
    pub(super) fn retain_storage_cells(
        &mut self,
        cells: Vec<(
            CapturedStorageCoordinate,
            TypeReferenceHandle,
            StructuralTerm,
        )>,
    ) {
        let program = self.program;
        let values = std::sync::Arc::make_mut(&mut self.stored_values);
        let mut by_coordinate: std::collections::HashMap<String, Vec<usize>> =
            std::collections::HashMap::new();
        for (index, stored) in values.iter().enumerate() {
            by_coordinate
                .entry(format!("{:?}", stored.coordinate))
                .or_default()
                .push(index);
        }
        for (coordinate, reference, value) in cells {
            let value = structural_terms::owned_projection_observation(program, reference, value);
            let indices = by_coordinate.entry(format!("{coordinate:?}")).or_default();
            if let Some(index) = indices
                .iter()
                .copied()
                .find(|index| values[*index].coordinate == coordinate)
            {
                values[index].current = Some(value);
                values[index].completed_read = None;
            } else {
                indices.push(values.len());
                values.push(CapturedStorageValue {
                    coordinate,
                    entry: None,
                    current: Some(value),
                    completed_read: None,
                });
            }
        }
    }
}
