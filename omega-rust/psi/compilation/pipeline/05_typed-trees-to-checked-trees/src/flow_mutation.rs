/*
    Storage places a statement or call writes, as `CanonicalPlace`s:
    `statement_storage_writes`, `call_mutated_places` (which falls back to the
    declared signature ceiling in `ceiling`), and `frame_storage_writes`. When a
    statement's or call's write set is `None`, flow drops every active context.
*/

use validation::CallSite;
use validation::CanonicalPlace;
use validation::call_site_argument_expressions;
use validation::expression_root_symbol;
use validation::find_call_site;

use crate::flow::canonical_place_from_symbol;
use crate::flow::canonical_place_segments_may_overlap;
use crate::published::places::place_at;

use checked_trees::expression::ExpressionNode;
use checked_trees::statement::StatementNode;
use checked_trees::{BorrowCallFact, BorrowFacts};
use symbols::SymbolHandle;
mod ceiling {
    /*
        The declared-signature ceiling of a call whose write frame is unknown.

        An opaque, unresolved, generic or dispatched callee, or an actual whose
        storage origin the frame resolver cannot summarize, retains "the
        signature/authority ceiling", never an empty write set and never a guess
        narrower than the declaration (wiki/spec/language/dependent_values.md,
        "Mutation frames"). The ceiling includes exclusive referents and separately
        owned capability content. A reference-free descriptor can still carry a
        resident owner; copying its geometry does not isolate that content.
        Unknown capability content retires observations without authorizing any
        declared-storage replacement. Exact loan origins remain necessary to
        restore a caller's replaced ownership homes at normal completion.
    */
    use super::local_origins::rebase_local_write_regions;
    use super::local_origins::rebase_storage_region;
    use super::local_origins::write_source_region;
    use super::summary::WriteDestination;
    use super::summary::captured_argument_write_regions;
    use crate::flow::{expressions, literal_expression_projections, shared_call_frames_or};
    use crate::published::places::place_at;
    use crate::shared::collections::{UniqueRoster, push_unique};
    use facts::PlaceRoot;
    use facts::PlaceSegment;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::machine::Machine;
    use typed_trees::state::State;
    use validation::expression_owners::expression_contains;
    use validation::field_domains::declared_owned_field_obligations;
    use validation::{
        CallFrameResolver, CallSite, CanonicalPlace, ContentsRequirement,
        ResidentOwnerQualifications, call_site_argument_expressions,
        canonical_place_type_reference, find_call_site, has_contents,
        project_type_reference_from_segments, reference_shell, resident_owner_qualifications,
    };

    use super::StorageRegion;
    use checked_trees::{
        BorrowCallFact, BorrowFacts, BorrowLoanLineage, BorrowLoanOwnerSegment as OwnerSegment,
    };
    use symbols::SymbolHandle;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    pub(crate) fn signature_ceiling_places(
        program: &dyn ::validation::CheckingInput,
        caller_machine_symbol: SymbolHandle,
        caller_state_symbol: SymbolHandle,
        borrow: Option<&BorrowFacts>,
        borrow_call: &BorrowCallFact,
        call_frames: Option<&CallFrameResolver<'_>>,
    ) -> Option<Vec<CanonicalPlace>> {
        StorageRegion::declared_places(signature_ceiling_regions(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow,
            borrow_call,
            call_frames,
        )?)
    }

    pub(crate) fn signature_ceiling_regions(
        program: &dyn ::validation::CheckingInput,
        caller_machine_symbol: SymbolHandle,
        caller_state_symbol: SymbolHandle,
        borrow: Option<&BorrowFacts>,
        borrow_call: &BorrowCallFact,
        call_frames: Option<&CallFrameResolver<'_>>,
    ) -> Option<Vec<StorageRegion>> {
        selected_signature_ceiling_regions(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow,
            borrow_call,
            call_frames,
            |_| true,
        )
    }

    /*
        Claim restoration concerns replaced caller values, not claims transferred
        with an owned argument. A writable target certified as plain unrestricted
        storage has no owned claim or descriptor to restore. This is a sufficient
        certificate, not an inference from an empty or unresolved claim frontier.
        Ordinary mutation invalidation still uses the full signature ceiling.
    */
    pub(crate) fn linear_restoration_ceiling_regions(
        program: &dyn ::validation::CheckingInput,
        caller_machine_symbol: SymbolHandle,
        caller_state_symbol: SymbolHandle,
        borrow: Option<&BorrowFacts>,
        borrow_call: &BorrowCallFact,
        call_frames: Option<&CallFrameResolver<'_>>,
    ) -> Option<Vec<StorageRegion>> {
        selected_signature_ceiling_regions(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow,
            borrow_call,
            call_frames,
            |reference| {
                let Some(paths) =
                    validation::borrow_contracts::carried_reference_paths(program, reference)
                else {
                    return true;
                };
                paths.into_iter().any(|(path, access)| {
                    if !access.is_exclusive() {
                        return false;
                    }
                    let path = crate::borrow::owner_place_segments(&path);
                    let Some(leaf) =
                        project_type_reference_from_segments(program, reference, &path)
                            .and_then(|leaf| reference_shell(program, leaf))
                    else {
                        return true;
                    };
                    !validation::has_no_owned_cleanup_obligations(program, leaf.referee)
                })
            },
        )
    }

    fn selected_signature_ceiling_regions(
        program: &dyn ::validation::CheckingInput,
        caller_machine_symbol: SymbolHandle,
        caller_state_symbol: SymbolHandle,
        borrow: Option<&BorrowFacts>,
        borrow_call: &BorrowCallFact,
        call_frames: Option<&CallFrameResolver<'_>>,
        include_parameter: impl Fn(TypeReferenceHandle) -> bool,
    ) -> Option<Vec<StorageRegion>> {
        let machine = program.machine_by_symbol(caller_machine_symbol)?;
        let state = program.state_by_symbol(caller_state_symbol)?;
        if program
            .symbols
            .builtin_function_for_symbol(borrow_call.target_symbol)
            .is_some()
        {
            return Some(Vec::new());
        }
        let site = find_call_site(
            program,
            machine.symbol,
            state.symbol,
            borrow_call.statement_index,
            borrow_call.call_ordinal,
        )?;
        // A checked requirement citation observes its subjects without executing.
        // Reuse the same complete-contract judgment as ownership and erasure:
        // an abstract subject is not a writable runtime argument to this call.
        if let CallSite::Statement(call) = site
            && validation::is_fact_only_requirement_citation(program, machine.symbol, call)
        {
            return Some(Vec::new());
        }
        let parameters = site.parameters(program, machine.symbol)?;
        let arguments = call_site_argument_expressions(program, &site);
        if parameters.len() != arguments.len() {
            return None;
        }
        let mut owned_frames = None;
        let resolver = shared_call_frames_or(call_frames, program, &mut owned_frames);
        let mut places = Vec::new();
        let mut argument_index = 0usize;
        for parameter in parameters {
            let actual = {
                let argument = arguments[argument_index];
                argument_index += 1;
                if !include_parameter(parameter.type_reference) {
                    continue;
                }
                let residents = resident_parameter_regions(
                    program,
                    machine,
                    state,
                    borrow_call,
                    &site,
                    argument_index - 1,
                    parameter.type_reference,
                    resolver,
                );
                for region in residents? {
                    push_unique(&mut places, region);
                }
                if !is_exclusive_reference(program, parameter.type_reference) {
                    if resolver.is_none_or(|resolver| {
                        resolver.local_requires_write_origin(parameter.type_reference)
                    }) {
                        let referents = borrow.and_then(|borrow| {
                            carried_exclusive_referents(
                                program,
                                borrow,
                                machine,
                                state,
                                borrow_call.statement_index,
                                arguments[argument_index - 1],
                                parameter.type_reference,
                                resolver?,
                            )
                        })?;
                        for referent in referents {
                            for region in rebase_storage_region(
                                program,
                                state.symbol,
                                borrow_call.statement_index,
                                referent.region,
                                resolver,
                            )? {
                                if let StorageRegion::Declared(place) = &region {
                                    let inventory =
                                        resident_owners_in_type(program, referent.reference)?;
                                    for path in inventory.unresolved {
                                        let mut carrier = place.clone();
                                        carrier.segments.extend(path);
                                        places.push(StorageRegion::UnresolvedContent { carrier });
                                    }
                                    for (path, owner) in inventory.owners {
                                        let mut content_owner = place.clone();
                                        content_owner.segments.extend(path);
                                        let content = StorageRegion::ResidentContent {
                                            owner: content_owner,
                                            formation: Default::default(),
                                            granted: owner.granted,
                                            resident: owner.resident,
                                        };
                                        push_unique(&mut places, content);
                                    }
                                }
                                push_unique(&mut places, region);
                            }
                        }
                    }
                    continue;
                }
                argument
            };
            let captured = captured_argument_write_regions(
                program,
                machine.symbol,
                state.symbol,
                borrow_call.statement_index,
                borrow_call.call_ordinal,
                &site,
                argument_index - 1,
                parameter.type_reference,
                &[],
                WriteDestination::Referent,
                resolver,
            );
            let storage = captured.or_else(|| {
                let actual = place_at(program, state.symbol, borrow_call.statement_index, actual)?;
                rebase_local_write_regions(
                    program,
                    state.symbol,
                    borrow_call.statement_index,
                    actual,
                    resolver,
                )
            })?;
            for storage in storage {
                push_unique(&mut places, storage);
            }
        }
        if let Some(borrow) = borrow {
            for region in retained_loan_write_regions(
                program,
                caller_machine_symbol,
                caller_state_symbol,
                borrow,
                borrow_call,
            )? {
                push_unique(&mut places, region);
            }
        }
        Some(places)
    }

    /*
        Owned capability storage is not isolated merely because its carrier has
        no reference fields. Reuse the ordinary owned qualification roster for
        records, case payloads and arrays; a field containing a resident reaches
        the same content as a root resident. Shared ingress grants no write reach.
    */
    fn resident_parameter_regions(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        call: &BorrowCallFact,
        site: &CallSite<'_>,
        position: usize,
        reference: TypeReferenceHandle,
        frames: Option<&CallFrameResolver<'_>>,
    ) -> Option<Vec<StorageRegion>> {
        let mut regions = Vec::new();
        let inventory = resident_owners_in_type(program, reference)?;
        for path in inventory.unresolved {
            let actual = *call_site_argument_expressions(program, site).get(position)?;
            let mut carrier = place_at(program, state.symbol, call.statement_index, actual)
                .unwrap_or_else(|| CanonicalPlace {
                    root: PlaceRoot::Expression(actual),
                    segments: Vec::new(),
                });
            carrier.segments.extend(path);
            regions.push(StorageRegion::UnresolvedContent { carrier });
        }
        for (path, owner) in inventory.owners {
            let captured = captured_argument_write_regions(
                program,
                machine.symbol,
                state.symbol,
                call.statement_index,
                call.call_ordinal,
                site,
                position,
                reference,
                &path,
                WriteDestination::Referent,
                frames,
            )
            .or_else(|| {
                let actual = *call_site_argument_expressions(program, site).get(position)?;
                let mut place = place_at(program, state.symbol, call.statement_index, actual)?;
                place.segments.extend_from_slice(&path);
                rebase_local_write_regions(
                    program,
                    state.symbol,
                    call.statement_index,
                    place,
                    frames,
                )
            });
            let Some(captured) = captured else {
                if reference_shell(program, reference).is_some() {
                    return None;
                }
                let actual = *call_site_argument_expressions(program, site).get(position)?;
                regions.push(StorageRegion::UnresolvedContent {
                    carrier: CanonicalPlace {
                        root: PlaceRoot::Expression(actual),
                        segments: path,
                    },
                });
                continue;
            };
            for region in captured {
                regions.push(StorageRegion::ResidentContent {
                    owner: region.into_declared()?,
                    formation: Default::default(),
                    granted: owner.granted,
                    resident: owner.resident,
                });
            }
        }
        Some(regions)
    }

    #[derive(Default)]
    struct ResidentAuthorityInventory {
        owners: Vec<(Vec<PlaceSegment>, ResidentOwnerQualifications)>,
        unresolved: Vec<Vec<PlaceSegment>>,
    }

    fn resident_owners_in_type(
        program: &dyn ::validation::CheckingInput,
        mut reference: TypeReferenceHandle,
    ) -> Option<ResidentAuthorityInventory> {
        if let Some(shell) = reference_shell(program, reference) {
            if !shell.access.is_exclusive() {
                return Some(ResidentAuthorityInventory::default());
            }
            reference = shell.referee;
        }
        let roster = declared_owned_field_obligations(program, reference);
        let mut inventory = ResidentAuthorityInventory::default();
        for path in &roster.unresolved {
            let unresolved = project_type_reference_from_segments(program, reference, path)?;
            let plain = |reference| {
                has_contents(program, reference, &[], ContentsRequirement::NumericOwned)
            };
            let symbol = program.type_reference_table.type_symbol(
                program
                    .type_reference_table
                    .peeled_type_reference(unresolved),
            );
            if !plain(unresolved)
                && !program.access_view_plans.iter().any(|plan| {
                    plan.fields.iter().any(|field| {
                        field.accessor_data_symbol == symbol && plain(field.value_type)
                    })
                })
            {
                inventory.unresolved.push(path.clone());
            }
        }
        let mut paths = UniqueRoster::default();
        paths.push(Vec::new());
        for row in roster.domains {
            paths.push(row.identity.0);
        }
        for path in paths.into_vec() {
            let projected = project_type_reference_from_segments(program, reference, &path)?;
            if let Some(owner) = resident_owner_qualifications(program, projected) {
                inventory.owners.push((path, owner));
            }
        }
        Some(inventory)
    }

    pub(super) fn is_exclusive_reference(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        let mut reference = type_reference;
        while reference.is_valid() {
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                TypeReferenceNode::Reference { access, .. } => return access.is_exclusive(),
                _ => return false,
            }
        }
        false
    }

    /*
        The caller storage a consumed by-value actual hands the callee through
        carried exclusive loans. `None` when the actual's storage cannot be named
        exactly or a carried exclusive loan has no spellable referent — both keep
        the whole ceiling unrepresentable rather than guessing narrower than the
        declaration. Shared (`&`) loans contribute nothing: a read-only borrow
        inside the consumed value reaches no writable caller place.
    */
    struct CarriedStorage {
        region: StorageRegion,
        reference: TypeReferenceHandle,
    }

    fn carried_exclusive_referents(
        program: &dyn ::validation::CheckingInput,
        borrow: &BorrowFacts,
        machine: &Machine,
        state: &State,
        statement_index: usize,
        argument: ExpressionHandle,
        reference: TypeReferenceHandle,
        resolver: &CallFrameResolver<'_>,
    ) -> Option<Vec<CarriedStorage>> {
        let (_, caller_state) = borrow.first_state_borrow_by_symbol(state.symbol)?;
        let mut referents = Vec::new();
        for projection in literal_expression_projections(program, argument, reference, &[], true)? {
            let projected =
                project_type_reference_from_segments(program, reference, &projection.destination)?;
            if let Some(shell) = reference_shell(program, projected) {
                if !shell.access.is_exclusive() {
                    continue;
                }
                for source in resolver.reference_operand_write_sources(
                    machine,
                    projection.expression,
                    projected,
                )? {
                    referents.push(CarriedStorage {
                        region: write_source_region(
                            program,
                            state,
                            statement_index,
                            &source,
                            &projection.remaining,
                        )?,
                        reference: projected,
                    });
                }
                continue;
            }
            if resolver.value_is_reference_free(projected) {
                continue;
            }
            let mut actual = place_at(
                program,
                state.symbol,
                statement_index,
                projection.expression,
            )?;
            actual.segments.extend(projection.remaining);
            let owner_root = actual.root.as_symbol()?;
            for (_, loan) in borrow.state_loans(caller_state) {
                if !loan.kind.is_exclusive()
                    || loan.owner_symbol != owner_root
                    || !owner_path_overlaps(program, borrow.loan_owner_path(loan), &actual.segments)
                {
                    continue;
                }
                if !loan.root.is_valid() {
                    return None;
                }
                let descriptor = CanonicalPlace {
                    root: actual.root,
                    segments: borrow
                        .loan_owner_path(loan)
                        .iter()
                        .map(|segment| match segment {
                            OwnerSegment::Field(symbol) => PlaceSegment::Field { symbol: *symbol },
                            OwnerSegment::Case(variant) => PlaceSegment::Case { variant: *variant },
                            OwnerSegment::FixedIndex(index) => {
                                PlaceSegment::FixedIndex { index: *index }
                            }
                            OwnerSegment::DynamicIndex => PlaceSegment::Index {
                                expression: Default::default(),
                            },
                        })
                        .collect(),
                };
                referents.push(CarriedStorage {
                    region: loan_write_region(CanonicalPlace {
                        root: loan.root,
                        segments: borrow.loan_segments(loan).to_vec(),
                    }),
                    reference: canonical_place_type_reference(
                        program,
                        state.symbol,
                        statement_index,
                        &descriptor,
                    )?,
                });
            }
        }
        Some(referents)
    }

    /*
        Completing an opaque retained carrier may expose writes to its backing.
        Ending an ordinary reference's input lifetime creates no write: the call's
        actual writes have their own frame. Unknown carriers retain the conservative
        closing effect. These are invalidation coordinates only.
    */
    pub(super) fn retained_loan_write_regions(
        program: &dyn validation::CheckingInput,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        borrow: &BorrowFacts,
        call: &BorrowCallFact,
    ) -> Option<Vec<StorageRegion>> {
        let source_state = program.state_by_symbol(state_symbol)?;
        let order = program
            .statement_table
            .admitted_execution_order(source_state.statement_nodes);
        let (_, state) = borrow.first_state_borrow_by_symbol(state_symbol)?;
        let site = find_call_site(
            program,
            machine_symbol,
            state_symbol,
            call.statement_index,
            call.call_ordinal,
        )?;
        let arguments = call_site_argument_expressions(program, &site);
        let mut regions = Vec::new();
        for (_, loan) in borrow.state_loans(state) {
            if loan.lineage != BorrowLoanLineage::LifetimeDependency
                || !loan.kind.is_exclusive()
                || loan.last_use_statement_index != call.statement_index
            {
                continue;
            }
            let formed = order.precedes(loan.statement_index, call.statement_index)
                || (loan.formation_expression.is_valid()
                    && arguments.iter().any(|argument| {
                        expression_contains(program, *argument, loan.formation_expression)
                    }));
            if !formed {
                continue;
            }
            if !loan.root.is_valid() {
                return None;
            }
            let carrier = canonical_place_type_reference(
                program,
                state_symbol,
                call.statement_index,
                &CanonicalPlace {
                    root: loan.storage_owner,
                    segments: crate::borrow::owner_place_segments(borrow.loan_owner_path(loan)),
                },
            );
            if !carrier_retirement_may_write(program, carrier) {
                continue;
            }
            let region = loan_write_region(CanonicalPlace {
                root: loan.root,
                segments: borrow.loan_segments(loan).to_vec(),
            });
            push_unique(&mut regions, region);
        }
        Some(regions)
    }

    fn carrier_retirement_may_write(
        program: &dyn validation::CheckingInput,
        carrier: Option<TypeReferenceHandle>,
    ) -> bool {
        let Some(carrier) = carrier else {
            return true;
        };
        if reference_shell(program, carrier).is_some() {
            return false;
        }
        validation::borrow_contracts::retained_lifetime_owner_paths(program, carrier)
            .is_none_or(|paths| paths.iter().any(Vec::is_empty))
    }

    /*
        A checked provider-output loan retains its performed identity but names no
        declaration-relative backing. It cannot restore a caller ownership home.
    */
    fn loan_write_region(place: CanonicalPlace) -> StorageRegion {
        if matches!(place.root, PlaceRoot::InvocationOutput(_)) {
            StorageRegion::UnresolvedContent { carrier: place }
        } else {
            StorageRegion::Declared(place)
        }
    }

    /*
        Whether a recorded loan's owner path and a place's field path overlap.
        Same prefix law as `borrow::last_uses`'s owner-path matching: an absent
        or dynamic selector conservatively overlaps its counterpart.
    */
    fn owner_path_overlaps(
        program: &dyn ::validation::CheckingInput,
        owner_path: &[OwnerSegment],
        place_segments: &[PlaceSegment],
    ) -> bool {
        owner_path
            .iter()
            .zip(place_segments)
            .all(|(owner, place)| match (owner, place) {
                (
                    OwnerSegment::Field(owner_symbol),
                    PlaceSegment::Field {
                        symbol: place_symbol,
                    },
                ) => !place_symbol.is_valid() || owner_symbol == place_symbol,
                (
                    OwnerSegment::Case(owner_variant),
                    PlaceSegment::Case {
                        variant: place_variant,
                    },
                ) => owner_variant == place_variant,
                (
                    OwnerSegment::FixedIndex(owner_index),
                    PlaceSegment::FixedIndex { index: place_index },
                ) => owner_index == place_index,
                (OwnerSegment::FixedIndex(owner_index), PlaceSegment::Index { expression }) => {
                    expressions(program)
                        .constant_integer_value(*expression)
                        .and_then(|value| usize::try_from(value).ok())
                        .is_none_or(|place_index| *owner_index == place_index)
                }
                (
                    OwnerSegment::DynamicIndex,
                    PlaceSegment::FixedIndex { .. } | PlaceSegment::Index { .. },
                ) => true,
                _ => false,
            })
    }
}
mod local_origins {
    /*
        Project the shared frame engine's local storage origins into flow places.

        Alias transfer belongs to validation. This adapter resolves its canonical
        names to existing typed symbols. Complete runtime-selected paths retain
        their descendant fields for overlap, but do not supply exact identity.
        The inventory includes unknown reference sources. Demand a source only
        after selecting overlapping local storage; unrelated unknown leaves cannot
        erase a known effect. Alias invalidation conservatively retires unknown
        leaves on every write, without assigning them a referent. An exact-origin
        query must not discard unknown alternatives.
    */
    use super::summary::WriteDestination;
    use crate::flow;
    use crate::shared::collections::push_unique;
    use facts::PlaceRoot;
    use facts::PlaceSegment;
    use typed_trees::state::State;
    use validation::push_field_place_segments;
    use validation::resolve_member_symbol_from_place;
    use validation::{
        AssignmentWriteTarget, CallFrameResolver, CanonicalPlace, LocalWriteOrigin,
        assignment_stored_value_type, canonical_place_type_reference, declared_reference_access,
        find_state, place_has_builtin_coordinates, reference_shell,
    };

    use crate::flow::canonical_place_from_symbol;
    use crate::flow::canonical_place_segments_equal;
    use crate::flow::canonical_place_segments_may_overlap;
    use crate::flow::mutation::StorageRegion;
    use crate::flow::mutation::normalize_write_only_range_place;
    use crate::published::places::place_at;

    use checked_trees::expression::ExpressionNode;
    use checked_trees::statement::StatementNode;
    use symbols::SymbolHandle;
    mod coordinates {
        /*
            Transport frozen structural origins without replaying selector expressions.
        */
        use validation::CanonicalPlace;
        use validation::canonical_place_type_reference;

        use crate::flow::canonical_place_from_symbol;

        use crate::flow::mutation::StorageRegion;
        use crate::flow::mutation::local_origins::place_from_origin_path;
        use crate::flow::place_segment_has_unresolved_identity;

        #[derive(Clone, Copy, PartialEq, Eq)]
        pub(crate) enum OriginPrecision {
            Exact,
            /* Every structural dimension is retained, but an element is unknown. */
            MaySelect,
            /* Only an enclosing region survived. Descendant projection is unsound. */
            Enclosing,
        }

        /*
            Precision separates complete structural projection from exact identity.
            A runtime element selector retains its known field suffix for conservative
            overlap, but cannot feed value transport or
            returned-reference evidence re-anchoring as a current selector value.
        */
        pub(crate) fn origin_place(
            program: &dyn ::validation::CheckingInput,
            state: &typed_trees::state::State,
            statement_index: usize,
            origin: &validation::LocalWriteOrigin,
        ) -> Option<(CanonicalPlace, OriginPrecision)> {
            let source = origin.source.as_ref()?;
            source_place(program, state, statement_index, source)
        }

        fn source_place(
            program: &dyn ::validation::CheckingInput,
            state: &typed_trees::state::State,
            statement_index: usize,
            source: &validation::LocalWriteSource,
        ) -> Option<(CanonicalPlace, OriginPrecision)> {
            if source.invocation_output.is_some() || source.invocation_result.is_some() {
                return None;
            }
            if let Some(first) = source.recasts.first() {
                for recast in &source.recasts {
                    if matches!(
                        validation::reference_formation_source(
                            program,
                            recast.formation.expression()
                        )?
                        .interpretation,
                        validation::ReferenceFormationInterpretation::ResidentAccess { .. }
                    ) {
                        return None;
                    }
                }
                let backing = validation::LocalWriteSource {
                    root: first.root,
                    segments: first.segments.clone(),
                    recasts: Vec::new(),
                    representation: None,
                    ..source.clone()
                };
                let (mut place, _) = structural_origin(program, state, statement_index, &backing)?;
                validation::completed_storage::widen_element_anchor(&mut place.segments);
                return Some((place, OriginPrecision::Enclosing));
            }
            if source.representation.is_some() {
                return None;
            }
            if let Some(place) = structural_origin(program, state, statement_index, source) {
                return Some(place);
            }
            place_from_origin_path(program, state, statement_index, &source.path).map(|place| {
                (
                    place,
                    if source.collection_coarse {
                        OriginPrecision::Enclosing
                    } else {
                        OriginPrecision::Exact
                    },
                )
            })
        }

        /*
            Translate a callee's logical write into the captured actual's storage.
            Exact scalar geometry preserves disjoint backing fields. When a view has
            no exact leaf translation, its enclosing footprint is still a write;
            that conservative answer never becomes value-preservation evidence.
        */
        pub(in crate::flow::mutation) fn write_source_region(
            program: &dyn ::validation::CheckingInput,
            state: &typed_trees::state::State,
            statement_index: usize,
            source: &validation::LocalWriteSource,
            relative: &[facts::PlaceSegment],
        ) -> Option<StorageRegion> {
            if source.invocation_output.is_some() {
                return output_write_region(program, source, relative);
            }
            if let Some(home) = source.invocation_result {
                return result_write_region(program, source, home, relative);
            }
            if !source.root.is_valid() {
                return None;
            }
            for recast in &source.recasts {
                if let validation::ReferenceViewFormation::ExtentView(declaration) =
                    recast.formation
                {
                    if recast.representation.is_some()
                        || typed_trees::typed_trees::AccessExecutionDeclaration::authenticate(
                            program,
                            declaration.machine(),
                        )
                        .ok()??
                            != declaration
                        || declaration.operation()
                            != typed_trees::typed_trees::AccessLifecycleOperation::View
                    {
                        return None;
                    }
                    let backing = validation::LocalWriteSource {
                        root: recast.root,
                        segments: recast.segments.clone(),
                        recasts: Vec::new(),
                        representation: None,
                        ..source.clone()
                    };
                    let (owner, precision) =
                        structural_origin(program, state, statement_index, &backing)?;
                    if precision != OriginPrecision::Exact {
                        return None;
                    }
                    return Some(StorageRegion::NonresidentContent {
                        owner,
                        formation: declaration,
                    });
                }
                let formation =
                    validation::reference_formation_source(program, recast.formation.expression())?;
                if !matches!(
                    formation.interpretation,
                    validation::ReferenceFormationInterpretation::ResidentAccess { .. }
                ) {
                    continue;
                }
                let backing = validation::LocalWriteSource {
                    root: recast.root,
                    segments: recast.segments.clone(),
                    recasts: Vec::new(),
                    representation: None,
                    ..source.clone()
                };
                let (owner, precision) =
                    structural_origin(program, state, statement_index, &backing)?;
                if precision != OriginPrecision::Exact {
                    return None;
                }
                let endpoint =
                    canonical_place_type_reference(program, state.symbol, statement_index, &owner)?;
                let view = program
                    .type_reference_table
                    .peeled_type_reference(formation.view_type);
                let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
                    program.type_reference_table.type_reference(view)
                else {
                    return None;
                };
                let mut plans = program
                    .access_view_plans
                    .iter()
                    .filter(|plan| plan.data_symbol == *symbol);
                let (Some(plan), None) = (plans.next(), plans.next()) else {
                    return None;
                };
                let checked_trees::CheckedReferenceStorage::ResidentAccess {
                    granted,
                    resident,
                    ..
                } = formation
                    .interpretation
                    .access_storage(program, endpoint, plan)?
                else {
                    return None;
                };
                return Some(StorageRegion::ResidentContent {
                    owner,
                    formation: recast.formation.expression(),
                    granted,
                    resident,
                });
            }
            if !source.recasts.is_empty() {
                let mut projected = source.clone();
                projected.segments.extend_from_slice(relative);
                if let Some((_, selectors)) = &mut projected.representation {
                    selectors.extend_from_slice(relative);
                }
                if let Some(coordinate) = projected.scalar_storage_coordinate(program, state) {
                    return Some(StorageRegion::Declared(coordinate.place));
                }
            }
            let (place, precision) = source_place(program, state, statement_index, source)?;
            if precision == OriginPrecision::Enclosing {
                return Some(StorageRegion::Declared(place));
            }
            crate::flow::project_captured_view_place(place, relative).map(StorageRegion::Declared)
        }

        /*
            The chronological origin producer already retained this performed output.
            Rejoin its typed reference slot, without treating lifetime inputs or the
            display path as an address. Its backing may alias other storage.
        */
        fn output_write_region(
            program: &dyn validation::CheckingInput,
            source: &validation::LocalWriteSource,
            relative: &[facts::PlaceSegment],
        ) -> Option<StorageRegion> {
            if source.is_image_storage
                || !source.recasts.is_empty()
                || source.representation.is_some()
            {
                return None;
            }
            let identity = source.invocation_output?;
            let parameter = validation::invocation_output_parameter(program, identity)?;
            let shell = validation::reference_shell(program, parameter.type_reference)?;
            if !shell.access.is_exclusive() {
                return None;
            }
            let length = (0..=source.segments.len()).find(|length| {
                validation::project_type_reference_from_segments(
                    program,
                    shell.referee,
                    &source.segments[..*length],
                )
                .and_then(|reference| validation::reference_shell(program, reference))
                .is_some()
            })?;
            let mut projected = source.segments.clone();
            projected.extend_from_slice(relative);
            validation::project_type_reference_from_segments(program, shell.referee, &projected)?;
            Some(StorageRegion::UnresolvedContent {
                carrier: CanonicalPlace {
                    root: facts::PlaceRoot::InvocationOutput(identity),
                    segments: source.segments[..length].to_vec(),
                },
            })
        }

        /*
            A returned leaf's opaque referent carries the same unresolved-content
            answer as a performed output: the declared referee supplies projection
            geometry, but its physical backing may alias storage this frame cannot
            name.
        */
        fn result_write_region(
            program: &dyn validation::CheckingInput,
            source: &validation::LocalWriteSource,
            home: validation::InvocationResultHome,
            relative: &[facts::PlaceSegment],
        ) -> Option<StorageRegion> {
            if source.is_image_storage
                || !source.recasts.is_empty()
                || source.representation.is_some()
            {
                return None;
            }
            let length = (0..=source.segments.len()).find(|length| {
                validation::project_type_reference_from_segments(
                    program,
                    home.referent,
                    &source.segments[..*length],
                )
                .and_then(|reference| validation::reference_shell(program, reference))
                .is_some()
            })?;
            let mut projected = source.segments.clone();
            projected.extend_from_slice(relative);
            validation::project_type_reference_from_segments(program, home.referent, &projected)?;
            Some(StorageRegion::UnresolvedContent {
                carrier: CanonicalPlace {
                    root: facts::PlaceRoot::TypeReference(home.referent),
                    segments: source.segments[..length].to_vec(),
                },
            })
        }

        fn structural_origin(
            program: &dyn ::validation::CheckingInput,
            state: &typed_trees::state::State,
            statement_index: usize,
            source: &validation::LocalWriteSource,
        ) -> Option<(CanonicalPlace, OriginPrecision)> {
            let mut place = canonical_place_from_symbol(source.root)?;
            canonical_place_type_reference(program, state.symbol, statement_index, &place)?;
            let mut exact = true;
            for &segment in &source.segments {
                if place_segment_has_unresolved_identity(segment) {
                    return None;
                }
                let segment = match segment {
                    facts::PlaceSegment::Index { expression }
                        if program.expression_table.expression_is_valid(expression)
                            && program.expression_table.range_node(expression).is_some() =>
                    {
                        facts::PlaceSegment::OpaqueRange
                    }
                    _ => segment,
                };
                exact &= !matches!(
                    segment,
                    facts::PlaceSegment::Index { .. } | facts::PlaceSegment::OpaqueRange
                );
                place.segments.push(segment);
                crate::flow::compose_captured_view_segments(&mut place.segments)?;
                origin_type_reference(program, state, statement_index, &place)?;
            }
            Some((
                place,
                if exact {
                    OriginPrecision::Exact
                } else {
                    OriginPrecision::MaySelect
                },
            ))
        }

        /*
            Value and descriptor equality require the original resolved namespace.
            Conservative write paths may name enclosing bytes, but a recast can change
            element count and field interpretation without changing their address.
        */
        pub(super) fn exact_origin_place(
            program: &dyn ::validation::CheckingInput,
            state: &typed_trees::state::State,
            statement_index: usize,
            origin: &validation::LocalWriteOrigin,
        ) -> Option<CanonicalPlace> {
            let source = origin.source.as_ref()?;
            if !source.recasts.is_empty() || source.representation.is_some() {
                return None;
            }
            let (place, precision) = structural_origin(program, state, statement_index, source)?;
            (precision == OriginPrecision::Exact).then_some(place)
        }

        /*
            Translate a backing write into a captured alias's coordinates. A window
            shares its final dimension with the backing selector; it does not consume
            that selector from the alias path. Unknown offsets preserve an unknown
            element/window and the known descendant suffix, never a later selector value.
        */
        pub(in crate::flow) fn alias_projection(
            captured: &[facts::PlaceSegment],
            storage: &[facts::PlaceSegment],
        ) -> Option<Vec<facts::PlaceSegment>> {
            use facts::PlaceSegment;
            let mut suffix = storage.get(captured.len()..)?.to_vec();
            let Some(window @ (PlaceSegment::FixedRange { .. } | PlaceSegment::OpaqueRange)) =
                captured.last().copied()
            else {
                return Some(suffix);
            };
            let selected = *storage.get(captured.len().checked_sub(1)?)?;
            let unknown_element = PlaceSegment::Index {
                expression: typed_trees::expression::ExpressionHandle::invalid(),
            };
            let relative = match (window, selected) {
                (PlaceSegment::FixedRange { start, end }, PlaceSegment::FixedIndex { index })
                    if index >= start && index < end =>
                {
                    PlaceSegment::FixedIndex {
                        index: index.checked_sub(start)?,
                    }
                }
                (
                    PlaceSegment::FixedRange { start, end },
                    PlaceSegment::FixedRange {
                        start: selected_start,
                        end: selected_end,
                    },
                ) => {
                    let overlap_start = start.max(selected_start);
                    let overlap_end = end.min(selected_end);
                    if overlap_start >= overlap_end {
                        return None;
                    }
                    PlaceSegment::FixedRange {
                        start: overlap_start.checked_sub(start)?,
                        end: overlap_end.checked_sub(start)?,
                    }
                }
                (PlaceSegment::OpaqueRange, PlaceSegment::FixedIndex { .. })
                | (_, PlaceSegment::Index { .. }) => unknown_element,
                (_, PlaceSegment::OpaqueRange)
                | (PlaceSegment::OpaqueRange, PlaceSegment::FixedRange { .. }) => {
                    PlaceSegment::OpaqueRange
                }
                _ => return None,
            };
            suffix.insert(0, relative);
            Some(suffix)
        }

        /*
            A frozen window is an exact descriptor origin, not an unknown element.
            It retains its collection dimension; the type query below checks the
            underlying carrier without pretending the window selected one child.
        */
        fn origin_type_reference(
            program: &dyn ::validation::CheckingInput,
            state: &typed_trees::state::State,
            statement_index: usize,
            place: &CanonicalPlace,
        ) -> Option<typed_trees::types::TypeReferenceHandle> {
            let mut carrier = place.clone();
            if let Some(
                segment @ (facts::PlaceSegment::FixedRange { .. }
                | facts::PlaceSegment::OpaqueRange),
            ) = carrier.segments.last().copied()
            {
                if let facts::PlaceSegment::FixedRange { start, end } = segment
                    && start > end
                {
                    return None;
                }
                carrier.segments.pop();
                let reference = canonical_place_type_reference(
                    program,
                    state.symbol,
                    statement_index,
                    &carrier,
                )?;
                validation::collection_element_type_reference(program, reference)?;
                Some(reference)
            } else {
                canonical_place_type_reference(program, state.symbol, statement_index, place)
            }
        }

        /*
            The sole whole-local write origin of `local_symbol` before `statement`,
            restricted to referents rooted outside the local's own storage. The range
            referent lookup and the projected-binding walk share this admission.
        */
        pub(crate) fn sole_local_referent_origin(
            resolver: &validation::CallFrameResolver<'_>,
            machine: &typed_trees::machine::Machine,
            statement: &typed_trees::statement::StatementNode,
            local_symbol: symbols::SymbolHandle,
        ) -> Option<validation::LocalWriteOrigin> {
            let mut referents = resolver
                .local_write_origins_before_statement(machine, statement)?
                .into_iter()
                .filter(|origin| {
                    origin.local_symbol == local_symbol
                        && origin.local_segments.is_empty()
                        && origin
                            .source
                            .as_ref()
                            .is_none_or(|source| source.root != local_symbol)
                });
            let origin = referents.next()?;
            (referents.next().is_none()).then_some(origin)
        }

        #[cfg(test)]
        mod tests {
            use super::alias_projection;
            use facts::PlaceSegment::{self, FixedIndex, FixedRange, OpaqueRange};

            #[test]
            fn unknown_window_keeps_alias_row_and_descendant_column() {
                let projected = alias_projection(
                    &[OpaqueRange],
                    &[FixedIndex { index: 0 }, FixedIndex { index: 1 }],
                )
                .unwrap();
                assert!(
                    matches!(projected[0], PlaceSegment::Index { expression } if !expression.is_valid())
                );
                assert_eq!(projected[1..], [FixedIndex { index: 1 }]);
            }

            #[test]
            fn known_window_translates_only_its_boundary_dimension() {
                assert_eq!(
                    alias_projection(
                        &[FixedRange { start: 2, end: 6 }],
                        &[FixedIndex { index: 3 }, FixedRange { start: 1, end: 4 }],
                    ),
                    Some(vec![
                        FixedIndex { index: 1 },
                        FixedRange { start: 1, end: 4 }
                    ]),
                );
                assert_eq!(
                    alias_projection(
                        &[FixedRange { start: 2, end: 6 }],
                        &[FixedRange { start: 1, end: 4 }, FixedIndex { index: 1 }],
                    ),
                    Some(vec![
                        FixedRange { start: 0, end: 2 },
                        FixedIndex { index: 1 }
                    ]),
                );
            }
        }
    }
    pub(in crate::flow) use coordinates::alias_projection as captured_alias_projection;
    pub(super) use coordinates::write_source_region;
    pub(crate) use coordinates::{OriginPrecision, origin_place, sole_local_referent_origin};

    pub(crate) fn close_storage_places_over_aliases(
        program: &dyn ::validation::CheckingInput,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        places: Vec<CanonicalPlace>,
        resolver: Option<&CallFrameResolver<'_>>,
    ) -> Option<Vec<CanonicalPlace>> {
        if places.is_empty() {
            return Some(places);
        }
        let machine = program.machine_by_symbol(machine_symbol)?;
        let state = program.state_by_symbol(state_symbol)?;
        let statement = program
            .statement_table
            .statement_at(state, statement_index)?;
        let resolver = resolver?;
        let origins = resolver.local_write_origins_before_statement(machine, statement)?;
        close_storage_places_over_origins(program, state, statement_index, places, &origins)
    }

    /*
        Project physical writes into the names under which content facts live.
        The caller supplies the inventory at its observation frontier; a binding
        without preservation evidence is an unknown alias, never a disjoint one.
    */
    pub(crate) fn close_storage_places_over_origins(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        statement_index: usize,
        places: Vec<CanonicalPlace>,
        origins: &[LocalWriteOrigin],
    ) -> Option<Vec<CanonicalPlace>> {
        close_storage_places_over_written_origins(
            program,
            state,
            statement_index,
            places,
            &[],
            origins,
        )
    }

    /*
        `authored` names the places an assignment wrote through its own spelling
        (`view.flags`). Writing a strict descendant of an alias changes only that
        descendant of the alias's referent: record fields are disjoint storage, so
        sibling facts under that alias name survive. Every other alias of the same
        physical storage still closes over the complete physical footprint.
    */
    pub(crate) fn close_storage_places_over_written_origins(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        statement_index: usize,
        mut places: Vec<CanonicalPlace>,
        authored: &[CanonicalPlace],
        origins: &[LocalWriteOrigin],
    ) -> Option<Vec<CanonicalPlace>> {
        let storage = places.clone();
        let mut seen: std::collections::HashSet<CanonicalPlace> = storage.iter().cloned().collect();
        let within = |written: &CanonicalPlace, alias: &CanonicalPlace| {
            written.root == alias.root
                && written.segments.len() > alias.segments.len()
                && written.segments.starts_with(&alias.segments)
        };
        let alias_of = |origin: &LocalWriteOrigin| {
            canonical_place_from_symbol(origin.local_symbol).map(|mut alias| {
                alias.segments.extend_from_slice(&origin.local_segments);
                alias
            })
        };
        /*
            An authored place only stands in for an alias it lies strictly within;
            a spelling that crosses no such alias adds nothing to the footprint.
        */
        let authored = authored
            .iter()
            .filter(|written| {
                origins.iter().any(|origin| {
                    origin.source.is_some()
                        && alias_of(origin).is_some_and(|alias| within(written, &alias))
                })
            })
            .cloned()
            .collect::<Vec<_>>();
        for place in &authored {
            if seen.insert(place.clone()) {
                places.push(place.clone());
            }
        }
        for origin in origins {
            if origin
                .source
                .as_ref()
                .is_some_and(|source| source.is_image_storage)
            {
                continue;
            }
            let mut alias = canonical_place_from_symbol(origin.local_symbol)?;
            alias.segments.extend_from_slice(&origin.local_segments);
            if origin.source.is_none() {
                let written_within = storage.iter().any(|written| within(written, &alias));
                if !written_within && seen.insert(alias.clone()) {
                    places.push(alias);
                }
                continue;
            }
            if authored.iter().any(|written| within(written, &alias)) {
                continue;
            }
            let (source, precision) = origin_place(program, state, statement_index, origin)
                .or_else(|| {
                    let region = write_source_region(
                        program,
                        state,
                        statement_index,
                        origin.source.as_ref()?,
                        &[],
                    )?;
                    Some((region.anchor().clone(), OriginPrecision::Enclosing))
                })?;
            for place in &storage {
                if source.root != place.root {
                    continue;
                }
                let mut alias = alias.clone();
                if !canonical_place_segments_may_overlap(program, &place.segments, &source.segments)
                {
                    continue;
                }
                if place.segments.len() >= source.segments.len()
                    && precision != OriginPrecision::Enclosing
                {
                    alias.segments.extend(coordinates::alias_projection(
                        &source.segments,
                        &place.segments,
                    )?);
                }
                if seen.insert(alias.clone()) {
                    places.push(alias);
                }
            }
        }
        Some(places)
    }

    pub(super) fn assignment_storage_places(
        program: &dyn ::validation::CheckingInput,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        statement: &StatementNode,
        call_frames: Option<&CallFrameResolver<'_>>,
    ) -> Option<Vec<CanonicalPlace>> {
        StorageRegion::declared_places(assignment_storage_regions(
            program,
            machine_symbol,
            state_symbol,
            statement_index,
            statement,
            call_frames,
        )?)
    }

    pub(super) fn assignment_storage_regions(
        program: &dyn ::validation::CheckingInput,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        statement: &StatementNode,
        call_frames: Option<&CallFrameResolver<'_>>,
    ) -> Option<Vec<StorageRegion>> {
        let assignment = statement.as_assignment()?;
        let machine = program.machine_by_symbol(machine_symbol)?;
        let state = program.state_by_symbol(state_symbol)?;
        let mut projection = assignment.target;
        loop {
            projection = match program.expression_table.expression(projection) {
                ExpressionNode::Borrow(borrow) => borrow.target,
                ExpressionNode::Member(member) => member.receiver,
                ExpressionNode::Indexed(indexed)
                    if matches!(
                        program.expression_table.expression(indexed.index),
                        ExpressionNode::Range(_)
                    ) =>
                {
                    indexed.collection
                }
                ExpressionNode::Indexed(_)
                    if !place_has_builtin_coordinates(
                        program,
                        machine,
                        Some(state),
                        projection,
                    ) =>
                {
                    return None;
                }
                _ => break,
            };
        }
        let mut direct = place_at(program, state_symbol, statement_index, assignment.target);
        if let Some(place) = &mut direct {
            normalize_write_only_range_place(program, state_symbol, place);
            if !place_requires_local_write_origin(
                program,
                state_symbol,
                statement_index,
                place,
                call_frames,
            ) {
                return direct.map(|place| vec![StorageRegion::Declared(place)]);
            }
        }
        let mut owned_frames = None;
        let resolver = flow::shared_call_frames_or(call_frames, program, &mut owned_frames)?;
        let target = resolver.assignment_write_target(machine, statement);
        if matches!(
            target,
            Some(AssignmentWriteTarget::LocalBindingReplacement { .. })
        ) {
            return direct.map(|place| vec![StorageRegion::Declared(place)]);
        }
        if assignment_stored_value_type(program, machine, state, assignment)
            .is_some_and(|reference| reference_shell(program, reference).is_some())
        {
            let captured = resolver
                .assignment_storage_origins(machine, statement)
                .ok()?;
            let mut regions = Vec::new();
            for source in captured.destinations {
                let StorageRegion::Declared(place) = coordinates::write_source_region(
                    program,
                    state,
                    statement_index,
                    &source,
                    &[],
                )?
                else {
                    return None;
                };
                let reference =
                    canonical_place_type_reference(program, state_symbol, statement_index, &place)?;
                reference_shell(program, reference)?;
                for region in rebase_descriptor_slot_regions(
                    program,
                    state_symbol,
                    statement_index,
                    place,
                    call_frames,
                )? {
                    push_unique(&mut regions, region);
                }
            }
            return Some(regions);
        }
        let Some(target) = target else {
            return rebase_local_write_regions(
                program,
                state_symbol,
                statement_index,
                direct?,
                call_frames,
            );
        };
        match target {
            AssignmentWriteTarget::LocalBindingReplacement { .. } => {
                direct.map(|place| vec![StorageRegion::Declared(place)])
            }
            AssignmentWriteTarget::Storage { paths } => {
                if let Some(place) = direct {
                    rebase_local_write_regions(
                        program,
                        state_symbol,
                        statement_index,
                        place,
                        call_frames,
                    )
                } else {
                    paths
                        .iter()
                        .map(|path| {
                            frame_write_place(program, state, statement_index, path)
                                .map(StorageRegion::Declared)
                        })
                        .collect()
                }
            }
        }
    }

    fn place_requires_local_write_origin(
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        place: &CanonicalPlace,
        call_frames: Option<&CallFrameResolver<'_>>,
    ) -> bool {
        let Some(root) = place.root.as_symbol() else {
            return false;
        };
        let Some(state) = program.state_by_symbol(state_symbol) else {
            return false;
        };
        let Some((_, local)) = program
            .state_local_declaration(state, root)
            .filter(|&(ordinal, _)| ordinal < statement_index)
        else {
            return false;
        };
        let mut owned_frames = None;
        flow::shared_call_frames_or(call_frames, program, &mut owned_frames)
            .is_none_or(|resolver| resolver.local_requires_write_origin(local.type_reference))
    }

    /*
        Equality evidence needs one complete origin, not the conservative union
        used to retire facts after a possible write.
    */
    pub(crate) fn rebase_exact_local_place(
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        place: CanonicalPlace,
        call_frames: Option<&CallFrameResolver<'_>>,
    ) -> Option<CanonicalPlace> {
        if !place_requires_local_write_origin(
            program,
            state_symbol,
            statement_index,
            &place,
            call_frames,
        ) {
            return Some(place);
        }
        let root = place.root.as_symbol()?;
        let (machine, state) = find_state(program, state_symbol)?;
        let statement = program
            .statement_table
            .statement_at(state, statement_index)?;
        let mut owned_frames = None;
        let resolver = flow::shared_call_frames_or(call_frames, program, &mut owned_frames)?;
        let origins = resolver.local_write_origins_before_statement(machine, statement)?;
        let mut candidates = origins.iter().filter(|origin| {
            origin.local_symbol == root
                && canonical_place_segments_may_overlap(
                    program,
                    &place.segments,
                    &origin.local_segments,
                )
        });
        let origin = candidates.next()?;
        if candidates.next().is_some()
            || place.segments.len() < origin.local_segments.len()
            || !place
                .segments
                .iter()
                .zip(&origin.local_segments)
                .all(|(left, right)| canonical_place_segments_equal(program, *left, *right))
        {
            return None;
        }
        let canonical = coordinates::exact_origin_place(program, state, statement_index, origin)?;
        flow::project_captured_view_place(canonical, &place.segments[origin.local_segments.len()..])
    }

    pub(super) fn rebase_local_write_places(
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        place: CanonicalPlace,
        call_frames: Option<&CallFrameResolver<'_>>,
    ) -> Option<Vec<CanonicalPlace>> {
        StorageRegion::declared_places(rebase_local_write_regions(
            program,
            state_symbol,
            statement_index,
            place,
            call_frames,
        )?)
    }

    pub(super) fn rebase_storage_region(
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        region: StorageRegion,
        call_frames: Option<&CallFrameResolver<'_>>,
    ) -> Option<Vec<StorageRegion>> {
        rebase_region_owner(region, |owner| {
            rebase_local_write_regions(program, state_symbol, statement_index, owner, call_frames)
        })
    }

    /*
        Rebase only through the leaf roster captured with these invocation inputs.
        A future callee write cannot erase it; an unknown operand-time roster still
        cannot establish a reachable home. This supplies no access or replacement.
    */
    pub(super) fn rebase_captured_storage_region(
        program: &dyn validation::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        region: StorageRegion,
        references: Option<&[LocalWriteOrigin]>,
        destination: WriteDestination,
        call_frames: Option<&CallFrameResolver<'_>>,
    ) -> Option<Vec<StorageRegion>> {
        let descriptor_slot = matches!(destination, WriteDestination::DescriptorSlot);
        rebase_region_owner(region, |place| {
            if descriptor_slot
                && descriptor_slot_is_inline(program, state_symbol, statement_index, &place)?
            {
                return Some(vec![StorageRegion::Declared(place)]);
            }
            if !matches!(place.root, PlaceRoot::Symbol(symbol) if symbol.is_valid()) {
                return None;
            }
            if !place_requires_local_write_origin(
                program,
                state_symbol,
                statement_index,
                &place,
                call_frames,
            ) {
                return Some(vec![StorageRegion::Declared(place)]);
            }
            rebase_reachable_storage(
                program,
                state_symbol,
                statement_index,
                place,
                descriptor_slot,
                references?,
            )
        })
    }

    /* Change owner coordinates without turning content into declared storage. */
    fn rebase_region_owner(
        region: StorageRegion,
        mut rebase: impl FnMut(CanonicalPlace) -> Option<Vec<StorageRegion>>,
    ) -> Option<Vec<StorageRegion>> {
        match region {
            StorageRegion::Declared(place) => rebase(place),
            StorageRegion::ResidentContent {
                owner,
                formation,
                granted,
                resident,
            } => rebase(owner)?
                .into_iter()
                .map(|region| {
                    Some(StorageRegion::ResidentContent {
                        owner: region.into_declared()?,
                        formation,
                        granted,
                        resident,
                    })
                })
                .collect(),
            StorageRegion::NonresidentContent { owner, formation } => rebase(owner)?
                .into_iter()
                .map(|region| {
                    Some(StorageRegion::NonresidentContent {
                        owner: region.into_declared()?,
                        formation,
                    })
                })
                .collect(),
            region @ StorageRegion::UnresolvedContent { .. } => Some(vec![region]),
        }
    }

    /*
        A checked receiving reference field names its cell, not its old pointee.
        No descriptor load is needed for a path entirely inside owned storage.
    */
    pub(super) fn rebase_descriptor_slot_regions(
        program: &dyn validation::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        place: CanonicalPlace,
        call_frames: Option<&CallFrameResolver<'_>>,
    ) -> Option<Vec<StorageRegion>> {
        if descriptor_slot_is_inline(program, state_symbol, statement_index, &place)? {
            return Some(vec![StorageRegion::Declared(place)]);
        }
        rebase_write_regions(
            program,
            state_symbol,
            statement_index,
            place,
            call_frames,
            true,
        )
    }

    fn descriptor_slot_is_inline(
        program: &dyn validation::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        place: &CanonicalPlace,
    ) -> Option<bool> {
        let mut inline = true;
        for length in 0..place.segments.len() {
            let prefix = CanonicalPlace {
                root: place.root,
                segments: place.segments[..length].to_vec(),
            };
            let reference =
                canonical_place_type_reference(program, state_symbol, statement_index, &prefix)?;
            if reference_shell(program, reference).is_some() {
                inline = false;
                break;
            }
        }
        Some(inline)
    }

    pub(super) fn rebase_local_write_regions(
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        place: CanonicalPlace,
        call_frames: Option<&CallFrameResolver<'_>>,
    ) -> Option<Vec<StorageRegion>> {
        rebase_write_regions(
            program,
            state_symbol,
            statement_index,
            place,
            call_frames,
            false,
        )
    }

    fn rebase_write_regions(
        program: &dyn validation::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        place: CanonicalPlace,
        call_frames: Option<&CallFrameResolver<'_>>,
        descriptor_slot: bool,
    ) -> Option<Vec<StorageRegion>> {
        if !matches!(place.root, PlaceRoot::Symbol(symbol) if symbol.is_valid()) {
            return None;
        }
        if !place_requires_local_write_origin(
            program,
            state_symbol,
            statement_index,
            &place,
            call_frames,
        ) {
            return Some(vec![StorageRegion::Declared(place)]);
        }
        let root = place.root.as_symbol()?;
        let (machine, state) = find_state(program, state_symbol)?;
        let statements = program.statement_table.statements(state.statement_nodes);
        let mut owned_frames = None;
        let resolver = flow::shared_call_frames_or(call_frames, program, &mut owned_frames)?;
        let Some(origins) = resolver
            .local_write_origins_before_statement(machine, statements.get(statement_index)?)
        else {
            return reference_result_candidate_places(
                program,
                state_symbol,
                statement_index,
                root,
                &place,
                call_frames,
            )
            .map(|places| places.into_iter().map(StorageRegion::Declared).collect());
        };
        rebase_reachable_storage(
            program,
            state_symbol,
            statement_index,
            place,
            descriptor_slot,
            &origins,
        )
    }

    fn rebase_reachable_storage(
        program: &dyn validation::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        place: CanonicalPlace,
        descriptor_slot: bool,
        origins: &[LocalWriteOrigin],
    ) -> Option<Vec<StorageRegion>> {
        let (_, state) = find_state(program, state_symbol)?;
        let mut pending = vec![(place, Vec::<usize>::new())];
        let mut projected = Vec::new();
        while let Some((place, crossed)) = pending.pop() {
            let root = place.root.as_symbol()?;
            let mut retains_private_storage = true;
            for (origin_index, origin) in origins.iter().enumerate() {
                if origin.local_symbol != root
                    || !canonical_place_segments_may_overlap(
                        program,
                        &place.segments,
                        &origin.local_segments,
                    )
                {
                    continue;
                }
                if descriptor_slot && place.segments.len() == origin.local_segments.len() {
                    continue;
                }
                let descriptor = CanonicalPlace {
                    root: place.root,
                    segments: origin.local_segments.clone(),
                };
                let reference = canonical_place_type_reference(
                    program,
                    state_symbol,
                    statement_index,
                    &descriptor,
                )?;
                if declared_reference_access(program, reference)
                    .is_some_and(|access| !access.is_exclusive())
                    || origin
                        .source
                        .as_ref()
                        .is_some_and(|source| source.is_image_storage)
                {
                    continue;
                }
                let relative = if place.segments.len() >= origin.local_segments.len() {
                    retains_private_storage = false;
                    &place.segments[origin.local_segments.len()..]
                } else {
                    &[]
                };
                let region = write_source_region(
                    program,
                    state,
                    statement_index,
                    origin.source.as_ref()?,
                    relative,
                )?;
                let canonical = match region {
                    StorageRegion::Declared(place) => place,
                    region @ (StorageRegion::ResidentContent { .. }
                    | StorageRegion::NonresidentContent { .. }
                    | StorageRegion::UnresolvedContent { .. }) => {
                        for rebased in rebase_region_owner(region, |owner| {
                            rebase_reachable_storage(
                                program,
                                state_symbol,
                                statement_index,
                                owner,
                                false,
                                origins,
                            )
                        })? {
                            push_unique(&mut projected, rebased);
                        }
                        continue;
                    }
                };
                if canonical == place {
                    let region = StorageRegion::Declared(place.clone());
                    push_unique(&mut projected, region);
                    continue;
                }
                if crossed.contains(&origin_index) {
                    return None;
                }
                let mut next_crossed = crossed.clone();
                next_crossed.push(origin_index);
                pending.push((canonical, next_crossed));
            }
            let region = StorageRegion::Declared(place);
            if retains_private_storage && !projected.contains(&region) {
                projected.push(region);
            }
        }
        Some(projected)
    }

    /*
        A reference local bound from a checked reference result names one of
        finitely many candidate origins (flow/reference_places); a write through
        it lands on every candidate's corresponding place.
    */
    fn reference_result_candidate_places(
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        root: SymbolHandle,
        place: &CanonicalPlace,
        call_frames: Option<&CallFrameResolver<'_>>,
    ) -> Option<Vec<CanonicalPlace>> {
        let candidates = flow::reference_result_candidates_before_statement(
            program,
            state_symbol,
            statement_index,
            root,
            call_frames,
        )?;
        Some(
            candidates
                .into_iter()
                .map(|mut candidate| {
                    candidate.segments.extend_from_slice(&place.segments);
                    candidate
                })
                .collect(),
        )
    }

    /*
        A complete frame's terminal wildcard is an absorbing collection footprint,
        not a field name or an element identity. Retain the entire collection;
        ordinary origin rebasing still follows any carried reference leaves.
        Value/descriptor readers keep the exact dotted-path query below.
    */
    pub(super) fn frame_write_place(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        statement_index: usize,
        path: &str,
    ) -> Option<CanonicalPlace> {
        place_from_origin_path(
            program,
            state,
            statement_index,
            path.strip_suffix("[*]").unwrap_or(path),
        )
    }

    pub(crate) fn place_from_origin_path(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        statement_index: usize,
        path: &str,
    ) -> Option<CanonicalPlace> {
        let (path, collection_contents) = path
            .strip_suffix("[*]")
            .map_or((path, false), |collection| (collection, true));
        let mut members = path.split('.');
        let root_name = members.next()?;
        let root = program
            .state_parameters(state)
            .iter()
            .find_map(|parameter| {
                (parameter.name.as_str() == root_name).then_some(parameter.symbol)
            })
            .or_else(|| {
                flow::state_statements(program, state.statement_nodes)
                    .iter()
                    .take(statement_index)
                    .find_map(|statement| {
                        let local = statement.as_local_data()?;
                        (local.name.as_str() == root_name).then_some(local.symbol)
                    })
            })?;
        let mut place = canonical_place_from_symbol(root)?;
        for member in members {
            let symbol = resolve_member_symbol_from_place(
                program,
                &place,
                member,
                None,
                SymbolHandle::invalid(),
            )?;
            if !symbol.is_valid() {
                return None;
            }
            push_field_place_segments(program, &mut place.segments, symbol);
        }
        if collection_contents {
            place.segments.push(PlaceSegment::FixedRange {
                start: 0,
                end: usize::MAX,
            });
        }
        Some(place)
    }
}
pub(in crate::flow) use local_origins::captured_alias_projection;
mod operand_coordinates {
    /*
        Operation meaning required before call operands supply captured coordinates.
    */
    use validation::CallSite;
    use validation::call_site_argument_expressions;

    use checked_trees::expression::{ExpressionHandle, ExpressionNode};
    use symbols::SymbolHandle;

    pub(super) fn call_operands_have_builtin_coordinates(
        program: &dyn ::validation::CheckingInput,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        site: &CallSite<'_>,
    ) -> bool {
        let Some(machine) = program.machine_by_symbol(machine_symbol) else {
            return false;
        };
        let Some(state) = program.state_by_symbol(state_symbol) else {
            return false;
        };
        if !call_site_argument_expressions(program, site)
            .iter()
            .all(|argument| place_operands_are_builtin(program, machine, state, *argument, 0))
        {
            return false;
        }
        match site {
            CallSite::Expression { call, .. } if call.receiver.is_valid() => {
                place_operands_are_builtin(program, machine, state, call.receiver, 0)
            }
            _ => true,
        }
    }

    fn place_operands_are_builtin(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        expression: ExpressionHandle,
        depth: usize,
    ) -> bool {
        if depth >= 128 {
            return false;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow) => {
                place_operands_are_builtin(program, machine, state, borrow.target, depth + 1)
            }
            ExpressionNode::Member(member) => {
                place_operands_are_builtin(program, machine, state, member.receiver, depth + 1)
            }
            ExpressionNode::Indexed(indexed)
                if matches!(
                    program.expression_table.expression(indexed.index),
                    ExpressionNode::Range(_)
                ) =>
            {
                place_operands_are_builtin(program, machine, state, indexed.collection, depth + 1)
            }
            ExpressionNode::Indexed(_) => {
                validation::value_has_builtin_coordinates(program, machine, state, expression)
            }
            _ => true,
        }
    }
}
mod receiver {
    use validation::CallSite;
    use validation::CanonicalPlace;
    use validation::call_site_argument_expressions;
    use validation::canonical_place_from_expression;
    use validation::find_call_site;
    use validation::find_state_in_machine;

    use crate::flow::canonical_place_from_symbol;
    use crate::published::places::place_at;
    use crate::published::proof::contract_target_from_state_symbol;

    use checked_trees::{BorrowCallFact, BorrowFacts};
    use symbols::SymbolHandle;

    pub(crate) fn call_receiver_is_mutable(
        program: &dyn ::validation::CheckingInput,
        borrow: &BorrowFacts,
        borrow_call: &BorrowCallFact,
    ) -> bool {
        let Some((target_machine_symbol, target_state_symbol)) =
            contract_target_from_state_symbol(program, borrow_call.target_symbol)
        else {
            return false;
        };
        if find_state_in_machine(program, target_machine_symbol, target_state_symbol).is_none() {
            return false;
        }
        borrow_call.accesses.is_empty()
            && borrow.with_state_borrows(target_machine_symbol, target_state_symbol, |rows| {
                rows.iter()
                    .any(|handle| borrow.states.get(*handle).mutable_parameter_count > 0)
            })
    }

    pub(crate) fn call_receiver_mutated_place(
        program: &dyn ::validation::CheckingInput,
        caller_machine_symbol: SymbolHandle,
        caller_state_symbol: SymbolHandle,
        borrow_call: &BorrowCallFact,
    ) -> Option<CanonicalPlace> {
        let call_site = find_call_site(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow_call.statement_index,
            borrow_call.call_ordinal,
        )?;
        canonical_receiver_place_for_call_site(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            &call_site,
            borrow_call.statement_index,
        )
    }

    /*
        `statement_index` is the occurrence's recorded statement coordinate — the
        same index `find_call_site` resolved or the caller's own traversal
        recorded — so projected-receiver scope is derived from the recorded row,
        never from where the call node happens to sit in this arena slice. A
        replayed site carrying the same recorded coordinate reaches the same
        place; see `receiver_place_for_call_site`.
    */
    pub(crate) fn canonical_receiver_place_for_call_site(
        program: &dyn ::validation::CheckingInput,
        caller_machine_symbol: SymbolHandle,
        caller_state_symbol: SymbolHandle,
        call_site: &CallSite<'_>,
        statement_index: usize,
    ) -> Option<CanonicalPlace> {
        let place = receiver_place_for_call_site(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            call_site,
            statement_index,
        )?;
        Some(place)
    }

    fn receiver_place_for_call_site(
        program: &dyn ::validation::CheckingInput,
        caller_machine_symbol: SymbolHandle,
        caller_state_symbol: SymbolHandle,
        call_site: &CallSite<'_>,
        statement_index: usize,
    ) -> Option<CanonicalPlace> {
        match call_site {
            CallSite::Statement(statement) => {
                let state = program.state_by_symbol(caller_state_symbol)?;
                let statements = program.statement_table.statements(state.statement_nodes);
                if let Some(typed_trees::statement::StatementNode::Call(candidate)) =
                    statements.get(statement_index)
                    && *candidate == **statement
                    && let Some((root, segments)) = validation::projected_statement_receiver_place(
                        program,
                        state,
                        statement_index,
                        statement,
                    )
                {
                    return Some(CanonicalPlace {
                        root: facts::PlaceRoot::Symbol(root),
                        segments,
                    });
                }
                canonical_place_from_symbol(statement.receiver_symbol)
            }
            CallSite::Expression { call, .. } => {
                if call.receiver.is_valid()
                    && !program.call_has_no_runtime_receiver_for_target(call)
                {
                    return canonical_place_from_expression(program, call.receiver);
                }
                let argument = call_data_argument(program, caller_machine_symbol, call_site)?;
                place_at(program, caller_state_symbol, statement_index, argument)
            }
            CallSite::TransitionNamed { .. } => {
                let argument = call_data_argument(program, caller_machine_symbol, call_site)?;
                place_at(program, caller_state_symbol, statement_index, argument)
            }
        }
    }

    /*
        The record a machine's transfers carry: its own declared attachment, or the
        enclosing machine family's attachment reached through machine ancestors. A
        machine nested inside an attached host shares that host's data binding.
    */
    pub(crate) fn family_attached_data_symbol(
        program: &dyn ::validation::CheckingInput,
        machine_symbol: SymbolHandle,
    ) -> SymbolHandle {
        let mut scope = machine_symbol;
        loop {
            let Some(machine) = program.machine_by_symbol(scope) else {
                return SymbolHandle::invalid();
            };
            if machine.attached_data_symbol.is_valid() {
                return machine.attached_data_symbol;
            }
            let parent = program.symbols.get(scope).parent;
            if !parent.is_valid()
                || program.symbols.get(parent).kind != symbols::SymbolKind::Machine
            {
                return SymbolHandle::invalid();
            }
            scope = parent;
        }
    }

    /*
        Inherited declaration scope selects a type; the current signature must
        still contain exactly one explicit parameter of that type. An absent or
        ambiguous parameter supplies no storage and no ambient parent argument.
    */
    pub(crate) fn family_data_binding_parameter<'parameters>(
        program: &dyn ::validation::CheckingInput,
        machine: SymbolHandle,
        parameters: &'parameters [typed_trees::signature::StateParameter],
    ) -> Option<&'parameters typed_trees::signature::StateParameter> {
        program
            .data_binding_position(family_attached_data_symbol(program, machine), parameters)
            .map(|ordinal| &parameters[ordinal])
    }

    /*
        A selected call's explicit data operand, at its actual positional ordinal.
        The declaration qualifier supplies no storage: retain the actual root and
        projection, including when a different record is supplied to the successor.
    */
    pub(in crate::flow) fn call_data_argument(
        program: &dyn ::validation::CheckingInput,
        caller_machine_symbol: SymbolHandle,
        call_site: &CallSite<'_>,
    ) -> Option<typed_trees::expression::ExpressionHandle> {
        let target = program.state_by_symbol(validation::call_target_entry_symbol(
            program,
            call_site.target_symbol(program),
        ))?;
        let parameters = program.state_parameters(target);
        let subject = family_data_binding_parameter(program, caller_machine_symbol, parameters)?;
        let ordinal = typed_trees::signature::state_parameter_position(parameters, subject.symbol)?;
        call_site_argument_expressions(program, call_site)
            .get(ordinal)
            .copied()
    }
}
mod regions {
    /*
        A mutation footprint is not always storage inside its loan anchor.

        Extent views lend separately backed content, not the bytes of the Extent
        descriptor. Keep that boundary through summary substitution and completion:
        a content store invalidates observations but does not replace the owner.
        These are may-write coordinates, never evidence establishing a resident.
        Formation provenance comes from the captured, checked view operation; its
        type and numeric address cannot manufacture a content region.
    */
    use crate::flow::canonical_place_from_semantic_place;
    use crate::flow::domain::qualification_is_content_independent;
    use validation::CanonicalPlace;

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(crate) enum StorageRegion {
        Declared(CanonicalPlace),
        /*
            The carrier may hold externally writable capability storage whose
            backing cannot be enumerated. Its anchor is not that backing. This is
            a complete conservative effect (invalidate unknown observations), not
            authority to replace any additional declared ownership home.
        */
        UnresolvedContent {
            carrier: CanonicalPlace,
        },
        ResidentContent {
            owner: CanonicalPlace,
            formation: typed_trees::expression::ExpressionHandle,
            granted: language_semantics::SemanticDomainId,
            resident: language_semantics::SemanticDomainId,
        },
        /*
            A nonresident interpretation lends content through an unchanged
            descriptor. The authenticated operation identifies that address space;
            it does not establish backing/profile authority or a qualification.
        */
        NonresidentContent {
            owner: CanonicalPlace,
            formation: typed_trees::typed_trees::AccessExecutionDeclaration,
        },
    }

    impl StorageRegion {
        /*
            The declaration through which this effect is reached. For content this
            is a loan anchor, not an alternative spelling of the written bytes.
        */
        pub(crate) fn anchor(&self) -> &CanonicalPlace {
            match self {
                Self::Declared(place) => place,
                Self::ResidentContent { owner, .. } | Self::NonresidentContent { owner, .. } => {
                    owner
                }
                Self::UnresolvedContent { carrier } => carrier,
            }
        }

        pub(crate) fn into_declared(self) -> Option<CanonicalPlace> {
            match self {
                Self::Declared(place) => Some(place),
                Self::ResidentContent { .. }
                | Self::NonresidentContent { .. }
                | Self::UnresolvedContent { .. } => None,
            }
        }

        /*
            A consumer confined to declared value coordinates must retain unknown
            effects when another address space is present, never silently omit them.
        */
        pub(crate) fn declared_places(
            regions: impl IntoIterator<Item = Self>,
        ) -> Option<Vec<CanonicalPlace>> {
            regions.into_iter().map(Self::into_declared).collect()
        }

        pub(crate) fn same_storage_space(&self, other: &Self) -> bool {
            matches!(
                (self, other),
                (Self::Declared(_), Self::Declared(_))
                    | (Self::ResidentContent { .. }, Self::ResidentContent { .. })
                    | (
                        Self::NonresidentContent { .. },
                        Self::NonresidentContent { .. }
                    )
            )
        }

        pub(crate) fn contains(&self, other: &Self) -> bool {
            self.same_storage_space(other)
                && self.anchor().root == other.anchor().root
                && other.anchor().segments.starts_with(&self.anchor().segments)
        }

        /*
            Ordinary normal-return writes through a borrowed resident preserve its
            captured owner's qualifications. A nonresident content write can retain
            only an existing content-independent theory on the same descriptor.
            A runtime element selection may name a concrete incoming row: its content
            write leaves that row's descriptor unchanged whether selected or not. This
            conditional preservation is not selector equality or a new membership;
            fields, cases, roots and all other coordinates must still correspond.
            This retains an existing fact at that exact owner: it neither issues a
            qualification nor restores content observations. Borrowed-content
            checking must still restore every hole.
        */
        pub(crate) fn preserves_owner_qualification(
            &self,
            program: &dyn validation::CheckingInput,
            semantic: &facts::FactPlan,
            fact: &facts::Fact,
            place: facts::PlaceHandle,
        ) -> bool {
            let (domain_symbol, domain) = match fact.payload {
                facts::FactPayload::DomainMembership {
                    domain_symbol,
                    semantic_domain,
                    ..
                }
                | facts::FactPayload::ContractDomainMembership {
                    domain_symbol,
                    semantic_domain,
                    ..
                } => (domain_symbol, semantic_domain),
                _ => return false,
            };
            let owner = match self {
                Self::ResidentContent {
                    owner,
                    granted,
                    resident,
                    ..
                } if domain == *granted || domain == *resident => owner,
                Self::NonresidentContent { owner, .. }
                    if domain.is_valid()
                        && qualification_is_content_independent(program, domain_symbol) =>
                {
                    owner
                }
                _ => return false,
            };
            canonical_place_from_semantic_place(
                program,
                semantic,
                semantic.places.get(place),
            )
            .is_some_and(|place| {
                let same_segments = place.segments == owner.segments;
                let selected_descriptor = matches!(self, Self::NonresidentContent { .. })
                    && place.segments.len() == owner.segments.len()
                    && place.segments.iter().chain(&owner.segments).all(|segment| {
                        match *segment {
                            facts::PlaceSegment::Index { expression } =>
                                program.expression_table.expression_is_valid(expression)
                                    && program.expression_table.range_node(expression).is_none(),
                            facts::PlaceSegment::OpaqueRange => false,
                            _ => !validation::storage_path_relations::place_segment_has_unresolved_identity(*segment),
                        }
                    })
                    && place.segments.iter().zip(&owner.segments).all(|(left, right)| {
                        validation::storage_path_relations::canonical_place_segments_equal(program, *left, *right)
                            || (matches!(left, facts::PlaceSegment::FixedIndex { .. } | facts::PlaceSegment::Index { .. })
                                && matches!(right, facts::PlaceSegment::FixedIndex { .. } | facts::PlaceSegment::Index { .. })
                                && validation::storage_path_relations::canonical_place_segments_may_overlap(
                                    program, std::slice::from_ref(left), std::slice::from_ref(right),
                                ))
                    });
                if !same_segments && !selected_descriptor {
                    return false;
                }
                place.root == owner.root || matches!((place.root, owner.root),
                    (facts::PlaceRoot::Symbol(actual), facts::PlaceRoot::InvocationReferent { parameter, .. })
                    | (facts::PlaceRoot::InvocationReferent { parameter, .. }, facts::PlaceRoot::Symbol(actual))
                    if actual == parameter)
            })
        }
    }
}
mod summary {
    /*
        Structured caller-visible mutation summaries.

        `CallFrameResolver` remains the single owner of the
        complete-or-opaque call and cycle law. This module retains symbol-based
        field/range places for flow invalidation and propagates those places across
        the calls which the shared resolver admitted as complete.
        A boundary signature has no local state summary, but its complete frame is
        still a dependency: project its receiver and argument writes into caller
        storage before propagating them. Absence of a body is not opacity; absence
        of a complete frame or representable storage origin still is.
        Storage effects and exclusive access are separate: sealed atomic operations
        invalidate values but use shared access. Retain that operation provenance
        across calls instead of turning every caller-visible write into a mutable
        loan requirement. Ordinary replacement of an atomic cell remains exclusive.
    */
    use super::local_origins;
    use super::operand_coordinates::call_operands_have_builtin_coordinates;
    use facts::PlaceRoot;
    use facts::PlaceSegment;
    use typed_trees::state::State;
    use typed_trees::types::TypeReferenceHandle;
    use validation::call_site_argument_expressions;
    use validation::canonical_place_from_expression;
    use validation::find_call_site;
    use validation::{
        CallFrameResolver, CallSite, CanonicalPlace, ContentsRequirement, LocalWriteOrigin,
        assignment_stored_value_type, atomic_once_carrier, canonical_place_type_reference,
        checking_thread_pool, find_state, has_contents, project_type_reference_from_segments,
        reference_shell, storage_origin_types_match, transition_call_target,
    };

    use crate::flow::indexed_statements;
    use crate::flow::jump_loans::{HeldAccesses, TargetRun, held_accesses, target_run};
    use crate::flow::mutation::{CallMutationWrite, StorageRegion, WritePlaceNamespace};
    use crate::published::places::place_at;

    use checked_trees::expression::ExpressionNode;
    use checked_trees::{BorrowCallFact, BorrowFacts, StateBorrowFact};
    use symbols::{SymbolHandle, SymbolKeyMap};

    #[derive(Debug, Clone, Default)]
    pub(crate) struct StateMutationSummaryCache {
        /*
            Completed summaries depend only on the immutable program and borrow
            facts. Publish after the fixed point, never during recursive inference.
        */
        states: std::sync::OnceLock<BuiltStateMutationSummaries>,
    }

    impl StateMutationSummaryCache {
        pub(crate) fn prewarm(
            &self,
            program: &dyn ::validation::CheckingInput,
            borrow: &BorrowFacts,
            call_frames: Option<&CallFrameResolver<'_>>,
        ) {
            let _ = self
                .states
                .get_or_init(|| build_state_mutation_summaries(program, borrow, call_frames));
        }
    }

    #[derive(Debug, Clone)]
    struct BuiltStateMutationSummaries {
        summaries: Vec<StateMutationSummary>,
        index_by_state: SymbolKeyMap<SymbolHandle, usize>,
    }

    /*
        Complete transitive writes in a state's own incoming storage namespace.
        Cleanup uses the same summary as authored calls, without fabricating a
        source call coordinate or interpreting the frame's rendered report paths.
    */
    pub(crate) fn state_storage_writes(
        program: &dyn ::validation::CheckingInput,
        borrow: &BorrowFacts,
        cache: &StateMutationSummaryCache,
        state: &State,
        call_frames: Option<&CallFrameResolver<'_>>,
    ) -> Option<Vec<CanonicalPlace>> {
        StorageRegion::declared_places(
            state_mutation_summary(program, borrow, cache, state, call_frames)?
                .writes
                .iter()
                .map(|write| write.region.clone()),
        )
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct StateMutationSummary {
        state_symbol: SymbolHandle,
        complete: bool,
        /* The whole run's writes, which a call holds its argument loans across. */
        writes: Vec<MutationWrite>,
        /* Writes while each incoming parameter's loans remain held. */
        held_writes: Vec<HeldAccesses<MutationWrite>>,
    }

    /*
        A reference-valued receiving cell is distinct from the storage its value
        denotes. Preserve that destination judgment until caller projection ends.
    */
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(super) enum WriteDestination {
        Referent,
        DescriptorSlot,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct MutationWrite {
        region: StorageRegion,
        requires_exclusive_access: bool,
        destination: WriteDestination,
        checked_write_type: TypeReferenceHandle,
    }

    pub(super) fn instantiate_known_call_mutation_summary_places(
        program: &dyn ::validation::CheckingInput,
        caller_machine_symbol: SymbolHandle,
        caller_state_symbol: SymbolHandle,
        borrow: &BorrowFacts,
        borrow_call: &BorrowCallFact,
        cache: &StateMutationSummaryCache,
        namespace: WritePlaceNamespace,
        call_frames: Option<&CallFrameResolver<'_>>,
        loan: Option<&CanonicalPlace>,
    ) -> Option<Vec<CallMutationWrite>> {
        let target_state = program.state_by_symbol(borrow_call.target_symbol)?;
        let summary = state_mutation_summary(program, borrow, cache, target_state, call_frames);
        let summary = summary?;
        let run = target_run(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow_call,
        );
        let summary_places = run.accesses(program, &summary.writes, &summary.held_writes, loan);

        let mut instantiated = Vec::new();
        for write in summary_places {
            if matches!(namespace, WritePlaceNamespace::AccessRoute)
                && !write.requires_exclusive_access
            {
                continue;
            }
            let contributions = instantiate_call_relative_writes(
                program,
                caller_machine_symbol,
                caller_state_symbol,
                borrow_call,
                &write.region,
                write.destination,
                namespace,
                call_frames,
            )?;
            for mut contribution in contributions {
                contribution.checked_write_type = write.checked_write_type;
                crate::shared::collections::push_unique(&mut instantiated, contribution);
            }
        }

        Some(instantiated)
    }

    /*
        The callee's complete summary restricted to writes reached through its
        data binding, instantiated at the call's actual parent place: the whole
        run's writes for a call, the held run's for a jump, as far as the call's
        own argument loans reach. `None` when the callee has no complete summary or
        a receiver write cannot be placed.
    */
    pub(super) fn instantiate_known_call_receiver_summary_places(
        program: &dyn ::validation::CheckingInput,
        caller_machine_symbol: SymbolHandle,
        caller_state_symbol: SymbolHandle,
        borrow: &BorrowFacts,
        borrow_call: &BorrowCallFact,
        cache: &StateMutationSummaryCache,
        loan: &CanonicalPlace,
    ) -> Option<Vec<CanonicalPlace>> {
        let target_state = program.state_by_symbol(borrow_call.target_symbol)?;
        let target_machine_symbol = machine_symbol_for_state(program, target_state);
        let target_machine = program.machine_by_symbol(target_machine_symbol)?;
        let data_binding = super::family_data_binding_parameter(
            program,
            target_machine.symbol,
            program.state_parameters(target_state),
        );
        let summary = state_mutation_summary(program, borrow, cache, target_state, None)?;
        let run = target_run(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow_call,
        );
        let summary_places =
            run.accesses(program, &summary.writes, &summary.held_writes, Some(loan));

        let mut instantiated = Vec::new();
        for write in summary_places {
            if !write.requires_exclusive_access {
                continue;
            }
            let summary_place = write.region.anchor();
            let root = summary_place.root.as_symbol()?;
            let reached_through_receiver = root == target_machine_symbol
                || data_binding.is_some_and(|parameter| parameter.symbol == root);
            if !reached_through_receiver {
                continue;
            }
            for place in instantiate_call_relative_places(
                program,
                caller_machine_symbol,
                caller_state_symbol,
                borrow_call,
                summary_place,
                WritePlaceNamespace::AccessRoute,
                None,
            )? {
                crate::shared::collections::push_unique(&mut instantiated, place);
            }
        }

        Some(instantiated)
    }

    fn state_mutation_summary<'cache>(
        program: &dyn ::validation::CheckingInput,
        borrow: &BorrowFacts,
        cache: &'cache StateMutationSummaryCache,
        state: &State,
        call_frames: Option<&CallFrameResolver<'_>>,
    ) -> Option<&'cache StateMutationSummary> {
        let built = cache
            .states
            .get_or_init(|| build_state_mutation_summaries(program, borrow, call_frames));
        let &index = built.index_by_state.get(&state.symbol)?;
        built.summaries.get(index).filter(|entry| entry.complete)
    }

    fn build_state_mutation_summaries(
        program: &dyn ::validation::CheckingInput,
        borrow: &BorrowFacts,
        call_frames: Option<&CallFrameResolver<'_>>,
    ) -> BuiltStateMutationSummaries {
        checking_thread_pool().install(|| {
            use rayon::prelude::*;
            let mut owned_frames = None;
            let call_frames =
                crate::flow::shared_call_frames_or(call_frames, program, &mut owned_frames);

            let (mut states, borrow_states_for_summaries): (Vec<_>, Vec<_>) = {
                let borrow_state_refs: Vec<_> =
                    borrow.states.iter().map(|(_, state)| state).collect();
                borrow_state_refs
                    .into_par_iter()
                    .filter_map(|borrow_state| {
                        let state = program.state_by_symbol(borrow_state.state_symbol)?;
                        let direct_writes =
                            collect_state_mutation_summary_places(program, state, call_frames);
                        let direct_complete = direct_writes.is_some();
                        let mut writes = direct_writes.unwrap_or_default();
                        let cleanup_complete =
                            push_cleanup_summary_writes(program, state, call_frames, &mut writes)
                                .is_some();
                        let complete = direct_complete
                            && cleanup_complete
                            && state_has_concrete_body_signature(program, state)
                            && call_frames.is_some_and(|resolver| {
                                find_state(program, state.symbol).is_some_and(|(machine, _)| {
                                    resolver
                                        .inferred_state_write_frame(machine, state)
                                        .is_complete()
                                })
                            });
                        Some((
                            StateMutationSummary {
                                state_symbol: state.symbol,
                                held_writes: held_accesses(program, state, &writes),
                                complete,
                                writes,
                            },
                            borrow_state,
                        ))
                    })
                    .unzip()
            };

            let mut index_by_state: SymbolKeyMap<SymbolHandle, usize> = SymbolKeyMap::default();
            for (index, summary) in states.iter().enumerate() {
                index_by_state.entry(summary.state_symbol).or_insert(index);
            }

            {
                propagate_state_mutation_summaries(
                    program,
                    borrow,
                    &borrow_states_for_summaries,
                    &index_by_state,
                    &mut states,
                    call_frames,
                );
            };
            BuiltStateMutationSummaries {
                summaries: states,
                index_by_state,
            }
        })
    }

    /*
        Automatic cleanup is a write no statement spells: a `dropped by` hook
        runs when an owned value dies and may store through every exclusive
        reference that value holds. Validation owns which values may die in a
        state and what their hooks write, already carried to parameter-rooted
        places. Without these rows a callee whose only effect is to let such a
        value die has a complete empty summary, and a caller fact about the
        referent survives the call. An unknown cleanup footprint leaves the
        summary incomplete, which sends callers to the signature ceiling.
    */
    fn push_cleanup_summary_writes(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        call_frames: Option<&CallFrameResolver<'_>>,
        writes: &mut Vec<MutationWrite>,
    ) -> Option<()> {
        let resolver = call_frames?;
        let (machine, _) = find_state(program, state.symbol)?;
        for path in resolver.state_cleanup_write_paths(machine, state)? {
            let write = MutationWrite {
                region: StorageRegion::Declared(local_origins::frame_write_place(
                    program, state, 0, &path,
                )?),
                requires_exclusive_access: true,
                destination: WriteDestination::Referent,
                checked_write_type: Default::default(),
            };
            if state_summary_exposes_place(program, state, &write.region, call_frames)?
                && !writes.contains(&write)
            {
                writes.push(write);
            }
        }
        Some(())
    }

    fn propagate_state_mutation_summaries(
        program: &dyn ::validation::CheckingInput,
        borrow: &BorrowFacts,
        borrow_states: &[&StateBorrowFact],
        index_by_state: &SymbolKeyMap<SymbolHandle, usize>,
        states: &mut [StateMutationSummary],
        call_frames: Option<&CallFrameResolver<'_>>,
    ) {
        use rayon::prelude::*;
        let owners: Vec<_> = states
            .iter()
            .map(|summary| {
                program
                    .state_by_symbol(summary.state_symbol)
                    .map(|state| (state, machine_symbol_for_state(program, state)))
            })
            .collect();
        let exposed_parameters: Vec<Vec<SymbolHandle>> = owners
            .par_iter()
            .map(|owner| {
                let Some((caller_state, _)) = owner else {
                    return Vec::new();
                };
                program
                    .state_parameters(caller_state)
                    .iter()
                    .filter(|parameter| {
                        program
                            .primitive_type_reference(parameter.type_reference)
                            .is_none()
                            && call_frames.is_none_or(|frames| {
                                !frames.value_is_reference_free(parameter.type_reference)
                            })
                    })
                    .map(|parameter| parameter.symbol)
                    .collect()
            })
            .collect();
        let mut calls: Vec<_> = {
            (0..states.len())
                .into_par_iter()
                .map(|caller_index| {
                    let state = &states[caller_index];
                    let borrow_state = borrow_states[caller_index];
                    let owner = &owners[caller_index];
                    borrow
                        .calls
                        .span_or_empty(borrow_state.calls)
                        .iter()
                        .map(|call| {
                            let run = owner.map_or(TargetRun::Whole, |(_, machine_symbol)| {
                                target_run(program, machine_symbol, state.state_symbol, call)
                            });
                            let body_summary = find_state(program, call.target_symbol)
                                .filter(|(machine, _)| machine.body_is_present)
                                .and_then(|_| index_by_state.get(&call.target_symbol).copied());
                            let call_site = owner.and_then(|(_, machine_symbol)| {
                                find_call_site(
                                    program,
                                    machine_symbol,
                                    state.state_symbol,
                                    call.statement_index,
                                    call.call_ordinal,
                                )
                            });
                            let has_builtin_coordinates = match (*owner, call_site.as_ref()) {
                                (Some((_, machine_symbol)), Some(site)) => {
                                    call_operands_have_builtin_coordinates(
                                        program,
                                        machine_symbol,
                                        state.state_symbol,
                                        site,
                                    )
                                }
                                _ => false,
                            };
                            (
                                call,
                                body_summary,
                                run,
                                call_site,
                                has_builtin_coordinates,
                                Vec::<(
                                    (StorageRegion, WriteDestination),
                                    Option<Vec<MutationWrite>>,
                                )>::new(),
                                0usize,
                                Vec::<usize>::new(),
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .collect()
        };
        let mut callers = vec![Vec::new(); states.len()];
        for (caller_index, dependencies) in calls.iter().enumerate() {
            for (_, target_index, _, _, _, _, _, _) in dependencies {
                if let Some(target_index) = target_index
                    && !callers[*target_index].contains(&caller_index)
                {
                    callers[*target_index].push(caller_index);
                }
            }
        }
        let mut dirty_mask = vec![true; states.len()];
        let mut has_dirty = !states.is_empty();
        while has_dirty {
            let updates: Vec<_> = calls
                .par_iter_mut()
                .enumerate()
                .filter_map(|(caller_index, caller_calls)| {
                    if !dirty_mask[caller_index] || !states[caller_index].complete {
                        return None;
                    }
                    let caller_symbol = states[caller_index].state_symbol;
                    let (caller_state, caller_machine) = owners[caller_index]?;
                    let caller_exposed_params = &exposed_parameters[caller_index];
                    let caller = &states[caller_index];
                    let mut additions = Vec::new();
                    let mut held_additions = vec![Vec::new(); caller.held_writes.len()];
                    let mut expose =
                        |writes: Vec<MutationWrite>, held_writes: Vec<Vec<MutationWrite>>| {
                            let retain =
                                |existing: &[MutationWrite],
                                 pending: &mut Vec<MutationWrite>,
                                 writes: Vec<MutationWrite>| {
                                    for write in writes {
                                        let exposed = {
                                            let declared =
                                                matches!(write.region, StorageRegion::Declared(_));
                                            match write.region.anchor().root.as_symbol() {
                                                None => {
                                                    if !declared {
                                                        return None;
                                                    }
                                                    false
                                                }
                                                Some(root) if root == caller_machine => true,
                                                Some(root)
                                                    if program
                                                        .state_parameters(caller_state)
                                                        .iter()
                                                        .any(|parameter| {
                                                            parameter.symbol == root
                                                        }) =>
                                                {
                                                    !declared
                                                        || caller_exposed_params.contains(&root)
                                                }
                                                Some(_) => {
                                                    if !declared {
                                                        return None;
                                                    }
                                                    false
                                                }
                                            }
                                        };
                                        if exposed
                                            && !existing.contains(&write)
                                            && !pending.contains(&write)
                                        {
                                            pending.push(write);
                                        }
                                    }
                                    Some(())
                                };
                            retain(&caller.writes, &mut additions, writes)?;
                            for ((held, pending), writes) in caller
                                .held_writes
                                .iter()
                                .zip(&mut held_additions)
                                .zip(held_writes)
                            {
                                retain(&held.accesses, pending, writes)?;
                            }
                            Some(())
                        };
                    let mut complete = true;
                    for (
                        call,
                        target_index,
                        run,
                        call_site,
                        has_builtin_coordinates,
                        inst_cache,
                        seen_writes,
                        seen_held,
                    ) in caller_calls.iter_mut()
                    {
                        let Some(target_index) = *target_index else {
                            if *seen_writes > 0 {
                                continue;
                            }
                            let Some(writes) = super::shared_call_storage_regions(
                                program,
                                caller_machine,
                                caller_symbol,
                                borrow,
                                call,
                                call_frames,
                            ) else {
                                complete = false;
                                break;
                            };
                            let writes: Vec<_> = writes
                                .into_iter()
                                .map(|region| MutationWrite {
                                    region,
                                    requires_exclusive_access: true,
                                    destination: WriteDestination::Referent,
                                    checked_write_type: Default::default(),
                                })
                                .collect();
                            let held = vec![writes.clone(); caller.held_writes.len()];
                            if expose(writes, held).is_none() {
                                complete = false;
                                break;
                            }
                            *seen_writes = 1;
                            continue;
                        };
                        let target = &states[target_index];
                        if !target.complete {
                            complete = false;
                            break;
                        }
                        let Some(call_site) = call_site.as_ref() else {
                            complete = false;
                            break;
                        };
                        let Some((target_state, _)) = owners[target_index] else {
                            complete = false;
                            break;
                        };
                        let prev_writes = *seen_writes;
                        let new_writes = &target.writes[prev_writes.min(target.writes.len())..];
                        let is_whole_run = matches!(run, TargetRun::Whole);
                        if !is_whole_run && seen_held.len() < target.held_writes.len() {
                            seen_held.resize(target.held_writes.len(), 0);
                        }
                        if new_writes.is_empty()
                            && (is_whole_run
                                || seen_held
                                    .iter()
                                    .zip(&target.held_writes)
                                    .all(|(&s, h)| s >= h.accesses.len()))
                        {
                            continue;
                        }
                        let has_builtin_coords = *has_builtin_coordinates;
                        let call_ref = *call;
                        let mut instantiate =
                            |writes: &[MutationWrite],
                             use_cache: bool|
                             -> Option<Vec<MutationWrite>> {
                                if !writes.is_empty() && !has_builtin_coords {
                                    return None;
                                }
                                let mut instantiated = Vec::new();
                                for write in writes {
                                    if !use_cache {
                                        let contributions =
                                            instantiate_call_relative_writes_pre_resolved(
                                                program,
                                                caller_machine,
                                                caller_symbol,
                                                call_ref,
                                                call_site,
                                                has_builtin_coords,
                                                target_state,
                                                &write.region,
                                                write.destination,
                                                WritePlaceNamespace::Storage,
                                                call_frames,
                                            )?;
                                        for contribution in contributions {
                                            instantiated.push(MutationWrite {
                                                region: contribution.region,
                                                requires_exclusive_access: write
                                                    .requires_exclusive_access,
                                                destination: write.destination,
                                                checked_write_type: write.checked_write_type,
                                            });
                                        }
                                        continue;
                                    }
                                    let cached_index = inst_cache.iter().position(|((r, d), _)| {
                                        r == &write.region && *d == write.destination
                                    });
                                    let contributions = match cached_index {
                                        Some(idx) => inst_cache[idx].1.as_ref()?,
                                        None => {
                                            let computed =
                                                instantiate_call_relative_writes_pre_resolved(
                                                    program,
                                                    caller_machine,
                                                    caller_symbol,
                                                    call_ref,
                                                    call_site,
                                                    has_builtin_coords,
                                                    target_state,
                                                    &write.region,
                                                    write.destination,
                                                    WritePlaceNamespace::Storage,
                                                    call_frames,
                                                )
                                                .map(|contributions| {
                                                    contributions
                                                        .into_iter()
                                                        .map(|contribution| MutationWrite {
                                                            region: contribution.region,
                                                            requires_exclusive_access: write
                                                                .requires_exclusive_access,
                                                            destination: write.destination,
                                                            checked_write_type: write
                                                                .checked_write_type,
                                                        })
                                                        .collect::<Vec<_>>()
                                                });
                                            inst_cache.push((
                                                (write.region.clone(), write.destination),
                                                computed,
                                            ));
                                            inst_cache.last().unwrap().1.as_ref()?
                                        }
                                    };
                                    for item in contributions {
                                        instantiated.push(MutationWrite {
                                            region: item.region.clone(),
                                            requires_exclusive_access: write
                                                .requires_exclusive_access,
                                            destination: write.destination,
                                            checked_write_type: write.checked_write_type,
                                        });
                                    }
                                }
                                Some(instantiated)
                            };
                        let Some(whole) = instantiate(new_writes, !is_whole_run) else {
                            complete = false;
                            break;
                        };
                        let held = if is_whole_run {
                            vec![whole.clone(); caller.held_writes.len()]
                        } else {
                            let Some(held) = caller
                                .held_writes
                                .iter()
                                .map(|incoming| {
                                    let loan = CanonicalPlace {
                                        root: PlaceRoot::Symbol(incoming.parameter),
                                        segments: Vec::new(),
                                    };
                                    let selected: Vec<_> = run
                                        .new_accesses(
                                            program,
                                            &target.writes,
                                            prev_writes,
                                            &target.held_writes,
                                            seen_held,
                                            Some(&loan),
                                        )
                                        .cloned()
                                        .collect();
                                    instantiate(&selected, true)
                                })
                                .collect::<Option<Vec<_>>>()
                            else {
                                complete = false;
                                break;
                            };
                            held
                        };
                        if expose(whole, held).is_none() {
                            complete = false;
                            break;
                        }
                        *seen_writes = target.writes.len();
                        if !is_whole_run {
                            for (s, h) in seen_held.iter_mut().zip(&target.held_writes) {
                                *s = h.accesses.len();
                            }
                        }
                    }
                    if !complete
                        || !additions.is_empty()
                        || held_additions.iter().any(|pending| !pending.is_empty())
                    {
                        Some((caller_index, complete, additions, held_additions))
                    } else {
                        None
                    }
                })
                .collect();
            dirty_mask.fill(false);
            has_dirty = false;
            for (caller_index, complete, additions, held_additions) in updates {
                let summary = &mut states[caller_index];
                summary.complete = complete;
                if complete {
                    summary.writes.extend(additions);
                    for (held, additions) in summary.held_writes.iter_mut().zip(held_additions) {
                        held.accesses.extend(additions);
                    }
                } else {
                    summary.writes.clear();
                    summary.held_writes.clear();
                }
                for &dependent in &callers[caller_index] {
                    if !dirty_mask[dependent] {
                        dirty_mask[dependent] = true;
                        has_dirty = true;
                    }
                }
            }
        }
    }

    fn machine_symbol_for_state(
        program: &dyn ::validation::CheckingInput,
        state: &State,
    ) -> SymbolHandle {
        find_state(program, state.symbol)
            .map(|(machine, _)| machine.symbol)
            .unwrap_or_else(SymbolHandle::invalid)
    }

    fn state_has_concrete_body_signature(
        program: &dyn ::validation::CheckingInput,
        state: &State,
    ) -> bool {
        find_state(program, state.symbol).is_some_and(|(machine, _)| {
            machine.body_is_present
                && program
                    .state_parameters(state)
                    .iter()
                    .all(|parameter| parameter.type_reference.is_valid())
        })
    }

    /*
        Declared local storage can be private; content reached through a local owner
        cannot. Without an exact exportable owner coordinate the summary is unknown,
        so callers retain the signature ceiling rather than an empty effect.
    */
    fn state_summary_exposes_place(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        region: &StorageRegion,
        call_frames: Option<&CallFrameResolver<'_>>,
    ) -> Option<bool> {
        let declared = matches!(region, StorageRegion::Declared(_));
        let Some(root) = region.anchor().root.as_symbol() else {
            return declared.then_some(false);
        };
        if root == machine_symbol_for_state(program, state) {
            return Some(true);
        }
        let Some(parameter) = program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.symbol == root)
        else {
            return declared.then_some(false);
        };
        if !declared {
            return Some(true);
        }
        Some(
            program
                .primitive_type_reference(parameter.type_reference)
                .is_none()
                && call_frames
                    .is_none_or(|frames| !frames.value_is_reference_free(parameter.type_reference)),
        )
    }

    fn collect_state_mutation_summary_places(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        call_frames: Option<&CallFrameResolver<'_>>,
    ) -> Option<Vec<MutationWrite>> {
        let machine_symbol = machine_symbol_for_state(program, state);
        let machine = program.machine_by_symbol(machine_symbol)?;
        let mut writes = Vec::new();

        for (statement_index, statement) in indexed_statements(program, state.statement_nodes) {
            if let Some(local) = statement.as_local_data()
                && atomic_once_carrier(program, local.initial_value).is_some()
            {
                for place in super::atomic_once_storage_writes(
                    program,
                    machine_symbol,
                    state.symbol,
                    statement_index,
                    local.initial_value,
                    call_frames,
                )? {
                    let write = MutationWrite {
                        region: StorageRegion::Declared(place),
                        requires_exclusive_access: false,
                        destination: WriteDestination::Referent,
                        checked_write_type: Default::default(),
                    };
                    if state_summary_exposes_place(program, state, &write.region, call_frames)?
                        && !writes.contains(&write)
                    {
                        writes.push(write);
                    }
                }
                continue;
            }
            let Some(assignment) = statement.as_assignment() else {
                continue;
            };
            let requires_exclusive_access =
                super::assignment_requires_exclusive_access(program, machine, state, assignment);
            let regions = local_origins::assignment_storage_regions(
                program,
                machine_symbol,
                state.symbol,
                statement_index,
                statement,
                call_frames,
            )?;
            let checked_write_type =
                assignment_stored_value_type(program, machine, state, assignment)?;
            for region in regions {
                let destination = if reference_shell(program, checked_write_type).is_some() {
                    WriteDestination::DescriptorSlot
                } else {
                    WriteDestination::Referent
                };
                let write = MutationWrite {
                    region,
                    requires_exclusive_access,
                    destination,
                    checked_write_type: if requires_exclusive_access {
                        checked_write_type
                    } else {
                        Default::default()
                    },
                };
                if state_summary_exposes_place(program, state, &write.region, call_frames)?
                    && !writes.contains(&write)
                {
                    writes.push(write);
                }
            }
        }

        Some(writes)
    }

    pub(in crate::flow) fn instantiate_call_relative_places(
        program: &dyn ::validation::CheckingInput,
        caller_machine_symbol: SymbolHandle,
        caller_state_symbol: SymbolHandle,
        borrow_call: &BorrowCallFact,
        relative_place: &CanonicalPlace,
        namespace: WritePlaceNamespace,
        call_frames: Option<&CallFrameResolver<'_>>,
    ) -> Option<Vec<CanonicalPlace>> {
        instantiate_call_relative_writes(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow_call,
            &StorageRegion::Declared(relative_place.clone()),
            WriteDestination::Referent,
            namespace,
            call_frames,
        )
        .and_then(|writes| {
            StorageRegion::declared_places(writes.into_iter().map(|write| write.region))
        })
    }

    fn instantiate_call_relative_writes(
        program: &dyn ::validation::CheckingInput,
        caller_machine_symbol: SymbolHandle,
        caller_state_symbol: SymbolHandle,
        borrow_call: &BorrowCallFact,
        relative_region: &StorageRegion,
        destination: WriteDestination,
        namespace: WritePlaceNamespace,
        call_frames: Option<&CallFrameResolver<'_>>,
    ) -> Option<Vec<CallMutationWrite>> {
        let inputs = prepare_call_relative_inputs(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow_call,
            namespace,
        )?;
        instantiate_call_relative_writes_pre_resolved(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow_call,
            &inputs.site,
            inputs.has_builtin_coordinates,
            inputs.target,
            relative_region,
            destination,
            namespace,
            call_frames,
        )
    }

    /* Immutable coordinates shared by every relative path of one invocation. */
    struct CallRelativeInputs<'program> {
        site: CallSite<'program>,
        has_builtin_coordinates: bool,
        target: &'program State,
    }

    fn prepare_call_relative_inputs<'program>(
        program: &'program dyn ::validation::CheckingInput,
        caller_machine_symbol: SymbolHandle,
        caller_state_symbol: SymbolHandle,
        borrow_call: &BorrowCallFact,
        namespace: WritePlaceNamespace,
    ) -> Option<CallRelativeInputs<'program>> {
        let call_site = find_call_site(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow_call.statement_index,
            borrow_call.call_ordinal,
        )?;
        let has_builtin_coordinates = !matches!(namespace, WritePlaceNamespace::Storage)
            || call_operands_have_builtin_coordinates(
                program,
                caller_machine_symbol,
                caller_state_symbol,
                &call_site,
            );
        let target_state = program.state_by_symbol(borrow_call.target_symbol)?;
        Some(CallRelativeInputs {
            site: call_site,
            has_builtin_coordinates,
            target: target_state,
        })
    }

    /*
        One immutable access-route interpretation, resolved on its first path only.
        The varying relative place is still judged by the shared instantiation body.
    */
    pub(in crate::flow) struct AccessRouteReader<'input> {
        program: &'input dyn ::validation::CheckingInput,
        caller_machine: SymbolHandle,
        caller_state: SymbolHandle,
        call: &'input BorrowCallFact,
        inputs: std::cell::OnceCell<Option<CallRelativeInputs<'input>>>,
    }

    impl<'input> AccessRouteReader<'input> {
        pub(in crate::flow) fn new(
            program: &'input dyn ::validation::CheckingInput,
            caller_machine: SymbolHandle,
            caller_state: SymbolHandle,
            call: &'input BorrowCallFact,
        ) -> Self {
            Self {
                program,
                caller_machine,
                caller_state,
                call,
                inputs: std::cell::OnceCell::new(),
            }
        }

        pub(in crate::flow) fn instantiate(
            &self,
            relative_place: &CanonicalPlace,
        ) -> Option<Vec<CanonicalPlace>> {
            let inputs = self
                .inputs
                .get_or_init(|| {
                    prepare_call_relative_inputs(
                        self.program,
                        self.caller_machine,
                        self.caller_state,
                        self.call,
                        WritePlaceNamespace::AccessRoute,
                    )
                })
                .as_ref()?;
            instantiate_call_relative_writes_pre_resolved(
                self.program,
                self.caller_machine,
                self.caller_state,
                self.call,
                &inputs.site,
                inputs.has_builtin_coordinates,
                inputs.target,
                &StorageRegion::Declared(relative_place.clone()),
                WriteDestination::Referent,
                WritePlaceNamespace::AccessRoute,
                None,
            )
            .and_then(|writes| {
                StorageRegion::declared_places(writes.into_iter().map(|write| write.region))
            })
        }
    }

    fn instantiate_call_relative_writes_pre_resolved(
        program: &dyn ::validation::CheckingInput,
        caller_machine_symbol: SymbolHandle,
        caller_state_symbol: SymbolHandle,
        borrow_call: &BorrowCallFact,
        call_site: &CallSite<'_>,
        has_builtin_coordinates: bool,
        target_state: &State,
        relative_region: &StorageRegion,
        destination: WriteDestination,
        namespace: WritePlaceNamespace,
        call_frames: Option<&CallFrameResolver<'_>>,
    ) -> Option<Vec<CallMutationWrite>> {
        let relative_place = relative_region.anchor();
        let attributed = |regions: Vec<StorageRegion>| {
            regions
                .into_iter()
                .map(|region| CallMutationWrite {
                    region,
                    source: Some(relative_region.clone()),
                    checked_write_type: Default::default(),
                })
                .collect()
        };
        let parameter_symbol = relative_place.root.as_symbol()?;
        if matches!(namespace, WritePlaceNamespace::Storage) && !has_builtin_coordinates {
            return None;
        }
        let mut argument_index = 0usize;

        for parameter in program.state_parameters(target_state) {
            let base_place = {
                let argument = call_site_argument_expressions(program, call_site)
                    .get(argument_index)
                    .copied();
                argument_index = argument_index.saturating_add(1);
                if parameter.symbol != parameter_symbol {
                    continue;
                }
                if matches!(relative_region, StorageRegion::Declared(_))
                    && !write_reaches_caller(
                        program,
                        parameter.type_reference,
                        &relative_place.segments,
                        destination,
                    )?
                {
                    return Some(Vec::new());
                }
                if matches!(namespace, WritePlaceNamespace::Storage) {
                    let regions = captured_argument_write_regions(
                        program,
                        caller_machine_symbol,
                        caller_state_symbol,
                        borrow_call.statement_index,
                        borrow_call.call_ordinal,
                        call_site,
                        argument_index.checked_sub(1)?,
                        parameter.type_reference,
                        &relative_place.segments,
                        destination,
                        call_frames,
                    )?;
                    let regions = match relative_region {
                        StorageRegion::Declared(_) => regions,
                        StorageRegion::ResidentContent {
                            formation,
                            granted,
                            resident,
                            ..
                        } => regions
                            .into_iter()
                            .map(|region| {
                                Some(StorageRegion::ResidentContent {
                                    owner: region.into_declared()?,
                                    formation: *formation,
                                    granted: *granted,
                                    resident: *resident,
                                })
                            })
                            .collect::<Option<Vec<_>>>()?,
                        StorageRegion::NonresidentContent { formation, .. } => regions
                            .into_iter()
                            .map(|region| {
                                Some(StorageRegion::NonresidentContent {
                                    owner: region.into_declared()?,
                                    formation: *formation,
                                })
                            })
                            .collect::<Option<Vec<_>>>()?,
                        StorageRegion::UnresolvedContent { .. } => regions
                            .into_iter()
                            .map(|region| StorageRegion::UnresolvedContent {
                                carrier: region.anchor().clone(),
                            })
                            .collect(),
                    };
                    return Some(attributed(regions));
                }
                if let Some(expression) = argument
                    && matches!(namespace, WritePlaceNamespace::AccessRoute)
                    && matches!(
                        program.expression_table.expression(expression),
                        ExpressionNode::StructLiteral(_) | ExpressionNode::ArrayLiteral(_)
                    )
                    && let Some(places) = crate::flow::literal_argument_access_places(
                        program,
                        caller_state_symbol,
                        borrow_call.statement_index,
                        expression,
                        parameter.type_reference,
                        &relative_place.segments,
                    )
                {
                    return Some(attributed(
                        places.into_iter().map(StorageRegion::Declared).collect(),
                    ));
                }
                argument.and_then(|expression| {
                    place_at(
                        program,
                        caller_state_symbol,
                        borrow_call.statement_index,
                        expression,
                    )
                    .or_else(|| canonical_place_from_expression(program, expression))
                })
            }?;

            let mut instantiated = base_place;
            instantiated
                .segments
                .extend(relative_place.segments.iter().copied());
            return Some(attributed(vec![StorageRegion::Declared(instantiated)]));
        }

        None
    }

    pub(super) fn captured_argument_write_regions(
        program: &dyn ::validation::CheckingInput,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        call_ordinal: usize,
        site: &CallSite<'_>,
        position: usize,
        parameter_type: TypeReferenceHandle,
        relative: &[PlaceSegment],
        destination: WriteDestination,
        call_frames: Option<&CallFrameResolver<'_>>,
    ) -> Option<Vec<StorageRegion>> {
        let machine = program.machine_by_symbol(machine_symbol)?;
        let state = program.state_by_symbol(state_symbol)?;
        let resolver = call_frames?;
        let captured = match site {
            CallSite::Statement(call) => resolver.call_write_origins(machine, call),
            CallSite::Expression { expression, .. } => {
                resolver.expression_call_write_origins(machine, *expression)
            }
            CallSite::TransitionNamed { .. } => {
                transition_call_target(program, machine, state, statement_index, call_ordinal)
                    .and_then(|target| resolver.transition_write_origins(machine, target))
            }
        };
        let references = captured
            .as_ref()
            .and_then(|origins| origins.references.as_deref());
        if reference_shell(program, parameter_type).is_none() {
            let actual = *call_site_argument_expressions(program, site).get(position)?;
            let mut regions = Vec::new();
            for projection in crate::flow::literal_expression_projections(
                program,
                actual,
                parameter_type,
                relative,
                false,
            )? {
                let projected_type = project_type_reference_from_segments(
                    program,
                    parameter_type,
                    &projection.destination,
                )?;
                let projected_regions = if reference_shell(program, projected_type).is_some() {
                    resolver
                        .reference_operand_write_sources(
                            machine,
                            projection.expression,
                            projected_type,
                        )?
                        .iter()
                        .map(|source| {
                            local_origins::write_source_region(
                                program,
                                state,
                                statement_index,
                                source,
                                &projection.remaining,
                            )
                        })
                        .collect::<Option<Vec<_>>>()?
                } else {
                    let mut place = place_at(
                        program,
                        state_symbol,
                        statement_index,
                        projection.expression,
                    )?;
                    place.segments.extend_from_slice(&projection.remaining);
                    vec![StorageRegion::Declared(place)]
                };
                for region in projected_regions {
                    for region in rebase_projected_call_region(
                        program,
                        state_symbol,
                        statement_index,
                        region,
                        projected_type,
                        &projection.remaining,
                        destination,
                        references,
                        call_frames,
                    )? {
                        crate::shared::collections::push_unique(&mut regions, region);
                    }
                }
            }
            return Some(regions);
        }
        let origins = captured.as_ref()?;
        let mut places = Vec::new();
        for source in origins.arguments.get(position)?.as_ref()? {
            let projected = local_origins::write_source_region(
                program,
                state,
                statement_index,
                source,
                relative,
            )?;
            if matches!(projected, StorageRegion::Declared(_))
                && !storage_place_has_declared_identity(projected.anchor())
            {
                return None;
            }
            let rebased = rebase_projected_call_region(
                program,
                state_symbol,
                statement_index,
                projected,
                parameter_type,
                relative,
                destination,
                references,
                call_frames,
            )?;
            for place in rebased {
                crate::shared::collections::push_unique(&mut places, place);
            }
        }
        Some(places)
    }

    /*
        Rebase the selected source coordinate with its genuine receiving mode.
        Shape matching is never reference existence, access or lifetime evidence.
    */
    fn rebase_projected_call_region(
        program: &dyn validation::CheckingInput,
        state: SymbolHandle,
        statement: usize,
        region: StorageRegion,
        parameter: TypeReferenceHandle,
        path: &[PlaceSegment],
        destination: WriteDestination,
        references: Option<&[LocalWriteOrigin]>,
        frames: Option<&CallFrameResolver<'_>>,
    ) -> Option<Vec<StorageRegion>> {
        if destination == WriteDestination::Referent {
            return local_origins::rebase_captured_storage_region(
                program,
                state,
                statement,
                region,
                references,
                destination,
                frames,
            );
        }
        let StorageRegion::Declared(place) = region else {
            return None;
        };
        let root_type =
            reference_shell(program, parameter).map_or(parameter, |shell| shell.referee);
        let expected = project_type_reference_from_segments(program, root_type, path)?;
        let actual = canonical_place_type_reference(program, state, statement, &place)?;
        reference_shell(program, expected)?;
        reference_shell(program, actual)?;
        if !storage_origin_types_match(program, expected, actual) {
            return None;
        }
        local_origins::rebase_captured_storage_region(
            program,
            state,
            statement,
            StorageRegion::Declared(place),
            references,
            destination,
            frames,
        )
    }

    fn write_reaches_caller(
        program: &dyn validation::CheckingInput,
        parameter: TypeReferenceHandle,
        path: &[PlaceSegment],
        destination: WriteDestination,
    ) -> Option<bool> {
        if crate::flow::operand_contexts::operand_binds_live_view(program, parameter) {
            return Some(true);
        }
        if !has_contents(program, parameter, &[], ContentsRequirement::StoredValue) {
            return None;
        }
        for length in 1..=path.len() {
            let reference =
                project_type_reference_from_segments(program, parameter, &path[..length])?;
            if reference_shell(program, reference).is_some()
                && (length < path.len() || destination == WriteDestination::Referent)
            {
                return Some(true);
            }
        }
        Some(false)
    }

    fn storage_place_has_declared_identity(place: &CanonicalPlace) -> bool {
        matches!(place.root, PlaceRoot::Symbol(symbol) if symbol.is_valid())
            && place.segments.iter().all(|segment| match segment {
                PlaceSegment::Field { symbol } => symbol.is_valid(),
                PlaceSegment::Case { variant } => variant.is_valid(),
                _ => true,
            })
    }
}

/*
    A sealed atomic event on dedicated atomic storage uses shared access: a
    shared loan of that cell authorizes it, while an exclusive or write-only
    loan still excludes it. Every other assignment, including ordinary
    replacement of an atomic cell, demands exclusive access. Call summaries and
    the statement borrow check both read this one judgment.
*/
pub(crate) fn assignment_requires_exclusive_access(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    assignment: &typed_trees::statement::TableAssignment,
) -> bool {
    !(validation::atomic_assignment_carrier(program, assignment).is_some()
        && validation::place_is_atomic_storage(program, machine, Some(state), assignment.target))
}

pub(crate) use ceiling::linear_restoration_ceiling_regions;
pub(crate) use ceiling::signature_ceiling_places;
pub(crate) use local_origins::close_storage_places_over_aliases;
pub(crate) use local_origins::close_storage_places_over_written_origins;
pub(crate) use local_origins::place_from_origin_path;
pub(crate) use local_origins::rebase_exact_local_place;
pub(crate) use local_origins::{OriginPrecision, origin_place, sole_local_referent_origin};
pub(in crate::flow) use receiver::call_data_argument;
pub(crate) use receiver::{
    call_receiver_is_mutable, call_receiver_mutated_place, canonical_receiver_place_for_call_site,
    family_data_binding_parameter,
};
pub(crate) use regions::StorageRegion;
use summary::instantiate_known_call_mutation_summary_places;
pub(in crate::flow) use summary::{AccessRouteReader, instantiate_call_relative_places};
pub(crate) use summary::{StateMutationSummaryCache, state_storage_writes};

#[derive(Clone, Copy)]
pub(super) enum WritePlaceNamespace {
    /* Preserve the caller's reference binding through which access occurs. */
    AccessRoute,
    /* Rebase reference bindings to the storage whose facts may be invalidated. */
    Storage,
}

/*
    One possible write retains the callee storage path that supplied it.
    Equal caller footprints reached through different formals remain distinct:
    one writer's restored contract cannot excuse another writer's corruption.
*/
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::flow) struct CallMutationWrite {
    pub region: StorageRegion,
    pub source: Option<StorageRegion>,
    /*
        Exact destination presentation of the checked assignment, retained
        through call projection. A may-write footprint alone proves no domain.
    */
    pub checked_write_type: typed_trees::types::TypeReferenceHandle,
}

/*
    Caller storage footprint. `None` requires full invalidation; an empty
    complete footprint preserves facts. Neither may be represented by an
    unresolved reference-binding root.
    Reuse the resolver prepared for this immutable program; local origins still
    resolve at the exact caller prefix, not from an earlier query's facts.
*/
pub(crate) fn call_mutated_places(
    program: &dyn ::validation::CheckingInput,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    state_mutation_summaries: &StateMutationSummaryCache,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    call_mutation_writes(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow,
        borrow_call,
        state_mutation_summaries,
        call_frames,
    )
    .and_then(|writes| StorageRegion::declared_places(writes.into_iter().map(|write| write.region)))
}

pub(crate) fn call_mutated_regions(
    program: &dyn ::validation::CheckingInput,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    state_mutation_summaries: &StateMutationSummaryCache,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<StorageRegion>> {
    call_mutation_writes(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow,
        borrow_call,
        state_mutation_summaries,
        call_frames,
    )
    .map(|writes| writes.into_iter().map(|write| write.region).collect())
}

pub(in crate::flow) fn call_mutation_writes(
    program: &dyn ::validation::CheckingInput,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    state_mutation_summaries: &StateMutationSummaryCache,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CallMutationWrite>> {
    if validation::invocation_is_fact_only(program, borrow_call.target_symbol) {
        return Some(Vec::new());
    }
    let site = find_call_site(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    )?;
    if !operand_coordinates::call_operands_have_builtin_coordinates(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        &site,
    ) {
        return None;
    }
    if let CallSite::Expression { expression, .. } = site
        && typed_trees::byte_predicates::byte_predicate_call(program, expression).is_some()
    {
        return Some(Vec::new());
    }
    let mut writes = call_write_places(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow,
        borrow_call,
        state_mutation_summaries,
        WritePlaceNamespace::Storage,
        call_frames,
        None,
    )?;
    // `_ = call(...)` lets the result die at the statement, so its cleanup
    // writes join the call's own.
    if let CallSite::Statement(call) = &site
        && call.discards_result
        && let Some(frames) = call_frames
        && let Some(machine) = program.machine_by_symbol(caller_machine_symbol)
    {
        let frame = frames.discarded_result_cleanup_write_frame(machine, call);
        let cleanup = if frame.complete_paths().is_some_and(|paths| paths.is_empty()) {
            Vec::new()
        } else {
            frame_storage_writes(
                program,
                caller_machine_symbol,
                caller_state_symbol,
                borrow_call.statement_index,
                &frame,
                call_frames,
            )?
        };
        for place in cleanup {
            let region = StorageRegion::Declared(place);
            if !writes.iter().any(|write| write.region == region) {
                writes.push(CallMutationWrite {
                    region,
                    source: None,
                    checked_write_type: Default::default(),
                });
            }
        }
    }
    for region in ceiling::retained_loan_write_regions(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow,
        borrow_call,
    )? {
        if !writes.iter().any(|write| write.region == region) {
            writes.push(CallMutationWrite {
                region,
                source: None,
                checked_write_type: Default::default(),
            });
        }
    }
    Some(writes)
}

/*
    The access route retains the local loan owner; storage rebasing must not
    turn a write through that loan into an independent write to its referent.
    Only exclusive writes belong here. Shared atomic operations keep their
    ordinary call-access checks and remain writes in `call_mutated_places`,
    but do not freeze other shared loans as an ordinary store would.
    A captured loan limits a local jump to the states that still carry it;
    ordinary calls retain it through their complete run. The caller must first
    authenticate the source binding's closure; this spatial query grants no
    lifetime authority. Without a loan, callers
    inspect the complete write set (for example, restored-use certificates).
*/
pub(crate) fn call_write_accesses(
    program: &dyn ::validation::CheckingInput,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    state_mutation_summaries: &StateMutationSummaryCache,
    loan: Option<&CanonicalPlace>,
) -> Vec<CanonicalPlace> {
    call_write_places(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow,
        borrow_call,
        state_mutation_summaries,
        WritePlaceNamespace::AccessRoute,
        None,
        loan,
    )
    .expect("access-route projection always retains the ownership fallback")
    .into_iter()
    .map(|write| write.region.anchor().clone())
    .collect()
}

/*
    The subset of `call_write_accesses` a call writes through its implicit
    receiver: the callee's complete exclusive writes reached through `self`, or
    the whole receiver place when no complete summary is known. Writes through
    explicit `&mut` arguments are excluded; argument checks own those.
*/
pub(crate) fn call_receiver_write_accesses(
    program: &dyn ::validation::CheckingInput,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    state_mutation_summaries: &StateMutationSummaryCache,
    loan: &CanonicalPlace,
) -> Vec<CanonicalPlace> {
    summary::instantiate_known_call_receiver_summary_places(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow,
        borrow_call,
        state_mutation_summaries,
        loan,
    )
    .or_else(|| {
        call_receiver_mutated_place(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow_call,
        )
        .map(|place| vec![place])
    })
    .unwrap_or_default()
}

fn call_write_places(
    program: &dyn ::validation::CheckingInput,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    state_mutation_summaries: &StateMutationSummaryCache,
    namespace: WritePlaceNamespace,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    loan: Option<&CanonicalPlace>,
) -> Option<Vec<CallMutationWrite>> {
    if let Some(operation) = validation::compiler_operations::global_descriptor_table_operation(
        program,
        borrow_call.target_symbol,
    ) {
        if matches!(
            operation,
            language_core::inline_assembly::AsmGlobalDescriptorTableOperation::Load
        ) {
            return Some(Vec::new());
        }
        let regions = match namespace {
            WritePlaceNamespace::Storage => descriptor_table_written_regions(
                program,
                caller_machine_symbol,
                caller_state_symbol,
                borrow_call,
                call_frames,
            )?,
            WritePlaceNamespace::AccessRoute => exclusive_loan_places(
                program,
                caller_machine_symbol,
                caller_state_symbol,
                borrow,
                borrow_call,
            )
            .into_iter()
            .map(StorageRegion::Declared)
            .collect(),
        };
        return Some(unattributed_writes(regions));
    }
    if validation::activation_accessor_has_empty_write_frame(program, borrow_call.target_symbol)
        || validation::access_borrow_has_empty_write_frame(program, borrow_call.target_symbol)
    {
        return Some(Vec::new());
    }

    let summarized_places = instantiate_known_call_mutation_summary_places(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow,
        borrow_call,
        state_mutation_summaries,
        namespace,
        call_frames,
        loan,
    );
    if summarized_places.is_none() && matches!(namespace, WritePlaceNamespace::Storage) {
        let boundary_target = program.traits().iter().any(|definition| {
            definition.is_boundary
                && program
                    .trait_machine_signature_by_symbol(definition, borrow_call.target_symbol)
                    .is_some()
        });
        if boundary_target {
            return shared_call_storage_regions(
                program,
                caller_machine_symbol,
                caller_state_symbol,
                borrow,
                borrow_call,
                call_frames,
            )
            .map(unattributed_writes);
        }
        return ceiling::signature_ceiling_regions(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            Some(borrow),
            borrow_call,
            call_frames,
        )
        .map(unattributed_writes);
    }
    if let Some(summarized_places) = summarized_places {
        return Some(summarized_places);
    }
    let places = exclusive_loan_places(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow,
        borrow_call,
    );
    Some(unattributed_writes(
        places.into_iter().map(StorageRegion::Declared).collect(),
    ))
}

/*
    Exclusive argument, mutable-parameter, and mutable-receiver places the call
    lends. These form the conservative ceiling when no complete body frame is
    available, not extra writes to append to an already complete frame.
*/
fn exclusive_loan_places(
    program: &dyn ::validation::CheckingInput,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
) -> Vec<CanonicalPlace> {
    let mut places = Vec::new();
    for access in borrow.argument_accesses.span_or_empty(borrow_call.accesses) {
        if access.kind.is_exclusive() && access.root.is_valid() {
            let mut place = CanonicalPlace {
                root: access.root,
                segments: Vec::new(),
            };
            place.extend_segments(borrow.access_segments.span_or_empty(access.segments));
            crate::shared::collections::push_unique(&mut places, place);
        }
    }

    if let Some(call_site) = find_call_site(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    ) && let Some(target_state) = program.state_by_symbol(borrow_call.target_symbol)
    {
        let mut argument_index = 0usize;
        for parameter in program.state_parameters(target_state) {
            let argument = call_site_argument_expressions(program, &call_site)
                .get(argument_index)
                .copied();
            argument_index = argument_index.saturating_add(1);

            if !parameter.is_mutable {
                continue;
            }

            if let Some(argument) = argument
                && let Some(place) = place_at(
                    program,
                    caller_state_symbol,
                    borrow_call.statement_index,
                    argument,
                )
                && !places.contains(&place)
            {
                places.push(place);
            }
        }
    }

    if borrow_call.has_receiver
        && call_receiver_is_mutable(program, borrow, borrow_call)
        && let Some(place) = call_receiver_mutated_place(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow_call,
        )
        && !places.contains(&place)
    {
        places.push(place);
    }
    places
}

fn unattributed_writes(regions: Vec<StorageRegion>) -> Vec<CallMutationWrite> {
    regions
        .into_iter()
        .map(|region| CallMutationWrite {
            region,
            source: None,
            checked_write_type: Default::default(),
        })
        .collect()
}

fn shared_call_storage_regions(
    program: &dyn ::validation::CheckingInput,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<StorageRegion>> {
    if validation::activation_accessor_has_empty_write_frame(program, borrow_call.target_symbol)
        || validation::access_borrow_has_empty_write_frame(program, borrow_call.target_symbol)
    {
        return Some(Vec::new());
    }
    if let Some(operation) = validation::compiler_operations::global_descriptor_table_operation(
        program,
        borrow_call.target_symbol,
    ) {
        return match operation {
            language_core::inline_assembly::AsmGlobalDescriptorTableOperation::Load => {
                Some(Vec::new())
            }
            language_core::inline_assembly::AsmGlobalDescriptorTableOperation::Store => {
                descriptor_table_written_regions(
                    program,
                    caller_machine_symbol,
                    caller_state_symbol,
                    borrow_call,
                    call_frames,
                )
            }
        };
    }
    let machine = program.machine_by_symbol(caller_machine_symbol)?;
    let state = program.state_by_symbol(caller_state_symbol)?;
    let site = find_call_site(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    )?;
    let resolver = call_frames?;
    let frame = resolver.signature_invocation_write_frame(machine, &site);
    let trace = ::validation::trace_flags::call_write_trace();
    if trace {
        eprintln!(
            "CALLWRITE boundary-frame state={caller_state_symbol:?} statement={} ordinal={} frame={frame:?}",
            borrow_call.statement_index, borrow_call.call_ordinal,
        );
    }
    let (exclusive_referents, coarse_anchors, refinable) = boundary_frame_access_places(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow,
        borrow_call,
        &site,
        call_frames,
    );
    let mut places = Vec::new();
    for path in frame.complete_paths()? {
        let source =
            local_origins::frame_write_place(program, state, borrow_call.statement_index, path);
        if trace {
            eprintln!(
                "CALLWRITE frame-source state={caller_state_symbol:?} statement={} path={path:?} source={source:?}",
                borrow_call.statement_index
            );
        }
        let regions = local_origins::rebase_local_write_regions(
            program,
            caller_state_symbol,
            borrow_call.statement_index,
            source?,
            call_frames,
        );
        if trace {
            eprintln!(
                "CALLWRITE frame-regions state={caller_state_symbol:?} statement={} path={path:?} regions={regions:?}",
                borrow_call.statement_index
            );
        }
        for region in regions? {
            let StorageRegion::Declared(coarse) = region else {
                crate::shared::collections::push_unique(&mut places, region);
                continue;
            };
            let identity = coarse.clone();
            let blocked = !refinable
                || coarse_anchors
                    .iter()
                    .any(|anchor| storage_extends_frame_place(program, anchor, &identity));
            let refinements: Vec<CanonicalPlace> = if blocked {
                Vec::new()
            } else {
                exclusive_referents
                    .iter()
                    .filter(|referent| storage_extends_frame_place(program, referent, &identity))
                    .map(|referent| CanonicalPlace {
                        root: coarse.root,
                        segments: referent.segments.clone(),
                    })
                    .collect()
            };
            if refinements.is_empty() {
                let region = StorageRegion::Declared(coarse);
                crate::shared::collections::push_unique(&mut places, region);
                continue;
            }
            for refined in refinements {
                let region = StorageRegion::Declared(refined);
                crate::shared::collections::push_unique(&mut places, region);
            }
        }
    }
    let ceiling = ceiling::signature_ceiling_regions(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        Some(borrow),
        borrow_call,
        call_frames,
    );
    if trace {
        eprintln!(
            "CALLWRITE boundary-ceiling state={caller_state_symbol:?} statement={} ceiling={ceiling:?}",
            borrow_call.statement_index
        );
    }
    for region in ceiling? {
        if !matches!(region, StorageRegion::Declared(_)) && !places.contains(&region) {
            places.push(region);
        }
    }
    Some(places)
}

/*
    The call's recorded argument accesses in the storage namespace frame paths
    resolve to: exclusive borrow referents that may refine a coarsened path, and
    the other contributions that render to the same coarse path and must keep
    it — a non-`borrow` actual on an exclusive parameter carries a reference
    whose referent is only coarsely accounted, and the receiver contributes its
    own place. `false` disables refinement entirely when an exclusive actual
    cannot be spelled as caller storage at all.
*/
fn boundary_frame_access_places(
    program: &dyn ::validation::CheckingInput,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    site: &CallSite<'_>,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> (Vec<CanonicalPlace>, Vec<CanonicalPlace>, bool) {
    let mut exclusive_referents = Vec::new();
    let mut coarse_anchors = Vec::new();
    let mut refinable = true;
    for access in borrow.argument_accesses.span_or_empty(borrow_call.accesses) {
        if !access.kind.is_exclusive() {
            continue;
        }
        if !access.root.is_valid() {
            refinable = false;
            break;
        }
        let mut place = CanonicalPlace {
            root: access.root,
            segments: Vec::new(),
        };
        place.extend_segments(borrow.access_segments.span_or_empty(access.segments));
        let Some(rebased) = local_origins::rebase_local_write_places(
            program,
            caller_state_symbol,
            borrow_call.statement_index,
            place,
            call_frames,
        ) else {
            refinable = false;
            break;
        };
        for rebased in rebased {
            crate::shared::collections::push_unique(&mut exclusive_referents, rebased);
        }
    }
    if refinable
        && let Some(parameters) =
            validation::call_target_parameters(program, borrow_call.target_symbol)
    {
        let arguments = call_site_argument_expressions(program, site);
        if parameters.len() != arguments.len() {
            refinable = false;
        } else {
            for (argument_index, parameter) in parameters.iter().enumerate() {
                let argument = arguments[argument_index];
                if !ceiling::is_exclusive_reference(program, parameter.type_reference) {
                    continue;
                }
                if matches!(
                    program.expression_table.expression(argument),
                    ExpressionNode::Borrow(_)
                ) {
                    continue;
                }
                let Some(actual) = place_at(
                    program,
                    caller_state_symbol,
                    borrow_call.statement_index,
                    argument,
                ) else {
                    refinable = false;
                    break;
                };
                if !matches!(actual.root, facts::PlaceRoot::Symbol(_)) {
                    refinable = false;
                    break;
                }
                let mut coarse = actual;
                if let Some(index) = coarse
                    .segments
                    .iter()
                    .position(|segment| is_index_place_segment(*segment))
                {
                    coarse.segments.truncate(index);
                }
                match local_origins::rebase_local_write_places(
                    program,
                    caller_state_symbol,
                    borrow_call.statement_index,
                    coarse,
                    call_frames,
                ) {
                    Some(rebased) => {
                        for anchor in rebased {
                            crate::shared::collections::push_unique(&mut coarse_anchors, anchor);
                        }
                    }
                    None => {
                        refinable = false;
                        break;
                    }
                }
            }
        }
    } else if refinable {
        refinable = false;
    }
    if refinable
        && borrow_call.has_receiver
        && let Some(receiver) = call_receiver_mutated_place(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow_call,
        )
    {
        match local_origins::rebase_local_write_places(
            program,
            caller_state_symbol,
            borrow_call.statement_index,
            receiver,
            call_frames,
        ) {
            Some(rebased) => {
                for receiver in rebased {
                    crate::shared::collections::push_unique(&mut coarse_anchors, receiver);
                }
            }
            None => refinable = false,
        }
    }
    (exclusive_referents, coarse_anchors, refinable)
}

fn is_index_place_segment(segment: facts::PlaceSegment) -> bool {
    matches!(
        segment,
        facts::PlaceSegment::FixedIndex { .. }
            | facts::PlaceSegment::FixedRange { .. }
            | facts::PlaceSegment::Index { .. }
    )
}

/*
    Whether `exact` names storage inside `coarse` whose first segment past
    `coarse`'s own length is an index selector — the only extension a dotted
    frame path could have coarsened away. An equal-length place (including a
    whole-collection borrow or receiver) also counts: it contributed the path.
*/
fn storage_extends_frame_place(
    program: &dyn ::validation::CheckingInput,
    exact: &CanonicalPlace,
    coarse: &CanonicalPlace,
) -> bool {
    exact.root == coarse.root
        && exact.segments.len() >= coarse.segments.len()
        && canonical_place_segments_may_overlap(
            program,
            &coarse.segments,
            &exact.segments[..coarse.segments.len()],
        )
        && (exact.segments.len() == coarse.segments.len()
            || is_index_place_segment(exact.segments[coarse.segments.len()]))
}

pub(crate) fn statement_mutated_place(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
) -> Option<CanonicalPlace> {
    let mut place = match statement {
        StatementNode::Assignment(assignment) => {
            place_at(program, state_symbol, statement_index, assignment.target).or_else(|| {
                expression_root_symbol(assignment.target, &program.expression_table, machine_symbol)
                    .and_then(canonical_place_from_symbol)
            })
        }
        _ => None,
    }?;
    normalize_write_only_range_place(program, state_symbol, &mut place);
    Some(place)
}

/*
    A single attempt may replace one atomic resident after its operands finish.
    Resolve incoming/local reference origins before alias closure; an unresolved
    origin stays opaque. Shared atomic authority never becomes exclusive access.
*/
pub(crate) fn atomic_once_storage_writes(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    atomic_once_write_effects(
        program,
        machine_symbol,
        state_symbol,
        statement_index,
        expression,
        call_frames,
    )
    .map(|(_, observations)| observations)
}

pub(crate) fn atomic_once_write_effects(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<(Vec<CanonicalPlace>, Vec<CanonicalPlace>)> {
    let machine = program.machine_by_symbol(machine_symbol)?;
    let state = program.state_by_symbol(state_symbol)?;
    let target = validation::atomic_once_write_place(program, machine, state, expression)?;
    let place = place_at(program, state_symbol, statement_index, target)?;
    let places = local_origins::rebase_local_write_places(
        program,
        state_symbol,
        statement_index,
        place,
        call_frames,
    )?;
    let observations = local_origins::close_storage_places_over_aliases(
        program,
        machine_symbol,
        state_symbol,
        statement_index,
        places.clone(),
        call_frames,
    )?;
    Some((places, observations))
}

/*
    Storage writes for fact invalidation, including compiler-owned operations
    on borrowed receivers. An unresolved origin requires full invalidation.
*/
pub(crate) fn statement_storage_writes(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    if let Some(local) = statement.as_local_data()
        && validation::atomic_once_carrier(program, local.initial_value).is_some()
    {
        return atomic_once_storage_writes(
            program,
            machine_symbol,
            state_symbol,
            statement_index,
            local.initial_value,
            call_frames,
        );
    }
    // A displaced value and unnamed temporaries die inside the statement, and
    // their cleanup may store through the exclusive references they held,
    // which no destination place names.
    let mut places = Vec::new();
    if let Some(frames) = call_frames
        && let Some(machine) = program.machine_by_symbol(machine_symbol)
    {
        let frame = frames.statement_cleanup_write_frame(machine, statement);
        // An empty frame names no place, so there is nothing to project.
        if frame.complete_paths().is_none_or(|paths| !paths.is_empty()) {
            places = frame_storage_writes(
                program,
                machine_symbol,
                state_symbol,
                statement_index,
                &frame,
                call_frames,
            )?;
        }
    } else if program.cleanup_selections().next().is_some() {
        // Nothing can name the values that die here, so their cleanup
        // footprint is unknown rather than empty.
        return None;
    }
    if !matches!(statement, StatementNode::Assignment(_)) {
        return Some(places);
    }
    let targets = local_origins::assignment_storage_places(
        program,
        machine_symbol,
        state_symbol,
        statement_index,
        statement,
        call_frames,
    )?;
    for place in local_origins::close_storage_places_over_aliases(
        program,
        machine_symbol,
        state_symbol,
        statement_index,
        targets.clone(),
        call_frames,
    )
    .unwrap_or(targets)
    {
        crate::shared::collections::push_unique(&mut places, place);
    }
    Some(places)
}

/*
    Project a shared complete call frame into the exact caller storage namespace.
    Coarse selectors remain conservative writes; they are not value provenance.
*/
pub(crate) fn frame_storage_writes(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    frame: &facts::NormalizedWriteFrame,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    let state = program.state_by_symbol(state_symbol)?;
    let mut places = Vec::new();
    for path in frame.complete_paths()? {
        let source = local_origins::frame_write_place(program, state, statement_index, path)?;
        for place in local_origins::rebase_local_write_places(
            program,
            state_symbol,
            statement_index,
            source,
            call_frames,
        )? {
            crate::shared::collections::push_unique(&mut places, place);
        }
    }
    local_origins::close_storage_places_over_aliases(
        program,
        machine_symbol,
        state_symbol,
        statement_index,
        places,
        call_frames,
    )
}

fn normalize_write_only_range_place(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    place: &mut CanonicalPlace,
) {
    let Some(root_symbol) = place.root.as_symbol() else {
        return;
    };
    let Some(state) = program.state_by_symbol(state_symbol) else {
        return;
    };
    let root_is_write_only = program
        .state_parameter_by_symbol(state, root_symbol)
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            program
                .state_local_declaration(state, root_symbol)
                .map(|(_, local)| local.type_reference)
        })
        .is_some_and(|type_reference| {
            matches!(
                program.type_reference_table.type_reference(type_reference),
                typed_trees::types::TypeReferenceNode::Reference {
                    access: language_semantics::ReferenceAccess::WriteOnly,
                    ..
                }
            )
        });
    if !root_is_write_only {
        return;
    }

    let mut bound_lookup = None;
    for segment in &mut place.segments {
        let facts::PlaceSegment::Index { expression } = *segment else {
            continue;
        };
        let Some(range) = program.expression_table.range_node(expression) else {
            continue;
        };
        let bound_lookup =
            bound_lookup.get_or_insert_with(|| validation::ImmutableBoundLookup::new(program));
        let start = if range.start.is_valid() {
            validation::normalize_immutable_integer_bound_to_usize(
                program,
                bound_lookup,
                range.start,
            )
        } else {
            Some(0)
        };
        let end = if !range.end.is_valid() {
            None
        } else {
            validation::normalize_immutable_integer_bound_to_usize(program, bound_lookup, range.end)
                .and_then(|end| {
                    if range.end_inclusive {
                        end.checked_add(1)
                    } else {
                        Some(end)
                    }
                })
        };
        if let (Some(start), Some(end)) = (start, end) {
            *segment = facts::PlaceSegment::FixedRange { start, end };
        }
    }
}

/* A completed SGDT changes the lent ten bytes; its pointer binding remains intact. */
fn descriptor_table_written_regions(
    program: &dyn validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    call: &BorrowCallFact,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<StorageRegion>> {
    let machine = program.machine_by_symbol(machine_symbol)?;
    let state = program.state_by_symbol(state_symbol)?;
    let site = find_call_site(
        program,
        machine_symbol,
        state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let arguments = call_site_argument_expressions(program, &site);
    let [operand] = arguments else {
        return None;
    };
    let reference = validation::compiler_operations::global_descriptor_table_operand(
        program,
        machine,
        Some(state),
        *operand,
        language_core::inline_assembly::AsmGlobalDescriptorTableOperation::Store,
    )
    .ok()?;
    summary::captured_argument_write_regions(
        program,
        machine_symbol,
        state_symbol,
        call.statement_index,
        call.call_ordinal,
        &site,
        0,
        reference,
        &[],
        summary::WriteDestination::Referent,
        call_frames,
    )
}
