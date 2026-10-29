//! One declaration walk for ordinary scalar observations. Dynamic selectors
//! retain their source and collection geometry; this query proves neither a
//! selector bound, a case guard, nor the lifetime of a loaded reference.

use super::*;
use typed_trees::signature::StateParameter;

#[derive(Clone)]
pub enum ScalarCoordinateProjection {
    Storage(checked_trees::CheckedUnitStructuralPathSegment),
    Element {
        selector: checked_trees::CheckedEntryIndexSelector,
        collection_path: Vec<facts::PlaceSegment>,
        source_step: usize,
        collection_type: TypeReferenceHandle,
        element_type: TypeReferenceHandle,
        extent: Option<usize>,
    },
}

pub struct StructuralScalarCoordinate {
    pub root: facts::PlaceRoot,
    pub type_reference: TypeReferenceHandle,
    pub result_type_reference: TypeReferenceHandle,
    pub primitive_type: PrimitiveType,
    pub domain: ArithmeticDomain,
    pub projections: Vec<ScalarCoordinateProjection>,
    /// Exact frame sources still owed independently of typed geometry. Inline
    /// indexed storage needs its containing collection; any stored-reference
    /// crossing needs the complete coordinate's retained binding evidence.
    pub source_requirements: Vec<Vec<facts::PlaceSegment>>,
    endpoint: Endpoint,
}

enum Endpoint {
    Field {
        carrier: TypeReferenceHandle,
        symbol: SymbolHandle,
        identity: checked_trees::CheckedMemberIdentity,
        position: usize,
    },
    Element {
        carrier: TypeReferenceHandle,
        position: usize,
    },
    Root,
}

fn ordinary_carrier(
    program: &dyn crate::CheckingInput,
    carrier: TypeReferenceHandle,
) -> Option<()> {
    let declared = program
        .type_reference_table
        .unconstrained_type_reference(carrier);
    if program
        .placed_view_plan_for_type_reference(declared)
        .is_some()
        || program
            .placed_view_field_plan_for_type_reference(declared)
            .is_some()
        || matches!(program.type_reference_table.type_reference(declared),
            TypeReferenceNode::Named { symbol, .. } if program.symbols.builtin_type_atom(*symbol).is_some_and(|atom| atom.is_atomic()))
    {
        return None;
    }
    Some(())
}

/// Admit exactly the selected declaration under a formal telescope. Readable
/// shells, qualified scalar type/policy and every projection remain distinct.
/// Current load evidence, selector formation and bounds belong to consumers.
pub fn structural_scalar_coordinate(
    program: &dyn crate::CheckingInput,
    parameters: &[StateParameter],
    parameter: SymbolHandle,
    coordinate: &[facts::PlaceSegment],
) -> Option<StructuralScalarCoordinate> {
    let mut inputs = parameters.iter().filter(|input| {
        input.symbol == parameter && !input.is_const && !input.relevance.is_erased()
    });
    let input = inputs.next()?;
    if inputs.next().is_some() {
        return None;
    }
    structural_scalar_coordinate_in_type(
        program,
        facts::PlaceRoot::Symbol(parameter),
        input.type_reference,
        coordinate,
    )
}

pub fn structural_scalar_coordinate_in_type(
    program: &dyn crate::CheckingInput,
    root: facts::PlaceRoot,
    source_type: TypeReferenceHandle,
    coordinate: &[facts::PlaceSegment],
) -> Option<StructuralScalarCoordinate> {
    let source_type = match program
        .type_reference_table
        .type_reference(crate::peel_declared_constraints(program, source_type))
    {
        TypeReferenceNode::Reference {
            referee, access, ..
        } if access.is_readable() => *referee,
        TypeReferenceNode::Reference { .. } => return None,
        _ => source_type,
    };
    let mut carrier = source_type;
    let mut projections = Vec::new();
    let mut selected_case = SymbolHandle::invalid();
    let mut endpoint = Endpoint::Root;
    let mut stored_crossing = false;
    for (source_step, segment) in coordinate.iter().enumerate() {
        let mut crossings = Vec::new();
        carrier = readable_projection_carrier(program, carrier, &mut crossings)?;
        stored_crossing |= !crossings.is_empty();
        projections.extend(
            crossings
                .into_iter()
                .map(ScalarCoordinateProjection::Storage),
        );
        ordinary_carrier(program, carrier)?;
        match segment {
            facts::PlaceSegment::Case { variant } => {
                if selected_case.is_valid() {
                    return None;
                }
                let record = program.closed_data_definition(carrier)?;
                let mut cases = program
                    .data_members(record)
                    .iter()
                    .filter_map(|member| member.as_variant())
                    .filter(|case| case.symbol == *variant);
                let case = cases.next()?;
                if cases.next().is_some() {
                    return None;
                }
                selected_case = case.symbol;
                projections.push(ScalarCoordinateProjection::Storage(
                    checked_trees::CheckedUnitStructuralPathSegment::Case(
                        checked_trees::CheckedMemberIdentity::of_member(
                            case.identity,
                            case.name.clone(),
                        ),
                    ),
                ));
                endpoint = Endpoint::Root;
            }
            facts::PlaceSegment::Field { symbol } => {
                let record = program.closed_data_definition(carrier)?;
                let fields = if selected_case.is_valid() {
                    let case = program
                        .data_members(record)
                        .iter()
                        .filter_map(|member| member.as_variant())
                        .find(|case| case.symbol == selected_case)?;
                    program.data_payload_fields(case).iter().collect::<Vec<_>>()
                } else {
                    program
                        .data_members(record)
                        .iter()
                        .filter_map(|member| {
                            if let DataMember::Field(field) = member {
                                Some(field)
                            } else {
                                None
                            }
                        })
                        .collect()
                };
                let mut fields = fields.into_iter().filter(|field| field.symbol == *symbol);
                let field = fields.next()?;
                if fields.next().is_some()
                    || field.relevance.is_erased()
                    || program.symbols.get(field.symbol).kind != symbols::SymbolKind::Field
                    || program.symbols.get(field.symbol).parent
                        != if selected_case.is_valid() {
                            selected_case
                        } else {
                            record.symbol
                        }
                {
                    return None;
                }
                let identity = checked_trees::CheckedMemberIdentity::of_member(
                    field.identity,
                    field.name.clone(),
                );
                endpoint = Endpoint::Field {
                    carrier,
                    symbol: field.symbol,
                    identity: identity.clone(),
                    position: projections.len(),
                };
                projections.push(ScalarCoordinateProjection::Storage(
                    checked_trees::CheckedUnitStructuralPathSegment::Field(identity),
                ));
                carrier = field.type_reference;
                selected_case = SymbolHandle::invalid();
            }
            facts::PlaceSegment::FixedIndex { .. } | facts::PlaceSegment::Index { .. }
                if !selected_case.is_valid() =>
            {
                let (element, extent) = if let Some((element, extent)) =
                    crate::value_custody::field_domains::readable_fixed_array_elements(
                        program, carrier,
                    ) {
                    (element, Some(extent))
                } else {
                    (
                        crate::value_custody::field_domains::readable_slice_element(
                            program, carrier,
                        )?,
                        None,
                    )
                };
                endpoint = Endpoint::Element {
                    carrier,
                    position: projections.len() + 1,
                };
                match segment {
                    facts::PlaceSegment::FixedIndex { index } if extent.is_some() => {
                        if *index >= extent? {
                            return None;
                        }
                        projections.push(ScalarCoordinateProjection::Storage(
                            checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                                u64::try_from(*index).ok()?,
                            ),
                        ));
                    }
                    _ => {
                        let selector = match segment {
                            facts::PlaceSegment::FixedIndex { index } => {
                                checked_trees::CheckedEntryIndexSelector::Constant(
                                    u64::try_from(*index).ok()?,
                                )
                            }
                            facts::PlaceSegment::Index { expression } if expression.is_valid() => {
                                checked_trees::CheckedEntryIndexSelector::Expression(*expression)
                            }
                            _ => return None,
                        };
                        projections.push(ScalarCoordinateProjection::Element {
                            selector,
                            collection_path: coordinate[..source_step].to_vec(),
                            source_step,
                            collection_type: carrier,
                            element_type: element,
                            extent,
                        });
                    }
                }
                carrier = element;
            }
            _ => return None,
        }
    }
    if selected_case.is_valid() {
        return None;
    }
    let result_type_reference = carrier;
    let mut crossings = Vec::new();
    let readable_leaf = readable_projection_carrier(program, carrier, &mut crossings)?;
    ordinary_carrier(program, readable_leaf)?;
    let primitive_type = realized_scalar_carrier(program, result_type_reference)?;
    stored_crossing |= !crossings.is_empty();
    let prefix = if stored_crossing {
        coordinate
    } else {
        // Typed element observations include literal slice coordinates. Both
        // selector spellings depend on the containing descriptor binding.
        let end = projections
            .iter()
            .find_map(|step| match step {
                ScalarCoordinateProjection::Element { source_step, .. } => Some(*source_step),
                _ => None,
            })
            .unwrap_or(coordinate.len());
        &coordinate[..end]
    };
    let source_requirements = vec![prefix.to_vec()];
    projections.extend(
        crossings
            .into_iter()
            .map(ScalarCoordinateProjection::Storage),
    );
    Some(StructuralScalarCoordinate {
        root,
        type_reference: source_type,
        result_type_reference,
        primitive_type,
        domain: program.arithmetic_domain_for_type_reference(readable_leaf),
        projections,
        source_requirements,
        endpoint,
    })
}

/// Legacy static-field recipes retain their original final-field split.
/// Dynamic indexed observations use the complete typed coordinate above.
pub fn structural_scalar_field_at_coordinate(
    program: &dyn crate::CheckingInput,
    parameters: &[StateParameter],
    parameter: SymbolHandle,
    coordinate: &[facts::PlaceSegment],
) -> Option<StructuralScalarField> {
    let selected = structural_scalar_coordinate(program, parameters, parameter, coordinate)?;
    let Endpoint::Field {
        carrier,
        symbol,
        identity,
        position,
    } = selected.endpoint
    else {
        return None;
    };
    let path = selected
        .projections
        .into_iter()
        .take(position)
        .map(|step| {
            if let ScalarCoordinateProjection::Storage(step) = step {
                Some(step)
            } else {
                None
            }
        })
        .collect::<Option<Vec<_>>>()?;
    Some(StructuralScalarField {
        root: selected.root,
        receiver_value: ExpressionHandle::invalid(),
        statement_ordinal: 0,
        type_reference: selected.type_reference,
        carrier_type_reference: carrier,
        result_type_reference: selected.result_type_reference,
        path,
        field: symbol,
        field_identity: identity,
        primitive_type: selected.primitive_type,
        domain: selected.domain,
    })
}

pub fn structural_scalar_field_at_element_coordinate(
    program: &dyn crate::CheckingInput,
    state: SymbolHandle,
    parameter: SymbolHandle,
    coordinate: &[facts::PlaceSegment],
) -> Option<StructuralScalarField> {
    let state = program.state_by_symbol(state)?;
    let reference = program
        .state_parameter_by_symbol(state, parameter)
        .filter(|input| !input.is_const && !input.relevance.is_erased())
        .map(|input| input.type_reference)
        .or_else(|| {
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .filter_map(|statement| statement.as_local_data())
                .find(|local| local.symbol == parameter)
                .map(|local| local.type_reference)
        })?;
    let selected = structural_scalar_coordinate_in_type(
        program,
        facts::PlaceRoot::Symbol(parameter),
        reference,
        coordinate,
    )?;
    let Endpoint::Element { carrier, position } = selected.endpoint else {
        return None;
    };
    let path = selected
        .projections
        .into_iter()
        .take(position)
        .map(|step| {
            if let ScalarCoordinateProjection::Storage(step) = step {
                Some(step)
            } else {
                None
            }
        })
        .collect::<Option<Vec<_>>>()?;
    Some(StructuralScalarField {
        root: selected.root,
        receiver_value: ExpressionHandle::invalid(),
        statement_ordinal: 0,
        type_reference: selected.type_reference,
        carrier_type_reference: carrier,
        result_type_reference: selected.result_type_reference,
        path,
        field: SymbolHandle::invalid(),
        field_identity: Default::default(),
        primitive_type: selected.primitive_type,
        domain: selected.domain,
    })
}
