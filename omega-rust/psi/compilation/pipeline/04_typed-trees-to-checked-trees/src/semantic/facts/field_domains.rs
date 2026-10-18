//! Declared default-domain fields become entry facts at exact storage places.
//! Ordinary mutations invalidate these facts; a nominal annotation alone does
//! not restore them. Call and state-arrival checks require the same field
//! obligations over the current incoming values before reusing an entry fact.
//!
//! Zero-initialized storage requires its ZII value to satisfy the domain.
//! Nominal input storage relies on the checked incoming argument, independently
//! of whether its machine declaration has a qualified name.
//! Write-only input views supply no facts read from incoming contents, but
//! they do carry the caller's declared field invariants as entry premises —
//! the same rows a readable borrow gets, since every admitted write through
//! the view re-establishes them. Independent fields
//! use separate contexts so invalidation does not erase unrelated evidence.
//!
//! Collection elements carry their declared fields at TWO coordinates: every
//! literal element at its exact `FixedIndex`, and one whole-extent row at the
//! `0..usize::MAX` window that any element index narrows from. The window row
//! is the only elementwise spelling a slice can have (no static extent exists
//! to enumerate); it is still exact evidence -- a write to any element
//! overlaps the containing window and retires it, so an unresolved index is
//! never itself a fact place and never universal coverage. Where per-element
//! rows exist (fixed arrays, machine storage) the window row is seeded under
//! `StatementTransfer` as evidence only -- the element rows already spell the
//! complete call-boundary obligation. For a slice the window row is the ONLY
//! elementwise claim, so it rides the obligation origin and every caller owes
//! the whole extent.
//! Element qualifications use the same declaration roster as record fields:
//! the whole-extent fact for `[i32 in D]` denotes every live scalar element,
//! rather than treating its static element type as proof at a later read.
//!
//! Immutable scalar parameters also carry their enforced float ranges. The
//! range-obligation owner checks incoming values; these source-free predicates
//! transport that promise into ordinary operand captures. They are not new
//! domain-membership obligations and cannot restore a fact after a mutation.

use facts::{
    Fact, FactOrigin, FactPayload, FactPlace, FactPlan, PlaceSegment, ProgramPoint,
    QualificationEvidence,
};
use symbols::SymbolHandle;
use typed_trees::data::DataMember;
use typed_trees::expression::ExpressionNode;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

use validation::field_domains::{
    FieldDomainRow, WHOLE_ELEMENT_EXTENT, readable_fixed_array_elements,
    readable_nominal_definition, readable_slice_element,
};

/// The declared surface of a state parameter for entry-invariant seeding:
/// `&`, `&mut`, and `&write` shells all peel, along with `Constrained` domain
/// wrappers. `readable_type_reference` refuses write-only references because
/// they expose no readable contents, but these seeded rows are the caller's
/// declared field obligations — premises a `requires <place> in D` discharge
/// consumes — never reads of the incoming value.
fn lent_type_reference(
    program: &dyn ::validation::CheckingInput,
    mut reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    while reference.is_valid() {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Reference { referee, .. } => reference = *referee,
            _ => return Some(reference),
        }
    }
    None
}

/// Seed only domains established by a local's zero initialization. Incoming
/// nominal values use the declared parameter obligations instead: zero validity
/// says nothing about an established argument or its machine's qualification.
///
/// Fixed-array fields additionally enumerate every element at its exact
/// `FixedIndex` place: `value.rows[i].label` is live evidence for the single
/// element `i`, invalidated only by writes overlapping that index. Collection
/// fields (fixed array or slice) also carry the whole-extent `0..usize::MAX`
/// row `self.rows[*].label`, the only elementwise spelling that survives a
/// runtime selector or a slice view. Coverage is never encoded at an
/// unresolved `Index` -- a runtime selector cannot be a fact place.
///
/// Zero-initialized locals use this walker under
/// `ProgramPoint::State`/`StatementTransfer` (their binding statement's
/// zeroed storage is the evidence -- see `append_local_zii_field_domain_facts`).
#[allow(clippy::too_many_arguments)]
fn append_zero_initialized_field_domain_facts(
    program: &dyn ::validation::CheckingInput,
    facts: &mut FactPlan,
    root_symbol: symbols::SymbolHandle,
    point: ProgramPoint,
    origin: FactOrigin,
    evidence: QualificationEvidence,
    data: &typed_trees::data::DataDefinition,
    prefix: &[PlaceSegment],
    visited: &[&str],
    refs: &mut arena::HandleSpan<facts::FactRef>,
) {
    for member in program.data_members(data) {
        let Some(field) = member.as_field() else {
            continue;
        };

        let mut field_path = prefix.to_vec();
        validation::push_field_place_segments(program, &mut field_path, field.symbol);

        // SOUNDNESS GATE (per field, one-level or nested): surface the
        // entry-invariant only when the field's ZERO/ZII value provably satisfies
        // the domain -- a read-with-no-prior-write discharges against it, sound
        // only if this exact carrier's zero image is in-domain. Preserve the
        // full instance: a fixed-array capacity is part of its domain application,
        // and a family symbol alone cannot satisfy an indexed call obligation.
        // Domains the zero image violates stay gated until an establishing write.
        for (domain_symbol, semantic_domain) in
            validation::field_domains::predicate_domain_constraint_identities(
                program,
                field.type_reference,
            )
            .into_iter()
            .filter(|(symbol, _)| {
                validation::field_domains::domain_admits_zero_byte_carrier(
                    program,
                    *symbol,
                    field.type_reference,
                )
            })
        {
            // Place `<root>.<prefix…>.<field>`: root the local symbol + the segment
            // chain, so the canonical label matches a `root.a.b` read exactly
            // where the nested write established it.
            let place = facts.append_symbol_place(root_symbol);
            for segment in &field_path {
                facts.push_place_segment(place, *segment);
            }

            let fact = facts.append_fact(Fact {
                place: FactPlace::Place(place),
                point,
                origin,
                evidence,
                payload: FactPayload::DomainMembership {
                    value: typed_trees::expression::ExpressionHandle::invalid(),
                    domain: arena::HandleSpan::empty(),
                    domain_symbol,
                    semantic_domain,
                },
            });
            facts.append_ref(refs, fact);
        }

        // Descend into a struct-typed field so its own domained fields are seeded
        // too. The cycle guard keeps a self-referential data type from looping.
        if let Some(nested) =
            validation::field_domains::data_definition_for_field_type(program, field.type_reference)
            && !visited.contains(&nested.name.as_str())
        {
            let mut next_visited = visited.to_vec();
            next_visited.push(nested.name.as_str());
            append_zero_initialized_field_domain_facts(
                program,
                facts,
                root_symbol,
                point,
                origin,
                evidence,
                nested,
                &field_path,
                &next_visited,
                refs,
            );
        }

        // A slice-typed field's elements carry their declared fields through
        // the whole-extent row alone: no static extent exists to enumerate.
        if let Some(element_type) = readable_slice_element(program, field.type_reference)
            && let Some(nested) = readable_nominal_definition(program, element_type)
            && !visited.contains(&nested.name.as_str())
        {
            let mut next_visited = visited.to_vec();
            next_visited.push(nested.name.as_str());
            let mut element_path = field_path.clone();
            element_path.push(WHOLE_ELEMENT_EXTENT);
            append_zero_initialized_field_domain_facts(
                program,
                facts,
                root_symbol,
                point,
                origin,
                evidence,
                nested,
                &element_path,
                &next_visited,
                refs,
            );
        }

        // A fixed array of nominal elements carries each element's declared
        // fields at `field[i]`; the same ZII gate applies to the leaf domains.
        // The whole-extent `field[0..usize::MAX]` row is seeded alongside so a
        // runtime selector or a slice view can narrow coverage from it. It is
        // EVIDENCE ONLY (`StatementTransfer`): the per-element rows already
        // spell the complete obligation, so collecting the rollup row as a
        // requirement would only double-report every corrupted element.
        let Some((element_type, length)) =
            readable_fixed_array_elements(program, field.type_reference)
        else {
            continue;
        };
        let Some(nested) = readable_nominal_definition(program, element_type) else {
            continue;
        };
        if visited.contains(&nested.name.as_str()) {
            continue;
        }
        let mut next_visited = visited.to_vec();
        next_visited.push(nested.name.as_str());
        for index in 0..length {
            let mut element_path = field_path.clone();
            element_path.push(PlaceSegment::FixedIndex { index });
            append_zero_initialized_field_domain_facts(
                program,
                facts,
                root_symbol,
                point,
                origin,
                evidence,
                nested,
                &element_path,
                &next_visited,
                refs,
            );
        }
        let mut element_path = field_path.clone();
        element_path.push(WHOLE_ELEMENT_EXTENT);
        append_zero_initialized_field_domain_facts(
            program,
            facts,
            root_symbol,
            point,
            FactOrigin::StatementTransfer,
            evidence,
            nested,
            &element_path,
            &next_visited,
            refs,
        );
    }
}

/// #66/P1a param entry-assumption: an IMMUTABLE state parameter declared with a
/// domain qualification carries that domain throughout the state body. The
/// param's implicit `requires param in Domain` makes every caller establish
/// membership (by proof for a bodyful predicate, by retained evidence for a
/// bodyless qualification), and immutability means it can never be reassigned
/// out of the domain. Surfaced at `ProgramPoint::State` (the param belongs to the state),
/// which `build_state_flow_fact` folds into the state entry; the flow's context
/// threading then carries it to every call, including guarded-transition
/// fallthrough arms (now that a transition no longer leaks its branch-taken exit
/// context onto its sibling fallthrough -- see flow/statements.rs).
///
/// Nominal parameters additionally carry their declared field predicates on
/// entry, including mutable and write-only references: calls and transitions
/// are default-domain consumption points. These are live entry facts, not
/// facts restored after arbitrary writes. Ordinary storage invalidation
/// retires them. A write-only view still cannot inspect the incoming value —
/// the seeded rows are the caller's declared invariants, and the lent place's
/// `requires <place> in D` obligation at a nested `&write` subloan discharges
/// against them.
pub(super) fn append_state_parameter_domain_facts(
    program: &dyn ::validation::CheckingInput,
    facts: &mut FactPlan,
) {
    let mut field_domain_walks: Vec<(TypeReferenceHandle, Vec<FieldDomainRow>)> = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut refs = arena::HandleSpan::empty();
            for parameter in program.state_parameters(state) {
                let obligation_origin = FactOrigin::StateParameterDomain {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                };
                // A `&write` parameter keeps its declared field domains as
                // entry premises: they are invariants the caller maintained
                // and every write through the view must re-establish, not
                // observations of incoming contents (which remain impossible).
                // Without these rows a `requires <place> in D` on a nested
                // `&write` subloan could never discharge, even though the
                // lent place's own declaration already carries the atom.
                let lent = lent_type_reference(program, parameter.type_reference);
                if let Some(lent) = lent {
                    let row_index = field_domain_walks
                        .iter()
                        .position(|(reference, _)| *reference == lent)
                        .unwrap_or_else(|| {
                            let index = field_domain_walks.len();
                            field_domain_walks.push((
                                lent,
                                validation::field_domains::declared_type_domain_rows(program, lent),
                            ));
                            index
                        });
                    let rows = &field_domain_walks[row_index].1;
                    append_state_parameter_data_field_domain_facts(
                        facts,
                        machine.symbol,
                        state.symbol,
                        parameter.symbol,
                        obligation_origin,
                        &[],
                        rows,
                        &mut refs,
                    );
                }
                // Declared IEEE field bounds are live entry predicates, just as
                // field domains are. Ordinary capture/copy/join and mutation
                // invalidation retain their exact observed value and lifetime.
                for bound in
                    validation::declared_float_field_bounds(program, parameter.type_reference)
                {
                    let Some(predicate) = bound.predicate() else {
                        continue;
                    };
                    let place = facts.append_symbol_place(parameter.symbol);
                    for symbol in bound.path {
                        facts.push_place_segment(place, PlaceSegment::Field { symbol });
                    }
                    let fact = facts.append_fact(Fact {
                        place: FactPlace::Place(place),
                        point: ProgramPoint::State {
                            machine_symbol: machine.symbol,
                            state_symbol: state.symbol,
                        },
                        origin: obligation_origin,
                        evidence: QualificationEvidence::from_origin(
                            language_semantics::QualificationEvidenceOrigin::Propagated,
                            state.symbol,
                        ),
                        payload: FactPayload::FloatPredicate { predicate },
                    });
                    facts.append_ref(&mut refs, fact);
                }
                // Keep the existing root qualification/resource permission
                // rule separate from default-domain fields of nominal values.
                if parameter.is_mutable {
                    continue;
                }
                let mut predicates = Vec::new();
                crate::values::float_predicates::declared_type_predicates(
                    program,
                    parameter.type_reference,
                    &mut predicates,
                );
                if !predicates.is_empty() {
                    let place = facts.append_symbol_place(parameter.symbol);
                    for predicate in predicates {
                        let fact = facts.append_fact(Fact {
                            place: FactPlace::Place(place),
                            point: ProgramPoint::State {
                                machine_symbol: machine.symbol,
                                state_symbol: state.symbol,
                            },
                            // Numeric range admission already has its own
                            // incoming-value obligation, not a membership row.
                            origin: FactOrigin::StatementTransfer,
                            evidence: QualificationEvidence::from_origin(
                                language_semantics::QualificationEvidenceOrigin::Propagated,
                                state.symbol,
                            ),
                            payload: FactPayload::FloatPredicate { predicate },
                        });
                        facts.append_ref(&mut refs, fact);
                    }
                }
                let mut has_resource_claim = false;
                for (domain_symbol, semantic_domain) in
                    validation::field_domains::domain_constraint_identities(
                        program,
                        parameter.type_reference,
                    )
                {
                    has_resource_claim |= state_parameter_domain_is_resource_claim(
                        program,
                        parameter.type_reference,
                        domain_symbol,
                    );
                    append_state_parameter_domain_fact(
                        facts,
                        machine.symbol,
                        state.symbol,
                        parameter.symbol,
                        obligation_origin,
                        &[],
                        domain_symbol,
                        semantic_domain,
                        &mut refs,
                    );
                }
                if has_resource_claim {
                    append_state_parameter_carry_origin(
                        facts,
                        machine.symbol,
                        state.symbol,
                        parameter.symbol,
                        &mut refs,
                    );
                }
                for permission in carry_constraint_permissions(program, parameter.type_reference) {
                    append_state_parameter_carry_fact(
                        facts,
                        machine.symbol,
                        state.symbol,
                        parameter.symbol,
                        permission,
                        &mut refs,
                    );
                }
            }

            if refs.is_empty() {
                continue;
            }
            append_independent_place_contexts(
                facts,
                ProgramPoint::State {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                },
                refs,
            );
        }
    }
}

fn append_independent_place_contexts(
    facts: &mut FactPlan,
    point: ProgramPoint,
    refs: arena::HandleSpan<facts::FactRef>,
) {
    // Invalidating one storage coordinate must not discard independent
    // parameter or machine-field facts. Facts over the same place stay coupled.
    let mut groups: Vec<(FactPlace, Vec<facts::FactRef>)> = Vec::new();
    for reference in facts.refs.span_or_empty(refs) {
        let place = facts.facts.get(reference.fact).place;
        if let Some((_, group)) =
            groups
                .iter_mut()
                .find(|(candidate, _)| match (*candidate, place) {
                    (FactPlace::Place(left), FactPlace::Place(right)) => {
                        facts.places_equal(left, right)
                    }
                    _ => *candidate == place,
                })
        {
            group.push(*reference);
        } else {
            groups.push((place, vec![*reference]));
        }
    }
    for (_, group) in groups {
        let mut refs = arena::HandleSpan::empty();
        for reference in group {
            facts.refs.append_to_span(&mut refs, reference);
        }
        facts.append_context(point, refs);
    }
}

fn state_parameter_domain_is_resource_claim(
    program: &dyn ::validation::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
    domain_symbol: SymbolHandle,
) -> bool {
    crate::checks::type_multiplicity(program, type_reference)
        == language_semantics::Multiplicity::Linear
        && program
            .domain_definition(domain_symbol)
            .is_some_and(|domain| {
                domain.predicate_body == language_semantics::DomainPredicateBody::Bodyless
            })
}

/// The `Place(parameter_symbol)` + `State` point + `StateParameterDomain`
/// origin + `Propagated` evidence scaffolding both carry facts share.
fn append_state_parameter_carry(
    facts: &mut FactPlan,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    parameter_symbol: SymbolHandle,
    payload: FactPayload,
    refs: &mut arena::HandleSpan<facts::FactRef>,
) {
    let place = facts.append_symbol_place(parameter_symbol);
    let fact = facts.append_fact(Fact {
        place: FactPlace::Place(place),
        point: ProgramPoint::State {
            machine_symbol,
            state_symbol,
        },
        origin: FactOrigin::StateParameterDomain {
            machine_symbol,
            state_symbol,
        },
        evidence: QualificationEvidence::from_origin(
            language_semantics::QualificationEvidenceOrigin::Propagated,
            state_symbol,
        ),
        payload,
    });
    facts.append_ref(refs, fact);
}

fn append_state_parameter_carry_origin(
    facts: &mut FactPlan,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    parameter_symbol: SymbolHandle,
    refs: &mut arena::HandleSpan<facts::FactRef>,
) {
    append_state_parameter_carry(
        facts,
        machine_symbol,
        state_symbol,
        parameter_symbol,
        FactPayload::CarryOrigin {
            value: typed_trees::expression::ExpressionHandle::invalid(),
        },
        refs,
    );
}

fn carry_constraint_permissions(
    program: &dyn ::validation::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Vec<language_semantics::CarryPermission> {
    match program.type_reference_table.type_reference(type_reference) {
        typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            carry_constraint_permissions(program, *referee)
        }
        typed_trees::types::TypeReferenceNode::Constrained { constraints, .. } => program
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .filter_map(|constraint| match constraint {
                typed_trees::types::TypeConstraintNode::Domain(domain) => {
                    language_semantics::CarryPermission::from_name(domain.name.as_str())
                }
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn append_state_parameter_carry_fact(
    facts: &mut FactPlan,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    parameter_symbol: SymbolHandle,
    permission: language_semantics::CarryPermission,
    refs: &mut arena::HandleSpan<facts::FactRef>,
) {
    append_state_parameter_carry(
        facts,
        machine_symbol,
        state_symbol,
        parameter_symbol,
        FactPayload::CarryPermission {
            value: typed_trees::expression::ExpressionHandle::invalid(),
            permission,
        },
        refs,
    );
}

#[allow(clippy::too_many_arguments)]
fn append_state_parameter_data_field_domain_facts(
    facts: &mut FactPlan,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    parameter_symbol: SymbolHandle,
    origin: FactOrigin,
    prefix: &[PlaceSegment],
    rows: &[FieldDomainRow],
    refs: &mut arena::HandleSpan<facts::FactRef>,
) {
    let mut path = Vec::new();
    for row in rows {
        path.clear();
        path.extend_from_slice(prefix);
        path.extend_from_slice(&row.path);
        append_state_parameter_domain_fact(
            facts,
            machine_symbol,
            state_symbol,
            parameter_symbol,
            if row.transfer {
                FactOrigin::StatementTransfer
            } else {
                origin
            },
            &path,
            row.domain_symbol,
            row.semantic_domain,
            refs,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn append_state_parameter_domain_fact(
    facts: &mut FactPlan,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    parameter_symbol: SymbolHandle,
    origin: FactOrigin,
    path: &[PlaceSegment],
    domain_symbol: SymbolHandle,
    semantic_domain: language_semantics::SemanticDomainId,
    refs: &mut arena::HandleSpan<facts::FactRef>,
) {
    let place = facts.append_symbol_place(parameter_symbol);
    for segment in path {
        facts.push_place_segment(place, *segment);
    }
    let fact = facts.append_fact(Fact {
        place: FactPlace::Place(place),
        point: ProgramPoint::State {
            machine_symbol,
            state_symbol,
        },
        origin,
        evidence: QualificationEvidence::from_origin(
            language_semantics::QualificationEvidenceOrigin::Propagated,
            state_symbol,
        ),
        payload: FactPayload::DomainMembership {
            value: typed_trees::expression::ExpressionHandle::invalid(),
            domain: arena::HandleSpan::empty(),
            domain_symbol,
            semantic_domain,
        },
    });
    facts.append_ref(refs, fact);
}

/// #66 case-payload forwarding: a local constructed as a sum CASE with a
/// domain-refined payload (`let cmd = Command::Say { text: "ok" }`, where
/// `Command::Say`'s `text` is `&[u8] in Utf8`) carries that payload domain on
/// `cmd.<payload>`. Construction enforcement (#60-1c, checks::contracts::writes)
/// already proved the payload value in-domain at construction, so a later read of
/// the payload -- in particular a destructured `Command::Say { text }` forwarded
/// as a `requires <arg> in D` call argument, which resolves to `cmd.<payload>` --
/// discharges with no re-proof. Surfaced at `ProgramPoint::State`, folded into the
/// state entry and threaded to the matched arm's calls; the flow's mutation
/// invalidation drops it if `cmd` is reassigned (so it is sound for mutable locals
/// too -- no immutability gate needed).
///
/// CONSERVATIVE: a sibling arm that matches a DIFFERENT case is dead when `cmd` was
/// constructed as one specific case; its `cmd.<other-payload>` obligation shares
/// the same canonical place label, so it discharges only when the domains agree
/// (a mixed-domain dead arm would be rejected -- sound, just incomplete).
pub(super) fn append_local_case_payload_domain_facts(
    program: &dyn ::validation::CheckingInput,
    facts: &mut FactPlan,
) {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut refs = arena::HandleSpan::empty();
            for statement in program.statement_table.statements(state.statement_nodes) {
                let Some(local_data) = statement.as_local_data() else {
                    continue;
                };
                if !local_data.initial_value.is_valid() {
                    continue;
                }
                let ExpressionNode::StructLiteral(literal) = program
                    .expression_table
                    .expression(local_data.initial_value)
                else {
                    continue;
                };
                let Some(case_name) = literal.case_name.as_ref() else {
                    continue;
                };
                let Some(data) = program.data_definition_by_name_str(literal.type_name.as_str())
                else {
                    continue;
                };
                let Some(variant) =
                    program
                        .data_members(data)
                        .iter()
                        .find_map(|member| match member {
                            DataMember::Variant(variant)
                                if variant.name.as_str() == case_name.as_str() =>
                            {
                                Some(variant)
                            }
                            _ => None,
                        })
                else {
                    continue;
                };

                for payload_field in program.data_payload_fields(variant) {
                    for domain_symbol in field_domain_symbols(program, payload_field.type_reference)
                    {
                        // Place `cmd.<payload-field>`: root the local symbol + a Field
                        // segment for the variant's payload field, matching how a
                        // destructured payload arg resolves at the call site.
                        let place = facts.append_symbol_place(local_data.symbol);
                        facts.push_place_segment(
                            place,
                            PlaceSegment::Case {
                                variant: variant.symbol,
                            },
                        );
                        facts.push_place_segment(
                            place,
                            PlaceSegment::Field {
                                symbol: payload_field.symbol,
                            },
                        );
                        let fact = facts.append_fact(Fact {
                            place: FactPlace::Place(place),
                            point: ProgramPoint::State {
                                machine_symbol: machine.symbol,
                                state_symbol: state.symbol,
                            },
                            origin: FactOrigin::LocalCasePayloadDomain {
                                machine_symbol: machine.symbol,
                                state_symbol: state.symbol,
                            },
                            evidence: QualificationEvidence::from_origin(
                                language_semantics::QualificationEvidenceOrigin::Prover,
                                state.symbol,
                            ),
                            payload: FactPayload::DomainMembership {
                                value: typed_trees::expression::ExpressionHandle::invalid(),
                                domain: arena::HandleSpan::empty(),
                                domain_symbol,
                                semantic_domain: language_semantics::SemanticDomainId::NULL,
                            },
                        });
                        facts.append_ref(&mut refs, fact);
                    }
                }
            }

            if refs.is_empty() {
                continue;
            }
            facts.append_context(
                ProgramPoint::State {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                },
                refs,
            );
        }
    }
}

/// `let value: T;` binds zero-initialized storage (dependent_values.md:
/// zero-initializability is a storage representation guarantee). When `T`'s
/// declared field domains admit the zero/empty byte sequence, that zero value
/// already satisfies each such field, so `value.<field> in D` is live evidence
/// from the binding onward. Seeded at `ProgramPoint::State` with the
/// `StatementTransfer` origin: the binding statement is the formation point,
/// and the symbol cannot resolve before its declaration, so folding the rows
/// into state entry is exact. Ordinary mutation invalidation retires every
/// row, and domains the zero value violates stay gated -- a read of a gated
/// field still requires an establishing write first.
///
/// An initializer (`let value: T = expr;`) gets its domain evidence from the
/// assignment transfer instead; seeding the zero here too would be wrong
/// because the initializer replaces the zero before any observation.
pub(super) fn append_local_zii_field_domain_facts(
    program: &dyn ::validation::CheckingInput,
    facts: &mut FactPlan,
) {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut refs = arena::HandleSpan::empty();
            for statement in program.statement_table.statements(state.statement_nodes) {
                let Some(local_data) = statement.as_local_data() else {
                    continue;
                };
                if local_data.initial_value.is_valid() {
                    continue;
                }
                let point = ProgramPoint::State {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                };
                let origin = FactOrigin::StatementTransfer;
                let evidence = QualificationEvidence::from_origin(
                    language_semantics::QualificationEvidenceOrigin::CheckedValidation,
                    state.symbol,
                );

                // Whole-value `let x: T in D;` rows: zero satisfies `D` only
                // when the domain admits the empty byte sequence -- the same
                // ZII gate the per-field rows below use.
                for (domain_symbol, semantic_domain) in
                    validation::field_domains::domain_constraint_identities(
                        program,
                        local_data.type_reference,
                    )
                    .into_iter()
                    .filter(|(domain_symbol, _)| {
                        validation::field_domains::domain_admits_zero_byte_carrier(
                            program,
                            *domain_symbol,
                            local_data.type_reference,
                        )
                    })
                {
                    let place = facts.append_symbol_place(local_data.symbol);
                    let fact = facts.append_fact(Fact {
                        place: FactPlace::Place(place),
                        point,
                        origin,
                        evidence,
                        payload: FactPayload::DomainMembership {
                            value: typed_trees::expression::ExpressionHandle::invalid(),
                            domain: arena::HandleSpan::empty(),
                            domain_symbol,
                            semantic_domain,
                        },
                    });
                    facts.append_ref(&mut refs, fact);
                }

                let Some(data) = validation::field_domains::owned_nominal_data_definition(
                    program,
                    local_data.type_reference,
                ) else {
                    continue;
                };
                append_zero_initialized_field_domain_facts(
                    program,
                    facts,
                    local_data.symbol,
                    point,
                    origin,
                    evidence,
                    data,
                    &[],
                    &[data.name.as_str()],
                    &mut refs,
                );
            }

            if refs.is_empty() {
                continue;
            }
            append_independent_place_contexts(
                facts,
                ProgramPoint::State {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                },
                refs,
            );
        }
    }
}

/// Every normalized predicate-domain symbol on a field type, looking through a
/// leading reference (`&[u8] in Utf8 & NoNul`). Semantic-only constraints do
/// not become flow facts.
fn field_domain_symbols(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Vec<SymbolHandle> {
    validation::field_domains::predicate_domain_constraint_symbols(program, type_reference)
}
