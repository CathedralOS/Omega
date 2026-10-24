//! Exact collection extents, kept separate from scalar parameter values.
//! Collection lengths compose through borrows and range formation regardless
//! of whether the source extent is declared or observed. Saved initializers
//! use the same evaluator without caller facts: a later observation cannot
//! retroactively describe the descriptor captured by an earlier binding.
use super::coordinate::{Coordinate, CoordinateFamily, CoordinateKind, CoordinateSet};
use super::fields::{rooted_carrier, unique_literal_field};
use super::{
    BTreeMap, BigInt, BinaryOperator, Comparison, Engine, ExpressionHandle, ExpressionNode,
    Machine, Polynomial, RankSubjectCoordinate, State, TypeReferenceNode, projections,
};
use symbols::SymbolHandle;
use typed_trees::signature::StateParameter;
use typed_trees::types::TypeReferenceHandle;

pub(super) use crate::value_custody::places::binding_observations::{
    BindingObservation, binding_is_exclusively_exposed, binding_is_written,
};

mod calls;

pub(super) fn is_slice(program: &dyn crate::CheckingInput, reference: TypeReferenceHandle) -> bool {
    crate::value_custody::type_shapes::has_slice_base(program, reference)
}

pub(super) fn parameter<'program>(
    program: &'program dyn crate::CheckingInput,
    state: &State,
    expression: ExpressionHandle,
) -> Option<&'program StateParameter> {
    // The bare authored spelling resolves through the fields owner; this
    // family adds only the slice carrier filter. Mutability is a storage
    // capability, not a value change: the edge owners supply a
    // parameter-preserving evaluated prefix, so a mutable slice still denotes
    // its arrival sequence -- and its arrival length -- at the edge.
    let parameter = super::fields::parameter(program, state, expression)?;
    is_slice(program, parameter.type_reference).then_some(parameter)
}

pub(super) fn bindings(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    entry_parameters: Option<&super::RankStateBindings>,
) -> Vec<(SymbolHandle, String)> {
    let mut bindings = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| {
            parameter.symbol.is_valid()
                && !parameter.is_const
                && is_slice(program, parameter.type_reference)
        })
        .map(|parameter| {
            (
                parameter.symbol,
                format!("\0ranking:length:{:?}", parameter.symbol),
            )
        })
        .collect::<Vec<_>>();
    let Some(entries) = entry_parameters else {
        return bindings;
    };
    let Some(root) = program.machine_states(machine).first() else {
        return bindings;
    };
    for selected in &entries.coordinates {
        if !selected.entry.path.is_empty() {
            continue;
        }
        let Some(entry) = program.state_parameter_by_symbol(root, selected.entry.parameter) else {
            continue;
        };
        if !is_slice(program, entry.type_reference) {
            continue;
        }
        let identity = if selected.destination.path.is_empty() {
            bindings
                .iter()
                .find(|(symbol, _)| *symbol == selected.destination.parameter)
                .map(|(_, identity)| identity.clone())
        } else {
            SliceCoordinate::at_coordinate(
                program,
                state,
                &selected.destination,
                CoordinateKind::Length,
            )
            .map(|coordinate| coordinate.identity)
        };
        let Some(identity) = identity else {
            continue;
        };
        if let Some((_, existing)) = bindings.iter().find(|(symbol, _)| *symbol == entry.symbol) {
            if *existing != identity {
                return Vec::new();
            }
        } else {
            bindings.push((entry.symbol, identity));
        }
    }
    bindings
}

pub(super) fn install(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    root: &State,
    bindings: &[(SymbolHandle, String)],
    engine: &mut Engine<'_>,
    expressions: &[ExpressionHandle],
) -> Option<()> {
    // Fixed extents belong to declared storage, including projected fields.
    // They need no dynamic slice binding, so an empty binding roster cannot
    // skip metadata installation. Authenticate `.len` before substituting;
    // an ordinary field with that spelling is still a stored value.
    projections::for_each_projection_expression(
        program,
        expressions,
        |member, expression, visit| {
            for scope in [state, root] {
                let Some(receiver) = crate::value_custody::places::collection_length_receiver(
                    program,
                    machine,
                    Some(scope),
                    expression,
                ) else {
                    continue;
                };
                let value = crate::value_custody::places::declared_collection_extent(
                    program,
                    machine,
                    Some(scope),
                    receiver,
                )
                .map(|extent| Polynomial::constant(BigInt::from_u64(extent as u64)))
                .or_else(|| {
                    let parameter = parameter(program, scope, receiver)?;
                    let identity = crate::substitutions::actual_for(bindings, parameter.symbol)?;
                    Some(Polynomial::atom(identity))
                });
                if let Some(value) = value
                    && !engine.bind_strict_projection(expression, value)
                {
                    return None;
                }
            }
            visit(member.receiver);
            Some(())
        },
    )
}

pub(super) fn actual(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    bindings: &[(SymbolHandle, String)],
    engine: &mut Engine<'_>,
) -> Option<Polynomial> {
    evaluate_length(program, machine, state, expression, bindings, engine, 0)
}

fn evaluate_length(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    bindings: &[(SymbolHandle, String)],
    engine: &mut Engine<'_>,
    depth: usize,
) -> Option<Polynomial> {
    let node = program
        .expression_table
        .expression_at_depth(expression, depth)?;
    if let ExpressionNode::Borrow(borrow) = node {
        if !matches!(
            borrow.access,
            language_core::ReferenceAccess::Shared | language_core::ReferenceAccess::Mutable
        ) {
            return None;
        }
        return evaluate_length(
            program,
            machine,
            state,
            borrow.target,
            bindings,
            engine,
            depth + 1,
        );
    }
    let coordinate = |expression| {
        if let Some(parameter) = parameter(program, state, expression) {
            let identity = crate::substitutions::actual_for(bindings, parameter.symbol)?;
            return Some(Polynomial::atom(identity));
        }
        // A member chain ending at a slice leaf names the same canonical
        // coordinate the projected-storage owner installs for `.len`
        // spellings; its atom is fabricated here only as a substitution
        // result, so a chain the state never established stays an unknown
        // value and fails the membership proofs it would have needed.
        SliceCoordinate::resolve_length(program, state, expression)
            .map(|coordinate| coordinate.value())
    };
    if let Some(length) = coordinate(expression) {
        return Some(length);
    }
    let contracted_extent =
        crate::value_custody::places::range_operator_establishes_endpoint_extent(
            program,
            machine,
            Some(state),
            expression,
        );
    if crate::value_custody::places::has_builtin_subslice_meaning(
        program,
        machine,
        Some(state),
        expression,
    ) || contracted_extent
    {
        let Some(indexed) = program.expression_table.indexed_node(expression) else {
            return None;
        };
        let range = program.expression_table.range_node(indexed.index)?;
        // Constant-extent queries also enter here without the call owner's
        // meaning pass. Never interpret an authored operator as arithmetic
        // merely because its operands happen to normalize to constants.
        // An operator that ensures `result.len == end - start` is that
        // arithmetic: the endpoints are the extent, and the operator's
        // `requires` own ordering and inclusion.
        if !contracted_extent {
            for endpoint in [range.start, range.end] {
                if endpoint.is_valid() {
                    super::meanings::admitted_carrier(
                        program,
                        machine,
                        state,
                        endpoint,
                        depth + 1,
                    )?;
                }
            }
        }
        let start = if range.start.is_valid() {
            engine.normalize(range.start)?
        } else if contracted_extent {
            return None;
        } else {
            Polynomial::default()
        };
        if contracted_extent && range.end.is_valid() && !range.end_inclusive {
            let end = engine.normalize(range.end)?;
            return Some(end.sub(&start));
        }
        let source = evaluate_length(
            program,
            machine,
            state,
            indexed.collection,
            bindings,
            engine,
            depth + 1,
        )?;
        let mut end = if range.end.is_valid() {
            engine.normalize(range.end)?
        } else {
            source.clone()
        };
        let authored_end = end.clone();
        if range.end_inclusive {
            if !range.end.is_valid() {
                return None;
            }
            end = end.add(&Polynomial::constant(BigInt::from_u64(1)));
        }
        let prove = |difference: Polynomial| {
            engine.requires_unsatisfiable
                || engine.prove_at_least(&engine.substituted(&difference), &BigInt::zero())
        };
        // Normalizing -1 to zero must not make an invalid inclusive index a
        // valid empty window. Check the authored index before its successor.
        return (prove(start.clone())
            && (!range.end_inclusive || prove(authored_end))
            && prove(end.sub(&start))
            && prove(source.sub(&end)))
        .then(|| end.sub(&start));
    }
    // Array extent belongs to the declared place, not to its current contents.
    // Replacing elements or the whole array cannot resize it.
    if let Some(length) = crate::value_custody::places::declared_collection_extent(
        program,
        machine,
        Some(state),
        expression,
    ) {
        return Some(Polynomial::constant(BigInt::from_u64(length as u64)));
    }
    let length = match program.expression_table.expression(expression) {
        ExpressionNode::Name(_) => {
            let Some(symbol) =
                crate::expression_traversal::single_segment_name_symbol(program, expression)
            else {
                return None;
            };
            return saved_length(program, machine, state, symbol, engine, depth);
        }
        // Quoted bytes form an immutable image-backed view. Its live extent
        // is the byte count, just as an array literal's extent is its element
        // count; this supplies geometry only, never an encoding predicate.
        ExpressionNode::String(bytes) => u64::try_from(bytes.len()).ok(),
        ExpressionNode::ArrayLiteral(values) => u64::try_from(values.len()).ok(),
        ExpressionNode::Call(_) => calls::normal_return_length(program, machine, state, expression),
        _ => None,
    }?;
    Some(Polynomial::constant(BigInt::from_u64(length)))
}

/// The stable descriptor captured by one local declaration. Both expression
/// and canonical-place consumers use this gate; writes and exclusive exposure
/// prevent replaying an obsolete initializer as the current descriptor.
pub(super) fn saved_length(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    symbol: SymbolHandle,
    engine: &Engine<'_>,
    depth: usize,
) -> Option<Polynomial> {
    let (_, local) = program.state_local_declaration(state, symbol)?;
    if local.is_mutable
        || !local.initial_value.is_valid()
        || binding_is_exclusively_exposed(program, state, symbol)
        || binding_is_written(program, state, symbol, BindingObservation::Descriptor)
    {
        return None;
    }
    let mut captured = Engine::strict(program, machine, &engine.closed_symbol_bindings()?);
    evaluate_length(
        program,
        machine,
        state,
        local.initial_value,
        &[],
        &mut captured,
        depth + 1,
    )
}

/// A constant produced extent, independent of current caller facts. Reuse
/// ordinary length composition with an empty arithmetic environment so saved
/// initializers never borrow facts established after their evaluation.
pub(super) fn produced_length(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<u64> {
    let mut engine = Engine::strict(program, machine, &[]);
    evaluate_length(program, machine, state, expression, &[], &mut engine, depth)?
        .constant_value()?
        .to_u64()
}

/// A produced length coordinate over projected storage: the slice leaf of an
/// exact member chain rooted at a state formal, `parameter.steps...field`.
/// The atom names the collection's produced length, never its contents. Like
/// the field coordinate of an integer leaf, the chain is resolved against the
/// formal's own declaration and re-resolved onto the arrival's carrier through
/// the discovered telescope — a telescope names a role, so duplicated record
/// roles resolve to no coordinate rather than a guessed copy.
/// The produced-length coordinate: a [`coordinate::Coordinate`] whose step
/// owners are exact named records and whose leaf is a slice field.
pub(super) type SliceCoordinate<'program> = Coordinate<'program>;

impl<'program> Coordinate<'program> {
    /// The coordinate an authored member chain names: every step an exact
    /// declared field, by resolved symbol and spelling, of the record before
    /// it, and the leaf a slice. Bare formals keep the `bindings` coordinate;
    /// a slice leaf reached through a reference boundary inside the chain is
    /// not a projected coordinate (the field owner has the same rule).
    pub(super) fn resolve_length(
        program: &'program dyn crate::CheckingInput,
        state: &State,
        expression: ExpressionHandle,
    ) -> Option<Self> {
        Self::resolve_member(program, state, expression, CoordinateKind::Length)
    }

    /// The length this coordinate holds after `expression` arrives in its
    /// formal: the formal itself forwarded, a record carrier behind the
    /// destination's `&` or a member-target `p.f` whose projection prefix
    /// lands on this coordinate's root, or the literal chain rebuilt
    /// declaration by declaration down to the slice leaf, whose own length
    /// `actual` then computes. `arrival_borrowed` is the destination formal's
    /// own reference boundary, so a borrowed arrival unwraps one `&` no matter
    /// how the source slot stored its record. A literal of another
    /// declaration, a foreign record, or a missing step is not this
    /// coordinate.
    pub(super) fn arrived_length(
        &self,
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        engine: &mut Engine<'_>,
        expression: ExpressionHandle,
        arrival_borrowed: bool,
        bindings: &[(SymbolHandle, String)],
    ) -> Option<Polynomial> {
        let mut current = expression;
        if arrival_borrowed {
            // An owned actual into a borrowed formal has no reference boundary
            // to read through; only a `&` spelling reaches the record the
            // destination denotes.
            let Some(borrow) = program.expression_table.borrow_expression(current) else {
                return None;
            };
            current = borrow.target;
        }
        if let Some((carrier, prefix)) = rooted_carrier(program, state, current) {
            return Some(self.through_carrier(program, carrier, &prefix)?.value());
        }
        // An unchanged saved record can capture a descriptor whose extent is
        // fixed by storage type even when its referent's contents can change.
        // Recover only closed metadata, with no later caller hypotheses or
        // scalar initializer observations. Binding replacement/exposure still
        // makes the saved descriptor unavailable.
        let mut saved_record = false;
        for depth in 0..128 {
            let Some(symbol) = super::saved_arguments::actual(program, state, current) else {
                break;
            };
            let (_, local) = program.state_local_declaration(state, symbol)?;
            if !local.initial_value.is_valid() || depth == 127 {
                return None;
            }
            current = local.initial_value;
            saved_record = true;
        }
        let mut owner = self.root;
        let mut leaf = current;
        for (depth, field) in self.steps.iter().chain([&self.field]).enumerate() {
            if depth > 0
                && !saved_record
                && let Some((carrier, prefix)) = rooted_carrier(program, state, leaf)
            {
                // `leaf` names the record this step reads through a carrier
                // instead of a literal: a formal forward or a member
                // projection inside the rebuilt record. Its prefix must land
                // on the record the chain expects at this step; the remaining
                // chain then resolves under the carrier's own declaration, so
                // a forward of this coordinate's own prefix keeps this
                // coordinate itself.
                return self
                    .through_carrier_at(program, carrier, &prefix, owner, depth)
                    .map(|coordinate| coordinate.value());
            }
            leaf = unique_literal_field(program, leaf, owner, field.symbol, &field.name)?;
            if let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(
                    program
                        .type_reference_table
                        .unconstrained_type_reference(field.type_reference),
                )
            {
                owner = *symbol;
            }
        }
        if saved_record {
            let mut captured = Engine::strict(program, machine, &[]);
            let value = actual(program, machine, state, leaf, &[], &mut captured)?;
            value.constant_value().map(Polynomial::constant)
        } else {
            actual(program, machine, state, leaf, bindings, engine)
        }
    }
}
/// The projected-length coordinate family: coordinates resolved from the
/// receiver an authored `x.chain.len` spelling reads.
pub(super) struct LengthFamily;

pub(super) type SliceCoordinates<'program> = CoordinateSet<'program, LengthFamily>;

impl<'program> CoordinateFamily<'program> for LengthFamily {
    fn resolve(
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        scope: &State,
        expression: ExpressionHandle,
    ) -> Option<Coordinate<'program>> {
        let receiver = crate::value_custody::places::collection_length_receiver(
            program,
            machine,
            Some(scope),
            expression,
        )?;
        Coordinate::resolve_length(program, scope, receiver)
    }

    /// An authored `.len` spelling asks for the length it computes, so every
    /// installed projected length is demanded; the receiver resolution above
    /// already declined every reading that names no projected slice.
    fn demanded(_demanded: &[RankSubjectCoordinate], _entry: &RankSubjectCoordinate) -> bool {
        true
    }

    fn arrived_actual(
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        destination_state: &State,
        destination: &RankSubjectCoordinate,
        engine: &mut Engine<'_>,
        argument: ExpressionHandle,
        bindings: &[(SymbolHandle, String)],
    ) -> Option<Polynomial> {
        super::arrival_length_actual(
            program,
            machine,
            state,
            destination_state,
            destination,
            engine,
            argument,
            bindings,
        )
    }

    /// A produced length is a natural coordinate; every installed coordinate
    /// contributes its `>= 0` fact exactly as the bare-formal bindings do.
    fn comparisons(
        _program: &dyn crate::CheckingInput,
        coordinate: &Coordinate<'program>,
    ) -> Vec<Comparison> {
        vec![(
            BinaryOperator::GreaterOrEqual,
            coordinate.value(),
            Polynomial::default(),
        )]
    }
}
