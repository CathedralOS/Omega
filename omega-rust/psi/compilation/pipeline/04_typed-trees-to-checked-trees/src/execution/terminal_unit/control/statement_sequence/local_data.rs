//! Planning one `let` statement of a single-state body.
//!
//! Each initializer kind has its own producer, tried in order: an erased or
//! record-pattern marker plans nothing; a mutable view local establishes its
//! descriptor home and stores its initializer there, a call's completed view
//! included; a byte literal establishes its immutable storage; an atomic placeholder or load binds
//! the atomic event; a selected boundary operator or IEEE FMA binds its
//! realization; a move out of borrowed storage opens a window; a view
//! subslice narrows an established view; a constructed structural value, a
//! closed scalar array, a mutable primitive local and a scalar computation
//! establish their binding directly. What remains is a call initializer:
//! its result binding goes back to the sequence, which plans the call.
//!
//! Reference captures precede structural construction and establish their
//! descriptor bindings. A silent place loan plans no operation; its row
//! records the place every read and store through it rejoins. Runtime scalar
//! parameters are backing roots alongside structural parameters and earlier
//! locals; their view reads retain the parameter's declared carrier for
//! ordinary representation-view replay.
//! Passing such an alias onward still requires its backing storage to be
//! established by lowering; admitting the read does not fabricate that home.
use super::{
    CheckFacts, CheckedScalarExpressionRole, CheckedStructuralAccess,
    CheckedTrivialAffineStructuralLocalPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitEntryClaimPlan, CheckedUnitScalarResultBindingPlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralResultBindingPlan, ExpressionNode,
    LocalConstructionTrace, Multiplicity, ScalarCalleeClosure, ShapeCollector, SymbolHandle,
    checked_structural_result_type, consume_value_places, is_record_pattern_marker,
    retain_selected_sources, scalar_computation_local_at, structural_operands,
};
use crate::execution::terminal_unit::initialized_locals::initialized_local;
use crate::execution::terminal_unit::types::{
    parameter_qualifications, supports_primitive_storage,
};

/// The sequence state a `let` statement reads and extends.
pub(super) struct Planner<'a, 'program, 'shapes> {
    pub(super) program: &'program dyn ::validation::CheckingInput,
    pub(super) facts: &'program CheckFacts,
    pub(super) scalar_callees: ScalarCalleeClosure,
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
    pub(super) place_loans: &'a mut Vec<checked_trees::CheckedPlaceLoanPlan>,
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
    // Relevance checking already checked the initializer as proof evidence.
    // An erased binding establishes neither storage nor an executable call.
    if local.relevance.is_erased() {
        return Some(LocalPlan::Planned);
    }
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
        place_loans,
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
        && validation::zeroed_structural_parts(program, local.type_reference)
        && !validation::type_requires_establishment(program, local.type_reference);
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
            establish_declared_home(
                program,
                machine,
                state,
                shapes,
                binders,
                operations,
                structural_count,
                structural_local_symbols,
                structural_results,
                index,
                statement_index,
                local,
            )?;
            return Some(LocalPlan::Planned);
        }
        return None;
    }
    let reference_capture =
        super::super::reference_captures::plan(program, facts, machine, state, index, local);
    let structural_root = facts.values.structural_values.root_for_expression(
        state.symbol,
        statement_index,
        local.initial_value,
    );
    if (reference_capture.is_none()
        && structural_root.is_none()
        && erased_locals.contains(&local.symbol))
        || is_record_pattern_marker(local)
    {
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
            local: initialized_local(program, machine, state, local),
        };
        *structural_count = structural_count.checked_add(1)?;
        structural_results.push((result.clone(), facts::PlaceRoot::Symbol(local.symbol)));
        structural_local_symbols.push(local.symbol);
        let qualifications =
            parameter_qualifications(program, shapes, local.type_reference, binders)?;
        operations.push(CheckedUnitEffectOperationPlan::EstablishLocal(
            checked_trees::CheckedUnitLocalBindingPlan::ByteSequence {
                result,
                value: checked_trees::CheckedByteSequenceValuePlan::literal(bytes),
                qualifications,
            },
        ));
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
            local: initialized_local(program, machine, state, local),
        };
        *structural_count = structural_count.checked_add(1)?;
        structural_results.push((result.clone(), facts::PlaceRoot::Symbol(local.symbol)));
        structural_local_symbols.push(local.symbol);
        let qualifications =
            parameter_qualifications(program, shapes, local.type_reference, binders)?;
        operations.push(CheckedUnitEffectOperationPlan::EstablishLocal(
            checked_trees::CheckedUnitLocalBindingPlan::ByteSequence {
                result,
                value,
                qualifications,
            },
        ));
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
    // A retained initializer value owns its actual descriptor and evaluation
    // schedule. Its loan backing is only custody evidence, not an alternative
    // whole-extent producer. Place-only aliases below may still form a view.
    let subslice = structural_root
        .is_none()
        .then(|| {
            crate::execution::terminal_unit::calls::view_subslice::admit(
                program,
                facts,
                machine,
                state,
                structural_parameters,
                subslice_target,
                local.initial_value,
                index,
                checked_trees::CheckedSubsliceSite::LocalBinding,
            )
            .map(|subslice| (subslice, None))
            // The `coll[a..b]` spelling declined; the whole-extent spelling
            // (`let v: &T = coll`) is the same view with both endpoints omitted.
            // Its element-declared sibling (`let v: u8 = coll`) is not a view
            // binding at all, so only a view-declared local qualifies here.
            .or_else(|| {
                declared_is_view.then(|| {
                    crate::execution::terminal_unit::calls::view_subslice::whole_extent(
                        program,
                        machine,
                        state,
                        structural_parameters,
                        subslice_target,
                        local.initial_value,
                        index,
                    )
                    .map(|(subslice, storage)| (subslice, Some(storage)))
                })?
            })
        })
        .flatten();
    if let Some((subslice, storage)) = subslice {
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
        if let Some(storage) = storage {
            // The whole-extent spelling mints no indexed collection, so
            // nothing else enrolls the backing's own structural type.
            shapes.add_type(storage, binders, &[])?;
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
            local: initialized_local(program, machine, state, local),
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
    // The retained value owns both the carrier and its evaluation schedule.
    // A place capture is only the establishment route when no value was
    // retained; selecting it first would replace a copied descriptor with a
    // reference to its storage and discard captured selector computations.
    if structural_root.is_none()
        && let Some(capture) = reference_capture
    {
        local_phase("statement sequence: local data: reference capture");
        // A recast's backing is a separate catalog root from its logical
        // reference type. Enroll the actual projected endpoint even when no
        // signature otherwise names it (for example a byte inside an array).
        shapes.add_type(capture.backing_type, binders, &[])?;
        let result = CheckedUnitStructuralResultBindingPlan {
            statement_index,
            binding_ordinal: u32::try_from(*structural_count).ok()?,
            type_identity: shapes.add_reference_type(local.type_reference, binders)?,
            multiplicity: Multiplicity::Affine,
            local: initialized_local(program, machine, state, local),
        };
        *structural_count = structural_count.checked_add(1)?;
        structural_results.push((result.clone(), facts::PlaceRoot::Symbol(local.symbol)));
        structural_local_symbols.push(local.symbol);
        operations.push(CheckedUnitEffectOperationPlan::EstablishReference {
            formation_expression: local.initial_value,
            backing_type: capture.backing_type,
            result,
            source: capture.source,
            storage: capture.storage,
        });
        return Some(LocalPlan::Planned);
    }
    local_phase("statement sequence: local data: shared place loan");
    // A retained reference root establishes its carrier before later reads.
    // Borrowed aliases without such a value root retain their existing loan
    // route; they cannot invent a carrier by replaying an initializer at a use.
    if structural_root.is_none()
        && let Some(loan) = place_loan(
            program,
            facts,
            machine,
            state,
            structural_parameters,
            statement_index,
            local,
            CheckedStructuralAccess::SharedBorrow,
        )
    {
        if let PlaceLoan::Captured(loan) = loan {
            place_loans.push(loan);
        }
        return Some(LocalPlan::Planned);
    }
    // An initializer call may establish structural argument values at this
    // same statement. Only the initializer's own occurrence can establish the
    // local directly; argument roots stay with the ordinary call planner.
    if let Some(root) = structural_root {
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
            windows,
        )?;
        local_phase("statement sequence: local data: structural value: source custody");
        if facts
            .flow
            .ownership
            .owned_selection_at(state.symbol, statement_index)
            .is_some()
        {
            retain_selected_sources(program, facts, state, statement_index, &mut *operations)?;
        } else {
            consume_value_places(facts, root.root, structural_results, &mut *operations)?;
        }
        super::consume_value_call_sources(
            facts,
            structural_results,
            &mut *operations,
            &calls,
            root.root,
        )?;
        local_phase("statement sequence: local data: structural value: type shape");
        // A stored loan retains its reference shell, unlike argument
        // presentation of its referent. Slice views are different carriers:
        // they retain their element-view descriptor, not a reference to an
        // unsized slice. Classify storage here, not function-result admission.
        let type_identity = if validation::reference_result_custody::stored_loan_parts(
            program,
            local.type_reference,
        )
        .is_some()
        {
            shapes.add_reference_type(local.type_reference, binders)?
        } else {
            shapes.add_type(local.type_reference, binders, &[])?
        };
        // A local's checked initializer judgments need the same domain/carrier
        // catalogue as signature judgments. Register their theory here even
        // when no machine parameter happens to mention those domains; this
        // does not establish membership or add runtime representation.
        parameter_qualifications(program, shapes, local.type_reference, binders)?;
        let result = CheckedUnitStructuralResultBindingPlan {
            statement_index,
            binding_ordinal: u32::try_from(*structural_count).ok()?,
            type_identity,
            // This root materializes a carrier, including a reusable shared
            // reference's lifetime resource. Borrowed-formal presentation
            // multiplicity cannot replace the producer's result custody.
            multiplicity: validation::reference_result_custody::result_multiplicity(
                program,
                local.type_reference,
            ),
            local: initialized_local(program, machine, state, local),
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
        let call_coordinate = establishment.call().map(|call| call.coordinate);
        operations.push(establishment);
        if let Some(coordinate) = call_coordinate {
            // A call-valued local owns the same normal operand continuation
            // as a statement call. The result remains with the local; only
            // the exact anonymous argument owners retire here.
            super::super::super::cleanup::anonymous::append_continuation(
                program,
                facts,
                shapes,
                machine,
                state,
                coordinate,
                structural_results,
                operations,
            )?;
        }
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
            local: initialized_local(program, machine, state, local),
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
        operations.push(CheckedUnitEffectOperationPlan::EstablishLocal(
            checked_trees::CheckedUnitLocalBindingPlan::ScalarArray {
                source: checked_trees::CheckedArrayConstructionSource::Statement,
                result,
                elements,
            },
        ));
        return Some(LocalPlan::Planned);
    }
    local_phase("statement sequence: local data: exclusive place loan");
    if let Some(loan) = place_loan(
        program,
        facts,
        machine,
        state,
        structural_parameters,
        statement_index,
        local,
        CheckedStructuralAccess::MutableBorrow,
    ) {
        if let PlaceLoan::Captured(loan) = loan {
            place_loans.push(loan);
        }
        return Some(LocalPlan::Planned);
    }
    if let Some(primitive_type) = program.primitive_type_reference(local.type_reference) {
        if local.is_mutable {
            local_phase("statement sequence: local data: mutable primitive local: storage type");
            // Storage keeps the complete declared receiving type. The same
            // eligibility applies to mutable parameters and later replacements.
            if !supports_primitive_storage(program, local.type_reference) {
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
            operations.push(CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::Primitive {
                    statement_index,
                    symbol: local.symbol,
                    type_identity: shapes.add_type(local.type_reference, binders, &[])?,
                    primitive_type,
                    value,
                },
            ));
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
            operations.push(CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::Scalar { result, value },
            ));
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
        result.local = initialized_local(program, machine, state, local);
        Some(LocalPlan::StructuralCall(result))
    }
}

/// A silent loan's admission: the place it names, or, for a recast whose
/// place the checked walk cannot name, admission alone.
enum PlaceLoan {
    Captured(checked_trees::CheckedPlaceLoanPlan),
    /// The §5b recast judgment admitted the loan, but its place carries a
    /// selector no checked coordinate names (`self.bytes[k * 2]`). No use of
    /// it can rejoin a place, so lowering refuses each one.
    Unnamed,
}

/// A `let` binding a loan of a place: `let seen: &mut u8 = &mut self.byte;`
/// or `let borrowed: &u32 = &self.size;`. The binding is immutable and a
/// reference cannot be reseated, so the name denotes one place for its whole
/// scope and carries no runtime value of its own: every read through it is a
/// read of that place, every write through an exclusive loan is the ordinary
/// store of that place, and the borrow layer owns the loan's lifetime. The
/// statement therefore plans no operation, like a record pattern's marker;
/// its row records the place lowering rejoins each use to.
///
/// The conditions are what make that rejoin exact, not a convenience. The
/// place must root at a structural parameter of this state or at an earlier
/// local of this state — the borrow ledger names the loaned storage in
/// either spelling — and its projection must bottom out in exactly the
/// declared referent: a loan of a wider or narrower leaf is not this local's
/// place. A shared loan may also alias a runtime scalar parameter; it has no
/// write authority, and the read keeps the parameter's declared carrier. A
/// runtime index stays a `RuntimeIndex` segment for the terminal's bound
/// re-proof. A `&place as &T` recast (either access) skips the referent
/// equality: the §5b judgment already proved the two shapes compatible. The
/// referent may be a record as well as a primitive.
fn place_loan(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    statement_index: u32,
    local: &typed_trees::statement::TableLocalData,
    access: CheckedStructuralAccess,
) -> Option<PlaceLoan> {
    if local.is_mutable {
        return None;
    }
    let formation = validation::reference_formation_source(program, local.initial_value)?;
    let (referee, _) = match access {
        CheckedStructuralAccess::SharedBorrow => {
            if formation.access != language_core::ReferenceAccess::Shared {
                return None;
            }
            super::super::super::reference_results::shared_place_borrowed_parts(
                program,
                local.type_reference,
            )?
        }
        CheckedStructuralAccess::MutableBorrow => {
            if formation.access != language_core::ReferenceAccess::Mutable {
                return None;
            }
            super::super::super::reference_results::mutable_borrowed_parts(
                program,
                local.type_reference,
            )?
        }
        CheckedStructuralAccess::Owned | CheckedStructuralAccess::WriteOnlyBorrow => return None,
    };
    let statement = usize::try_from(statement_index).ok()?;
    let place = validation::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement,
        formation.source,
    )?;
    let symbol = place.root.as_symbol()?;
    // The loaned storage must root at a structural parameter this state
    // already writes through, or at an earlier local of this state — the
    // same named-custody family in both spellings. A computed expression
    // root has no rejoin the ledger can name. Scalar parameters own storage
    // too, but only a shared loan may alias one: erased and const formals
    // have no runtime storage to lend.
    let parameters = program.state_parameters(state);
    let (root, root_type) =
        match typed_trees::signature::state_parameter_position(parameters, symbol) {
            Some(position) => {
                let parameter = &parameters[position];
                let root = match structural_parameters
                    .iter()
                    .position(|candidate| candidate.position as usize == position)
                {
                    Some(dense) => checked_trees::CheckedPlaceLoanRoot::StructuralParameter {
                        parameter_index: u32::try_from(dense).ok()?,
                    },
                    None if access == CheckedStructuralAccess::SharedBorrow
                        && !parameter.is_const
                        && !parameter.relevance.is_erased()
                        && program
                            .primitive_type_reference(parameter.type_reference)
                            .is_some() =>
                    {
                        checked_trees::CheckedPlaceLoanRoot::ScalarParameter {
                            position: u32::try_from(position).ok()?,
                        }
                    }
                    None => return None,
                };
                (root, parameter.type_reference)
            }
            None => {
                let (ordinal, root_local) = program.state_local_declaration(state, symbol)?;
                if ordinal >= statement {
                    return None;
                }
                (
                    checked_trees::CheckedPlaceLoanRoot::Local { symbol },
                    root_local.type_reference,
                )
            }
        };
    // The root denotes its storage through its own declared reference shells.
    let mut root_type = root_type;
    while let typed_trees::types::TypeReferenceNode::Reference { referee, .. } =
        program.type_reference_table.type_reference(root_type)
    {
        root_type = *referee;
    }
    // A recast reinterprets its place under the place's own declared type;
    // without that type the view has nothing to read beneath it.
    let recast_source = if formation.view_type.is_valid() {
        validation::declared_place_type_raw(program, machine, Some(state), formation.source)
            .filter(|source| source.is_valid())?
    } else {
        typed_trees::types::TypeReferenceHandle::default()
    };
    // The declaration owns this evaluation. Consume the same checked scalar
    // source used by ordinary value operands, including established storage.
    let projected = crate::execution::terminal_unit::calls::projected_evaluated_path(
        program,
        machine.symbol,
        state.symbol,
        statement,
        &place,
        Some(&|expression| {
            crate::execution::terminal_unit::calls::value_index_segment(
                program,
                &facts.operators,
                state,
                statement,
                expression,
            )
        }),
    );
    let path = match projected {
        Some((storage, path)) => {
            if !recast_source.is_valid()
                && program.normalized_type_identity(storage)
                    != program.normalized_type_identity(referee)
            {
                return None;
            }
            path
        }
        None if recast_source.is_valid() => return Some(PlaceLoan::Unnamed),
        None => return None,
    };
    // A mutable indexed recast's admission already proved the byte window —
    // `index + view footprint <= region length` — through
    // `interior_byte_region_source`. The same predicates the borrow ledger
    // asked decide each store's emitted obligations once, here, so lowering
    // consumes this row instead of re-walking the proof.
    let decided_byte_view_store = access == CheckedStructuralAccess::MutableBorrow
        && (validation::validate_literal_indexed_recast_footprint(
            program,
            machine,
            state,
            statement,
            local.initial_value,
            referee,
            language_core::ReferenceAccess::Mutable,
        )
        .is_some()
            || validation::validate_runtime_indexed_recast_source(
                program,
                machine,
                state,
                statement,
                local.initial_value,
                referee,
                language_core::ReferenceAccess::Mutable,
            )
            .is_some());
    Some(PlaceLoan::Captured(checked_trees::CheckedPlaceLoanPlan {
        statement_index,
        symbol: local.symbol,
        access,
        referee,
        root,
        root_type,
        path,
        recast_source,
        decided_byte_view_store,
    }))
}

/// Publish a declared local's home before any member or whole-slot store
/// establishes its contents. Borrows and writers rejoin the binding by
/// ordinal the same way established values do.
#[allow(clippy::too_many_arguments)]
fn establish_declared_home(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    shapes: &mut ShapeCollector<'_>,
    binders: &[(SymbolHandle, String)],
    operations: &mut Vec<CheckedUnitEffectOperationPlan>,
    structural_count: &mut usize,
    structural_local_symbols: &mut Vec<SymbolHandle>,
    structural_results: &mut Vec<(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)>,
    declaration_ordinal: usize,
    statement_index: u32,
    local: &typed_trees::statement::TableLocalData,
) -> Option<()> {
    let result = CheckedUnitStructuralResultBindingPlan {
        statement_index,
        binding_ordinal: u32::try_from(*structural_count).ok()?,
        type_identity: shapes.add_type(local.type_reference, binders, &[])?,
        multiplicity: validation::reference_result_custody::binding_multiplicity(
            program,
            local.type_reference,
        ),
        local: initialized_local(program, machine, state, local),
    };
    *structural_count = structural_count.checked_add(1)?;
    // A descriptor home declares the local's byte domains; every store into
    // it then proves them. Other homes keep their members' own invariants.
    let qualifications =
        if crate::execution::terminal_unit::structural_scalar_store::is_shared_view_field(
            program,
            local.type_reference,
        ) {
            parameter_qualifications(program, shapes, local.type_reference, binders)?
        } else {
            Vec::new()
        };
    operations.push(CheckedUnitEffectOperationPlan::EstablishLocal(
        checked_trees::CheckedUnitLocalBindingPlan::Structural {
            statement_index,
            declaration_ordinal: u32::try_from(declaration_ordinal).ok()?,
            symbol: local.symbol,
            result: result.clone(),
            qualifications,
            discard_result_on_return: result.multiplicity == Multiplicity::Affine,
        },
    ));
    structural_results.push((result, facts::PlaceRoot::Symbol(local.symbol)));
    structural_local_symbols.push(local.symbol);
    Some(())
}
