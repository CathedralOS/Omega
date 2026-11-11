/*
    Where the actual of each erased scalar lane comes from.

    An erased primitive rides one erased scalar lane; an erased closed record
    rides one lane per scalar field (`TypedTrees::erased_scalar_lanes`). A
    call must supply each lane without reading anything at run time, so every
    lane has exactly one of the sources below. This module is the single
    owner of that decision for both kinds of lane: source checking refuses a
    call whose record leaf has no source, and checked lowering lowers the
    source this module names.
*/
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

/// The source of one erased scalar lane's actual.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErasedLeaf {
    /// The matching erased scalar lane of the caller's own erased formal.
    Formal { position: usize },
    /// A scalar expression lowered as a proof-side value: the primitive
    /// actual itself, or the scalar written at the lane's field of an
    /// immutable closed construction.
    Value(ExpressionHandle),
    /// That field of the caller's runtime record formal, observed at its place.
    ParameterField {
        position: u32,
        segments: Vec<facts::PlaceSegment>,
    },
    /// That field of a structural local, observed at its place.
    LocalField {
        symbol: SymbolHandle,
        segments: Vec<facts::PlaceSegment>,
    },
}

const BOOLEAN_PLACE: &str = "a `bool` field can be supplied only by a record construction or the caller's own erased binding, not observed at a local's or parameter's place";
const MUTABLE_FORMAL: &str =
    "a `mut` or `const` parameter has no entry place whose field the call can observe";
const MUTABLE_BORROW: &str = "a `&mut` borrow is not a shared view of the value";
const NOT_A_LANE: &str = "the path below the caller's binding is not one of its scalar fields";
const NOT_A_SHARED_BORROW: &str =
    "a reference local names its referent only when it is initialized by a shared borrow";
const NOT_CLOSED: &str = "the local's initializer does not write that field as a closed value";
const TEMPORARY: &str = "a temporary has no place to observe; bind it to a local first, or write the record construction in the argument";
const UNINITIALIZED: &str = "the local is not initialized before the call";
const UNNAMED_CONSTRUCTION: &str = "a construction written in the argument supplies only fields whose values are closed; bind the record to a local first";

/*
    Find the source of one erased scalar lane without reading anything at run
    time. `fields` is the lane's path below the erased formal: empty for a
    primitive lane, whose actual is itself the scalar, and the field path for
    a record leaf.

    One walk serves both. Shared borrows and immutable shared-reference
    locals name their referent, and an immutable local initialized by a
    closed construction supplies the scalar written at the selected field.
    The lane kind decides only where the walk stops:

    - A primitive lane stops at the first expression that is not such a
      local and lowers it as an ordinary proof-side value (a parameter, a
      constant, a mutable local, arithmetic).
    - A record leaf stops at the caller's erased formal, whose lane is
      forwarded; at a closed construction (written in place, bound to an
      immutable local, or named by a constant), whose field initializer is
      the leaf; or at a record local or runtime record formal, whose own
      place is observed (a mutable local always, since its initializer may be
      stale).

    Every other actual is refused with its reason, which source checking
    reports at the argument: a `mut` formal, a temporary, a `&mut` view, and
    a `bool` leaf observed at a place (the checked Boolean vocabulary has no
    place observation outside a formal's own contract).
*/
#[allow(clippy::too_many_arguments)]
pub fn resolve_leaf(
    program: &dyn crate::CheckingInput,
    state_symbol: SymbolHandle,
    before_statement: u32,
    expression: ExpressionHandle,
    formal_type: TypeReferenceHandle,
    fields: &[SymbolHandle],
    primitive_type: PrimitiveType,
    authored_parameters: &[StateParameter],
) -> Result<ErasedLeaf, &'static str> {
    let state = program
        .state_by_symbol(state_symbol)
        .ok_or("the calling state is unknown")?;
    let primitive_lane = fields.is_empty();
    let record_type = match program.type_reference_table.type_reference(formal_type) {
        TypeReferenceNode::Reference { referee, .. } => *referee,
        _ => formal_type,
    };
    let leaf_segments = |prefix: &[facts::PlaceSegment]| {
        prefix
            .iter()
            .cloned()
            .chain(
                fields
                    .iter()
                    .map(|symbol| facts::PlaceSegment::Field { symbol: *symbol }),
            )
            .collect::<Vec<_>>()
    };
    let is_constant = |expression: ExpressionHandle| {
        program
            .expression_table
            .name_path_node(expression)
            .filter(|path| path.symbol.is_valid())
            .and_then(|path| {
                program
                    .const_declarations()
                    .iter()
                    .find(|constant| constant.symbol == path.symbol)
            })
    };
    let mut current = expression;
    for _ in 0..=program.expression_table.expression_count() {
        if let ExpressionNode::Borrow(borrow) = program.expression_table.expression(current) {
            if borrow.access != language_core::ReferenceAccess::Shared {
                return Err(MUTABLE_BORROW);
            }
            current = borrow.target;
            continue;
        }
        // The place this expression names, when it is rooted at a binding.
        let named = if static_place_syntax(program, current) {
            crate::canonical_place_from_expression(program, current)
                .and_then(|place| Some((place.root.as_symbol()?, place)))
        } else {
            None
        };
        let local = named.as_ref().and_then(|(symbol, _)| {
            program
                .state_local_declaration(state, *symbol)
                .map(|(ordinal, local)| (ordinal, local))
        });
        if primitive_lane && local.is_none() {
            return Ok(ErasedLeaf::Value(current));
        }
        if let Some(constant) = is_constant(current) {
            let initializer = if constant.authored_initializer.is_valid() {
                constant.authored_initializer
            } else {
                constant.materialized_initializer
            };
            let (leaf, _) = crate::closed_scalar_place_projection(
                program,
                initializer,
                constant.declared_type,
                &leaf_segments(&[]),
            )
            .ok_or("the constant's initializer does not write that field as a closed value")?;
            return Ok(ErasedLeaf::Value(leaf));
        }
        if !static_place_syntax(program, current) {
            let (leaf, _) = crate::closed_scalar_place_projection(
                program,
                current,
                record_type,
                &leaf_segments(&[]),
            )
            .ok_or(
                if matches!(
                    program.expression_table.expression(current),
                    ExpressionNode::StructLiteral(_)
                ) {
                    UNNAMED_CONSTRUCTION
                } else {
                    TEMPORARY
                },
            )?;
            return Ok(ErasedLeaf::Value(leaf));
        }
        let (symbol, place) =
            named.ok_or("the argument is not a place rooted at a binding of the calling state")?;
        let segments = leaf_segments(&place.segments);
        if let Some((position, parameter)) = authored_parameters
            .iter()
            .enumerate()
            .find(|(_, parameter)| parameter.symbol == symbol)
        {
            if parameter.is_mutable || parameter.is_const {
                return Err(MUTABLE_FORMAL);
            }
            if !parameter.relevance.is_erased() {
                if primitive_type == PrimitiveType::Bool {
                    return Err(BOOLEAN_PLACE);
                }
                return Ok(ErasedLeaf::ParameterField {
                    position: u32::try_from(position).map_err(|_| NOT_A_LANE)?,
                    segments,
                });
            }
            let wanted = segments
                .iter()
                .map(|segment| match segment {
                    facts::PlaceSegment::Field { symbol } => Some(*symbol),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()
                .ok_or(NOT_A_LANE)?;
            let lane = program
                .erased_scalar_lanes(parameter.type_reference)
                .iter()
                .position(|lane| lane.fields == wanted)
                .ok_or(NOT_A_LANE)?;
            return Ok(ErasedLeaf::Formal {
                position: program.erased_scalar_lane_base(authored_parameters, position) + lane,
            });
        }
        let (ordinal, local) =
            local.ok_or("the argument is not a local or parameter of the calling state")?;
        if ordinal >= before_statement as usize || !local.initial_value.is_valid() {
            return Err(UNINITIALIZED);
        }
        // Where a walk that cannot see through this local stops: a primitive
        // lane lowers the place as a value, a record leaf observes it.
        let observed = |segments| {
            if primitive_lane {
                Ok(ErasedLeaf::Value(current))
            } else if primitive_type == PrimitiveType::Bool {
                Err(BOOLEAN_PLACE)
            } else {
                Ok(ErasedLeaf::LocalField { symbol, segments })
            }
        };
        let reference_access = match program
            .type_reference_table
            .type_reference(local.type_reference)
        {
            TypeReferenceNode::Reference { access, .. } => Some(*access),
            _ => None,
        };
        if local.is_mutable {
            // A mutable local's initializer may be stale; only its own place
            // holds the value the call observes.
            if reference_access.is_some() && !primitive_lane {
                return Err("a `mut` reference local may be rebound, so its referent is not exact");
            }
            return observed(segments);
        }
        if let Some(access) = reference_access
            && (place.segments.is_empty() || !primitive_lane)
        {
            let ExpressionNode::Borrow(borrow) =
                program.expression_table.expression(local.initial_value)
            else {
                return if primitive_lane {
                    observed(segments)
                } else {
                    Err(NOT_A_SHARED_BORROW)
                };
            };
            if !place.segments.is_empty()
                || access != language_core::ReferenceAccess::Shared
                || borrow.access != access
            {
                return Err(NOT_A_SHARED_BORROW);
            }
            current = borrow.target;
            continue;
        }
        if let Some((leaf, _)) = crate::closed_scalar_place_projection(
            program,
            local.initial_value,
            local.type_reference,
            &segments,
        ) {
            return Ok(ErasedLeaf::Value(leaf));
        }
        if primitive_lane {
            // A whole primitive local is its own value; a field of a local
            // that is not a closed construction has no exact value here.
            return if place.segments.is_empty() {
                observed(segments)
            } else {
                Err(NOT_CLOSED)
            };
        }
        // `let record: Witness = RECORD;` names the constant's value.
        if place.segments.is_empty() && is_constant(local.initial_value).is_some() {
            current = local.initial_value;
            continue;
        }
        // Any other record local (a call result, a match join) holds the
        // value the callee names; its leaf is observed at the local's place.
        return observed(segments);
    }
    Err("the argument does not resolve to a value the call can name")
}

fn static_place_syntax(
    program: &dyn crate::CheckingInput,
    mut expression: ExpressionHandle,
) -> bool {
    for _ in 0..program.expression_table.expression_count() {
        match program.expression_table.expression(expression) {
            ExpressionNode::Name(_) => return true,
            ExpressionNode::Member(member) => expression = member.receiver,
            ExpressionNode::Indexed(indexed) => expression = indexed.collection,
            _ => return false,
        }
    }
    false
}

/*
    Refuse, at source checking and at the call the user wrote, a call or
    transition whose erased record actual has a leaf with no source, and a
    nested machine whose erased record formal a transition cannot carry.
    Checked lowering asks the same resolver, so a call that passes here has a
    term for every lane; without this the program would fail inside lowering
    with no usable message.
*/
pub(crate) fn validate(program: &dyn crate::CheckingInput, diagnostics: &mut Vec<Diagnostic>) {
    let proof_only = crate::proof_only_classification(program);
    for machine in program.machines() {
        if proof_only.is_proof_machine(program, machine) {
            continue;
        }
        for (state_index, state) in program.machine_states(machine).iter().enumerate() {
            let parameters = program.state_parameters(state);
            // A transition argument row is keyed by its authored position and
            // carries one erased scalar, so a nested machine can receive an
            // erased record only when it has a single scalar field.
            for parameter in parameters
                .iter()
                .filter(|parameter| state_index != 0 && parameter.relevance.is_erased())
            {
                let lanes = program.erased_scalar_lanes(parameter.type_reference);
                if lanes.len() > 1 {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}::{}` erased parameter `{}` is a record with {} scalar fields; a transition into a nested machine carries one erased scalar per argument, so pass each field as its own erased parameter",
                        machine.name,
                        state.name,
                        parameter.name,
                        lanes.len(),
                    )));
                }
            }
            let statement_count = program
                .statement_table
                .statements(state.statement_nodes)
                .len();
            for statement_index in 0..statement_count {
                let Ok(before_statement) = u32::try_from(statement_index) else {
                    continue;
                };
                for call_site in
                    crate::collect_call_sites_in_statement(program, machine, state, statement_index)
                {
                    let (target_symbol, call_span) = match &call_site {
                        crate::CallSite::Statement(call) => (call.target_symbol, call.source_span),
                        crate::CallSite::Expression { expression, call } => (
                            crate::call_target_symbol(program, call),
                            program.expression_table.source_span(*expression),
                        ),
                        crate::CallSite::TransitionNamed {
                            path, source_span, ..
                        } => (path.symbol, *source_span),
                    };
                    let Some(target_parameters) =
                        crate::call_target_parameters(program, target_symbol)
                    else {
                        continue;
                    };
                    let arguments = crate::call_site_argument_expressions(program, &call_site);
                    if arguments.len() != target_parameters.len() {
                        continue;
                    }
                    for (argument, target) in arguments.iter().zip(target_parameters) {
                        if !target.relevance.is_erased() {
                            continue;
                        }
                        let refusal = program
                            .erased_scalar_lanes(target.type_reference)
                            .into_iter()
                            .filter(|lane| !lane.fields.is_empty())
                            .find_map(|lane| {
                                resolve_leaf(
                                    program,
                                    state.symbol,
                                    before_statement,
                                    *argument,
                                    target.type_reference,
                                    &lane.fields,
                                    lane.primitive_type,
                                    parameters,
                                )
                                .err()
                                .map(|reason| (lane, reason))
                            });
                        if let Some((lane, reason)) = refusal {
                            let field = lane
                                .fields
                                .iter()
                                .map(|field| program.symbols.name(*field))
                                .collect::<Vec<_>>()
                                .join(".");
                            diagnostics.push(
                                Diagnostic::error(format!(
                                    "machine `{}::{}` cannot pass this argument as erased record `{}` of `{}`: field `{field}` has no exact value at the call; {reason}",
                                    machine.name,
                                    state.name,
                                    target.name,
                                    program
                                        .machine_by_entry_state_symbol(target_symbol)
                                        .map_or(program.symbols.name(target_symbol), |(callee, _)| {
                                            callee.name.as_str()
                                        }),
                                ))
                                .with_source_span(call_span),
                            );
                        }
                    }
                }
            }
        }
    }
}
