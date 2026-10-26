//! Type-directed maximal residual complement for finite structural transfers.
//!
//! Validate only the complement: moved subtrees leave with their own custody.
//! Keep untouched finite cleanup-free trees maximal, refine paths to nominal
//! boundaries, and retain recursive reverse-field/index disposal order.
//! Source checking supplies case reachability. Established cases need no
//! condition; possible alternatives keep outer-to-inner runtime conditions,
//! including empty alternatives whose disposed region is the sum itself.
//! Common fields run once after payload selection. These are independent
//! alternatives, not a Cartesian expansion of unrelated discriminators.
//!
//! This walk does not admit hooks, prove membership, or publish executable
//! control. Its caller owns those joins and must retain every condition. The
//! static residual API uses the same walk but accepts no runtime conditions.
//! Count before array enumeration and reject recursive by-value complements;
//! an output bound must apply before a huge dimension can allocate or expand.
use checked_trees::{
    CheckedUnitPartialAffineDiscardPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralFieldType, CheckedUnitStructuralPathSegment,
    CheckedUnitStructuralTypePlan, CheckedUnitStructuralTypeShape, is_partial_affine_field_type,
};
use std::collections::BTreeMap;
use typed_trees::type_identity::TypeIdentityId;

/// Source checking decides whether a case is impossible, established, or must
/// be selected at execution. Geometry never turns a possible case into a fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupCaseReachability {
    Inactive,
    Selected,
    Runtime,
}

/// Construction-time condition for a cleanup region. Conditions are ordered
/// outermost first: a nested tag must not be read before its enclosing case is
/// selected. The path names the sum, not the payload being disposed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupCaseCondition {
    pub path: Vec<CheckedUnitStructuralPathSegment>,
    pub type_identity: TypeIdentityId,
    pub case_identity: checked_trees::CheckedMemberIdentity,
}

/// Invocation-owned geometry scratch, not another durable checked IR. The
/// publishing caller must retain conditions with its cleanup control plan;
/// dropping them would authorize disposal of inactive payloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseSelectedCleanupRegion {
    pub disposition: CheckedUnitPartialAffineDiscardPlan,
    pub conditions: Vec<CleanupCaseCondition>,
}

struct ResidualTraversal<'selection> {
    select_case: &'selection dyn Fn(
        &[CheckedUnitStructuralPathSegment],
        TypeIdentityId,
        &checked_trees::CheckedMemberIdentity,
    ) -> Option<CleanupCaseReachability>,
    /// Exact conditional-transfer cuts refine otherwise maximal cleanup-free
    /// regions. They supply no hook/disposition authority and dispose nothing.
    partition_paths: &'selection [Vec<CheckedUnitStructuralPathSegment>],
    conditions: Vec<CleanupCaseCondition>,
    regions: Vec<CaseSelectedCleanupRegion>,
    active_types: Vec<TypeIdentityId>,
}

impl ResidualTraversal<'_> {
    fn emit(
        &mut self,
        source: &CheckedUnitStructuralArgumentSourcePlan,
        path: &[CheckedUnitStructuralPathSegment],
        type_identity: TypeIdentityId,
    ) {
        self.regions.push(CaseSelectedCleanupRegion {
            disposition: CheckedUnitPartialAffineDiscardPlan {
                source: source.clone(),
                path: path.to_vec(),
                type_identity,
                case_conditions: Default::default(),
            },
            conditions: self.conditions.clone(),
        });
    }
}

fn validate_type(
    types: &BTreeMap<TypeIdentityId, &CheckedUnitStructuralTypePlan>,
    current_type: TypeIdentityId,
    active: &mut Vec<TypeIdentityId>,
    complete: &mut Vec<TypeIdentityId>,
) -> Option<()> {
    if complete.contains(&current_type) {
        return Some(());
    }
    if active.contains(&current_type) {
        return None;
    }
    let declaration = *types.get(&current_type)?;
    active.push(current_type);
    match &declaration.shape {
        // Array elements have standalone type rows, while record fields carry
        // their scalar leaf inline. Both use the same cleanup-free leaf rule;
        // neither representation grants authority to discard an opaque carrier.
        CheckedUnitStructuralTypeShape::PrimitiveScalar(primitive)
            if declaration.scalar_restriction.as_ref().map_or_else(
                || {
                    is_partial_affine_field_type(&CheckedUnitStructuralFieldType::Scalar(
                        *primitive,
                    ))
                },
                is_partial_affine_field_type,
            ) => {}
        CheckedUnitStructuralTypeShape::Record { fields, .. } => {
            for (index, field) in fields.iter().enumerate() {
                if !field.identity.is_valid()
                    || field.relevance.is_erased()
                    || (!is_partial_affine_field_type(&field.field_type)
                        && !field.cleanup_type_identity().is_valid())
                    || fields[..index]
                        .iter()
                        .any(|earlier| earlier.identity == field.identity)
                {
                    return None;
                }
                let type_identity = field.cleanup_type_identity();
                if type_identity.is_valid() {
                    validate_type(types, type_identity, active, complete)?;
                }
            }
        }
        CheckedUnitStructuralTypeShape::FixedArray {
            element_type_identity,
            length,
        } => {
            // Empty arrays contain no values whose cleanup eligibility can
            // constrain this complement. They retain their own exact shape,
            // just as an empty record does, without visiting an absent element.
            // A recipe extent may be nonempty, so its element is visited.
            if length.literal() != Some(0) {
                validate_type(types, *element_type_identity, active, complete)?;
            }
        }
        // A residual loan ends its stored descriptor, never its referent.
        // Stored-reference catalogs keep the exact referent identity without
        // expanding its fields: requiring a referent storage row here would
        // reject primitive backing and recursively borrowed type families.
        CheckedUnitStructuralTypeShape::Reference {
            referent_identity,
            access,
        } if *access != checked_trees::CheckedStructuralAccess::Owned
            && referent_identity.is_valid() => {}
        CheckedUnitStructuralTypeShape::ByteSequence(carrier)
            if carrier.is_borrowed()
                && declaration.descriptor_cleanup
                && !declaration.owns_linear_claim => {}
        CheckedUnitStructuralTypeShape::BorrowedSliceView { .. }
            if declaration.descriptor_cleanup && !declaration.owns_linear_claim => {}
        CheckedUnitStructuralTypeShape::Sum { cases } => {
            validate_inline_alternatives(&[], cases)?;
        }
        CheckedUnitStructuralTypeShape::Mixed { fields, cases } => {
            validate_inline_alternatives(fields, cases)?;
        }
        _ => return None,
    }
    active.pop();
    complete.push(current_type);
    Some(())
}

/// No selected-case authority is needed to discard a complete inline value.
/// Apply the existing custody-free field rule to every closed alternative;
/// structural payloads retain their separate case-dependent cleanup contract.
fn validate_inline_alternatives(
    common: &[checked_trees::CheckedUnitStructuralFieldPlan],
    cases: &[checked_trees::CheckedUnitStructuralCasePlan],
) -> Option<()> {
    let fields_are_inline = |fields: &[checked_trees::CheckedUnitStructuralFieldPlan]| {
        fields.iter().enumerate().all(|(position, field)| {
            field.identity.is_valid()
                && !field.relevance.is_erased()
                && is_partial_affine_field_type(&field.field_type)
                && !matches!(
                    field.field_type,
                    CheckedUnitStructuralFieldType::Structural { .. }
                )
                && !fields[..position]
                    .iter()
                    .any(|earlier| earlier.identity == field.identity)
        })
    };
    (!cases.is_empty()
        && fields_are_inline(common)
        && cases.iter().enumerate().all(|(position, case)| {
            case.identity.is_valid()
                && !cases[..position]
                    .iter()
                    .any(|earlier| earlier.identity == case.identity)
                && fields_are_inline(&case.fields)
        }))
    .then_some(())
}

/// Count first, traversing only touched array children. Thus a huge dimension
/// cannot trigger enumeration before host-size and output-allocation checks.
fn visit(
    types: &BTreeMap<TypeIdentityId, &CheckedUnitStructuralTypePlan>,
    source: &CheckedUnitStructuralArgumentSourcePlan,
    current_type: TypeIdentityId,
    moved_paths: &[(&[CheckedUnitStructuralPathSegment], TypeIdentityId)],
    boundaries: &[(&[CheckedUnitStructuralPathSegment], TypeIdentityId)],
    prefix: &mut Vec<CheckedUnitStructuralPathSegment>,
    traversal: &mut ResidualTraversal<'_>,
    emit: bool,
) -> Option<usize> {
    if moved_paths.iter().any(|(path, _)| path.is_empty()) {
        return (moved_paths.len() == 1 && moved_paths[0].1 == current_type).then_some(0);
    }
    if traversal.active_types.contains(&current_type) {
        return None;
    }
    traversal.active_types.push(current_type);
    let result = visit_region(
        types,
        source,
        current_type,
        moved_paths,
        boundaries,
        prefix,
        traversal,
        emit,
    );
    traversal.active_types.pop();
    result
}

fn visit_region(
    types: &BTreeMap<TypeIdentityId, &CheckedUnitStructuralTypePlan>,
    source: &CheckedUnitStructuralArgumentSourcePlan,
    current_type: TypeIdentityId,
    moved_paths: &[(&[CheckedUnitStructuralPathSegment], TypeIdentityId)],
    boundaries: &[(&[CheckedUnitStructuralPathSegment], TypeIdentityId)],
    prefix: &mut Vec<CheckedUnitStructuralPathSegment>,
    traversal: &mut ResidualTraversal<'_>,
    emit: bool,
) -> Option<usize> {
    if moved_paths.is_empty()
        && boundaries.is_empty()
        && !traversal
            .partition_paths
            .iter()
            .any(|path| path.starts_with(prefix) && path.len() > prefix.len())
    {
        // Only the complement is discarded here, so only the complement must
        // be a finite record/array tree of cleanup-free leaves. A moved
        // subtree leaves whole with its own custody and is never inspected.
        if validate_type(types, current_type, &mut Vec::new(), &mut Vec::new()).is_some() {
            if emit {
                traversal.emit(source, prefix, current_type);
            }
            return Some(1);
        }
        // A sum with structural payloads needs selected-case geometry, even
        // when none of those payloads has a hook. Do not flatten it into a
        // whole-root discard merely because no nominal boundary was recorded.
    }
    let nominal_boundary = boundaries.iter().any(|(path, _)| path.is_empty());
    if nominal_boundary
        && (!moved_paths.is_empty()
            || boundaries
                .iter()
                .any(|(path, identity)| path.is_empty() && identity != &current_type))
    {
        return None;
    }
    // A nominal boundary is the owner's hook, not a cut that suppresses its
    // structural children. The hook receives the intact owner first; only its
    // normal return permits recursive reverse-declaration cleanup. Keep that
    // ordering in this same traversal rather than flattening hooks into a set
    // of disjoint leaves or disposing the whole receiver twice.
    if nominal_boundary && emit {
        traversal.emit(source, prefix, current_type);
    }
    let declaration = *types.get(&current_type)?;
    let mut count = usize::from(nominal_boundary);
    match &declaration.shape {
        CheckedUnitStructuralTypeShape::Record { fields, .. } => {
            let child_boundaries = boundaries
                .iter()
                .copied()
                .filter(|(path, _)| !path.is_empty())
                .collect::<Vec<_>>();
            count = count.checked_add(visit_fields(
                types,
                source,
                fields,
                moved_paths,
                &child_boundaries,
                prefix,
                traversal,
                emit,
            )?)?;
        }
        CheckedUnitStructuralTypeShape::FixedArray {
            element_type_identity,
            length,
        } => {
            let mut touched = Vec::new();
            for (path, _) in moved_paths {
                let CheckedUnitStructuralPathSegment::FixedIndex(index) = path[0] else {
                    return None;
                };
                if !length.admits_fixed_index(index) {
                    return None;
                }
                touched.push(index);
            }
            for (path, _) in boundaries {
                let [CheckedUnitStructuralPathSegment::FixedIndex(index), ..] = *path else {
                    return None;
                };
                if !length.admits_fixed_index(*index) {
                    return None;
                }
                touched.push(*index);
            }
            for partition in traversal.partition_paths {
                if let Some([CheckedUnitStructuralPathSegment::FixedIndex(index), ..]) =
                    partition.strip_prefix(prefix.as_slice())
                {
                    if !length.admits_fixed_index(*index) {
                        return None;
                    }
                    touched.push(*index);
                }
            }
            touched.sort_unstable();
            touched.dedup();
            // The untouched complement counts elements, which a recipe extent
            // only fixes at closure; partial moves out of such an array keep
            // no residual plan here.
            let length = length.literal()?;
            let untouched = usize::try_from(length).ok()?.checked_sub(touched.len())?;
            if untouched > 0 {
                // Counting each untouched element as one region is valid only
                // for a maximal cleanup-free subtree. A case-dependent element
                // needs its own checked selection/boundary rows; never discover
                // a larger conditional roster after reserving the output.
                validate_type(
                    types,
                    *element_type_identity,
                    &mut Vec::new(),
                    &mut Vec::new(),
                )?;
            }
            if !emit {
                count = count.checked_add(untouched)?;
            }
            // Enumeration is needed only when writing the already-sized output.
            let indexes = (0..if emit { length } else { 0 })
                .rev()
                .chain(touched.into_iter().rev().filter(|_| !emit));
            for index in indexes {
                let matching = moved_paths.iter().filter_map(|(path, moved_type)| {
                    matches!(path[0], CheckedUnitStructuralPathSegment::FixedIndex(candidate) if candidate == index)
                        .then_some((&path[1..], *moved_type))
                }).collect::<Vec<_>>();
                let child_boundaries = boundaries.iter().filter_map(|(path, boundary_type)| {
                    let (segment, remaining) = path.split_first()?;
                    matches!(segment, CheckedUnitStructuralPathSegment::FixedIndex(candidate) if *candidate == index)
                        .then_some((remaining, *boundary_type))
                }).collect::<Vec<_>>();
                prefix.push(CheckedUnitStructuralPathSegment::FixedIndex(index));
                count = count.checked_add(visit(
                    types,
                    source,
                    *element_type_identity,
                    &matching,
                    &child_boundaries,
                    prefix,
                    traversal,
                    emit,
                )?)?;
                prefix.pop();
            }
        }
        CheckedUnitStructuralTypeShape::Sum { cases }
        | CheckedUnitStructuralTypeShape::Mixed { cases, .. } => {
            let common = match &declaration.shape {
                CheckedUnitStructuralTypeShape::Mixed { fields, .. } => fields.as_slice(),
                _ => &[],
            };
            if cases.is_empty()
                || cases.iter().enumerate().any(|(case_index, case)| {
                    !case.identity.is_valid()
                        || cases[..case_index]
                            .iter()
                            .any(|earlier| earlier.identity == case.identity)
                })
                || moved_paths
                    .iter()
                    .chain(boundaries)
                    .any(|(path, _)| match path.first() {
                        Some(CheckedUnitStructuralPathSegment::Case(identity)) => {
                            !cases.iter().any(|case| &case.identity == identity)
                        }
                        Some(CheckedUnitStructuralPathSegment::Field(_)) => !matches!(
                            declaration.shape,
                            CheckedUnitStructuralTypeShape::Mixed { .. }
                        ),
                        None => !nominal_boundary,
                        _ => true,
                    })
            {
                return None;
            }
            let common_moves = moved_paths
                .iter()
                .copied()
                .filter(|(path, _)| {
                    matches!(
                        path.first(),
                        Some(CheckedUnitStructuralPathSegment::Field(_))
                    )
                })
                .collect::<Vec<_>>();
            let common_boundaries = boundaries
                .iter()
                .copied()
                .filter(|(path, _)| {
                    matches!(
                        path.first(),
                        Some(CheckedUnitStructuralPathSegment::Field(_))
                    )
                })
                .collect::<Vec<_>>();
            let mut reachable_cases = 0_usize;
            let mut established_cases = 0_usize;
            for case in cases {
                let reachability = (traversal.select_case)(prefix, current_type, &case.identity)?;
                if reachability == CleanupCaseReachability::Inactive {
                    continue;
                }
                reachable_cases = reachable_cases.checked_add(1)?;
                if reachability == CleanupCaseReachability::Selected {
                    established_cases = established_cases.checked_add(1)?;
                }
                let condition_depth = traversal.conditions.len();
                if reachability == CleanupCaseReachability::Runtime {
                    traversal.conditions.push(CleanupCaseCondition {
                        path: prefix.clone(),
                        type_identity: current_type,
                        case_identity: case.identity.clone(),
                    });
                }
                let payload_moves = moved_paths.iter().filter_map(|(path, identity)| {
                    matches!(path.first(), Some(CheckedUnitStructuralPathSegment::Case(candidate)) if candidate == &case.identity)
                        .then(|| (&path[1..], *identity))
                }).collect::<Vec<_>>();
                let payload_boundaries = boundaries.iter().filter_map(|(path, identity)| {
                    matches!(path.first(), Some(CheckedUnitStructuralPathSegment::Case(candidate)) if candidate == &case.identity)
                        .then(|| (&path[1..], *identity))
                }).collect::<Vec<_>>();
                // An empty/inline selected alternative still owes disposal of
                // the sum occurrence. Its condition names the tag separately:
                // a bare Case segment is not a typed payload projection.
                if !nominal_boundary
                    && payload_moves.is_empty()
                    && payload_boundaries.is_empty()
                    && common_moves.is_empty()
                    && common_boundaries.is_empty()
                    && !common.iter().any(|field| {
                        matches!(
                            field.field_type,
                            CheckedUnitStructuralFieldType::Structural { .. }
                        )
                    })
                    && validate_inline_alternatives(common, std::slice::from_ref(case)).is_some()
                {
                    if emit {
                        traversal.emit(source, prefix, current_type);
                    }
                    count = count.checked_add(1)?;
                } else {
                    prefix.push(CheckedUnitStructuralPathSegment::Case(
                        case.identity.clone(),
                    ));
                    count = count.checked_add(visit_fields(
                        types,
                        source,
                        &case.fields,
                        &payload_moves,
                        &payload_boundaries,
                        prefix,
                        traversal,
                        emit,
                    )?)?;
                    prefix.pop();
                }
                traversal.conditions.truncate(condition_depth);
            }
            if reachable_cases == 0 || (established_cases > 0 && reachable_cases != 1) {
                return None;
            }
            // Common storage is independent of the active payload. Schedule it
            // once after payload selection, not once in every alternative.
            count = count.checked_add(visit_fields(
                types,
                source,
                common,
                &common_moves,
                &common_boundaries,
                prefix,
                traversal,
                emit,
            )?)?;
        }
        _ => return None,
    }
    Some(count)
}

/// Walk one declaration-ordered namespace. Every move and hook must resolve
/// to its exact structural child; scalar width never substitutes for custody.
fn visit_fields(
    types: &BTreeMap<TypeIdentityId, &CheckedUnitStructuralTypePlan>,
    source: &CheckedUnitStructuralArgumentSourcePlan,
    fields: &[checked_trees::CheckedUnitStructuralFieldPlan],
    moved_paths: &[(&[CheckedUnitStructuralPathSegment], TypeIdentityId)],
    boundaries: &[(&[CheckedUnitStructuralPathSegment], TypeIdentityId)],
    prefix: &mut Vec<CheckedUnitStructuralPathSegment>,
    traversal: &mut ResidualTraversal<'_>,
    emit: bool,
) -> Option<usize> {
    if fields.iter().enumerate().any(|(position, field)| {
        !field.identity.is_valid()
            || field.relevance.is_erased()
            || (!is_partial_affine_field_type(&field.field_type)
                && !field.cleanup_type_identity().is_valid())
            || fields[..position]
                .iter()
                .any(|earlier| earlier.identity == field.identity)
    }) {
        return None;
    }
    let mut count = 0_usize;
    let mut matched = 0_usize;
    let mut matched_boundaries = 0_usize;
    for field in fields.iter().rev() {
        let matching = moved_paths.iter().filter_map(|(path, identity)| {
            let (segment, remaining) = path.split_first()?;
            matches!(segment, CheckedUnitStructuralPathSegment::Field(candidate) if candidate == &field.identity)
                .then_some((remaining, *identity))
        }).collect::<Vec<_>>();
        let child_boundaries = boundaries.iter().filter_map(|(path, identity)| {
            let (segment, remaining) = path.split_first()?;
            matches!(segment, CheckedUnitStructuralPathSegment::Field(candidate) if candidate == &field.identity)
                .then_some((remaining, *identity))
        }).collect::<Vec<_>>();
        matched = matched.checked_add(matching.len())?;
        matched_boundaries = matched_boundaries.checked_add(child_boundaries.len())?;
        let type_identity = field.cleanup_type_identity();
        if !type_identity.is_valid() {
            if !matching.is_empty() || !child_boundaries.is_empty() {
                return None;
            }
            continue;
        }
        prefix.push(CheckedUnitStructuralPathSegment::Field(
            field.identity.clone(),
        ));
        count = count.checked_add(visit(
            types,
            source,
            type_identity,
            &matching,
            &child_boundaries,
            prefix,
            traversal,
            emit,
        )?)?;
        prefix.pop();
    }
    (matched == moved_paths.len() && matched_boundaries == boundaries.len()).then_some(count)
}

/// The same count-before-enumeration walk serves ordinary no-code residuals.
pub fn reconstruct(
    types: &BTreeMap<TypeIdentityId, &CheckedUnitStructuralTypePlan>,
    source: &CheckedUnitStructuralArgumentSourcePlan,
    root_type: TypeIdentityId,
    moved_paths: &[(&[CheckedUnitStructuralPathSegment], TypeIdentityId)],
    boundaries: &[(&[CheckedUnitStructuralPathSegment], TypeIdentityId)],
    max_residuals: usize,
) -> Option<Vec<CheckedUnitPartialAffineDiscardPlan>> {
    let select_case = |prefix: &[CheckedUnitStructuralPathSegment],
                       _: TypeIdentityId,
                       case: &checked_trees::CheckedMemberIdentity| {
        let mut selected = None;
        for (path, _) in moved_paths.iter().chain(boundaries) {
            let Some([CheckedUnitStructuralPathSegment::Case(identity), ..]) =
                path.strip_prefix(prefix)
            else {
                continue;
            };
            if selected
                .is_some_and(|earlier: &checked_trees::CheckedMemberIdentity| earlier != identity)
            {
                return None;
            }
            selected = Some(identity);
        }
        Some(if selected? == case {
            CleanupCaseReachability::Selected
        } else {
            CleanupCaseReachability::Inactive
        })
    };
    reconstruct_case_selected_regions(
        types,
        source,
        root_type,
        moved_paths,
        boundaries,
        &[],
        max_residuals,
        &select_case,
    )?
    .into_iter()
    .map(|region| region.conditions.is_empty().then_some(region.disposition))
    .collect()
}

/// Reconstruct the same maximal complement while retaining runtime case
/// conditions. The source owner supplies reachability, not hook geometry.
/// Alternatives are independent branches, never a product of unrelated cases;
/// recursive payload conditions retain their enclosing selection. Count before
/// enumeration still bounds arrays and the complete published region roster.
/// Conditional-transfer paths only split cleanup-free subtrees so their later
/// checked actions can be selected independently. Nominal boundaries still own
/// intact receivers and precede all their structural descendants.
pub fn reconstruct_case_selected_regions(
    types: &BTreeMap<TypeIdentityId, &CheckedUnitStructuralTypePlan>,
    source: &CheckedUnitStructuralArgumentSourcePlan,
    root_type: TypeIdentityId,
    moved_paths: &[(&[CheckedUnitStructuralPathSegment], TypeIdentityId)],
    boundaries: &[(&[CheckedUnitStructuralPathSegment], TypeIdentityId)],
    partition_paths: &[Vec<CheckedUnitStructuralPathSegment>],
    max_residuals: usize,
    select_case: &dyn Fn(
        &[CheckedUnitStructuralPathSegment],
        TypeIdentityId,
        &checked_trees::CheckedMemberIdentity,
    ) -> Option<CleanupCaseReachability>,
) -> Option<Vec<CaseSelectedCleanupRegion>> {
    if boundaries.iter().enumerate().any(|(position, (path, _))| {
        boundaries[..position]
            .iter()
            .any(|(earlier, _)| path == earlier)
            || moved_paths
                .iter()
                .any(|(moved, _)| path.starts_with(moved) || moved.starts_with(path))
    }) {
        return None;
    }
    if moved_paths.iter().enumerate().any(|(index, (path, _))| {
        path.is_empty()
            || moved_paths[..index]
                .iter()
                .any(|(earlier, _)| path.starts_with(earlier) || earlier.starts_with(path))
    }) {
        return None;
    }
    let mut traversal = ResidualTraversal {
        select_case,
        partition_paths,
        conditions: Vec::new(),
        regions: Vec::new(),
        active_types: Vec::new(),
    };
    let count = visit(
        types,
        source,
        root_type,
        moved_paths,
        boundaries,
        &mut Vec::new(),
        &mut traversal,
        false,
    )?;
    if count > max_residuals {
        return None;
    }
    traversal.regions.try_reserve_exact(count).ok()?;
    let emitted = visit(
        types,
        source,
        root_type,
        moved_paths,
        boundaries,
        &mut Vec::new(),
        &mut traversal,
        true,
    )?;
    (emitted == count && traversal.regions.len() == count).then_some(traversal.regions)
}
