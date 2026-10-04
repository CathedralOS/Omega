//! Resolve where one authored assignment writes: the storage root it names,
//! the root's write authority, and the checked carrier path from that root.
//!
//! The destination is resolved without looking at the right-hand side. Each
//! store kind in `super` then reads the resolved place and decides whether
//! its own leaf and value compose with it.
use super::super::{
    CheckFacts, CheckedStructuralAccess, CheckedStructuralScalarParameterPlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment, DataMember,
    DataShapeKind, ExpressionNode, Multiplicity, StatementNode, SymbolHandle, TypeReferenceNode,
    TypedTrees,
};
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::control::statement_sequence::bounded_byte_values;
use crate::execution::terminal_unit::types::terminal_field_identity;

/// The storage roots one planner lane can write through.
///
/// An attached Unit body writes through its exclusive borrowed parameters.
/// The scalar-graph lane writes through its own established record locals;
/// that lane's lowering evaluates the value in its own scalar namespace, so
/// its value and leaf policies (`Root::Local`) are narrower.
#[derive(Clone, Copy)]
pub(super) enum StoreRoots<'a> {
    Parameters {
        structural: &'a [CheckedUnitStructuralParameterPlan],
        scalar: &'a [CheckedStructuralScalarParameterPlan],
    },
    RecordLocals,
}

/// The resolved root of one destination place.
pub(super) enum Root<'a> {
    Parameter {
        plan: &'a CheckedUnitStructuralParameterPlan,
        parameter: &'a typed_trees::signature::StateParameter,
    },
    Local {
        symbol: SymbolHandle,
    },
}

impl Root<'_> {
    pub(super) fn destination(&self) -> checked_trees::CheckedStoreRoot {
        match self {
            Self::Parameter { plan, .. } => checked_trees::CheckedStoreRoot::Parameter {
                position: plan.position,
            },
            Self::Local { symbol } => checked_trees::CheckedStoreRoot::Local { symbol: *symbol },
        }
    }

    pub(super) fn parameter_position(&self) -> Option<u32> {
        match self {
            Self::Parameter { plan, .. } => Some(plan.position),
            Self::Local { .. } => None,
        }
    }
}

/// One authored destination: a canonical place below a resolved root. A
/// trailing selector is split off as `dynamic_index` for the byte stores,
/// whose live-length operations take it as an operand; every other runtime
/// selector of the target is a `RuntimeIndex` carrier segment named by
/// `selectors`.
pub(super) struct Destination<'a> {
    pub(super) root: Root<'a>,
    pub(super) place: crate::flow::CanonicalPlace,
    pub(super) dynamic_index: Option<typed_trees::expression::ExpressionHandle>,
    /// The declared referee primitive of a recast loan local the target
    /// names (`let view: &mut u32 = &mut self.buf[k] as &mut u32`): the
    /// carrier the authored store writes at, distinct from the rejoined
    /// leaf's own primitive.
    pub(super) recast_view: Option<typed_trees::types::PrimitiveType>,
    /// The local root is an exclusive borrowed view established by the loan
    /// ledger (`let view: &mut T = ...`), not owned storage: its referent
    /// extents admit runtime element selectors whatever the referee's
    /// declared shape — slice, record, or fixed array — where an owned
    /// record local lowers a static carrier only.
    pub(super) view_loan: bool,
    /// The target's admitted element selectors, when every indexed step is
    /// an element of a declared fixed array.
    selectors: Option<super::selectors::TargetSelectors>,
    /// Declared type of the root's referent (the borrowed record, or the
    /// local's own type) and its record owner when the root is a record.
    pub(super) root_type: typed_trees::types::TypeReferenceHandle,
    root_owner: Option<&'a typed_trees::data::DataDefinition>,
    /// Operator selections, which decide whether a carrier's `where` facts
    /// are spelled in the Terminal requirement vocabulary.
    operators: &'a checked_trees::CheckedOperatorFacts,
}

/// The checked carrier path to a record, and that record's declaration.
pub(super) struct Carrier<'a> {
    pub(super) path: Vec<CheckedUnitStructuralPathSegment>,
    pub(super) owner: &'a typed_trees::data::DataDefinition,
}

pub(super) struct StorageProjection<'a> {
    pub(super) path: Vec<CheckedUnitStructuralPathSegment>,
    pub(super) type_reference: typed_trees::types::TypeReferenceHandle,
    /// A declared scalar field keeps its field-store range obligation. An
    /// array element or dereferenced primitive instead names primitive storage.
    pub(super) ends_at_field: bool,
    owner: Option<&'a typed_trees::data::DataDefinition>,
}

/// Resolve the authored target, its root and the root's write authority.
pub(super) fn resolve<'a>(
    program: &'a TypedTrees,
    facts: &'a CheckFacts,
    machine: &'a typed_trees::machine::Machine,
    state: &'a typed_trees::state::State,
    roots: StoreRoots<'a>,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    trace: &LocalConstructionTrace,
) -> Option<Destination<'a>> {
    trace.phase("structural field store: target place");
    let ordinal = usize::try_from(statement_index).ok()?;
    let (target, dynamic_index) = match program.expression_table.expression(assignment.target) {
        ExpressionNode::Indexed(indexed) => {
            if !validation::place_has_builtin_coordinates(
                program,
                machine,
                Some(state),
                assignment.target,
            ) || matches!(
                program.expression_table.expression(indexed.index),
                ExpressionNode::Range(_)
            ) {
                return None;
            }
            (indexed.collection, Some(indexed.index))
        }
        _ => (assignment.target, None),
    };
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        ordinal,
        target,
    )?;
    let authored_place = place.clone();
    let (mut place, recast_view) = rejoin_loan_local(program, machine, state, ordinal, place)?;
    if recast_view.is_none()
        && let facts::PlaceRoot::Symbol(authored) = authored_place.root
        && let Some(alias) = super::super::receiver_aliases::aliases(program, facts, machine, state)
            .unwrap_or_default()
            .into_iter()
            .find(|alias| alias.owner == authored)
    {
        // Erased aliases carry no pointer home. Their validated capture roster
        // supplies the storage path, including nested aliases and selectors.
        // Materialized reference fields never enter this substitution.
        let root = if alias.root == machine.symbol {
            program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.is_self)?
                .symbol
        } else {
            alias.root
        };
        place.root = facts::PlaceRoot::Symbol(root);
        place.segments = alias.segments;
        place.segments.extend(authored_place.segments);
    }
    let facts::PlaceRoot::Symbol(root_symbol) = place.root else {
        return None;
    };
    let selectors = super::selectors::TargetSelectors::resolve(
        program,
        facts,
        machine,
        state,
        statement_index,
        assignment.target,
    );
    trace.phase("structural field store: destination parameter");
    match roots {
        StoreRoots::Parameters { structural, .. } => {
            let source_parameters = program.state_parameters(state);
            // The assignment selects its destination; unrelated borrowed
            // inputs do not change that root's authority or require it to
            // occupy position zero.
            let mut destinations = structural.iter().filter_map(|plan| {
                let parameter = source_parameters.get(plan.position as usize)?;
                (parameter.symbol == root_symbol).then_some((plan, parameter))
            });
            let Some((plan, parameter)) = destinations.next() else {
                // An established owned structural local roots a field store
                // without any parameter loan: emission resolves its live home
                // place through the structural-local roster, so admission here
                // pins the earlier declaration, not permission to transfer
                // all its contents. A reference leaf retains its own custody.
                return borrowed_view_local_root(
                    program,
                    facts,
                    machine,
                    state,
                    root_symbol,
                    ordinal,
                    dynamic_index,
                    place.clone(),
                    selectors.clone(),
                )
                .or_else(|| {
                    owned_local_root(
                        program,
                        &facts.operators,
                        state,
                        root_symbol,
                        ordinal,
                        dynamic_index,
                        place,
                        selectors,
                    )
                });
            };
            if destinations.next().is_some() {
                return None;
            }
            let TypeReferenceNode::Reference { referee, .. } = program
                .type_reference_table
                .type_reference(parameter.type_reference)
            else {
                return None;
            };
            // A receiver's source referent is `Self`; its declaration identity
            // comes from the machine attachment, not a global lookup of that
            // spelling. A borrowed fixed-array root has no record owner of its
            // own: its element record resolves through the first carrier
            // `FixedIndex` hop, so an unresolved owner stays admissible only
            // behind an index segment.
            let root_owner = if plan.is_self {
                Some(
                    program
                        .data_definitions()
                        .iter()
                        .find(|data| data.symbol == machine.attached_data_symbol)?,
                )
            } else {
                crate::facts::field_domain::data_definition_for_field_type(program, *referee)
            };
            Some(Destination {
                root: Root::Parameter { plan, parameter },
                place,
                dynamic_index,
                recast_view,
                view_loan: false,
                selectors,
                root_type: *referee,
                root_owner,
                operators: &facts.operators,
            })
        }
        StoreRoots::RecordLocals => {
            // A local root is a unique owned record established earlier in
            // this state, with the scalar-graph lane's record shape and a
            // checked disposition.
            let (local_ordinal, local) = program.state_local_declaration(state, root_symbol)?;
            if local_ordinal >= ordinal || dynamic_index.is_some() {
                return None;
            }
            super::super::scalar_graph_record_shapes(program, local.type_reference)?;
            validation::record_local_disposition(
                program,
                facts,
                machine.symbol,
                state.symbol,
                u32::try_from(local_ordinal).ok()?,
            )?;
            Some(Destination {
                root: Root::Local {
                    symbol: root_symbol,
                },
                place,
                dynamic_index,
                recast_view: None,
                view_loan: false,
                selectors,
                root_type: local.type_reference,
                root_owner: crate::facts::field_domain::data_definition_for_field_type(
                    program,
                    local.type_reference,
                ),
                operators: &facts.operators,
            })
        }
    }
}

/// An owned local store root under the parameter lane: a unique `LocalData`
/// declaration earlier in the state that establishes its own storage. Unlike the scalar-graph lane's record local this root needs no
/// disposition row — the attached lane's emission writes through the live
/// home place directly rather than rebuilding the local's scalar ledger.
fn owned_local_root<'a>(
    program: &'a TypedTrees,
    operators: &'a checked_trees::CheckedOperatorFacts,
    state: &'a typed_trees::state::State,
    root_symbol: SymbolHandle,
    ordinal: usize,
    dynamic_index: Option<typed_trees::expression::ExpressionHandle>,
    place: crate::flow::CanonicalPlace,
    selectors: Option<super::selectors::TargetSelectors>,
) -> Option<Destination<'a>> {
    let (local_ordinal, local) = program.state_local_declaration(state, root_symbol)?;
    // Bounded byte owners keep their declared predicates and live extent;
    // they are not plain fixed arrays. Only an explicitly mutable whole local
    // with an admitted value establishment supplies this indexed write root.
    let bounded_byte_local = local.is_mutable
        && place.segments.is_empty()
        && dynamic_index.is_some()
        && bounded_byte_values::value_plan(
            program,
            operators,
            state,
            u32::try_from(local_ordinal).ok()?,
            local,
        )
        .is_some();
    // A declared local roots memberwise fills through its structural type
    // alone: the declaration supplies the home each member operation
    // establishes into, whether the source wrote an initializer or leaves
    // the shape to per-member establishment. The member store's leaf
    // dispatch validates each field's own shape; which members reads may
    // rely on is the custody ledger's question, not this root's. Indexed
    // whole-root writes stay on the bounded byte lane.
    let declared_home = matches!(
        program
            .type_reference_table
            .type_reference(local.type_reference),
        TypeReferenceNode::Named { .. }
            | TypeReferenceNode::Generic { .. }
            | TypeReferenceNode::FixedArray { .. }
    );
    let primitive_local =
        super::primitive::primitive_local_before(program, state, ordinal, root_symbol).is_some();
    // Root establishment and projected-leaf validity are independent. A stored
    // reference does not make its carrier's own home unavailable for borrowing
    // or mutation; crossing it must retain explicit permission and geometry.
    if local_ordinal >= ordinal
        || (dynamic_index.is_some() && selectors.is_none() && !bounded_byte_local)
        || !(program
            .expression_table
            .expression_is_valid(local.initial_value)
            || declared_home
            || primitive_local)
        || !(bounded_byte_local || primitive_local || declared_home)
        || program.type_multiplicity(local.type_reference) == Multiplicity::Linear
        || !validation::linear_claim_frontier(program, local.type_reference).is_empty()
    {
        return None;
    }
    Some(Destination {
        root: Root::Local {
            symbol: root_symbol,
        },
        place,
        dynamic_index,
        recast_view: None,
        view_loan: false,
        selectors,
        root_type: local.type_reference,
        root_owner: crate::facts::field_domain::data_definition_for_field_type(
            program,
            local.type_reference,
        ),
        operators,
    })
}

impl<'a> Destination<'a> {
    /// The full projection includes the outer selector split off for byte-view
    /// operations. All other stores consume this same root and selector roster.
    pub(super) fn full_segments(&self, program: &TypedTrees) -> Option<Vec<facts::PlaceSegment>> {
        let mut segments = self.place.segments.clone();
        if let Some(index) = self.dynamic_index {
            segments.push(
                match program.expression_table.constant_integer_value(index) {
                    Some(value) => facts::PlaceSegment::FixedIndex {
                        index: usize::try_from(value).ok()?,
                    },
                    None => facts::PlaceSegment::Index { expression: index },
                },
            );
        }
        Some(segments)
    }

    /// The root's exclusive write authority for a projected store: an
    /// unqualified, non-linear mutable or write-only borrow whose declared
    /// reference agrees with the checked access, inside a signature whose
    /// complete ABI roster the plan covers. An established owned local
    /// supplies its own authority.
    pub(super) fn exclusive_authority(
        &self,
        program: &TypedTrees,
        state: &typed_trees::state::State,
        roots: StoreRoots<'_>,
        trace: &LocalConstructionTrace,
    ) -> Option<()> {
        let Root::Parameter { plan, parameter } = self.root else {
            return Some(());
        };
        let StoreRoots::Parameters { structural, scalar } = roots else {
            return None;
        };
        if plan.multiplicity == Multiplicity::Linear
            || !matches!(
                plan.access,
                CheckedStructuralAccess::MutableBorrow | CheckedStructuralAccess::WriteOnlyBorrow
            )
            || !plan.qualifications.is_empty()
        {
            return None;
        }
        trace.phase("structural field store: parameter access");
        // Signature planning can leave borrowed self in the ambient attachment.
        // Its omitted row must not shift authored positions or allow an ordinary
        // argument to disappear. Rejoin every row by position, not total arity.
        let mut planned_parameters = 0;
        for (position, parameter) in program.state_parameters(state).iter().enumerate() {
            let rows = structural
                .iter()
                .filter(|plan| plan.position as usize == position)
                .count()
                + scalar
                    .iter()
                    .filter(|plan| plan.source_position as usize == position)
                    .count();
            planned_parameters += rows;
            if parameter.relevance.is_erased() {
                if rows != 0 {
                    return None;
                }
            } else if rows != 1
                && !(rows == 0
                    && parameter.is_self
                    && super::super::types::is_reference(program, parameter.type_reference))
            {
                return None;
            }
        }
        if planned_parameters != scalar.len() + structural.len()
            || parameter.is_self != plan.is_self
            || parameter.is_const
            || !parameter.is_mutable
        {
            return None;
        }
        let TypeReferenceNode::Reference { access, .. } = program
            .type_reference_table
            .type_reference(parameter.type_reference)
        else {
            return None;
        };
        let expected_access = match access {
            language_semantics::ReferenceAccess::Mutable => CheckedStructuralAccess::MutableBorrow,
            language_semantics::ReferenceAccess::WriteOnly => {
                CheckedStructuralAccess::WriteOnlyBorrow
            }
            language_semantics::ReferenceAccess::Shared => return None,
        };
        (plan.access == expected_access).then_some(())
    }

    /// The carrier of one scalar leaf store: [`Self::carrier`], except that
    /// the walk may also end at a mixed shape whose common field the leaf
    /// writes. A common field sits at the same offset in every case, so the
    /// write needs no case knowledge and leaves the tag and payload intact.
    /// Common fields are scalar primitives (checking's `validate_data_shape`),
    /// so a mixed shape is only ever the last owner, never an intermediate hop.
    pub(super) fn leaf_carrier(
        &self,
        program: &'a TypedTrees,
        segments: &[facts::PlaceSegment],
    ) -> Option<Carrier<'a>> {
        let projection = self.walk(program, segments)?;
        let owner = projection.owner?;
        ((plain_record(owner, program, self.operators)
            || plain_mixed(owner, program, self.operators))
            && !projection
                .path
                .contains(&CheckedUnitStructuralPathSegment::Referent))
        .then_some(Carrier {
            path: projection.path,
            owner,
        })
    }

    pub(super) fn storage_projection(
        &self,
        program: &'a TypedTrees,
    ) -> Option<StorageProjection<'a>> {
        self.walk(program, &self.full_segments(program)?)
    }

    fn walk(
        &self,
        program: &'a TypedTrees,
        segments: &[facts::PlaceSegment],
    ) -> Option<StorageProjection<'a>> {
        let operators = self.operators;
        if let Some(owner) = self.root_owner
            && !segments.is_empty()
            && !plain_record(owner, program, operators)
        {
            return None;
        }
        let mut path = Vec::with_capacity(segments.len());
        let mut carrier_type = self.root_type;
        let mut owner = self.root_owner;
        let mut ends_at_field = false;
        for segment in segments {
            match segment {
                facts::PlaceSegment::Field { symbol } => {
                    let field_owner = owner?;
                    if !plain_record(field_owner, program, operators) {
                        return None;
                    }
                    let carrier = exact_relevant_field(program, field_owner, *symbol)?;
                    if !crate::facts::field_domain::domain_constraint_symbols(
                        program,
                        carrier.type_reference,
                    )
                    .is_empty()
                    {
                        return None;
                    }
                    path.push(CheckedUnitStructuralPathSegment::Field(
                        terminal_field_identity(program, carrier.symbol)?,
                    ));
                    carrier_type = carrier.type_reference;
                    ends_at_field = true;
                }
                facts::PlaceSegment::FixedIndex { .. } | facts::PlaceSegment::Index { .. } => {
                    // An arithmetic-policy shell (`[Entity; 3] in Wrapping`)
                    // qualifies element operations, not the array's layout.
                    let array = validation::unwrapped_type_reference(program, carrier_type)?;
                    let (element_type, length) =
                        match program.type_reference_table.type_reference(array) {
                            TypeReferenceNode::FixedArray {
                                element_type,
                                length: typed_trees::types::FixedArrayLength::Literal(length),
                            } => (*element_type, Some(*length)),
                            TypeReferenceNode::Slice { element_type } => (*element_type, None),
                            _ => return None,
                        };
                    path.push(match segment {
                        facts::PlaceSegment::FixedIndex { index }
                            if length.is_none_or(|length| *index < length) =>
                        {
                            CheckedUnitStructuralPathSegment::FixedIndex(
                                u64::try_from(*index).ok()?,
                            )
                        }
                        facts::PlaceSegment::Index { expression } => {
                            self.selectors.as_ref()?.runtime_segment(*expression)?
                        }
                        _ => return None,
                    });
                    carrier_type = element_type;
                    ends_at_field = false;
                }
                _ => return None,
            }
            // A stored reference has a pointer slot. A slice reference is a
            // descriptor instead and retains its existing view operation.
            // Explicit crossings keep pointer geometry separate from inline
            // record offsets; later custody checks own the live permission.
            let mut reference_types = Vec::new();
            while let TypeReferenceNode::Reference {
                referee, access, ..
            } = program.type_reference_table.type_reference(carrier_type)
            {
                if reference_types.contains(&carrier_type) {
                    return None;
                }
                reference_types.push(carrier_type);
                if matches!(
                    program.type_reference_table.type_reference(*referee),
                    TypeReferenceNode::Slice { .. }
                ) {
                    break;
                }
                if *access != language_semantics::ReferenceAccess::Mutable {
                    return None;
                }
                path.push(CheckedUnitStructuralPathSegment::Referent);
                carrier_type = *referee;
                ends_at_field = false;
            }
            owner =
                crate::facts::field_domain::data_definition_for_field_type(program, carrier_type);
        }
        Some(StorageProjection {
            path,
            type_reference: carrier_type,
            ends_at_field,
            owner,
        })
    }
}

/// A record whose scalar leaves a store may write through.
///
/// Lifetime binders are deliberately not part of this test. A declared
/// lifetime is an erased region, "separate from runtime generic arity, layout
/// and monomorphization" (wiki/spec/language/lifetimes.md, Binders and source
/// applications), and store admission retains "the complete home declaration,
/// path, field, access, qualifications/claims, scalar type, and dominating
/// definition" (wiki/spec/terminal-psi/structural_access.md, Store
/// vocabulary) -- a binder changes none of them. `exact_relevant_field` skips
/// erased fields. Pointer crossings remain explicit `Referent` steps, with
/// separate leaf-custody checks; canonical field facts cannot cross them.
/// Runtime generic arity is still
/// excluded by the type-parameter and owner-application tests below.
pub(super) fn plain_record(
    data: &typed_trees::data::DataDefinition,
    program: &TypedTrees,
    operators: &checked_trees::CheckedOperatorFacts,
) -> bool {
    plain_checked_shape(data, program, operators)
        && typed_trees::data::DataDefinition::shape_kind_from_members(program.data_members(data))
            == DataShapeKind::Record
}

/// A mixed shape whose common fields a leaf store may write, under the same
/// supply, generic and fact conditions as [`plain_record`].
fn plain_mixed(
    data: &typed_trees::data::DataDefinition,
    program: &TypedTrees,
    operators: &checked_trees::CheckedOperatorFacts,
) -> bool {
    plain_checked_shape(data, program, operators)
        && typed_trees::data::DataDefinition::shape_kind_from_members(program.data_members(data))
            == DataShapeKind::Mixed
}

fn plain_checked_shape(
    data: &typed_trees::data::DataDefinition,
    program: &TypedTrees,
    operators: &checked_trees::CheckedOperatorFacts,
) -> bool {
    data.supply_mode == language_semantics::DataSupplyMode::CheckedShape
        && program.data_type_parameters(data).is_empty()
        && retained_record_owner_application(data, program)
        && data.quotient.is_none()
        // A store may break a `where` fact only where Terminal re-proves the
        // whole default domain before relying on it: interval-only facts bound
        // the fields' own carriers, and every other fact must be carried
        // exactly by the declaration predicate roster, whose rows a call
        // proves before entry and a published guarantee proves at return. A
        // machine without rows relies on none. Zero validity alone proves
        // nothing per store.
        && validation::declaration_facts_carried(program, operators, data)
        && !data.zero_gated
}

fn retained_record_owner_application(
    data: &typed_trees::data::DataDefinition,
    program: &TypedTrees,
) -> bool {
    let Some(application) = data.generic_instance else {
        return true;
    };
    let TypeReferenceNode::Generic {
        base_symbol,
        lifetime_arguments,
        arguments,
        ..
    } = program.type_reference_table.type_reference(application)
    else {
        return false;
    };
    let Some(template) = program
        .data_definitions()
        .iter()
        .find(|template| template.symbol == *base_symbol)
    else {
        return false;
    };
    let parameters = program.data_type_parameters(template);
    // Generated-instance arguments are checked in an empty type-parameter
    // scope before flow planning. Retain that exact closed owner application,
    // rather than excluding its substituted fields merely for being generated.
    // Field shape, access, arithmetic policy, and mutation custody are still
    // checked independently by the store.
    base_symbol.is_valid()
        && *base_symbol != data.symbol
        && template.generic_instance.is_none()
        && template.lifetime_parameters.is_empty()
        && lifetime_arguments.is_empty()
        && !parameters.is_empty()
        && parameters.len()
            == program
                .type_reference_table
                .type_reference_handles(*arguments)
                .len()
}

/// Whether a declared field type names storage behind a reference.
///
/// Field facts describe inline storage of one referent. A pointer hop is not
/// an inline offset: primitive-store projection retains an explicit `Referent`
/// step and checks the reference leaf's authority separately. Consumers that
/// build canonical field facts use this query to exclude that crossing.
pub(super) fn crosses_reference(
    program: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { .. } => true,
        TypeReferenceNode::Constrained { base_type, .. } => crosses_reference(program, *base_type),
        _ => false,
    }
}

pub(super) fn exact_relevant_field<'a>(
    program: &'a TypedTrees,
    owner: &'a typed_trees::data::DataDefinition,
    symbol: SymbolHandle,
) -> Option<&'a typed_trees::data::DataField> {
    let mut fields = program
        .data_members(owner)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) if field.symbol == symbol && !field.relevance.is_erased() => {
                Some(field)
            }
            _ => None,
        });
    let field = fields.next()?;
    fields.next().is_none().then_some(field)
}

/// Rejoin a write through an exclusive loan local with the place it loans.
///
/// `let seen: &mut u8 = &mut self.byte; seen = 0;` writes `self.byte`. The
/// local is immutable and a reference cannot be reseated, so the name denotes
/// one place for its whole scope and the store is the ordinary parameter-rooted
/// store of that place with the loan's own segments in front. This adds no
/// authority: the loan's exclusivity, lifetime and conflicting-access rules are
/// already checked, and a shared loan is not a write destination.
///
/// A place that is not rooted at such a local is returned unchanged.
fn rejoin_loan_local(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    ordinal: usize,
    place: crate::flow::CanonicalPlace,
) -> Option<(
    crate::flow::CanonicalPlace,
    Option<typed_trees::types::PrimitiveType>,
)> {
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return Some((place, None));
    };
    let statements = program.statement_table.statements(state.statement_nodes);
    let mut declarations =
        statements
            .iter()
            .enumerate()
            .take(ordinal)
            .filter_map(|(declared, statement)| match statement {
                StatementNode::LocalData(local) if local.symbol == symbol => {
                    Some((declared, local))
                }
                _ => None,
            });
    let Some((_, local)) = declarations.next() else {
        return Some((place, None));
    };
    if declarations.next().is_some() {
        return None;
    }
    // `&mut place as &mut T` spells its recast as a cast of the borrow, so
    // the loan's leading recast peels before the borrow itself.
    let mut initial = local.initial_value;
    let mut recast = false;
    while let ExpressionNode::Cast(cast) = program.expression_table.expression(initial) {
        if !cast.form.is_recast() {
            break;
        }
        initial = cast.value;
        recast = true;
    }
    let ExpressionNode::Borrow(borrow) = program.expression_table.expression(initial) else {
        return Some((place, None));
    };
    // A reseatable binding does not denote one place, and a shared loan
    // carries no write authority to rejoin. A write-only loan carries
    // write authority exactly where a mutable one does: the loan ledger
    // already fenced reads, so a store through it is the same rejoin.
    if local.is_mutable
        || !matches!(
            borrow.access,
            language_core::ReferenceAccess::Mutable | language_core::ReferenceAccess::WriteOnly
        )
    {
        return None;
    }
    let mut target = borrow.target;
    while let ExpressionNode::Cast(cast) = program.expression_table.expression(target) {
        if !cast.form.is_recast() {
            break;
        }
        target = cast.value;
        recast = true;
    }
    let recast_view = if recast {
        match program
            .type_reference_table
            .type_reference(local.type_reference)
        {
            TypeReferenceNode::Reference { referee, .. } => {
                match program.primitive_type_reference(*referee) {
                    // A store to a record view's direct scalar field writes the
                    // source leaf the recast judgment matched at the same
                    // offset and width. It rejoins that leaf below the loaned
                    // place; an equal-width integer leaf of the other sign
                    // keeps the view's carrier as the stored value's.
                    None => {
                        if let Some((leaf_place, view)) = record_view_leaf(
                            program, machine, state, ordinal, target, *referee, &place,
                        ) {
                            return Some((leaf_place, view));
                        }
                        // Any other record view's fields belong to the view's
                        // own structure, not to the loaned place's segments,
                        // so the rejoin keeps the local as root: the exclusive
                        // borrowed view arm resolves its home through the loan
                        // ledger.
                        return Some((place, None));
                    }
                    primitive => primitive,
                }
            }
            _ => None,
        }
    } else {
        None
    };
    let mut loaned = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        ordinal,
        target,
    )?;
    loaned.segments.extend(place.segments);
    Some((loaned, recast_view))
}

/// The source leaf a store through a typed record view writes: the loaned
/// place followed by the leaf's own segments, and the view's carrier when it
/// differs from the leaf's. Only a direct field of the view rejoins.
fn record_view_leaf(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    ordinal: usize,
    target: typed_trees::expression::ExpressionHandle,
    referee: typed_trees::types::TypeReferenceHandle,
    place: &crate::flow::CanonicalPlace,
) -> Option<(
    crate::flow::CanonicalPlace,
    Option<typed_trees::types::PrimitiveType>,
)> {
    let [facts::PlaceSegment::Field { symbol: field }] = place.segments.as_slice() else {
        return None;
    };
    let source = validation::declared_place_type_raw(program, machine, Some(state), target)?;
    let leaf = validation::recast_record_source_leaf(program, source, referee, *field)?;
    let mut loaned = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        ordinal,
        target,
    )?;
    for segment in leaf.path {
        loaned.segments.push(match segment {
            validation::RecastLeafSegment::Field(symbol) => facts::PlaceSegment::Field { symbol },
            validation::RecastLeafSegment::Element(index) => {
                facts::PlaceSegment::FixedIndex { index }
            }
        });
    }
    let view = (leaf.view_primitive != leaf.primitive).then_some(leaf.view_primitive);
    Some((loaned, view))
}

/// An exclusive borrowed view is a root of its own: `let cell: &mut Cell =
/// self.pick(bag)` binds one immutable `&mut` local whose referent storage is
/// already established, so `cell.value = 7` is a field store through that
/// view -- a `Local` destination whose home the structural-local roster
/// resolves to the loaned place. The checked borrow ledger's single Mutable
/// loan on the binding is the write authority this arm re-reads; formation,
/// exclusivity and lifetime were all admitted by the loan itself.
fn borrowed_view_local_root<'a>(
    program: &'a TypedTrees,
    facts: &'a CheckFacts,
    machine: &'a typed_trees::machine::Machine,
    state: &'a typed_trees::state::State,
    root_symbol: SymbolHandle,
    ordinal: usize,
    dynamic_index: Option<typed_trees::expression::ExpressionHandle>,
    place: crate::flow::CanonicalPlace,
    selectors: Option<super::selectors::TargetSelectors>,
) -> Option<Destination<'a>> {
    let statements = program.statement_table.statements(state.statement_nodes);
    let mut locals = statements
        .iter()
        .enumerate()
        .filter_map(|(local_ordinal, statement)| match statement {
            StatementNode::LocalData(local) if local.symbol == root_symbol => {
                Some((local_ordinal, local))
            }
            _ => None,
        });
    let (local_ordinal, local) = locals.next()?;
    if locals.next().is_some()
        || local_ordinal >= ordinal
        || local.is_mutable
        || !program
            .expression_table
            .expression_is_valid(local.initial_value)
    {
        return None;
    }
    let (referee, access) =
        super::super::reference_results::mutable_borrowed_parts(program, local.type_reference)?;
    if access != CheckedStructuralAccess::MutableBorrow {
        return None;
    }
    let borrow_state = facts
        .borrow
        .states
        .iter()
        .map(|(_, borrow_state)| borrow_state)
        .find(|borrow_state| {
            borrow_state.machine_symbol == machine.symbol
                && borrow_state.state_symbol == state.symbol
        })?;
    let mut loans = facts
        .borrow
        .loans
        .span_or_empty(borrow_state.loans)
        .iter()
        .filter(|loan| loan.owner_symbol == root_symbol && loan.statement_index == local_ordinal);
    let loan = loans.next()?;
    if loans.next().is_some() || loan.kind != checked_trees::BorrowAccessKind::Mutable {
        return None;
    }
    Some(Destination {
        root: Root::Local {
            symbol: root_symbol,
        },
        place,
        dynamic_index,
        recast_view: None,
        view_loan: true,
        selectors,
        root_type: referee,
        root_owner: crate::facts::field_domain::data_definition_for_field_type(program, referee),
        operators: &facts.operators,
    })
}
