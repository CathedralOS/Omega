//! Planning one `let` statement of a single-state body.
//!
//! Each initializer kind has its own producer, tried in order: an erased or
//! record-pattern marker plans nothing; a byte literal establishes its immutable
//! storage; an atomic placeholder or load binds
//! the atomic event; a selected boundary operator or IEEE FMA binds its
//! realization; a move out of borrowed storage opens a window; a view
//! subslice narrows an established view; a constructed structural value, a
//! closed scalar array, a mutable primitive local and a scalar computation
//! establish their binding directly. What remains is a call initializer:
//! its result binding goes back to the sequence, which plans the call.
//!
//! Place-loan locals precede structural construction: an alias has no new
//! result home. Runtime scalar parameters are backing roots alongside
//! structural parameters and earlier locals; their view reads retain the
//! parameter's declared carrier for ordinary representation-view replay.
//! Passing such an alias onward still requires its backing storage to be
//! established by lowering; admitting the read does not fabricate that home.
use super::{
    CheckFacts, CheckedScalarExpressionRole, CheckedStructuralAccess,
    CheckedTrivialAffineStructuralLocalPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitEntryClaimPlan, CheckedUnitScalarResultBindingPlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralResultBindingPlan, ExpressionNode,
    LocalConstructionTrace, Multiplicity, ScalarCalleePlans, ShapeCollector, SymbolHandle,
    TypeReferenceNode, TypedTrees, checked_structural_result_type, consume_value_places,
    is_record_pattern_marker, retain_selected_sources, scalar_computation_local_at,
    structural_operands,
};
use crate::execution::terminal_unit::types::parameter_qualifications;

/// The sequence state a `let` statement reads and extends.
pub(super) struct Planner<'a, 'program, 'shapes> {
    pub(super) program: &'program TypedTrees,
    pub(super) facts: &'program CheckFacts,
    pub(super) scalar_callees: ScalarCalleePlans<'a>,
    pub(super) shapes: &'a mut ShapeCollector<'shapes>,
    pub(super) machine: &'program typed_trees::machine::Machine,
    pub(super) state: &'program typed_trees::state::State,
    pub(super) structural_parameters: &'a mut [CheckedUnitStructuralParameterPlan],
    pub(super) entry_claims: &'a [CheckedUnitEntryClaimPlan],
    pub(super) trivial_affine_locals:
        &'a [(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)],
    pub(super) trace: &'a LocalConstructionTrace,
    pub(super) binders: &'a [(SymbolHandle, String)],
    pub(super) erased_locals: &'a [SymbolHandle],
    pub(super) operations: &'a mut Vec<CheckedUnitEffectOperationPlan>,
    pub(super) scalar_count: &'a mut usize,
    pub(super) structural_count: &'a mut usize,
    pub(super) structural_local_symbols: &'a mut Vec<SymbolHandle>,
    pub(super) windows: &'a mut crate::execution::terminal_unit::borrowed_windows::OpenWindows,
    pub(super) array_bindings: &'a mut Vec<(SymbolHandle, CheckedUnitStructuralResultBindingPlan)>,
    pub(super) atomic_result: &'a mut Option<CheckedUnitScalarResultBindingPlan>,
    pub(super) structural_results:
        &'a mut Vec<(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)>,
}

/// What a `let` statement left for its own call.
pub(super) enum LocalPlan {
    /// Every operation the statement needs is planned.
    Planned,
    /// The initializer is a scalar call whose result binds here.
    ScalarCall(CheckedUnitScalarResultBindingPlan),
    /// The initializer is a structural call whose whole result binds the local.
    StructuralCall(CheckedUnitStructuralResultBindingPlan),
}

pub(super) fn plan(
    planner: Planner<'_, '_, '_>,
    index: usize,
    statement_index: u32,
    local: &typed_trees::statement::TableLocalData,
) -> Option<LocalPlan> {
    let Planner {
        program,
        facts,
        scalar_callees,
        shapes,
        machine,
        state,
        structural_parameters,
        entry_claims,
        trivial_affine_locals,
        trace,
        binders,
        erased_locals,
        operations,
        scalar_count,
        structural_count,
        structural_local_symbols,
        windows,
        array_bindings,
        atomic_result,
        structural_results,
    } = planner;
    // Guard-group markers beneath the local-data phase keep the
    // statement position that `phase` resets.
    let local_phase = |phase: &'static str| {
        trace.phase(phase);
        trace.statement(Some(statement_index));
    };
    local_phase("statement sequence: local data: initializer expression");
    // A local with no initializer survives when its declared type
    // is zero-constructible. Structural values have a whole-shape zeroed
    // establishment root; plain primitives have a declaration-bound scalar
    // zero initializer. Both use their ordinary storage establishment below.
    let uninit_zeroed = !program
        .expression_table
        .expression_is_valid(local.initial_value)
        && validation::zeroed_structural_parts(program, local.type_reference);
    if !program
        .expression_table
        .expression_is_valid(local.initial_value)
        && !uninit_zeroed
    {
        // An uninitialized structural local still declares its binding: a
        // later `&mut` borrow or out-parameter write is what establishes its
        // storage, and those writers rejoin against the binding row by
        // symbol. Minting the row here is the only producer for locals that
        // are declared unassigned — skipping it leaves the whole machine's
        // unit plan omitted even though every writer plans elsewhere.
        if !erased_locals.contains(&local.symbol)
            && program
                .primitive_type_reference(local.type_reference)
                .is_none()
        {
            local_phase("statement sequence: local data: deferred structural binding");
            let result = CheckedUnitStructuralResultBindingPlan {
                statement_index,
                binding_ordinal: u32::try_from(*structural_count).ok()?,
                type_identity: shapes.add_type(local.type_reference, binders, &[])?,
                multiplicity: program.type_multiplicity(local.type_reference),
            };
            *structural_count = structural_count.checked_add(1)?;
            // The declaration supplies the home each member operation
            // establishes into; publishing its binding here lets borrows and
            // writers rejoin by ordinal the same way established values do.
            operations.push(CheckedUnitEffectOperationPlan::EstablishStructuralLocal {
                statement_index,
                declaration_ordinal: u32::try_from(index).ok()?,
                symbol: local.symbol,
                result: result.clone(),
                discard_result_on_return: result.multiplicity == Multiplicity::Affine,
            });
            structural_results.push((result, facts::PlaceRoot::Symbol(local.symbol)));
            structural_local_symbols.push(local.symbol);
            return Some(LocalPlan::Planned);
        }
        return None;
    }
    if erased_locals.contains(&local.symbol) || is_record_pattern_marker(local) {
        return Some(LocalPlan::Planned);
    }
    if !local.is_mutable
        && let Some(argument) =
            crate::execution::terminal_unit::calls::byte_sequence_literal_argument(
                program,
                local.type_reference,
                local.initial_value,
            )
    {
        local_phase("statement sequence: local data: byte literal");
        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { bytes } =
            argument.source
        else {
            return None;
        };
        let type_identity = shapes.add_type(local.type_reference, binders, &[])?;
        if type_identity != argument.type_identity
            || program.type_multiplicity(local.type_reference) != Multiplicity::Unrestricted
        {
            return None;
        }
        let result = CheckedUnitStructuralResultBindingPlan {
            statement_index,
            binding_ordinal: u32::try_from(*structural_count).ok()?,
            type_identity,
            multiplicity: Multiplicity::Unrestricted,
        };
        *structural_count = structural_count.checked_add(1)?;
        structural_results.push((result.clone(), facts::PlaceRoot::Symbol(local.symbol)));
        structural_local_symbols.push(local.symbol);
        let qualifications =
            parameter_qualifications(program, shapes, local.type_reference, binders)?;
        operations.push(CheckedUnitEffectOperationPlan::EstablishByteSequence {
            result,
            value: checked_trees::CheckedByteSequenceValuePlan::literal(bytes),
            qualifications,
        });
        return Some(LocalPlan::Planned);
    }
    // A bounded owner snapshots its initializer's bytes and live length.
    if let Some(value) = super::bounded_byte_values::value_plan(
        program,
        &facts.operators,
        state,
        statement_index,
        local,
    ) {
        local_phase("statement sequence: local data: bounded byte literal");
        let result = CheckedUnitStructuralResultBindingPlan {
            statement_index,
            binding_ordinal: u32::try_from(*structural_count).ok()?,
            type_identity: shapes.add_type(local.type_reference, binders, &[])?,
            multiplicity: Multiplicity::Unrestricted,
        };
        *structural_count = structural_count.checked_add(1)?;
        structural_results.push((result.clone(), facts::PlaceRoot::Symbol(local.symbol)));
        structural_local_symbols.push(local.symbol);
        let qualifications =
            parameter_qualifications(program, shapes, local.type_reference, binders)?;
        operations.push(CheckedUnitEffectOperationPlan::EstablishByteSequence {
            result,
            value,
            qualifications,
        });
        return Some(LocalPlan::Planned);
    }
    // An atomic carrier's result placeholder plans nothing: it
    // reserves the dense binding the next statement's event binds
    // to the observed prior. A load local plans its event here.
    if let Some(primitive_type) = program.primitive_type_reference(local.type_reference)
        && super::super::super::atomic_operations::result_placeholder(program, state, index, local)
    {
        local_phase("statement sequence: local data: atomic result placeholder");
        *atomic_result = Some(CheckedUnitScalarResultBindingPlan {
            statement_index,
            binding_ordinal: u32::try_from(*scalar_count).ok()?,
            primitive_type,
        });
        *scalar_count = scalar_count.checked_add(1)?;
        return Some(LocalPlan::Planned);
    }
    if validation::atomic_load_carrier(program, local.initial_value).is_some() {
        local_phase("statement sequence: local data: atomic load");
        let binding_ordinal = u32::try_from(*scalar_count).ok()?;
        operations.push(CheckedUnitEffectOperationPlan::AtomicAccess(
            super::super::super::atomic_operations::load_event(
                program,
                facts,
                machine,
                state,
                structural_parameters,
                statement_index,
                local,
                binding_ordinal,
                trace,
            )?,
        ));
        *scalar_count = scalar_count.checked_add(1)?;
        return Some(LocalPlan::Planned);
    }
    // A consuming move out of exclusive borrowed storage opens a
    // window the body repairs later; the local binds the moved
    // value like any other structural binding.
    local_phase("statement sequence: local data: borrowed window move");
    if let Some((operation, result)) = windows.move_out(
        program,
        facts,
        shapes,
        machine,
        state,
        structural_parameters,
        binders,
        statement_index,
        local,
        u32::try_from(*structural_count).ok()?,
    ) {
        *structural_count = structural_count.checked_add(1)?;
        structural_results.push((result, facts::PlaceRoot::Symbol(local.symbol)));
        structural_local_symbols.push(local.symbol);
        operations.push(operation);
        return Some(LocalPlan::Planned);
    }
    // A `let` narrowing an established view binds the range the
    // argument lane admits for a call operand: one admission, one
    // source vocabulary, keyed at this statement's binding site.
    // The result retains its checked access in the structural namespace, so
    // edges, calls and observations read it as any view local.
    local_phase("statement sequence: local data: view subslice");
    // The declaration names the narrowed view (`let v: &[u8]`) or only
    // its element (`let v: u8 = main.bytes[0..2]`): the binding's target
    // is the subslice's own result type in both spellings, and the
    // element spelling must agree with the view's element below.
    let declared_is_view = crate::execution::terminal_unit::calls::view_subslice::view_kind(
        program,
        local.type_reference,
    )
    .is_some()
        || crate::execution::terminal_unit::types::borrowed_slice_view_element(
            program,
            local.type_reference,
            &[],
        )
        .is_some();
    let subslice_target = if declared_is_view {
        local.type_reference
    } else {
        validation::expression_result_type_reference(program, machine, state, local.initial_value)
            .filter(|result| validation::slice_view_shell(program, *result, &[]).is_some())
            .unwrap_or(local.type_reference)
    };
    if let Some(subslice) = crate::execution::terminal_unit::calls::view_subslice::admit(
        program,
        facts,
        machine,
        state,
        structural_parameters,
        subslice_target,
        local.initial_value,
        index,
        checked_trees::CheckedSubsliceSite::LocalBinding,
    ) {
        if let checked_trees::CheckedStorageRoot::ViewLocal { symbol } = subslice.range.root
            && !structural_local_symbols.contains(&symbol)
        {
            return None;
        }
        let type_identity = shapes.add_type(subslice_target, binders, &[])?;
        if type_identity != subslice.range.type_identity {
            return None;
        }
        if !declared_is_view
            && program.normalized_type_identity(local.type_reference)
                != validation::slice_view_shell(program, subslice_target, &[])
                    .map(|view| program.normalized_type_identity(view.element))?
        {
            return None;
        }
        let result = CheckedUnitStructuralResultBindingPlan {
            statement_index,
            binding_ordinal: u32::try_from(*structural_count).ok()?,
            type_identity,
            // The local holds the exclusive loan once, as ordinary mutable
            // structural results do; parameter reference shells are separate.
            multiplicity: match subslice.range.access {
                CheckedStructuralAccess::MutableBorrow => Multiplicity::Affine,
                _ => Multiplicity::Unrestricted,
            },
        };
        *structural_count = structural_count.checked_add(1)?;
        structural_results.push((result.clone(), facts::PlaceRoot::Symbol(local.symbol)));
        structural_local_symbols.push(local.symbol);
        operations.push(CheckedUnitEffectOperationPlan::EstablishViewSubslice {
            result,
            source: subslice.argument(),
        });
        return Some(LocalPlan::Planned);
    }
    if let Some(source) =
        validation::reference_result_custody::primitive_local_source(program, state, index)
    {
        local_phase("statement sequence: local data: primitive reference");
        let storage = match source.source {
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::ScalarParameter {
                parameter_index,
            } => {
                program
                    .state_parameters(state)
                    .iter()
                    .filter(|parameter| {
                        !parameter.is_const
                            && !parameter.relevance.is_erased()
                            && program
                                .primitive_type_reference(parameter.type_reference)
                                .is_some()
                    })
                    .nth(parameter_index as usize)?
                    .type_reference
            }
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol } => {
                program
                    .state_local_declaration(state, symbol)?
                    .1
                    .type_reference
            }
            _ => return None,
        };
        if shapes.add_type(storage, binders, &[])? != source.type_identity {
            return None;
        }
        let result = CheckedUnitStructuralResultBindingPlan {
            statement_index,
            binding_ordinal: u32::try_from(*structural_count).ok()?,
            type_identity: shapes.add_reference_type(local.type_reference, binders)?,
            multiplicity: Multiplicity::Affine,
        };
        *structural_count = structural_count.checked_add(1)?;
        structural_results.push((result.clone(), facts::PlaceRoot::Symbol(local.symbol)));
        structural_local_symbols.push(local.symbol);
        operations.push(CheckedUnitEffectOperationPlan::EstablishReference { result, source });
        return Some(LocalPlan::Planned);
    }
    local_phase("statement sequence: local data: shared place loan");
    // The shared `&` loan arms before the structural-value root lookup: a
    // shared borrow registers a value root upstream (exclusive `&mut`
    // borrows do not), so a `&`-typed local would enter the block below and
    // die at its root-identity gate before the loan arm could admit it. A
    // whole record borrow whose root passes that gate (`let view: &Payload =
    // &a;`) keeps its established `Reference` value instead: a call can
    // forward that value as a structural local, while a loan carrier plans
    // nothing a call operand could name.
    if !established_record_borrow(
        program,
        facts,
        machine.symbol,
        state,
        statement_index,
        local,
    ) && is_shared_place_loan(
        program,
        machine.symbol,
        state,
        structural_parameters,
        statement_index,
        local,
    ) {
        return Some(LocalPlan::Planned);
    }
    // An initializer call may establish structural argument values at this
    // same statement. Only the initializer's own occurrence can establish the
    // local directly; argument roots stay with the ordinary call planner.
    if let Some(root) = facts.values.structural_values.root_for_expression(
        state.symbol,
        statement_index,
        local.initial_value,
    ) {
        local_phase("statement sequence: local data: structural value: reference record loans");
        if super::super::super::reference_results::local_owes_record_loans(
            program,
            local.type_reference,
        ) {
            super::super::super::reference_results::local_record_loans(
                program,
                facts,
                machine.symbol,
                state,
                statement_index,
            )?;
        }
        local_phase("statement sequence: local data: structural value: root identity");
        if root.machine != machine.symbol
            || root.expression != local.initial_value
            || root.type_reference != local.type_reference
        {
            return None;
        }
        local_phase("statement sequence: local data: structural value: value calls");
        let calls = structural_operands::value_calls(
            program,
            facts,
            scalar_callees,
            shapes,
            machine,
            state,
            structural_parameters,
            trivial_affine_locals,
            entry_claims,
            structural_results,
            &*operations,
            &mut *structural_count,
            root.root,
            trace,
        )?;
        local_phase("statement sequence: local data: structural value: source custody");
        if facts
            .flow
            .ownership
            .owned_selection_at(state.symbol, statement_index)
            .is_some()
        {
            retain_selected_sources(
                facts,
                state.symbol,
                statement_index,
                structural_results,
                &mut *operations,
            )?;
        } else {
            consume_value_places(facts, root.root, structural_results, &mut *operations)?;
        }
        super::consume_value_call_sources(facts, structural_results, &mut *operations, &calls)?;
        local_phase("statement sequence: local data: structural value: type shape");
        let result = CheckedUnitStructuralResultBindingPlan {
            statement_index,
            binding_ordinal: u32::try_from(*structural_count).ok()?,
            type_identity: shapes.add_type(local.type_reference, binders, &[])?,
            // A `&mut`-view local's binding is affine loan custody of the
            // referent's storage even though the borrow shell itself spells
            // `Unrestricted` — its one holder is the loan, and naming it a
            // second time must dispose it.
            multiplicity: if super::super::super::reference_results::mutable_borrowed_parts(
                program,
                local.type_reference,
            )
            .is_some()
            {
                Multiplicity::Affine
            } else {
                program.type_multiplicity(local.type_reference)
            },
        };
        *structural_count = structural_count.checked_add(1)?;
        structural_results.push((result.clone(), facts::PlaceRoot::Symbol(local.symbol)));
        structural_local_symbols.push(local.symbol);
        // Only affine values create disposal debt; copy locals
        // retain their result identity without a cleanup action.
        let discard_result_on_return = result.multiplicity == Multiplicity::Affine;
        let establishment = structural_operands::establish_value(
            facts,
            result,
            root.root,
            calls,
            None,
            discard_result_on_return,
            operations,
        )?;
        operations.push(establishment);
        return Some(LocalPlan::Planned);
    }
    if validation::is_closed_primitive_array_type(program, local.type_reference)
        && !matches!(
            program.expression_table.expression(local.initial_value),
            ExpressionNode::Call(_)
        )
    {
        // Construction establishes owned storage once. The binding's later
        // writes and loans retain their own checked place/access obligations;
        // initializer evaluation does not require an immutable local.
        let result = CheckedUnitStructuralResultBindingPlan {
            statement_index,
            binding_ordinal: u32::try_from(*structural_count).ok()?,
            type_identity: shapes.add_type(local.type_reference, binders, &[])?,
            multiplicity: Multiplicity::Unrestricted,
        };
        local_phase("statement sequence: local data: scalar array: elements");
        let elements = super::super::scalar_arrays::elements(
            program,
            facts,
            machine.symbol,
            state.symbol,
            statement_index,
            checked_trees::CheckedArrayConstructionSource::Statement,
            local.initial_value,
            local.type_reference,
        )?;
        *structural_count = structural_count.checked_add(1)?;
        array_bindings.push((local.symbol, result.clone()));
        structural_results.push((result.clone(), facts::PlaceRoot::Symbol(local.symbol)));
        structural_local_symbols.push(local.symbol);
        operations.push(CheckedUnitEffectOperationPlan::EstablishScalarArray {
            source: checked_trees::CheckedArrayConstructionSource::Statement,
            result,
            elements,
        });
        return Some(LocalPlan::Planned);
    }
    // A runtime-indexed dispatch subject is erased: each arm observes the
    // borrowed element itself (`case_observation_alias`).
    local_phase("statement sequence: local data: erased dispatch subject");
    if crate::execution::terminal_unit::control::case_observation_alias::erased_subject(
        program, state, index, local,
    )
    .is_some()
    {
        return Some(LocalPlan::Planned);
    }
    local_phase("statement sequence: local data: exclusive place loan");
    if is_exclusive_place_loan(
        program,
        machine.symbol,
        state,
        structural_parameters,
        statement_index,
        local,
    ) {
        return Some(LocalPlan::Planned);
    }
    local_phase("statement sequence: local data: view loan binding");
    if is_call_view_loan_binding(program, facts, machine, state, statement_index, local) {
        return Some(LocalPlan::Planned);
    }
    if let Some(primitive_type) = program.primitive_type_reference(local.type_reference) {
        if local.is_mutable {
            local_phase("statement sequence: local data: mutable primitive local: storage type");
            // Arithmetic policy qualifies operations, not storage. Preserve the
            // exact type below; other qualifications still need their own evidence.
            if !matches!(
                program
                    .type_reference_table
                    .type_reference(local.type_reference),
                TypeReferenceNode::Named { .. }
            ) && !validation::is_arithmetic_policy_only_integer(program, local.type_reference)
            {
                return None;
            }
            // A call or ordered selection initializes storage through its
            // retained computation, exactly as a pure value does: the local
            // is still established at its own declaration.
            let value = super::storage_initializer::value(
                program,
                facts,
                machine.symbol,
                state,
                statement_index,
                local,
                primitive_type,
                trace,
            )?;
            operations.push(CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal {
                statement_index,
                symbol: local.symbol,
                type_identity: shapes.add_type(local.type_reference, binders, &[])?,
                primitive_type,
                value,
            });
            return Some(LocalPlan::Planned);
        }
        // The binding takes the next dense ordinal, which also counts the
        // call results of preceding field stores. Its source value is keyed
        // by the authored local ordinal instead (`immutable_scalar_locals_before`),
        // so look the source up by that and keep the dense ordinal only on
        // the result.
        let binding_ordinal = u32::try_from(*scalar_count).ok()?;
        *scalar_count = scalar_count.checked_add(1)?;
        let source_ordinal = u32::try_from(
            program.immutable_scalar_locals_before(state, usize::try_from(statement_index).ok()?),
        )
        .ok()?;
        if !matches!(
            program.expression_table.expression(local.initial_value),
            ExpressionNode::Call(_)
        ) || facts
            .values
            .scalar_computations
            .root_at(
                state.symbol,
                statement_index,
                CheckedScalarExpressionRole::LocalInitializer {
                    binding_ordinal: source_ordinal,
                },
            )
            .is_some()
        {
            let (mut result, value) = scalar_computation_local_at(
                program,
                facts,
                machine.symbol,
                state,
                statement_index,
                source_ordinal,
                local,
                trace,
            )?;
            result.binding_ordinal = binding_ordinal;
            operations.push(CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, value });
            return Some(LocalPlan::Planned);
        }
        local_phase("statement sequence: local data: scalar call binding");
        Some(LocalPlan::ScalarCall(CheckedUnitScalarResultBindingPlan {
            statement_index,
            binding_ordinal,
            primitive_type,
        }))
    } else {
        // A structural local that is not a constructed value binds
        // the whole result of the call its initializer performs;
        // any other initializer has no producer here. The binding
        // is then admitted or refused by the result type's Unit
        // shape, which is the same classifier a discarded
        // structural call answers to.
        local_phase("statement sequence: local data: structural call binding");
        if !matches!(
            program.expression_table.expression(local.initial_value),
            ExpressionNode::Call(_)
        ) {
            return None;
        }
        local_phase("statement sequence: local data: structural result shape");
        let mut result =
            checked_structural_result_type(program, shapes, local.type_reference, binders)?;
        result.statement_index = statement_index;
        result.binding_ordinal = u32::try_from(*structural_count).ok()?;
        Some(LocalPlan::StructuralCall(result))
    }
}

/// A `let` binding an exclusive loan of a place: `let seen: &mut u8 = &mut
/// self.byte;`. The binding is immutable and a reference cannot be reseated,
/// so the name denotes one place for its whole scope and carries no runtime
/// value of its own: every write through it is the ordinary store of that
/// place, rejoined by `structural_scalar_store::destination`. The statement
/// therefore plans no operation, like a record pattern's marker, and
/// `composed_control`'s body count recognizes it as the same kind of
/// compile-time carrier.
///
/// The conditions are what make that rejoin exact, not a convenience. The
/// place must root at a structural parameter this state already writes
/// through, or at an earlier local of this state — the borrow ledger names
/// the loaned storage in either spelling — and its projection must bottom
/// out in exactly the declared referent: a loan of a wider or narrower leaf
/// is not this local's place. A runtime index stays a `RuntimeIndex` segment
/// for the terminal's bound re-proof, and a `&mut ... as &mut T` recast skips
/// the referent equality entirely — the §5b judgment already proved the two
/// shapes compatible. Only an exclusive `&mut` loan is admitted. A shared
/// loan carries no write authority and its reads are not rejoined here; the
/// referent may be a record as well as a primitive, because the projection
/// check below pins the loaned storage either way.
fn is_exclusive_place_loan(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    statement_index: u32,
    local: &typed_trees::statement::TableLocalData,
) -> bool {
    exclusive_place_loan_referent(
        program,
        machine,
        state,
        structural_parameters,
        statement_index,
        local,
    )
    .is_some()
}

fn exclusive_place_loan_referent(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    statement_index: u32,
    local: &typed_trees::statement::TableLocalData,
) -> Option<()> {
    let (target, recast) = match program.expression_table.expression(local.initial_value) {
        ExpressionNode::Borrow(borrow)
            if borrow.access == language_core::ReferenceAccess::Mutable =>
        {
            match program.expression_table.expression(borrow.target) {
                // A §5b recast sits inside its borrow: `&mut p as &mut T`
                // lowers to a mutable borrow of the cast that re-views p's
                // bytes as T. The recast judgment already proved fact
                // implication both ways, so the arm admits the operand
                // place without demanding its element type equal the
                // declared referent.
                ExpressionNode::Cast(cast)
                    if cast.form == language_core::cast_form::CastForm::RecastMutable =>
                {
                    (cast.value, true)
                }
                _ => (borrow.target, false),
            }
        }
        _ => return None,
    };
    let (referee, access) = super::super::super::reference_results::mutable_borrowed_parts(
        program,
        local.type_reference,
    )?;
    if access != CheckedStructuralAccess::MutableBorrow || local.is_mutable {
        return None;
    }
    let statement = usize::try_from(statement_index).ok()?;
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement,
        target,
    )?;
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return None;
    };
    // The loaned storage must root at a structural parameter this state
    // already writes through, or at an earlier local of this state — the
    // same named-custody family in both spellings. A computed expression
    // root has no rejoin the ledger can name.
    let parameter_root = program
        .state_parameters(state)
        .iter()
        .position(|parameter| parameter.symbol == symbol)
        .is_some_and(|position| {
            structural_parameters
                .iter()
                .any(|parameter| parameter.position as usize == position)
        });
    let local_root = program
        .state_local_declaration(state, symbol)
        .is_some_and(|(ordinal, _)| ordinal < statement);
    if !parameter_root && !local_root {
        return None;
    }
    if recast {
        // The §5b judgment already proved operand and referent compatible,
        // and the local plans no operation: the checked segment walk only
        // serves the non-recast referent comparison below, so a selector it
        // cannot resolve no longer fences the loan.
        return Some(());
    }
    let (storage, _) = crate::execution::terminal_unit::calls::projected_borrowed_path(
        program,
        machine,
        state.symbol,
        statement,
        &place,
    )?;
    (program.normalized_type_identity(storage) == program.normalized_type_identity(referee))
        .then_some(())
}

/// A `let` binding a shared loan of a place: `let borrowed: &u32 =
/// &self.size;`. Like the exclusive carrier above, the immutable binding
/// denotes one place for its whole scope and plans no operation: every read
/// through it is a read of that place, and the borrow layer owns the loan's
/// lifetime. The conditions mirror the exclusive ones — the place roots at
/// a structural parameter or an earlier local and its projection bottoms out
/// in exactly the declared referent, with the same recast peel and runtime
/// index admission — minus write authority: the loan is `&`, never `&mut`,
/// and the referent may still be a record or a primitive.
fn is_shared_place_loan(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    statement_index: u32,
    local: &typed_trees::statement::TableLocalData,
) -> bool {
    shared_place_loan_referent(
        program,
        machine,
        state,
        structural_parameters,
        statement_index,
        local,
    )
    .is_some()
}

fn shared_place_loan_referent(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    statement_index: u32,
    local: &typed_trees::statement::TableLocalData,
) -> Option<()> {
    let (target, recast) = match program.expression_table.expression(local.initial_value) {
        ExpressionNode::Borrow(borrow)
            if borrow.access == language_core::ReferenceAccess::Shared =>
        {
            match program.expression_table.expression(borrow.target) {
                // The shared recast is the same nested shape as the
                // exclusive one: `&p as &T` loans p's storage spelled as
                // T under read-only custody.
                ExpressionNode::Cast(cast)
                    if cast.form == language_core::cast_form::CastForm::RecastShared =>
                {
                    (cast.value, true)
                }
                _ => (borrow.target, false),
            }
        }
        _ => return None,
    };
    let (referee, access) = super::super::super::reference_results::shared_place_borrowed_parts(
        program,
        local.type_reference,
    )?;
    if access != CheckedStructuralAccess::SharedBorrow || local.is_mutable {
        return None;
    }
    let statement = usize::try_from(statement_index).ok()?;
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement,
        target,
    )?;
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return None;
    };
    let parameter_root = program
        .state_parameters(state)
        .iter()
        .position(|parameter| parameter.symbol == symbol)
        .is_some_and(|position| {
            structural_parameters
                .iter()
                .any(|parameter| parameter.position as usize == position)
                // Scalar parameters own storage too. The shared local retains
                // a checked alias of that exact parameter, rather than a new
                // structural result; its reads preserve the source carrier
                // through the ordinary representation-view operation. Erased
                // and const formals have no runtime storage to lend.
                || {
                    let parameter = &program.state_parameters(state)[position];
                    !parameter.is_const
                        && !parameter.relevance.is_erased()
                        && program
                            .primitive_type_reference(parameter.type_reference)
                            .is_some()
                }
        });
    let local_root = program
        .state_local_declaration(state, symbol)
        .is_some_and(|(ordinal, _)| ordinal < statement);
    if !parameter_root && !local_root {
        return None;
    }
    if recast {
        // The §5b judgment already proved operand and referent compatible,
        // and the local plans no operation: the checked segment walk only
        // serves the non-recast referent comparison below, so a selector it
        // cannot resolve no longer fences the loan.
        return Some(());
    }
    let (storage, _) = crate::execution::terminal_unit::calls::projected_borrowed_path(
        program,
        machine,
        state.symbol,
        statement,
        &place,
    )?;
    (program.normalized_type_identity(storage) == program.normalized_type_identity(referee))
        .then_some(())
}

/// A `&record` initializer whose structural-value root names exactly this
/// local's initializer and declared type: the root block establishes it as
/// a `Reference` value, as it does for a selection arm's borrow. A primitive
/// referent stays a loan carrier, whose reads rejoin the loaned place; the
/// state graph has no custody accounting for its `Reference` value.
fn established_record_borrow(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    local: &typed_trees::statement::TableLocalData,
) -> bool {
    super::super::super::reference_results::shared_place_borrowed_parts(
        program,
        local.type_reference,
    )
    .is_some_and(|(referee, _)| program.primitive_type_reference(referee).is_none())
        && facts
            .values
            .structural_values
            .root_for_expression(state.symbol, statement_index, local.initial_value)
            .is_some_and(|root| {
                root.machine == machine
                    && root.expression == local.initial_value
                    && root.type_reference == local.type_reference
            })
}

/// A `let` binding a borrowed-view result of a call that left no flow call
/// row: `let slots: &mut [Cell] = self.cells.as_mut_slice()` or
/// `let view: &[u8] = self.buf.as_slice()`. Collection-view openers mint the
/// binding's loan directly into the borrow ledger — the name then denotes
/// that loaned place for its whole scope, the same compile-time carrier as a
/// `&place`/`&mut place` borrow local, so the statement plans no operation.
/// Calls that did leave a flow row are ordinary call results and still plan
/// a call with a structural result.
fn is_call_view_loan_binding(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: u32,
    local: &typed_trees::statement::TableLocalData,
) -> bool {
    if local.is_mutable
        || !matches!(
            program.expression_table.expression(local.initial_value),
            ExpressionNode::Call(_)
        )
    {
        return false;
    }
    let expected_loan_kind = if super::super::super::reference_results::mutable_borrowed_parts(
        program,
        local.type_reference,
    )
    .is_some()
    {
        checked_trees::BorrowAccessKind::Mutable
    } else if super::super::super::reference_results::shared_place_borrowed_parts(
        program,
        local.type_reference,
    )
    .is_some()
    {
        checked_trees::BorrowAccessKind::Read
    } else {
        return false;
    };
    let index = usize::try_from(statement_index).ok();
    let Some(index) = index else { return false };
    let flow =
        crate::execution::terminal_unit::types::state_flow(facts, machine.symbol, state.symbol);
    let has_call_row = flow.is_some_and(|flow| {
        facts
            .flow
            .control
            .calls
            .span_or_empty(flow.calls)
            .iter()
            .any(|call| call.statement_index == index && call.call_ordinal == 0)
    });
    if has_call_row {
        return false;
    }
    let Some(borrow_state) = facts
        .borrow
        .states
        .iter()
        .map(|(_, candidate)| candidate)
        .find(|candidate| {
            candidate.machine_symbol == machine.symbol && candidate.state_symbol == state.symbol
        })
    else {
        return false;
    };
    let mut loans = facts.borrow.loans.iter().filter(|(handle, loan)| {
        facts.borrow.state_owns_loan(borrow_state, *handle)
            && loan.owner_symbol == local.symbol
            && loan.statement_index == index
            && loan.kind == expected_loan_kind
            && facts.borrow.loan_owner_path(loan).is_empty()
    });
    loans.next().is_some() && loans.next().is_none()
}
