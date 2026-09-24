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
    pub(super) fn destination(
        &self,
    ) -> checked_trees::CheckedStructuralScalarFieldStoreDestination {
        match self {
            Self::Parameter { plan, .. } => {
                checked_trees::CheckedStructuralScalarFieldStoreDestination::Parameter {
                    position: plan.position,
                }
            }
            Self::Local { symbol } => {
                checked_trees::CheckedStructuralScalarFieldStoreDestination::Local {
                    symbol: *symbol,
                }
            }
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
    /// The target's admitted element selectors, when every indexed step is
    /// an element of a declared fixed array.
    selectors: Option<super::selectors::TargetSelectors>,
    /// Declared type of the root's referent (the borrowed record, or the
    /// local's own type) and its record owner when the root is a record.
    root_type: typed_trees::types::TypeReferenceHandle,
    root_owner: Option<&'a typed_trees::data::DataDefinition>,
}

/// The checked carrier path to a record, and that record's declaration.
pub(super) struct Carrier<'a> {
    pub(super) path: Vec<CheckedUnitStructuralPathSegment>,
    pub(super) owner: &'a typed_trees::data::DataDefinition,
}

/// Resolve the authored target, its root and the root's write authority.
pub(super) fn resolve<'a>(
    program: &'a TypedTrees,
    facts: &CheckFacts,
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
            let (plan, parameter) = destinations.next()?;
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
                selectors,
                root_type: *referee,
                root_owner,
            })
        }
        StoreRoots::RecordLocals => {
            // A local root is a unique owned record established earlier in
            // this state, with the scalar-graph lane's record shape and a
            // checked disposition.
            let statements = program.statement_table.statements(state.statement_nodes);
            let mut locals = statements
                .iter()
                .enumerate()
                .filter_map(|(ordinal, statement)| match statement {
                    StatementNode::LocalData(local) if local.symbol == root_symbol => {
                        Some((ordinal, local))
                    }
                    _ => None,
                });
            let (local_ordinal, local) = locals.next()?;
            if locals.next().is_some() || local_ordinal >= ordinal || dynamic_index.is_some() {
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
                selectors,
                root_type: local.type_reference,
                root_owner: crate::facts::field_domain::data_definition_for_field_type(
                    program,
                    local.type_reference,
                ),
            })
        }
    }
}

impl<'a> Destination<'a> {
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
        if crate::execution::terminal_unit::types::abi_parameter_count(
            program.state_parameters(state),
        ) != scalar.len() + structural.len()
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

    /// Walk `segments` from the root as direct storage access: relevant plain
    /// record fields (never through a reference, never through a
    /// domain-constrained carrier whose invariant a leaf write could break)
    /// and elements of declared fixed arrays, literal within the extent or
    /// runtime-selected, in any order. The walk ends at a record, whose
    /// declaration is returned.
    pub(super) fn carrier(
        &self,
        program: &'a TypedTrees,
        segments: &[facts::PlaceSegment],
    ) -> Option<Carrier<'a>> {
        if let Some(owner) = self.root_owner
            && !plain_record(owner, program)
        {
            return None;
        }
        let mut path = Vec::with_capacity(segments.len());
        let mut carrier_type = self.root_type;
        let mut owner = self.root_owner;
        for segment in segments {
            match segment {
                facts::PlaceSegment::Field { symbol } => {
                    let field_owner = owner?;
                    if !plain_record(field_owner, program) {
                        return None;
                    }
                    let carrier = exact_relevant_field(program, field_owner, *symbol)?;
                    if !crate::facts::field_domain::domain_constraint_symbols(
                        program,
                        carrier.type_reference,
                    )
                    .is_empty()
                        || crosses_reference(program, carrier.type_reference)
                    {
                        return None;
                    }
                    path.push(CheckedUnitStructuralPathSegment::Field(
                        terminal_field_identity(program, carrier.symbol)?,
                    ));
                    carrier_type = carrier.type_reference;
                }
                facts::PlaceSegment::FixedIndex { .. } | facts::PlaceSegment::Index { .. } => {
                    // An arithmetic-policy shell (`[Entity; 3] in Wrapping`)
                    // qualifies element operations, not the array's layout.
                    let array = validation::unwrapped_type_reference(program, carrier_type)?;
                    let TypeReferenceNode::FixedArray {
                        element_type,
                        length: typed_trees::types::FixedArrayLength::Literal(length),
                    } = program.type_reference_table.type_reference(array)
                    else {
                        return None;
                    };
                    if crosses_reference(program, *element_type) {
                        return None;
                    }
                    path.push(match segment {
                        facts::PlaceSegment::FixedIndex { index } if *index < *length => {
                            CheckedUnitStructuralPathSegment::FixedIndex(
                                u64::try_from(*index).ok()?,
                            )
                        }
                        facts::PlaceSegment::Index { expression } => {
                            self.selectors.as_ref()?.runtime_segment(*expression)?
                        }
                        _ => return None,
                    });
                    carrier_type = *element_type;
                }
                _ => return None,
            }
            owner =
                crate::facts::field_domain::data_definition_for_field_type(program, carrier_type);
        }
        let owner = owner?;
        plain_record(owner, program).then_some(Carrier { path, owner })
    }

    /// The canonical mutation-summary spelling of this destination's place:
    /// the receiver is `self` and other parameters are positional, even when
    /// the receiver occupies structural position zero.
    pub(super) fn mutation_path(&self, program: &TypedTrees) -> Option<String> {
        let Root::Parameter { plan, .. } = self.root else {
            return None;
        };
        let source_path =
            facts::canonical_place_label_from_parts(program, self.place.root, &self.place.segments);
        let source_root = facts::canonical_place_label_from_parts(program, self.place.root, &[]);
        let mutation_root = if plan.is_self {
            "self".to_owned()
        } else {
            format!("$P{}", plan.position)
        };
        Some(format!(
            "{mutation_root}{}",
            source_path.strip_prefix(&source_root)?
        ))
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
/// vocabulary) -- a binder changes none of them. The borrow leaves a binder
/// names are excluded on their own terms instead: `exact_relevant_field` skips
/// erased fields, `crosses_reference` stops every carrier hop at a reference,
/// and a reference leaf has no primitive type. Runtime generic arity is still
/// excluded by the type-parameter and owner-application tests below.
pub(super) fn plain_record(data: &typed_trees::data::DataDefinition, program: &TypedTrees) -> bool {
    data.supply_mode == language_semantics::DataSupplyMode::CheckedShape
        && program.data_type_parameters(data).is_empty()
        && retained_record_owner_application(data, program)
        && data.quotient.is_none()
        && data.where_facts.is_empty()
        && !data.zero_gated
        && typed_trees::data::DataDefinition::shape_kind_from_members(program.data_members(data))
            == DataShapeKind::Record
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
/// A carrier path is direct storage access: its segments select record fields
/// and fixed-array indices of one referent. Stepping through a reference-typed
/// field would turn a pointer hop into an inline field offset, which the store
/// vocabulary rejects -- "Cases, reference crossings and nonprimitive leaves
/// reject. This is direct storage access, not a synthetic record field or an
/// introduced reference lifetime" (wiki/spec/terminal-psi/structural_access.md,
/// Store vocabulary). `data_definition_for_field_type` deliberately peels
/// `&`/`&mut` for the entry-invariant seed, so carrier hops test this first.
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
