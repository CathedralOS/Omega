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
    CallFrameResolver, CallSite, CanonicalPlace, ContentsRequirement, ResidentOwnerQualifications,
    call_site_argument_expressions, canonical_place_type_reference, find_call_site, has_contents,
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

/// Claim restoration concerns replaced caller values, not claims transferred
/// with an owned argument. A writable target certified as plain unrestricted
/// storage has no owned claim or descriptor to restore. This is a sufficient
/// certificate, not an inference from an empty or unresolved claim frontier.
/// Ordinary mutation invalidation still uses the full signature ceiling.
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
                let Some(leaf) = project_type_reference_from_segments(program, reference, &path)
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
    let site = find_call_site(
        program,
        machine.symbol,
        state.symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    )?;
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
        // Access routes belong to borrow checking and observation aliases.
        // Lending a captured view does not lend replacement of its descriptor
        // or the owner descriptor anchoring the content loan.
    }
    if let Some(borrow) = borrow {
        // Ending a retained loan can expose storage independently of the
        // final callee's parameters. Every ceiling query keeps this lane.
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

/// Owned capability storage is not isolated merely because its carrier has
/// no reference fields. Reuse the ordinary owned qualification roster for
/// records, case payloads and arrays; a field containing a resident reaches
/// the same content as a root resident. Shared ingress grants no write reach.
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
            rebase_local_write_regions(program, state.symbol, call.statement_index, place, frames)
        });
        let Some(captured) = captured else {
            if reference_shell(program, reference).is_some() {
                // An exclusive reference also lends declared replacement
                // authority, which still needs its actual storage origin.
                return None;
            }
            // A computed owned value supplies capability content, not a
            // caller-owned descriptor home. Unknown content must invalidate
            // observations without fabricating a replaceable declaration.
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
        let plain =
            |reference| has_contents(program, reference, &[], ContentsRequirement::NumericOwned);
        // A sealed accessor is an operation token for its retained
        // value type, not an arbitrary opaque capability. Its admitted scalar
        // contents cannot hide another resident owner. Other unresolved
        // storage keeps an unknown ceiling instead of an empty inventory.
        let symbol = program.type_reference_table.type_symbol(
            program
                .type_reference_table
                .peeled_type_reference(unresolved),
        );
        if !plain(unresolved)
            && !program.access_view_plans.iter().any(|plan| {
                plan.fields
                    .iter()
                    .any(|field| field.accessor_data_symbol == symbol && plain(field.value_type))
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

/// The caller storage a consumed by-value actual hands the callee through
/// carried exclusive loans. `None` when the actual's storage cannot be named
/// exactly or a carried exclusive loan has no spellable referent — both keep
/// the whole ceiling unrepresentable rather than guessing narrower than the
/// declaration. Shared (`&`) loans contribute nothing: a read-only borrow
/// inside the consumed value reaches no writable caller place.
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
    let (_, caller_state) = borrow
        .states
        .iter()
        .find(|(_, row)| row.state_symbol == state.symbol)?;
    let mut referents = Vec::new();
    // Constructors contribute their evaluated children; a named aggregate
    // contributes its retained descriptor inventory. The shared projection
    // owner handles fields, cases and elements for both, without turning an
    // inline constructor into a fictitious named storage allocation.
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
                // The carried descriptor supplies its qualification contract,
                // even when its referent's declaration is less refined and
                // the actual membership was established by flow.
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

/// Completing an opaque retained carrier may expose writes to its backing.
/// Ending an ordinary reference's input lifetime creates no write: the call's
/// actual writes have their own frame. Unknown carriers retain the conservative
/// closing effect. These are invalidation coordinates only.
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
    let (_, state) = borrow
        .states
        .iter()
        .find(|(_, row)| row.state_symbol == state_symbol)?;
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
        // A later operand can form a debt in this same statement. Its future
        // effects cannot invalidate facts before that formation. A completed
        // nested producer is available to the enclosing invocation.
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

/// A checked provider-output loan retains its performed identity but names no
/// declaration-relative backing. It cannot restore a caller ownership home.
fn loan_write_region(place: CanonicalPlace) -> StorageRegion {
    if matches!(place.root, PlaceRoot::InvocationOutput(_)) {
        StorageRegion::UnresolvedContent { carrier: place }
    } else {
        StorageRegion::Declared(place)
    }
}

/// Whether a recorded loan's owner path and a place's field path overlap.
/// Same prefix law as `borrow::last_uses`'s owner-path matching: an absent
/// or dynamic selector conservatively overlaps its counterpart.
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

#[cfg(test)]
mod retirement_effect_tests {
    use super::*;
    use arena::HandleSpan;
    use language_semantics::content::{
        ContentAlgebraIdentity, ContentPlaceRoot, ContentPlaceVersion, ContentProjectionExpression,
        ContentProjectionPlan, ContentStructuralPlace,
    };
    use language_semantics::{ReferenceAccess, SemanticDomainId};
    use std::sync::Arc;
    use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
    use typed_trees::name::Identifier;

    #[test]
    fn reference_retirement_does_not_invent_an_opaque_carriers_closing_write() {
        let mut symbols = SymbolTableBuilder::default();
        let token = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Token"));
        let domain = symbols.insert_root(SymbolKind::Domain, SymbolNameRef::Static("Retained"));
        let issuer = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("register"));
        let unknown = symbols.insert_root(SymbolKind::TypeParameter, SymbolNameRef::Static("T"));
        let mut program = typed_trees::TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let definition = program
            .data_definitions
            .insert(typed_trees::data::DataDefinition {
                symbol: token,
                name: Identifier::from("Token"),
                lifetime_parameters: vec![Identifier::from("source")],
                ..Default::default()
            });
        program.roots.data_definitions = HandleSpan::from_parts(definition, 1);
        let carrier = program
            .type_reference_table
            .insert(TypeReferenceNode::Generic {
                base_symbol: token,
                base_name: Identifier::from("Token"),
                lifetime_arguments: vec![Identifier::from("source")],
                arguments: HandleSpan::empty(),
            });
        let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let unknown = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: unknown,
                name: Identifier::from("T"),
            });
        let references = [
            ReferenceAccess::Shared,
            ReferenceAccess::Mutable,
            ReferenceAccess::WriteOnly,
        ]
        .map(|access| {
            program
                .type_reference_table
                .insert(TypeReferenceNode::Reference {
                    referee: carrier,
                    access,
                    lifetime: Some(Identifier::from("source")),
                })
        });
        let projection = ContentProjectionPlan {
            domain,
            semantic_domain: SemanticDomainId::NULL,
            carrier_identity: "Token".to_owned(),
            machine: issuer,
            algebra: ContentAlgebraIdentity::IntervalSet {
                coordinate_space: "bytes".to_owned(),
            },
            expression: ContentProjectionExpression::IntervalSet {
                members: Vec::new(),
            },
            report_fingerprint: 0,
        };
        let context =
            validation::CheckingContext::new(&program).with_retained_borrow_custodies(Arc::from([
                checked_trees::RetainedBorrowCustodyFact {
                    callable: issuer,
                    source: ContentStructuralPlace {
                        version: ContentPlaceVersion::Entry,
                        root: ContentPlaceRoot::Parameter {
                            position: 0,
                            symbol: token,
                            name: "source".to_owned(),
                        },
                        segments: Vec::new(),
                    },
                    result: ContentStructuralPlace {
                        version: ContentPlaceVersion::Current,
                        root: ContentPlaceRoot::Result,
                        segments: Vec::new(),
                    },
                    access: ReferenceAccess::Mutable,
                    lifetime: Identifier::from("source"),
                    callable_lifetime_parameter_ordinal: 0,
                    result_data: token,
                    result_lifetime_argument_ordinal: 0,
                    retained_semantic_domain: SemanticDomainId::NULL,
                    source_projection: projection.clone(),
                    result_projection: projection,
                },
            ]));
        assert_eq!(
            validation::borrow_contracts::retained_lifetime_owner_paths(&context, carrier),
            Some(vec![Vec::new()])
        );
        assert!(carrier_retirement_may_write(&context, Some(carrier)));
        for reference in references {
            assert!(!carrier_retirement_may_write(&context, Some(reference)));
        }
        assert!(!carrier_retirement_may_write(&context, Some(unit)));
        assert!(carrier_retirement_may_write(&context, Some(unknown)));
        assert!(carrier_retirement_may_write(&context, None));
    }
}
