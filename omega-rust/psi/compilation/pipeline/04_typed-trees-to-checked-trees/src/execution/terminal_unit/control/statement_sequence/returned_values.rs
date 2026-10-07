//! What a unit sequence returns: parameter pass-through, reference leaves,
//! named and mutable views, subslice windows, and byte-sequence literals —
//! each classifier selects one result spelling the completion emits.

use super::super::super::{
    CheckedUnitEntryClaimPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};
use super::super::{
    CheckFacts, CheckedStructuralAccess, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralResultBindingPlan, ExpressionNode,
    Multiplicity, StatementNode, SymbolHandle, TypedTrees,
};
use crate::execution::terminal_unit::types::ShapeCollector;
use checked_trees::CheckedUnitStructuralReturnPlan;

/// Completion forwards a whole parameter only when its exact output contract
/// and, for linear values, checked input-origin claim outcome agree.
pub(in crate::execution::terminal_unit) fn returned_parameter(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    entry_claims: &[CheckedUnitEntryClaimPlan],
) -> Option<CheckedUnitStructuralReturnPlan> {
    let StatementNode::Expression(expression) = program
        .statement_table
        .statements(state.statement_nodes)
        .last()?
    else {
        return None;
    };
    let ExpressionNode::Name(path) = program.expression_table.expression(*expression) else {
        return None;
    };
    if path.head_symbol != path.symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
    {
        return None;
    }
    let source_parameters = program.state_parameters(state);
    let (parameter_index, parameter) = parameters.iter().enumerate().find(|(_, parameter)| {
        source_parameters
            .get(parameter.position as usize)
            .is_some_and(|source| source.symbol == path.symbol)
    })?;
    let source = source_parameters.get(parameter.position as usize)?;
    if let Some(source) = super::super::super::reference_results::initializer_source(
        program,
        state,
        *expression,
        state.return_type,
    ) {
        if super::super::super::reference_results::source_parameter(program, state)?
            != parameter.position as usize
            || !parameter.qualifications.is_empty()
        {
            return None;
        }
        return Some(CheckedUnitStructuralReturnPlan {
            source: source.source.clone(),
            type_identity: program
                .normalized_type_identity(state.return_type)
                .into_string(),
            multiplicity: Multiplicity::Affine,
            reference_sources: vec![checked_trees::CheckedReferenceResultSourcePlan {
                path: Vec::new(),
                source,
            }],
        });
    }
    // A `&'a [u8]` whole-view return forwards the view parameter's own borrowed
    // storage: the same `Parameter` result source a whole owned forward mints,
    // carrying the shared-borrow access the view parameter was admitted with.
    // No reference is established — the loan passes through unchanged.
    if crate::execution::terminal_unit::types::borrowed_slice_view(program, state.return_type) {
        if parameter.access != CheckedStructuralAccess::SharedBorrow
            || !parameter.qualifications.is_empty()
            || program.normalized_type_identity(source.type_reference)
                != program.normalized_type_identity(state.return_type)
        {
            return None;
        }
        return Some(CheckedUnitStructuralReturnPlan {
            source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: u32::try_from(parameter_index).ok()?,
            },
            type_identity: parameter.type_identity.clone(),
            multiplicity: parameter.multiplicity,
            reference_sources: Vec::new(),
        });
    }
    if parameter.access != CheckedStructuralAccess::Owned
        || parameter.multiplicity != program.type_multiplicity(state.return_type)
        || program.normalized_type_identity(source.type_reference)
            != program.normalized_type_identity(state.return_type)
        || parameter.qualifications
            != validation::structural_result_qualifications(program, state.return_type).ok()?
    {
        return None;
    }
    if parameter.multiplicity == Multiplicity::Linear {
        let returned = validation::reconstruct_structural_parameter_return_claims(
            program,
            facts,
            machine.symbol,
            state.symbol,
            source.symbol,
        )
        .ok()?;
        let mut input = entry_claims
            .iter()
            .filter(|claim| claim.parameter_index as usize == parameter_index)
            .map(|claim| (claim.claim_identity, claim.path.clone()))
            .collect::<Vec<_>>();
        input.sort_by(|left, right| left.1.cmp(&right.1));
        // Completion and ordinary calls share one input-origin correspondence;
        // matching paths alone cannot replace the exact checked claim identities.
        if input != returned {
            return None;
        }
    }
    Some(CheckedUnitStructuralReturnPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
            parameter_index: u32::try_from(parameter_index).ok()?,
        },
        type_identity: parameter.type_identity.clone(),
        multiplicity: parameter.multiplicity,
        reference_sources: if validation::reference_result_custody::is_reference_record(
            program,
            state.return_type,
        ) {
            validation::reference_result_custody::returned_record_sources(program, state)?
        } else {
            Vec::new()
        },
    })
}

/// A `value.body`-style completion selects one borrowed carrier leaf of an
/// owned record formal rather than forwarding the whole parameter. The leaf
/// argument already carries its exact ingress path; the shared
/// reference-result block emits its establishment and rewrites the result
/// source to that binding. The establishment consumes the carrier whole, so
/// `source_leaf` only admits a record whose declared reference roster is
/// exactly the selected leaf.
pub(in crate::execution::terminal_unit) fn returned_reference_leaf(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
) -> Option<CheckedUnitStructuralReturnPlan> {
    let (_, source) = validation::reference_result_custody::source_leaf(program, state)?;
    let CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } = source.source
    else {
        return None;
    };
    let parameter = parameters.get(parameter_index as usize)?;
    if parameter.access != CheckedStructuralAccess::Owned
        || parameter.multiplicity != Multiplicity::Affine
        || !parameter.qualifications.is_empty()
        || parameter.fused_service_erasure.is_some()
    {
        return None;
    }
    Some(CheckedUnitStructuralReturnPlan {
        source: source.source.clone(),
        type_identity: program
            .normalized_type_identity(state.return_type)
            .into_string(),
        multiplicity: Multiplicity::Affine,
        reference_sources: vec![checked_trees::CheckedReferenceResultSourcePlan {
            path: Vec::new(),
            source,
        }],
    })
}

/// A `&'a V` named-result completion borrows the referent it names from one
/// readable structural carrier, either directly or through a field. The borrow is the
/// result — there are no subslice endpoints and no separate slice length.
/// Unlike `returned_reference_leaf`'s `&mut` record rosters the carrier may
/// itself be borrowed. A mutable input can lend shared observation; its
/// exclusivity does not require the output to remain mutable.
pub(in crate::execution::terminal_unit) fn returned_named_view(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    binders: &[(SymbolHandle, String)],
) -> Option<CheckedUnitStructuralReturnPlan> {
    if !crate::execution::terminal_unit::types::borrowed_named_view(program, state.return_type) {
        return None;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let statement_index = statements.len().checked_sub(1)?;
    // A bare `transition { _ -> &place }` completes the state exactly like a
    // trailing `(&place)` expression: its `Value` target is the authored
    // return expression.
    let expression = match statements.last()? {
        StatementNode::Expression(expression) => *expression,
        StatementNode::Transition(transition)
            if transition.exit == typed_trees::statement::TransitionExit::Ordinary =>
        {
            let typed_trees::statement::TransitionTargetNode::Value(expression) =
                program.statement_table.transition_target(transition.target)
            else {
                return None;
            };
            *expression
        }
        _ => return None,
    };
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        expression,
    )?;
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return None;
    };
    let authored = program.state_parameters(state);
    let position = authored
        .iter()
        .position(|parameter| parameter.symbol == symbol)?;
    let carrier = &authored[position];
    if !matches!(
        crate::execution::terminal_unit::types::structural_access_for_type_reference(
            program,
            carrier.type_reference,
        ),
        Some(
            CheckedStructuralAccess::SharedBorrow
                | CheckedStructuralAccess::MutableBorrow
                | CheckedStructuralAccess::Owned
        )
    ) {
        return None;
    }
    let parameter_index = parameters
        .iter()
        .position(|parameter| parameter.position as usize == position)?;
    let (storage, path) = crate::execution::terminal_unit::calls::projected_argument_path(
        program,
        state.symbol,
        statement_index,
        &place,
    )?;
    let referee = crate::execution::terminal_unit::types::borrowed_view_referent(
        program,
        state.return_type,
        language_semantics::ReferenceAccess::Shared,
    )?;
    // The stored value the projection bottoms out in must be exactly the
    // result's `&'a V` or its referent `V`.
    if program.normalized_type_identity(storage)
        != program.normalized_type_identity(state.return_type)
        && program.normalized_type_identity(storage) != program.normalized_type_identity(referee)
    {
        return None;
    }
    // Only a stored reference adds an indirection. A projected ordinary
    // value already names the result's referent; a whole parameter borrow
    // likewise already names its referent storage.
    let mut path = path;
    if !path.is_empty() && validation::reference_shell(program, storage).is_some() {
        path.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
    }
    let source = CheckedUnitStructuralArgumentPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
            parameter_index: u32::try_from(parameter_index).ok()?,
        },
        path,
        type_identity: shapes.add_type(referee, binders, &[])?,
        access: CheckedStructuralAccess::SharedBorrow,
    };
    Some(CheckedUnitStructuralReturnPlan {
        source: source.source.clone(),
        // The result spells its own `&'a V` type reference and loans the
        // referent's storage affinely — the same custody a `&mut` primitive
        // reference result carries.
        type_identity: shapes.add_view_type(
            state.return_type,
            binders,
            language_semantics::ReferenceAccess::Shared,
        )?,
        multiplicity: Multiplicity::Affine,
        reference_sources: vec![checked_trees::CheckedReferenceResultSourcePlan {
            path: Vec::new(),
            source,
        }],
    })
}

/// An authored `[start..end]` member or whole-parameter subslice completing a
/// borrowed `&[u8]`/`&[T]` view return: the last statement's own indexed
/// expression uses the ordinary checked subslice producer, as a local or call
/// operand does. Its descriptor carries backing and extent by value; Terminal
/// independently reconstructs the original backing through the producer.
pub(in crate::execution::terminal_unit) fn returned_subslice(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    count: &mut usize,
) -> Option<(
    CheckedUnitStructuralReturnPlan,
    CheckedUnitEffectOperationPlan,
)> {
    if !crate::execution::terminal_unit::types::borrowed_slice_view(program, state.return_type) {
        return None;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let StatementNode::Expression(expression) = statements.last()? else {
        return None;
    };
    let statement_ordinal = u32::try_from(statements.len() - 1).ok()?;
    let operation = super::super::super::state_graph::returns::view_result_operation(
        program,
        facts,
        shapes,
        machine,
        state,
        parameters,
        count,
        statement_ordinal,
        *expression,
    )?;
    let CheckedUnitEffectOperationPlan::EstablishViewSubslice { result, .. } = &operation else {
        return None;
    };
    Some((result.clone().into(), operation))
}

/// A `&mut place` tail completing an exclusive `&mut R`/`&mut [T]` view
/// return delegates to the state-graph's mutable-view producer: the denoted
/// place resolves through the state's own minted `&mut` loans until it roots
/// at a `&mut` parameter (or a `self` member the loan publishes as its own
/// root), and the projected path — runtime selectors included — spells the
/// result's reference source. The tail is the expression or unconditional
/// ordinary value-transition the state's exit spells, matching the shape the
/// terminator route reads.
pub(in crate::execution::terminal_unit) fn returned_mutable_view(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    count: &mut usize,
) -> Option<(
    CheckedUnitStructuralReturnPlan,
    CheckedUnitEffectOperationPlan,
)> {
    if !crate::execution::terminal_unit::types::mutable_borrowed_view(program, state.return_type) {
        return None;
    }
    let expression = validation::reference_result_custody::tail_return_expression(program, state)?;
    let statement_ordinal = u32::try_from(
        program
            .statement_table
            .statements(state.statement_nodes)
            .len()
            .checked_sub(1)?,
    )
    .ok()?;
    let operation = super::super::super::state_graph::returns::mutable_view_result_operation(
        program,
        facts,
        shapes,
        machine,
        state,
        parameters,
        count,
        statement_ordinal,
        expression,
    )?;
    let CheckedUnitEffectOperationPlan::EstablishReference { result, source } = operation else {
        return None;
    };
    // The minted `&mut` view ends exactly once at the result, so the binding
    // spells Affine rather than the exclusive borrow's Unrestricted type
    // multiplicity. This permission transfer remains affine.
    let result = CheckedUnitStructuralResultBindingPlan {
        multiplicity: Multiplicity::Affine,
        ..result
    };
    Some((
        CheckedUnitStructuralReturnPlan {
            source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal,
            },
            type_identity: result.type_identity.clone(),
            multiplicity: result.multiplicity,
            reference_sources: vec![checked_trees::CheckedReferenceResultSourcePlan {
                path: Vec::new(),
                source: source.clone(),
            }],
        },
        CheckedUnitEffectOperationPlan::EstablishReference { result, source },
    ))
}

/// A `("bytes")` tail completing a borrowed `&[u8]` view return: the literal
/// names its own immutable storage, so the result source is the spelled byte
/// sequence itself — the same source shape a boundary call argument carries.
/// No parameter, local, or result custody is involved and no reference source
/// is established: the verifier's literal place kind is self-describing.
pub(in crate::execution::terminal_unit) fn returned_byte_sequence_literal(
    program: &TypedTrees,
    state: &typed_trees::state::State,
) -> Option<CheckedUnitStructuralReturnPlan> {
    if !crate::execution::terminal_unit::types::borrowed_slice_view(program, state.return_type) {
        return None;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let StatementNode::Expression(expression) = statements.last()? else {
        return None;
    };
    let ExpressionNode::String(bytes) = program.expression_table.expression(*expression) else {
        return None;
    };
    // Peeling a shared descriptor's storage identity changes neither its
    // multiplicity nor the lifetime of the immutable literal backing.
    let (return_type, multiplicity) =
        match validation::reference_result_custody::shared_place_borrowed_parts(
            program,
            state.return_type,
        ) {
            Some((referee, _)) => (
                referee,
                validation::reference_result_custody::result_multiplicity(
                    program,
                    state.return_type,
                ),
            ),
            None => (
                state.return_type,
                program.type_multiplicity(state.return_type),
            ),
        };
    Some(CheckedUnitStructuralReturnPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral {
            bytes: bytes.to_vec(),
        },
        type_identity: program.normalized_type_identity(return_type).into_string(),
        multiplicity,
        reference_sources: Vec::new(),
    })
}
