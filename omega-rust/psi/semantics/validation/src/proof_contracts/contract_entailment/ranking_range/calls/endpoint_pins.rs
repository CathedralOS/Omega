//! Conserved endpoint inputs through members that author no range of their own.
//! Candidate slots name equal transported values, never inferred ranking views.
//!
//! An endpoint reads either an exact integer formal or the leaf coordinate a
//! member projection names in the origin member's entry scope (`limits.cap`).
//! The projected input is conserved through the same equality discipline as a
//! scalar input: each edge must prove the actual installs the source carrier's
//! own coordinate value, so a record that merely shares a field spelling --
//! or a rebuilt literal with a different leaf -- cannot pin the endpoint.
use super::super::super::SymbolHandle;
use super::super::{RankingRangeState, StrictArithmeticBindingValue, comparison_proven};
use super::{
    BinaryOperator, Engine, ExpressionHandle, ExpressionNode, Machine, Polynomial,
    RankingRangeCallMember, State, TypedTrees, collect_guard, entry_comparisons,
    exact_integer_parameter, field_coordinates, fields, integer_bindings, lengths, meanings,
    parameter_comparisons, projections, scalar_entry, telescoped_bindings, validate_mapping,
};
pub(crate) struct RankingRangeCallEdge<'program> {
    pub source: usize,
    pub destination: usize,
    /// The exact transition site inside the caller: its state and discovered
    /// telescope. Conservation reads caller entry values through that site's
    /// carriers, so an endpoint pinned at a subordinate arrival still names
    /// the same transported input.
    pub site_state: &'program State,
    pub entry_parameters: &'program [SymbolHandle],
    pub arguments: &'program [ExpressionHandle],
    pub guards: Vec<(ExpressionHandle, bool)>,
}

/// One input an authored endpoint reads in the origin member's entry scope.
#[derive(PartialEq)]
enum EndpointInput {
    /// An exact integer formal the endpoint spells directly.
    Scalar(SymbolHandle),
    /// A member projection resolved to its exact field coordinate, indexing
    /// the projected dependencies shared with every edge's equality table.
    Projection(usize),
    /// A `.len` leaf resolved to its produced length coordinate -- a
    /// non-polynomial input indexing the length dependencies shared with
    /// every edge's equality table.
    Length(usize),
}

/// The produced-length coordinate one `.len` endpoint leaf reads: either the
/// length of a bare slice formal's collection or the projected slice
/// coordinate a member chain names. Conservation transports that produced
/// value, never the collection's contents or a same-spelled accessor.
enum LengthDependency<'program> {
    /// `slice.len` on a bare slice formal.
    Bare(SymbolHandle),
    /// `root.steps…leaf.len` resolved to its exact projected coordinate.
    Projected(lengths::SliceCoordinate<'program>),
}

impl<'program> LengthDependency<'program> {
    /// The origin formal the authored receiver names.
    fn root_symbol(&self) -> SymbolHandle {
        match self {
            LengthDependency::Bare(symbol) => *symbol,
            LengthDependency::Projected(coordinate) => coordinate.parameter.symbol,
        }
    }

    /// Whether `formal`'s slot can carry this produced length at all: a slice
    /// formal holds its own length; a record formal holds the length of the
    /// unique slice leaf its declaration admits. Any other slot, or a record
    /// with two candidate leaves, cannot name this coordinate.
    fn carries(
        &self,
        program: &'program TypedTrees,
        formal: &typed_trees::signature::StateParameter,
    ) -> bool {
        match self {
            LengthDependency::Bare(_) => {
                lengths::is_slice(program, formal.type_reference)
                    || lengths::slice_leaf_coordinate(program, formal).is_some()
            }
            LengthDependency::Projected(coordinate) => {
                coordinate.for_carrier(program, formal).is_some()
            }
        }
    }

    /// The produced length `formal`'s entry role carries at `site`: a slice
    /// role reads the length atom the site bindings alias onto its carrier;
    /// a record role or projected chain re-resolves the coordinate onto the
    /// formal the telescope says holds the role.
    fn site_value(
        &self,
        program: &'program TypedTrees,
        site: RankingRangeState<'_>,
        length_bindings: &[(SymbolHandle, String)],
        formal: &typed_trees::signature::StateParameter,
    ) -> Option<Polynomial> {
        match self {
            LengthDependency::Bare(_) => {
                if lengths::is_slice(program, formal.type_reference) {
                    let (_, identity) = length_bindings
                        .iter()
                        .find(|(symbol, _)| *symbol == formal.symbol)?;
                    Some(Polynomial::atom(identity.clone()))
                } else {
                    lengths::slice_leaf_coordinate(program, formal)?
                        .at_arrival(program, site, formal.symbol)
                        .map(|coordinate| coordinate.value())
                }
            }
            LengthDependency::Projected(coordinate) => coordinate
                .at_arrival(program, site, formal.symbol)
                .map(|coordinate| coordinate.value()),
        }
    }

    /// The produced length `actual` installs at the destination's `formal`
    /// slot: a slice slot reads the actual's own produced length (subslice
    /// geometry included); a record slot walks the actual's forward, borrow,
    /// or literal rebuild down to the unique slice leaf this dependency
    /// names there.
    fn actual_value(
        &self,
        program: &'program TypedTrees,
        machine: &Machine,
        site: &State,
        engine: &mut Engine<'_>,
        formal: &typed_trees::signature::StateParameter,
        actual: ExpressionHandle,
        length_bindings: &[(SymbolHandle, String)],
    ) -> Option<Polynomial> {
        match self {
            LengthDependency::Bare(_) => {
                if lengths::is_slice(program, formal.type_reference) {
                    lengths::actual(program, machine, site, actual, length_bindings, engine)
                } else {
                    let coordinate = lengths::slice_leaf_coordinate(program, formal)?;
                    coordinate.arrived(
                        program,
                        machine,
                        site,
                        engine,
                        actual,
                        coordinate.borrowed,
                        length_bindings,
                    )
                }
            }
            LengthDependency::Projected(coordinate) => {
                let coordinate = coordinate.for_carrier(program, formal)?;
                coordinate.arrived(
                    program,
                    machine,
                    site,
                    engine,
                    actual,
                    coordinate.borrowed,
                    length_bindings,
                )
            }
        }
    }
}

/// Which source-carried values one destination slot's actual provably
/// installs. `sources` indexes the caller's entry formals for scalar inputs;
/// `projections[dependency][source]` records whether the actual's coordinate
/// value equals the dependency's site value carried by that source formal;
/// `lengths[dependency][source]` does the same for produced lengths.
struct SlotEvidence {
    sources: Vec<usize>,
    projections: Vec<Vec<bool>>,
    lengths: Vec<Vec<bool>>,
}

/// The caller supplies one strongly connected component and every intra-component
/// occurrence, including parallel calls. Entry/range meaning and each edge's
/// arithmetic obligations remain separate judgments.
pub(crate) fn mixed_call_endpoints_are_pinned(
    program: &TypedTrees,
    members: &[RankingRangeCallMember<'_>],
    edges: &[RankingRangeCallEdge<'_>],
) -> bool {
    let prove = || -> Option<()> {
        let entries = members
            .iter()
            .map(|member| scalar_entry(program, member).map(|(state, _)| state))
            .collect::<Option<Vec<_>>>()?;
        let parameters = entries
            .iter()
            .map(|state| {
                program
                    .state_parameters(state)
                    .iter()
                    .filter(|parameter| !parameter.is_self)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        // Discover every ranged member's endpoint inputs before building any
        // edge table: a projected input adds a coordinate the actual must
        // transport, so the dependencies decide which equalities each call
        // owes.
        let mut member_dependencies = Vec::new();
        let mut projections: Vec<fields::FieldCoordinate<'_>> = Vec::new();
        let mut length_dependencies: Vec<LengthDependency<'_>> = Vec::new();
        for (origin, member) in members.iter().enumerate() {
            if !member.range.is_valid() {
                continue;
            }
            let ExpressionNode::Range(range) = program.expression_table.expression(member.range)
            else {
                return None;
            };
            let mut inputs = Vec::new();
            for endpoint in [range.start, range.end] {
                endpoint_inputs(
                    program,
                    member.machine,
                    entries[origin],
                    endpoint,
                    &mut inputs,
                    &mut projections,
                    &mut length_dependencies,
                    0,
                )?;
            }
            member_dependencies.push((origin, inputs));
        }
        // Equality is independent of the shrinking endpoint candidates. Prepare
        // each call's exact arithmetic once, not once per dependency or round.
        let equalities = edges
            .iter()
            .map(|edge| {
                argument_sources(program, members, edge, &projections, &length_dependencies)
            })
            .collect::<Option<Vec<_>>>()?;
        for (origin, inputs) in member_dependencies {
            for input in inputs {
                let mut candidates = match input {
                    EndpointInput::Scalar(symbol) => {
                        let formal = parameters[origin]
                            .iter()
                            .find(|formal| formal.symbol == symbol)?;
                        let carrier = exact_integer_parameter(program, formal.type_reference)?;
                        parameters
                            .iter()
                            .enumerate()
                            .map(|(position, formals)| {
                                formals
                                    .iter()
                                    .map(|formal| {
                                        formal.symbol.is_valid()
                                            && !formal.is_const
                                            && exact_integer_parameter(
                                                program,
                                                formal.type_reference,
                                            ) == Some(carrier)
                                            && (position != origin || formal.symbol == symbol)
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .collect::<Vec<_>>()
                    }
                    EndpointInput::Projection(index) => {
                        let dependency = &projections[index];
                        parameters
                            .iter()
                            .enumerate()
                            .map(|(position, formals)| {
                                formals
                                    .iter()
                                    .map(|formal| {
                                        formal.symbol.is_valid()
                                            && !formal.is_const
                                            && dependency.for_carrier(program, formal).is_some()
                                            && (position != origin
                                                || formal.symbol == dependency.parameter.symbol)
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .collect::<Vec<_>>()
                    }
                    EndpointInput::Length(index) => {
                        // A slice formal carries its own produced length; a
                        // record formal carries the length of the unique
                        // slice leaf its declaration admits. Both stay exact:
                        // a slot with no such coordinate cannot pin the leaf.
                        let dependency = &length_dependencies[index];
                        parameters
                            .iter()
                            .enumerate()
                            .map(|(position, formals)| {
                                formals
                                    .iter()
                                    .map(|formal| {
                                        formal.symbol.is_valid()
                                            && !formal.is_const
                                            && dependency.carries(program, formal)
                                            && (position != origin
                                                || formal.symbol == dependency.root_symbol())
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .collect::<Vec<_>>()
                    }
                };
                // The origin starts with its authored input only. Since the
                // component is strongly connected, every surviving candidate
                // is reached from that singleton. Intersecting every incoming
                // occurrence prevents an unrooted cycle or parallel alternative
                // from manufacturing a second possible endpoint value.
                loop {
                    let mut changed = false;
                    for (edge, equalities) in edges.iter().zip(&equalities) {
                        for (ordinal, evidence) in equalities.iter().enumerate() {
                            if !candidates[edge.destination][ordinal] {
                                continue;
                            }
                            let preserved = match input {
                                EndpointInput::Scalar(_) => evidence
                                    .sources
                                    .iter()
                                    .any(|source| candidates[edge.source][*source]),
                                EndpointInput::Projection(index) => {
                                    evidence.projections[index].iter().enumerate().any(
                                        |(source, holds)| *holds && candidates[edge.source][source],
                                    )
                                }
                                EndpointInput::Length(index) => {
                                    evidence.lengths[index].iter().enumerate().any(
                                        |(source, holds)| *holds && candidates[edge.source][source],
                                    )
                                }
                            };
                            if !preserved {
                                candidates[edge.destination][ordinal] = false;
                                changed = true;
                            }
                        }
                    }
                    if candidates
                        .iter()
                        .any(|candidates| !candidates.iter().any(|candidate| *candidate))
                    {
                        return None;
                    }
                    if !changed {
                        break;
                    }
                }
            }
        }
        Some(())
    };
    prove().is_some()
}

/// Each destination slot retains every caller input proved equal to its actual.
/// No destination requirement or candidate correspondence is an arithmetic
/// premise. The component owner independently preserves the caller's prefix.
/// At a subordinate site the caller's entry values normalize through that
/// state's telescope: each entry role names the atom of the formal carrying it,
/// and kept duplicated carriers share one value under their bare-forward
/// equality. Requires clauses stay entry-site evidence; the site's own
/// constrained-type facts hold on every arrival.
///
/// A projected endpoint input adds a second evidence row per slot: the
/// destination formal's own resolution of the authored chain, walked over the
/// actual (`FieldCoordinate::arrived`), must equal the site coordinate of a
/// source formal carrying that input -- a forwarded record names its carrier's
/// coordinate, a member actual or literal rebuild reads the leaf it installs.
/// Member spellings inside the caller's own requires facts, guards, and
/// actuals bind to those same coordinates first, so `bounds.cap` in a guard
/// names the atom the endpoint reads.
fn argument_sources<'program>(
    program: &'program TypedTrees,
    members: &[RankingRangeCallMember<'program>],
    edge: &RankingRangeCallEdge<'program>,
    projections: &[fields::FieldCoordinate<'program>],
    length_dependencies: &[LengthDependency<'program>],
) -> Option<Vec<SlotEvidence>> {
    let caller = members.get(edge.source)?;
    let callee = members.get(edge.destination)?;
    let (entry, _) = scalar_entry(program, caller)?;
    let (destination, _) = scalar_entry(program, callee)?;
    let site = edge.site_state;
    let parameters = program
        .state_parameters(destination)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    if parameters.len() != edge.arguments.len() {
        return None;
    }
    let at_entry = site.symbol == entry.symbol;
    let (bindings, carrier_equalities) = if at_entry {
        (integer_bindings(program, site)?, Vec::new())
    } else {
        validate_mapping(program, caller.machine, site, edge.entry_parameters)?;
        telescoped_bindings(program, caller.machine, site, edge.entry_parameters)?
    };
    let mut engine = Engine::strict_with_symbol_bindings(program, caller.machine, &bindings);
    if !engine.strict_symbol_bindings_are_valid() {
        return None;
    }
    // Authored member projections in the caller's read surface bind to the
    // coordinate their root formal's role arrives on: an endpoint field in a
    // requires fact or guard, or a carrier projection inside a rebuilt actual.
    // This is the same installation the call-site judgment performs, so both
    // sides of an equality name one atom.
    let mut coordinates = field_coordinates::FieldCoordinates::empty();
    let mut expressions = Vec::new();
    expressions.extend(edge.arguments.iter().copied());
    expressions.extend(edge.guards.iter().map(|(guard, _)| *guard));
    expressions.extend(projections::entry_expressions(
        program,
        caller.machine,
        site,
    ));
    coordinates.install(
        program,
        site,
        entry,
        (!at_entry).then_some(edge.entry_parameters),
        &mut engine,
        &expressions,
    )?;
    // Produced lengths are non-polynomial inputs under the same discipline:
    // authored `.len` leaves inside guards, actuals, or requires facts bind
    // to the atom the site holds for the collection -- bare slice formals
    // through the length bindings, member chains through their projected
    // slice coordinate -- so both sides of an equality name one coordinate.
    let length_bindings = lengths::bindings(
        program,
        caller.machine,
        site,
        (!at_entry).then_some(edge.entry_parameters),
    );
    if !length_bindings.is_empty() {
        lengths::install(
            program,
            caller.machine,
            site,
            entry,
            &length_bindings,
            &mut engine,
            &expressions,
        )?;
    }
    let mut slice_coordinates = lengths::SliceCoordinates::empty();
    slice_coordinates.install(
        program,
        caller.machine,
        site,
        entry,
        (!at_entry).then_some(edge.entry_parameters),
        &mut engine,
        &expressions,
    )?;
    let mut comparisons = if at_entry {
        entry_comparisons(program, caller.machine, site, &mut engine, &bindings)?
    } else {
        parameter_comparisons(program, caller.machine, site, &mut engine, &bindings)?
    };
    comparisons.extend(carrier_equalities);
    comparisons.extend(length_bindings.iter().map(|(_, identity)| {
        (
            BinaryOperator::GreaterOrEqual,
            Polynomial::atom(identity.clone()),
            Polynomial::default(),
        )
    }));
    comparisons.extend(slice_coordinates.comparisons());
    for &(guard, holds) in &edge.guards {
        meanings::builtin(program, caller.machine, site, guard, 0)?;
        collect_guard(&mut engine, guard, holds, &mut comparisons, 0)?;
    }
    if !engine.install_hypotheses(comparisons) {
        return None;
    }
    let source_formals = program
        .state_parameters(entry)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    let source_values = source_formals
        .iter()
        .map(|parameter| {
            let binding = bindings
                .iter()
                .find(|binding| binding.symbol == parameter.symbol)?;
            Some(match &binding.value {
                StrictArithmeticBindingValue::Atom { identity, .. } => {
                    Polynomial::atom(identity.clone())
                }
                StrictArithmeticBindingValue::Integer(value) => Polynomial::constant(value.clone()),
            })
        })
        .collect::<Vec<_>>();
    // The value a projected dependency holds through each source formal at
    // this site: the authored chain re-resolved onto the formal carrying that
    // entry role, direct or at its unique nested path. A formal whose role is
    // unclaimed or contested at this site carries no dep value.
    let source_projections = projections
        .iter()
        .map(|dependency| {
            source_formals
                .iter()
                .map(|formal| {
                    dependency
                        .at_arrival(
                            program,
                            RankingRangeState {
                                state: site,
                                entry_parameters: edge.entry_parameters,
                            },
                            formal.symbol,
                        )
                        .map(|coordinate| coordinate.value())
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    // The produced length each source formal's role carries at this site, per
    // `.len` dependency: a slice role reads its carrier's length atom, a
    // record role its unique slice leaf's coordinate, a member chain its
    // re-resolved projection. An unclaimed or contested role carries no
    // length value for that dependency.
    let source_lengths = length_dependencies
        .iter()
        .map(|dependency| {
            source_formals
                .iter()
                .map(|formal| {
                    dependency.site_value(
                        program,
                        RankingRangeState {
                            state: site,
                            entry_parameters: edge.entry_parameters,
                        },
                        &length_bindings,
                        formal,
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    parameters
        .iter()
        .zip(edge.arguments)
        .map(|(parameter, argument)| {
            meanings::builtin(program, caller.machine, site, *argument, 0)?;
            // Normalize before vacuity: even an impossible arm cannot manufacture
            // custody for a foreign symbol or an unsupported actual expression.
            let sources = if exact_integer_parameter(program, parameter.type_reference).is_some() {
                let actual = engine.normalize(*argument)?;
                source_values
                    .iter()
                    .enumerate()
                    .filter_map(|(position, value)| {
                        let value = value.as_ref()?;
                        (engine.requires_unsatisfiable
                            || comparison_proven(&engine, BinaryOperator::Equal, &actual, value))
                        .then_some(position)
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let projections = projections
                .iter()
                .zip(&source_projections)
                .map(|(dependency, values)| {
                    let Some(coordinate) = dependency.for_carrier(program, parameter) else {
                        return vec![false; values.len()];
                    };
                    let carried = coordinate.arrived(
                        program,
                        site,
                        &mut engine,
                        *argument,
                        coordinate.borrowed,
                    );
                    values
                        .iter()
                        .map(|value| {
                            engine.requires_unsatisfiable
                                || matches!((&carried, value),
                                (Some(carried), Some(value))
                                    if comparison_proven(
                                        &engine,
                                        BinaryOperator::Equal,
                                        carried,
                                        value,
                                    ))
                        })
                        .collect()
                })
                .collect();
            let lengths = length_dependencies
                .iter()
                .zip(&source_lengths)
                .map(|(dependency, values)| {
                    let carried = dependency.actual_value(
                        program,
                        caller.machine,
                        site,
                        &mut engine,
                        parameter,
                        *argument,
                        &length_bindings,
                    );
                    values
                        .iter()
                        .map(|value| {
                            engine.requires_unsatisfiable
                                || matches!((&carried, value),
                                (Some(carried), Some(value))
                                    if comparison_proven(
                                        &engine,
                                        BinaryOperator::Equal,
                                        carried,
                                        value,
                                    ))
                        })
                        .collect()
                })
                .collect();
            Some(SlotEvidence {
                sources,
                projections,
                lengths,
            })
        })
        .collect()
}

fn direct_input(
    program: &TypedTrees,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<SymbolHandle> {
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => direct_input(program, atomic.value, depth + 1),
        ExpressionNode::Name(path)
            if path.symbol.is_valid()
                && path.head_symbol == path.symbol
                && program
                    .expression_table
                    .name_path_members(path.members)
                    .len()
                    == 1 =>
        {
            Some(path.symbol)
        }
        _ => None,
    }
}

/// The exact inputs an authored endpoint expression reads in `state`'s scope:
/// scalar formals for bare names, field coordinates for member projections,
/// produced length coordinates for `.len` leaves. Arithmetic combines inputs
/// without adding new ones; any other spelling -- a call, an index, a `.len`
/// on a computed or fixed-array receiver -- has no named input and keeps the
/// endpoint undiscovered rather than guessing a carrier.
fn endpoint_inputs<'program>(
    program: &'program TypedTrees,
    machine: &Machine,
    state: &'program State,
    expression: ExpressionHandle,
    inputs: &mut Vec<EndpointInput>,
    projections: &mut Vec<fields::FieldCoordinate<'program>>,
    length_dependencies: &mut Vec<LengthDependency<'program>>,
    depth: usize,
) -> Option<()> {
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(_) => Some(()),
        ExpressionNode::Atomic(atomic) => endpoint_inputs(
            program,
            machine,
            state,
            atomic.value,
            inputs,
            projections,
            length_dependencies,
            depth + 1,
        ),
        ExpressionNode::Name(_) => {
            let symbol = direct_input(program, expression, 0)?;
            let parameter = program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.symbol == symbol && !parameter.is_self)?;
            exact_integer_parameter(program, parameter.type_reference)?;
            let input = EndpointInput::Scalar(symbol);
            if !inputs.contains(&input) {
                inputs.push(input);
            }
            Some(())
        }
        ExpressionNode::Member(member) => {
            // A builtin `.len` reads the collection's produced length: a
            // non-polynomial input conserved like a projection. A `.len`
            // whose receiver resolves no slice formal or projected slice
            // coordinate -- a computed value, a fixed array, a subslice --
            // has no named input and stays undiscovered.
            if member.case_variant.is_none()
                && let Some(receiver) = crate::value_custody::places::collection_length_receiver(
                    program,
                    machine,
                    Some(state),
                    expression,
                )
            {
                let dependency = if let Some(formal) = lengths::parameter(program, state, receiver)
                {
                    LengthDependency::Bare(formal.symbol)
                } else {
                    LengthDependency::Projected(lengths::SliceCoordinate::resolve(
                        program, state, receiver,
                    )?)
                };
                let index = match length_dependencies.iter().position(|existing| {
                    match (existing, &dependency) {
                        (LengthDependency::Bare(left), LengthDependency::Bare(right)) => {
                            *left == *right
                        }
                        (LengthDependency::Projected(left), LengthDependency::Projected(right)) => {
                            left.identity == right.identity
                        }
                        _ => false,
                    }
                }) {
                    Some(index) => index,
                    None => {
                        length_dependencies.push(dependency);
                        length_dependencies.len() - 1
                    }
                };
                let input = EndpointInput::Length(index);
                if !inputs.contains(&input) {
                    inputs.push(input);
                }
                return Some(());
            }
            // A member projection's input is the exact leaf coordinate it
            // reads: conservation transports that value, not the record
            // spelling.
            let coordinate =
                fields::FieldCoordinate::resolve_projection(program, state, expression)?;
            let index = match projections
                .iter()
                .position(|existing| existing.identity == coordinate.identity)
            {
                Some(index) => index,
                None => {
                    projections.push(coordinate);
                    projections.len() - 1
                }
            };
            let input = EndpointInput::Projection(index);
            if !inputs.contains(&input) {
                inputs.push(input);
            }
            Some(())
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply
            ) =>
        {
            endpoint_inputs(
                program,
                machine,
                state,
                binary.left,
                inputs,
                projections,
                length_dependencies,
                depth + 1,
            )?;
            endpoint_inputs(
                program,
                machine,
                state,
                binary.right,
                inputs,
                projections,
                length_dependencies,
                depth + 1,
            )
        }
        _ => None,
    }
}
