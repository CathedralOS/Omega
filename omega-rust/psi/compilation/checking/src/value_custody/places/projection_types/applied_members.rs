//! Rebase member selectors through their exact applied storage owner.
use facts::PlaceSegment;
use typed_trees::types::TypeReferenceHandle;

/// A storage path may cross another prepared occurrence of its same applied
/// declaration. Keep exact existing coordinates; otherwise use
/// constructor correspondence, which authenticates the full application and
/// member ordinal. A same-spelled field in another owner supplies nothing.
/// Type projection does not authenticate case tags, so every case uses that
/// correspondence. This selects coordinates, not case, access or loan facts.
pub fn corresponding_place_projection(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    path: &[PlaceSegment],
) -> Option<Vec<PlaceSegment>> {
    if !path.iter().any(|segment| {
        matches!(
            segment,
            PlaceSegment::Field { .. } | PlaceSegment::Case { .. }
        )
    }) || (!path
        .iter()
        .any(|segment| matches!(segment, PlaceSegment::Case { .. }))
        && crate::project_type_reference_from_segments(program, reference, path).is_some())
    {
        return Some(path.to_vec());
    }
    let mut selected = Vec::with_capacity(path.len());
    let mut case = None;
    for segment in path {
        let current = crate::project_type_reference_from_segments(program, reference, &selected)?;
        let current = crate::unwrapped_type_reference(program, current)?;
        let segment = match *segment {
            PlaceSegment::Case { variant } => {
                if !variant.is_valid() || case.is_some() {
                    return None;
                }
                let owner = program.symbols.get(variant).parent;
                let variant = program
                    .data_constructor_case(current, owner, variant)?
                    .symbol;
                case = Some(variant);
                PlaceSegment::Case { variant }
            }
            PlaceSegment::Field { symbol } => {
                if !symbol.is_valid() {
                    return None;
                }
                let parent = program.symbols.get(symbol).parent;
                if !parent.is_valid() {
                    return None;
                }
                let owner = if program.symbols.get(parent).kind == symbols::SymbolKind::Variant {
                    program.symbols.get(parent).parent
                } else {
                    parent
                };
                let symbol = program
                    .data_constructor_field(current, owner, symbol)?
                    .symbol;
                if let Some(case) = case.take() {
                    let (_, case) = program.variant_owner(case)?;
                    if !program
                        .data_payload_fields(case)
                        .iter()
                        .any(|field| field.symbol == symbol)
                    {
                        return None;
                    }
                }
                PlaceSegment::Field { symbol }
            }
            segment => {
                if case.is_some() {
                    return None;
                }
                segment
            }
        };
        selected.push(segment);
    }
    Some(selected)
}
