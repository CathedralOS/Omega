//! Discriminator observations from actual owned zero formation.
//!
//! A zero-valid first case can leave every other payload unestablished. Calls
//! still owe the containing value's conditional field contracts, so retain the
//! tag that makes those inactive obligations vacuous. Only a local without an
//! initializer enters here; a type annotation or a borrowed zero pointer is
//! never a source of value observations. Normal mutation invalidation owns the
//! lifetime of each independent tag, just as it does constructor-written tags.

use facts::{
    Fact, FactOrigin, FactPayload, FactPlace, FactPlan, PlaceSegment, ProgramPoint,
    QualificationEvidence,
};
use symbols::SymbolHandle;
use typed_trees::data::DataMember;
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

struct ZeroCase {
    path: Vec<PlaceSegment>,
    variant: SymbolHandle,
}

pub(super) fn append(
    program: &dyn validation::CheckingInput,
    facts: &mut FactPlan,
    root: SymbolHandle,
    reference: TypeReferenceHandle,
    point: ProgramPoint,
    evidence: QualificationEvidence,
) {
    // Erased terms have no allocated runtime zero image or discriminator.
    if validation::proof_only_classification(program)
        .proof_only_mention(program, reference)
        .is_some()
    {
        return;
    }
    let mut rows = Vec::new();
    collect(
        program,
        reference,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut rows,
    );
    for row in rows {
        let place = facts.append_symbol_place(root);
        for segment in row.path {
            facts.push_place_segment(place, segment);
        }
        facts.append_fact_context(Fact {
            place: FactPlace::Place(place),
            point,
            origin: FactOrigin::StatementTransfer,
            evidence,
            payload: FactPayload::AssignedCase {
                variant: row.variant,
            },
        });
    }
}

fn collect(
    program: &dyn validation::CheckingInput,
    reference: TypeReferenceHandle,
    path: &mut Vec<PlaceSegment>,
    visited: &mut Vec<(SymbolHandle, TypeReferenceHandle)>,
    rows: &mut Vec<ZeroCase>,
) {
    let carrier = validation::peel_declared_constraints(program, reference);
    let node = program.type_reference_table.type_reference(carrier);
    if let TypeReferenceNode::FixedArray {
        element_type,
        length: FixedArrayLength::Literal(length),
    } = node
    {
        // Form the element's observations once, not once per array position.
        // Zero-length arrays contribute no child occurrence or invented tag.
        if *length == 0 {
            return;
        }
        let mut element_rows = Vec::new();
        collect(
            program,
            *element_type,
            &mut Vec::new(),
            visited,
            &mut element_rows,
        );
        if element_rows.is_empty() {
            return;
        }
        for index in 0..*length {
            for row in &element_rows {
                let mut placed = Vec::with_capacity(path.len() + 1 + row.path.len());
                placed.extend_from_slice(path);
                placed.push(PlaceSegment::FixedIndex { index });
                placed.extend_from_slice(&row.path);
                rows.push(ZeroCase {
                    path: placed,
                    variant: row.variant,
                });
            }
        }
        return;
    }
    // No reference, slice descriptor or pointer can manufacture observations
    // about its referent. Only inline nominal storage is traversed below.
    if !matches!(
        node,
        TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. }
    ) {
        return;
    }
    let Some(data) = program
        .data_application_definition(carrier)
        .or_else(|| program.closed_data_definition(carrier))
        .or_else(|| validation::field_domains::owned_nominal_data_definition(program, carrier))
    else {
        return;
    };
    let identity = (data.symbol, data.generic_instance.unwrap_or_default());
    if visited.contains(&identity) {
        return;
    }
    visited.push(identity);
    let members = program.data_members(data);
    // Common children can themselves be established even when another common
    // field leaves their parent gated. No parent case is needed to name them.
    for member in members {
        if let DataMember::Field(field) = member
            && !field.relevance.is_erased()
        {
            let prefix = path.len();
            validation::push_field_place_segments(program, path, field.symbol);
            collect(program, field.type_reference, path, visited, rows);
            path.truncate(prefix);
        }
    }
    if let Some(variant) = members.iter().find_map(|member| match member {
        DataMember::Variant(variant) => Some(variant),
        DataMember::Field(_) => None,
    }) && !validation::type_requires_establishment(program, reference)
    {
        rows.push(ZeroCase {
            path: path.clone(),
            variant: variant.symbol,
        });
        // Never choose a later possible case or descend into an inactive
        // payload. The full zero-validity judgment includes this first payload
        // and its case-local constraints before its tag becomes evidence.
        for field in program.data_payload_fields(variant) {
            if field.relevance.is_erased() {
                continue;
            }
            let prefix = path.len();
            validation::push_field_place_segments(program, path, field.symbol);
            collect(program, field.type_reference, path, visited, rows);
            path.truncate(prefix);
        }
    }
    visited.pop();
}
