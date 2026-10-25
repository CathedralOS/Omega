//! The declared-signature ceiling of a call whose write frame is unknown.
//!
//! An opaque, unresolved, generic or dispatched callee, or an actual whose
//! storage origin the frame resolver cannot summarize, retains "the
//! signature/authority ceiling", never an empty write set and never a guess
//! narrower than the declaration (wiki/spec/language/dependent_values.md,
//! "Mutation frames"). The ceiling includes exclusive referents and separately
//! owned capability content. A reference-free descriptor can still carry a
//! resident owner; copying its geometry does not isolate that content.
//! Unknown capability content retires observations without authorizing any
//! declared-storage replacement. Exact loan origins remain necessary to
//! restore a caller's replaced ownership homes at normal completion.

use validation::CanonicalPlace;

use super::StorageRegion;
use checked_trees::{BorrowCallFact, BorrowFacts, BorrowLoanOwnerSegment};
use symbols::SymbolHandle;
use typed_trees::types::TypeReferenceNode;

pub(crate) fn signature_ceiling_places(
    program: &dyn ::validation::CheckingInput,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: Option<&BorrowFacts>,
    borrow_call: &BorrowCallFact,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
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
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<StorageRegion>> {
    let machine = program.machine_by_symbol(caller_machine_symbol)?;
    let state = program.state_by_symbol(caller_state_symbol)?;
    // A builtin function (`min`, `max`, `sqrt`, the float classifiers, the
    // asm intrinsics) takes scalar values and owns no caller storage; its
    // ceiling is empty even though it declares no state parameters.
    if program
        .symbols
        .builtin_function_for_symbol(borrow_call.target_symbol)
        .is_some()
    {
        return Some(Vec::new());
    }
    let site = validation::find_call_site(
        program,
        machine.symbol,
        state.symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    )?;
    let parameters = site.parameters(program, machine.symbol)?;
    let arguments = validation::call_site_argument_expressions(program, &site);
    if parameters.iter().count() != arguments.len() {
        return None;
    }
    let mut owned_frames = None;
    let resolver = crate::flow::shared_call_frames_or(call_frames, program, &mut owned_frames);
    let mut places = Vec::new();
    let mut argument_index = 0usize;
    for parameter in parameters {
        let actual = {
            let argument = arguments[argument_index];
            argument_index += 1;
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
                if !places.contains(&region) {
                    places.push(region);
                }
            }
            if !is_exclusive_reference(program, parameter.type_reference) {
                // Declared reference-free storage cannot reach the caller. A
                // by-value actual that may carry references is still exact:
                // every borrow inside a value is a recorded loan, so the
                // caller storage a callee could write is the referent set of
                // the actual's carried exclusive loans. Shared `&` loans
                // grant the callee reads only and reach nothing writable.
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
                        for region in super::local_origins::rebase_storage_region(
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
                                    if !places.contains(&content) {
                                        places.push(content);
                                    }
                                }
                            }
                            if !places.contains(&region) {
                                places.push(region);
                            }
                        }
                    }
                }
                continue;
            }
            argument
        };
        let captured = super::summary::captured_argument_write_regions(
            program,
            machine.symbol,
            state.symbol,
            borrow_call.statement_index,
            borrow_call.call_ordinal,
            &site,
            argument_index - 1,
            parameter.type_reference,
            &[],
            super::summary::WriteDestination::Referent,
            resolver,
        );
        let storage = captured.or_else(|| {
            let actual = validation::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                borrow_call.statement_index,
                actual,
            )?;
            super::local_origins::rebase_local_write_regions(
                program,
                state.symbol,
                borrow_call.statement_index,
                actual,
                resolver,
            )
        })?;
        for storage in storage {
            if !places.contains(&storage) {
                places.push(storage);
            }
        }
        // Access routes belong to borrow checking and observation aliases.
        // Lending a captured view does not lend replacement of its descriptor
        // or the owner descriptor anchoring the content loan.
    }
    Some(places)
}

/// Owned capability storage is not isolated merely because its carrier has
/// no reference fields. Reuse the ordinary owned qualification roster for
/// records, case payloads and arrays; a field containing a resident reaches
/// the same content as a root resident. Shared ingress grants no write reach.
fn resident_parameter_regions(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    call: &BorrowCallFact,
    site: &validation::CallSite<'_>,
    position: usize,
    reference: typed_trees::types::TypeReferenceHandle,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<StorageRegion>> {
    let mut regions = Vec::new();
    let inventory = resident_owners_in_type(program, reference)?;
    for path in inventory.unresolved {
        let actual = *validation::call_site_argument_expressions(program, site).get(position)?;
        let mut carrier = validation::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            call.statement_index,
            actual,
        )
        .unwrap_or_else(|| CanonicalPlace {
            root: facts::PlaceRoot::Expression(actual),
            segments: Vec::new(),
        });
        carrier.segments.extend(path);
        regions.push(StorageRegion::UnresolvedContent { carrier });
    }
    for (path, owner) in inventory.owners {
        let captured = super::summary::captured_argument_write_regions(
            program,
            machine.symbol,
            state.symbol,
            call.statement_index,
            call.call_ordinal,
            site,
            position,
            reference,
            &path,
            super::summary::WriteDestination::Referent,
            frames,
        )
        .or_else(|| {
            let actual =
                *validation::call_site_argument_expressions(program, site).get(position)?;
            let mut place = validation::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                call.statement_index,
                actual,
            )?;
            place.segments.extend_from_slice(&path);
            super::local_origins::rebase_local_write_regions(
                program,
                state.symbol,
                call.statement_index,
                place,
                frames,
            )
        });
        let Some(captured) = captured else {
            if validation::reference_shell(program, reference).is_some() {
                // An exclusive reference also lends declared replacement
                // authority, which still needs its actual storage origin.
                return None;
            }
            // A computed owned value supplies capability content, not a
            // caller-owned descriptor home. Unknown content must invalidate
            // observations without fabricating a replaceable declaration.
            let actual =
                *validation::call_site_argument_expressions(program, site).get(position)?;
            regions.push(StorageRegion::UnresolvedContent {
                carrier: CanonicalPlace {
                    root: facts::PlaceRoot::Expression(actual),
                    segments: path,
                },
            });
            continue;
        };
        for region in captured {
            regions.push(StorageRegion::ResidentContent {
                owner: region.into_declared()?,
                // A signature bounds reach without claiming that a view
                // formation has executed. Zero is deliberate absence.
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
    owners: Vec<(
        Vec<facts::PlaceSegment>,
        validation::ResidentOwnerQualifications,
    )>,
    unresolved: Vec<Vec<facts::PlaceSegment>>,
}

fn resident_owners_in_type(
    program: &dyn ::validation::CheckingInput,
    mut reference: typed_trees::types::TypeReferenceHandle,
) -> Option<ResidentAuthorityInventory> {
    if let Some(shell) = validation::reference_shell(program, reference) {
        if !shell.access.is_exclusive() {
            return Some(ResidentAuthorityInventory::default());
        }
        reference = shell.referee;
    }
    let roster = validation::field_domains::declared_owned_field_obligations(program, reference);
    let mut inventory = ResidentAuthorityInventory::default();
    for path in &roster.unresolved {
        let unresolved =
            validation::project_type_reference_from_segments(program, reference, path)?;
        let plain = |reference| {
            validation::has_contents(
                program,
                reference,
                &[],
                validation::ContentsRequirement::NumericOwned,
            )
        };
        // A sealed placed accessor is an operation token for its retained
        // value type, not an arbitrary opaque capability. Its admitted scalar
        // contents cannot hide another resident owner. Other unresolved
        // storage keeps an unknown ceiling instead of an empty inventory.
        let symbol = program.type_reference_table.type_symbol(
            program
                .type_reference_table
                .peeled_type_reference(unresolved),
        );
        if !plain(unresolved)
            && !program.placed_view_plans.iter().any(|plan| {
                plan.fields
                    .iter()
                    .any(|field| field.accessor_data_symbol == symbol && plain(field.value_type))
            })
        {
            inventory.unresolved.push(path.clone());
        }
    }
    let mut paths = vec![Vec::new()];
    for (path, _, _) in roster.identities {
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    for path in paths {
        let projected =
            validation::project_type_reference_from_segments(program, reference, &path)?;
        if let Some(owner) = validation::resident_owner_qualifications(program, projected) {
            inventory.owners.push((path, owner));
        }
    }
    Some(inventory)
}

pub(super) fn is_exclusive_reference(
    program: &dyn ::validation::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
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

/// The caller storage a consumed by-value actual hands the callee through
/// carried exclusive loans. `None` when the actual's storage cannot be named
/// exactly or a carried exclusive loan has no spellable referent — both keep
/// the whole ceiling unrepresentable rather than guessing narrower than the
/// declaration. Shared (`&`) loans contribute nothing: a read-only borrow
/// inside the consumed value reaches no writable caller place.
struct CarriedStorage {
    region: StorageRegion,
    reference: typed_trees::types::TypeReferenceHandle,
}

fn carried_exclusive_referents(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    argument: typed_trees::expression::ExpressionHandle,
    reference: typed_trees::types::TypeReferenceHandle,
    resolver: &validation::CallFrameResolver<'_>,
) -> Option<Vec<CarriedStorage>> {
    let (_, caller_state) = borrow
        .states
        .iter()
        .find(|(_, row)| row.state_symbol == state.symbol)?;
    let mut referents = Vec::new();
    // Constructors contribute their evaluated children; a named aggregate
    // contributes its retained descriptor inventory. The shared projection
    // owner handles fields, cases and elements for both, without turning an
    // inline constructor into a fictitious named storage allocation.
    for projection in
        crate::flow::literal_expression_projections(program, argument, reference, &[], true)?
    {
        let projected = validation::project_type_reference_from_segments(
            program,
            reference,
            &projection.destination,
        )?;
        if let Some(shell) = validation::reference_shell(program, projected) {
            if !shell.access.is_exclusive() {
                continue;
            }
            for source in resolver.reference_operand_write_sources(
                machine,
                projection.expression,
                projected,
            )? {
                referents.push(CarriedStorage {
                    region: super::local_origins::write_source_region(
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
        let mut actual = validation::canonical_place_from_expression_in_state(
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
                        BorrowLoanOwnerSegment::Field(symbol) => {
                            facts::PlaceSegment::Field { symbol: *symbol }
                        }
                        BorrowLoanOwnerSegment::Case(variant) => {
                            facts::PlaceSegment::Case { variant: *variant }
                        }
                        BorrowLoanOwnerSegment::FixedIndex(index) => {
                            facts::PlaceSegment::FixedIndex { index: *index }
                        }
                        BorrowLoanOwnerSegment::DynamicIndex => facts::PlaceSegment::Index {
                            expression: Default::default(),
                        },
                    })
                    .collect(),
            };
            referents.push(CarriedStorage {
                region: StorageRegion::Declared(CanonicalPlace {
                    root: loan.root,
                    segments: borrow.loan_segments(loan).to_vec(),
                }),
                // The carried descriptor supplies its qualification contract,
                // even when its referent's declaration is less refined and
                // the actual membership was established by flow.
                reference: validation::canonical_place_type_reference(
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

/// Whether a recorded loan's owner path and a place's field path overlap.
/// Same prefix law as `borrow::last_uses`'s owner-path matching: an absent
/// or dynamic selector conservatively overlaps its counterpart.
fn owner_path_overlaps(
    program: &dyn ::validation::CheckingInput,
    owner_path: &[BorrowLoanOwnerSegment],
    place_segments: &[facts::PlaceSegment],
) -> bool {
    owner_path
        .iter()
        .zip(place_segments)
        .all(|(owner, place)| match (owner, place) {
            (
                BorrowLoanOwnerSegment::Field(owner_symbol),
                facts::PlaceSegment::Field {
                    symbol: place_symbol,
                },
            ) => !place_symbol.is_valid() || owner_symbol == place_symbol,
            (
                BorrowLoanOwnerSegment::Case(owner_variant),
                facts::PlaceSegment::Case {
                    variant: place_variant,
                },
            ) => owner_variant == place_variant,
            (
                BorrowLoanOwnerSegment::FixedIndex(owner_index),
                facts::PlaceSegment::FixedIndex { index: place_index },
            ) => owner_index == place_index,
            (
                BorrowLoanOwnerSegment::FixedIndex(owner_index),
                facts::PlaceSegment::Index { expression },
            ) => program
                .expression_table
                .constant_integer_value(*expression)
                .and_then(|value| usize::try_from(value).ok())
                .is_none_or(|place_index| *owner_index == place_index),
            (
                BorrowLoanOwnerSegment::DynamicIndex,
                facts::PlaceSegment::FixedIndex { .. } | facts::PlaceSegment::Index { .. },
            ) => true,
            _ => false,
        })
}
