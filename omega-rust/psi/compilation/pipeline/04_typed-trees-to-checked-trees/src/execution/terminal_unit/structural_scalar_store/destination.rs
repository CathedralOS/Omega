//! Resolve where one authored assignment writes: the storage root it names,
//! the root's write authority, and the checked carrier path from that root.
//!
//! The destination is resolved without looking at the right-hand side. Each
//! store kind in `super` then reads the resolved place and decides whether
//! its own leaf and value compose with it.
use super::super::{
    CheckFacts, CheckedStructuralAccess, CheckedStructuralScalarParameterPlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment, DataMember,
    DataShapeKind, ExpressionNode, StatementNode, SymbolHandle, TypeReferenceNode,
};
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::control::statement_sequence::bounded_byte_values;
use crate::execution::terminal_unit::types::terminal_field_identity;

/// The storage roots retained for source-validated stores: owned or exclusive
/// borrowed structural parameters, established locals, and the `mut` scalar
/// formals the body owns storage for.
#[derive(Clone, Copy)]
pub(super) struct StoreRoots<'a> {
    pub(super) structural: &'a [CheckedUnitStructuralParameterPlan],
    pub(super) scalar: &'a [CheckedStructuralScalarParameterPlan],
    pub(super) storage: &'a [checked_trees::CheckedUnitParameterStoragePlan],
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
    pub(super) place: validation::CanonicalPlace,
    pub(super) dynamic_index: Option<typed_trees::expression::ExpressionHandle>,
    /// The declared referee primitive of a recast loan local the target
    /// names (`let view: &mut u32 = &mut self.buf[k] as &mut u32`): the
    /// carrier the authored store writes at, distinct from the rejoined
    /// leaf's own primitive.
    pub(super) recast_view: Option<typed_trees::types::PrimitiveType>,
    /// The exact exclusive reference local's declared type, or zero for an
    /// owned/parameter root. Keep its shell: the referent alone cannot decide
    /// whether storage is a pointer slot or a byte/element descriptor.
    pub(super) loan_type: typed_trees::types::TypeReferenceHandle,
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
    selected_case: Option<&'a typed_trees::data::DataVariant>,
}

pub(super) struct StorageProjection<'a> {
    pub(super) path: Vec<CheckedUnitStructuralPathSegment>,
    /// Position of the last element-view descriptor crossing. Earlier
    /// selectors belong to the full structural operand locating this view;
    /// later selectors locate the scalar leaf inside its element.
    pub(super) element_view_index: Option<usize>,
    pub(super) type_reference: typed_trees::types::TypeReferenceHandle,
    /// A declared scalar field keeps its field-store range obligation. An
    /// array element or dereferenced primitive instead names primitive storage.
    pub(super) ends_at_field: bool,
    owner: Option<&'a typed_trees::data::DataDefinition>,
    selected_case: Option<&'a typed_trees::data::DataVariant>,
}

/// Resolve the authored target, its root and the root's write authority.
pub(super) fn resolve<'a>(
    program: &'a dyn ::validation::CheckingInput,
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
    let place = validation::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        ordinal,
        target,
    )?;
    let materialized = |root: facts::PlaceRoot| {
        let Some(symbol) = root.as_symbol() else {
            return false;
        };
        let Some((declared, local)) = program.state_local_declaration(state, symbol) else {
            return false;
        };
        let Ok(declared_index) = u32::try_from(declared) else {
            return false;
        };
        facts
            .values
            .structural_values
            .root_for_expression(state.symbol, declared_index, local.initial_value)
            .is_some()
            || crate::execution::terminal_unit::control::reference_captures::plan(
                program, facts, machine, state, declared, local,
            )
            .is_some()
    };
    let aliases =
        super::super::receiver_aliases::aliases(program, facts, machine, state).unwrap_or_default();
    let place =
        super::super::receiver_aliases::resolve(&aliases, &place, materialized).unwrap_or(place);
    let (place, recast_view) = if materialized(place.root) {
        (place, None)
    } else {
        rejoin_loan_local(program, machine, state, ordinal, place)?
    };
    let root_symbol = place.root.as_symbol()?;
    let selectors = super::selectors::TargetSelectors::resolve(
        program,
        facts,
        machine,
        state,
        statement_index,
        assignment.target,
    );
    trace.phase("structural field store: destination parameter");
    let StoreRoots {
        structural,
        storage,
        ..
    } = roots;
    let source_parameters = program.state_parameters(state);
    // The assignment selects its destination; unrelated borrowed
    // inputs do not change that root's authority or require it to
    // occupy position zero.
    let mut destinations = structural.iter().filter_map(|plan| {
        let parameter = source_parameters.get(plan.position as usize)?;
        (parameter.symbol == root_symbol).then_some((plan, parameter))
    });
    let Some((plan, parameter)) = destinations.next() else {
        // A `mut` scalar formal is storage the body establishes at
        // entry, so its assignment writes that storage exactly as a
        // `let mut` local's assignment writes the local's.
        if storage.iter().any(|row| row.symbol == root_symbol) {
            let formal = source_parameters
                .iter()
                .find(|parameter| parameter.symbol == root_symbol)?;
            return (dynamic_index.is_none() && place.segments.is_empty()).then(|| {
                local_destination(
                    program,
                    &facts.operators,
                    root_symbol,
                    formal.type_reference,
                    place,
                    dynamic_index,
                    selectors,
                )
            });
        }
        // An established owned structural local roots a field store
        // without any parameter loan: emission resolves its live home
        // place through the structural-local roster, so admission here
        // pins the earlier declaration, not permission to transfer
        // all its contents. A reference leaf retains its own custody.
        return exclusive_reference_local_root(
            program,
            facts,
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
    let root_type = match plan.access {
        CheckedStructuralAccess::Owned => parameter.type_reference,
        _ => validation::reference_shell(program, parameter.type_reference)?.referee,
    };
    // A borrowed fixed-array root has no record owner of its
    // own: its element record resolves through the first carrier
    // `FixedIndex` hop, so an unresolved owner stays admissible only
    // behind an index segment.
    let root_owner = validation::field_domains::data_definition_for_field_type(program, root_type);
    Some(Destination {
        root: Root::Parameter { plan, parameter },
        place,
        dynamic_index,
        recast_view,
        loan_type: Default::default(),
        selectors,
        root_type,
        root_owner,
        operators: &facts.operators,
    })
}

/// A store rooted at a declared local: the unique `LocalData` symbol names
/// the root and its declared type fixes the owner and projection base.
fn local_destination<'a>(
    program: &'a dyn ::validation::CheckingInput,
    operators: &'a checked_trees::CheckedOperatorFacts,
    root_symbol: SymbolHandle,
    root_type: typed_trees::types::TypeReferenceHandle,
    place: validation::CanonicalPlace,
    dynamic_index: Option<typed_trees::expression::ExpressionHandle>,
    selectors: Option<super::selectors::TargetSelectors>,
) -> Destination<'a> {
    Destination {
        root: Root::Local {
            symbol: root_symbol,
        },
        place,
        dynamic_index,
        recast_view: None,
        loan_type: Default::default(),
        selectors,
        root_type,
        root_owner: validation::field_domains::data_definition_for_field_type(program, root_type),
        operators,
    }
}

/// An owned local store root under the parameter lane: a unique `LocalData`
/// declaration earlier in the state that establishes its own storage.
/// Unlike the scalar-graph lane's record local this root needs no
/// disposition row — the attached lane's emission writes through the live
/// home place directly rather than rebuilding the local's scalar ledger.
fn owned_local_root<'a>(
    program: &'a dyn ::validation::CheckingInput,
    operators: &'a checked_trees::CheckedOperatorFacts,
    state: &'a typed_trees::state::State,
    root_symbol: SymbolHandle,
    ordinal: usize,
    dynamic_index: Option<typed_trees::expression::ExpressionHandle>,
    place: validation::CanonicalPlace,
    selectors: Option<super::selectors::TargetSelectors>,
) -> Option<Destination<'a>> {
    let (local_ordinal, local) = program.state_local_declaration(state, root_symbol)?;
    // Bounded byte owners keep their declared predicates and live extent;
    // they are not plain fixed arrays. Only an explicitly mutable whole local
    // with an admitted value establishment supplies this write root.
    let bounded_byte_local = local.is_mutable
        && place.segments.is_empty()
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
    // whole-root writes retain their element selectors. Qualification does not
    // change the declared home; leaf stores still prove the declared predicates.
    let declared_type = validation::peel_declared_constraints(program, local.type_reference);
    let declared_home = matches!(
        program.type_reference_table.type_reference(declared_type),
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
        || !(bounded_byte_local || primitive_local || declared_home)
        // Cleanup policy does not change the authority to establish fields.
        // A borrowed root must instead rejoin its exact loan.
        || validation::reference_shell(program, local.type_reference).is_some()
    {
        return None;
    }
    Some(local_destination(
        program,
        operators,
        root_symbol,
        local.type_reference,
        place,
        dynamic_index,
        selectors,
    ))
}

impl<'a> Destination<'a> {
    /// The full projection includes the outer selector split off for byte-view
    /// operations. All other stores consume this same root and selector roster.
    pub(super) fn full_segments(
        &self,
        program: &dyn ::validation::CheckingInput,
    ) -> Option<Vec<facts::PlaceSegment>> {
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

    /// Runtime storage authority belongs to the exact owned home or exclusive
    /// borrowed referent, independent of entry versus continuation spelling.
    /// Source assignment validation already judged binding mutability and the
    /// stored-reference access route. This join retains the complete signature
    /// roster and declared access; leaf planning keeps qualification and live
    /// custody checks. A second binding-mutability test would reject writes
    /// through references held inside an owned parameter.
    pub(super) fn exclusive_authority(
        &self,
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        roots: StoreRoots<'_>,
        trace: &LocalConstructionTrace,
    ) -> Option<()> {
        let Root::Parameter { plan, parameter } = self.root else {
            return Some(());
        };
        let StoreRoots {
            structural, scalar, ..
        } = roots;
        if !matches!(
            plan.access,
            CheckedStructuralAccess::Owned
                | CheckedStructuralAccess::MutableBorrow
                | CheckedStructuralAccess::WriteOnlyBorrow
        ) {
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
            } else if rows != 1 {
                return None;
            }
        }
        if planned_parameters != scalar.len() + structural.len() || parameter.is_const {
            return None;
        }
        let expected_access = match validation::reference_shell(program, parameter.type_reference) {
            Some(shell) => match shell.access {
                language_semantics::ReferenceAccess::Mutable => {
                    CheckedStructuralAccess::MutableBorrow
                }
                language_semantics::ReferenceAccess::WriteOnly => {
                    CheckedStructuralAccess::WriteOnlyBorrow
                }
                language_semantics::ReferenceAccess::Shared => return None,
            },
            None => CheckedStructuralAccess::Owned,
        };
        (plan.access == expected_access).then_some(())
    }

    /// Resolve the leaf's exact declaration namespace. A trailing case keeps
    /// its payload roster; otherwise the owner supplies its common fields.
    /// The same path and scalar-field obligations govern either namespace.
    pub(super) fn leaf_carrier(
        &self,
        program: &'a dyn ::validation::CheckingInput,
        segments: &[facts::PlaceSegment],
    ) -> Option<Carrier<'a>> {
        let projection = self.walk(program, segments)?;
        let owner = projection.owner?;
        // The shared walk already establishes each reference crossing's
        // mutable access. Keep that geometry for field stores just as for
        // primitive stores; the carrier need not be inline in the root.
        plain_checked_shape(owner, program, self.operators).then_some(Carrier {
            path: projection.path,
            owner,
            selected_case: projection.selected_case,
        })
    }

    pub(super) fn storage_projection(
        &self,
        program: &'a dyn ::validation::CheckingInput,
    ) -> Option<StorageProjection<'a>> {
        let mut projection = self.walk(program, &self.full_segments(program)?)?;
        // Primitive operations name the entire path from the stored root.
        // Stored references retain an explicit pointer crossing. Dynamic
        // slices and fixed-byte views instead name their descriptor's backing
        // directly; adding a stored-reference hop would dereference it twice.
        let descriptor = self.loan_type.is_valid()
            && (validation::slice_view_shell(program, self.loan_type, &[]).is_some()
                || matches!(
                    validation::byte_sequence_carrier(program, self.loan_type, &[]),
                    Some(
                        checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. }
                            | checked_trees::CheckedByteSequenceCarrier::BoundedView { .. }
                    )
                ));
        if self.loan_type.is_valid() && !descriptor {
            projection
                .path
                .insert(0, CheckedUnitStructuralPathSegment::Referent);
            if let Some(position) = &mut projection.element_view_index {
                *position += 1;
            }
        }
        Some(projection)
    }

    fn walk(
        &self,
        program: &'a dyn ::validation::CheckingInput,
        segments: &[facts::PlaceSegment],
    ) -> Option<StorageProjection<'a>> {
        let operators = self.operators;
        if let Some(owner) = self.root_owner
            && !segments.is_empty()
            && !plain_checked_shape(owner, program, operators)
        {
            return None;
        }
        let mut path = Vec::with_capacity(segments.len());
        let mut element_view_index = None;
        let mut carrier_type = self.root_type;
        let mut owner = self.root_owner;
        let mut ends_at_field = false;
        let mut selected_case = None;
        for segment in segments {
            match segment {
                facts::PlaceSegment::Field { symbol } => {
                    let field_owner = owner?;
                    if !plain_checked_shape(field_owner, program, operators) {
                        return None;
                    }
                    let carrier = relevant_field_in_namespace(
                        program,
                        field_owner,
                        selected_case.take(),
                        *symbol,
                    )?;
                    // Qualifications remain on the complete source/Terminal
                    // root contract. Storage traversal never selects geometry
                    // from a predicate; the receiving writer separately proves
                    // preservation against its full normalized footprint.
                    path.push(CheckedUnitStructuralPathSegment::Field(
                        terminal_field_identity(program, carrier.symbol)?,
                    ));
                    carrier_type = carrier.type_reference;
                    ends_at_field = true;
                }
                facts::PlaceSegment::Case { variant } => {
                    let case_owner = owner?;
                    if selected_case.is_some()
                        || !plain_checked_shape(case_owner, program, operators)
                    {
                        return None;
                    }
                    let mut cases = program
                        .data_members(case_owner)
                        .iter()
                        .filter_map(|member| match member {
                            DataMember::Variant(case)
                                if variant.is_valid() && case.symbol == *variant =>
                            {
                                Some(case)
                            }
                            _ => None,
                        });
                    let case = cases.next()?;
                    if cases.next().is_some() {
                        return None;
                    }
                    path.push(CheckedUnitStructuralPathSegment::Case(
                        checked_trees::CheckedMemberIdentity::of_member(
                            case.identity,
                            case.name.clone(),
                        ),
                    ));
                    selected_case = Some(case);
                    ends_at_field = false;
                    // A case selects a field namespace; it is not another
                    // value or a pointer crossing at the carrier's type.
                    continue;
                }
                facts::PlaceSegment::FixedIndex { .. } | facts::PlaceSegment::Index { .. } => {
                    if selected_case.is_some() {
                        return None;
                    }
                    // An arithmetic-policy shell (`[Entity; 3] in Wrapping`)
                    // qualifies element operations, not the array's layout.
                    let array = validation::unwrapped_type_reference(program, carrier_type)?;
                    // A recipe extent has no literal bound to check here, like
                    // a slice's live extent; its index obligation is recorded.
                    let (element_type, length) = match program
                        .type_reference_table
                        .type_reference(array)
                    {
                        TypeReferenceNode::FixedArray { element_type, .. } => (
                            *element_type,
                            program
                                .declared_array_extent(array)?
                                .literal()
                                .map(usize::try_from)
                                .transpose()
                                .ok()?,
                        ),
                        TypeReferenceNode::Slice { element_type } => {
                            if matches!(program.type_reference_table.type_reference(carrier_type),
                                    TypeReferenceNode::Reference { access, .. }
                                        if *access != language_semantics::ReferenceAccess::Mutable)
                            {
                                return None;
                            }
                            if program.primitive_type_reference(*element_type)
                                != Some(typed_trees::types::PrimitiveType::U8)
                            {
                                // Each crossing selects a new descriptor.
                                // Its prefix already carries the earlier
                                // selectors and their checked bounds; they
                                // must not become the final element's leaf.
                                element_view_index = Some(path.len());
                            }
                            (*element_type, None)
                        }
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
            loop {
                carrier_type = validation::peel_declared_constraints(program, carrier_type);
                let TypeReferenceNode::Reference {
                    referee, access, ..
                } = program.type_reference_table.type_reference(carrier_type)
                else {
                    break;
                };
                if reference_types.contains(&carrier_type) {
                    return None;
                }
                reference_types.push(carrier_type);
                if validation::slice_view_shell(program, carrier_type, &[]).is_some() {
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
                validation::field_domains::data_definition_for_field_type(program, carrier_type);
        }
        Some(StorageProjection {
            path,
            element_view_index,
            type_reference: carrier_type,
            ends_at_field,
            owner,
            selected_case,
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
    program: &dyn ::validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
) -> bool {
    plain_checked_shape(data, program, operators)
        && typed_trees::data::DataDefinition::shape_kind_from_members(program.data_members(data))
            == DataShapeKind::Record
}

fn plain_checked_shape(
    data: &typed_trees::data::DataDefinition,
    program: &dyn ::validation::CheckingInput,
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
        // nothing per store: a zero gate restricts zero provisioning, not
        // replacement, which binds the replacement value to the field's
        // retained declaration requirement (structural_access.md, Complete
        // record construction). A record whose zero is not a member, such as
        // one with `health >= 1`, is therefore written like any other.
        && validation::declaration_facts_carried(program, operators, data)
}

fn retained_record_owner_application(
    data: &typed_trees::data::DataDefinition,
    program: &dyn ::validation::CheckingInput,
) -> bool {
    let Some(application) = data.generic_instance else {
        return true;
    };
    // A prepared application may retain its template's symbol. The producer's
    // exact closed view, not a synthetic nominal symbol or lifetime spelling,
    // owns the substituted field telescope. Source checking already judged
    // the application; stores still retain their own leaf and custody checks.
    program
        .closed_data_definition(application)
        .is_some_and(|retained| retained == data)
}

/// Whether a declared field type names storage behind a reference.
///
/// Field facts describe inline storage of one referent. A pointer hop is not
/// an inline offset: primitive-store projection retains an explicit `Referent`
/// step and checks the reference leaf's authority separately. Consumers that
/// build canonical field facts use this query to exclude that crossing.
pub(super) fn crosses_reference(
    program: &dyn ::validation::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { .. } => true,
        TypeReferenceNode::Constrained { base_type, .. } => crosses_reference(program, *base_type),
        _ => false,
    }
}

pub(super) fn exact_relevant_field<'a>(
    program: &'a dyn ::validation::CheckingInput,
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

impl<'a> Carrier<'a> {
    pub(super) fn field(
        &self,
        program: &'a dyn validation::CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<&'a typed_trees::data::DataField> {
        relevant_field_in_namespace(program, self.owner, self.selected_case, symbol)
    }
}

fn relevant_field_in_namespace<'a>(
    program: &'a dyn validation::CheckingInput,
    owner: &'a typed_trees::data::DataDefinition,
    selected_case: Option<&'a typed_trees::data::DataVariant>,
    symbol: SymbolHandle,
) -> Option<&'a typed_trees::data::DataField> {
    let Some(case) = selected_case else {
        return exact_relevant_field(program, owner, symbol);
    };
    let mut fields = program.data_payload_fields(case).iter().filter(|field| {
        symbol.is_valid() && field.symbol == symbol && !field.relevance.is_erased()
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
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    ordinal: usize,
    place: validation::CanonicalPlace,
) -> Option<(
    validation::CanonicalPlace,
    Option<typed_trees::types::PrimitiveType>,
)> {
    let Some(symbol) = place.root.as_symbol() else {
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
    while let Some(cast) = program.expression_table.cast_node(initial) {
        if !cast.form.is_recast() {
            break;
        }
        initial = cast.value;
        recast = true;
    }
    let Some(borrow) = program.expression_table.borrow_expression(initial) else {
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
    while let Some(cast) = program.expression_table.cast_node(target) {
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
    let mut loaned = validation::canonical_place_from_expression_in_state(
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
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    ordinal: usize,
    target: typed_trees::expression::ExpressionHandle,
    referee: typed_trees::types::TypeReferenceHandle,
    place: &validation::CanonicalPlace,
) -> Option<(
    validation::CanonicalPlace,
    Option<typed_trees::types::PrimitiveType>,
)> {
    let [facts::PlaceSegment::Field { symbol: field }] = place.segments.as_slice() else {
        return None;
    };
    let source = validation::declared_place_type_raw(program, machine, Some(state), target)?;
    let leaf = validation::recast_record_source_leaf(program, source, referee, *field)?;
    let mut loaned = validation::canonical_place_from_expression_in_state(
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

/// An exclusive reference local names its current materialized carrier. The declaration
/// supplies its referent shape and access; source checking already admitted
/// this assignment's loan compatibility. Replaying the initializer's loan here
/// would confuse its old backing with the descriptor installed by a rebind.
fn exclusive_reference_local_root<'a>(
    program: &'a dyn ::validation::CheckingInput,
    facts: &'a CheckFacts,
    state: &'a typed_trees::state::State,
    root_symbol: SymbolHandle,
    ordinal: usize,
    dynamic_index: Option<typed_trees::expression::ExpressionHandle>,
    place: validation::CanonicalPlace,
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
        || !program
            .expression_table
            .expression_is_valid(local.initial_value)
    {
        return None;
    }
    let shell = validation::reference_shell(program, local.type_reference)?;
    if !matches!(
        shell.access,
        language_semantics::ReferenceAccess::Mutable
            | language_semantics::ReferenceAccess::WriteOnly
    ) {
        return None;
    }
    let referee = shell.referee;
    Some(Destination {
        root: Root::Local {
            symbol: root_symbol,
        },
        place,
        dynamic_index,
        recast_view: None,
        loan_type: local.type_reference,
        selectors,
        root_type: referee,
        root_owner: validation::field_domains::data_definition_for_field_type(program, referee),
        operators: &facts.operators,
    })
}
