//! Finite storage partitions for parameter values, not recursive referent snapshots.

use super::super::ProgressSubject;
use symbols::SymbolHandle;
use typed_trees::{TypedTrees, data::DataMember, machine::Machine};

/// Select a finite owned prefix along this demand alone. Unused sibling fields
/// are never enumerated, even when their type graph shares large subtrees.
pub(super) fn partition(
    program: &TypedTrees,
    machine: &Machine,
    subject: &ProgressSubject,
) -> Option<ProgressSubject> {
    use typed_trees::types::TypeReferenceNode;
    let parameter = program
        .machine_states(machine)
        .iter()
        .flat_map(|state| program.state_parameters(state))
        .find(|parameter| parameter.symbol == subject.root)?;
    let mut partition = ProgressSubject {
        root: subject.root,
        projections: Vec::new(),
    };
    let mut current = parameter.type_reference;
    let mut visiting = Vec::new();
    for projection in &subject.projections {
        loop {
            match program.type_reference_table.type_reference(current) {
                TypeReferenceNode::Constrained { base_type, .. } => current = *base_type,
                // A reference is one borrowed view of its referent, not a
                // boundary the demanded path stops at. An exact projection
                // still names storage through it; the replayed-type
                // visitation bound below keeps recursive referents finite.
                TypeReferenceNode::Reference { referee, .. } => current = *referee,
                _ => break,
            }
        }
        let field = match replay_partition_data(program, current, machine.symbol) {
            Some(data) => {
                if visiting.contains(&data.symbol) {
                    // Valid recursive proof shapes remain opaque at the
                    // repeated type.
                    return Some(partition);
                }
                visiting.push(data.symbol);
                program
                    .data_members(data)
                    .iter()
                    .flat_map(|member| match member {
                        DataMember::Field(field) => std::slice::from_ref(field),
                        DataMember::Variant(variant) => program.data_payload_fields(variant),
                    })
                    .find(|field| field.symbol == *projection)?
            }
            // An opaque leaf — an unresolved generic argument, a dynamic
            // trait, an unevaluated const type — cannot bound its own
            // fields. A projection that still names one exact declared
            // field carries its own provenance and resumes the replay at
            // that field's declared type; any other tail stays untracked.
            None => match exact_declared_field(program, *projection) {
                Some(field) => field,
                None => return Some(partition),
            },
        };
        partition.projections.push(field.symbol);
        current = field.type_reference;
    }
    Some(partition)
}

/// A more specific partition always wins, including when its origin is unknown.
/// Falling back to an enclosing value would resurrect an overwritten field.
pub(super) fn matching_prefix<'places>(
    places: impl Iterator<Item = &'places ProgressSubject>,
    subject: &ProgressSubject,
) -> Option<&'places ProgressSubject> {
    places
        .filter(|place| {
            place.root == subject.root && subject.projections.starts_with(&place.projections)
        })
        .max_by_key(|place| place.projections.len())
}

/// The data definition a partition may still replay: the same resolved
/// concrete types as correspondence validation, plus a generic application's
/// own declaration. The application arguments stay unresolved, so the
/// declared members verify only this projection's identity; the instantiated
/// field type is itself an opaque leaf for the next projection.
fn replay_partition_data<'program>(
    program: &'program TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
    machine_symbol: SymbolHandle,
) -> Option<&'program typed_trees::data::DataDefinition> {
    if let Some(data) =
        crate::checks::termination::progress::qualification_correspondences::replay_data_type(
            program,
            type_reference,
            machine_symbol,
        )
    {
        return Some(data);
    }
    match program.type_reference_table.type_reference(type_reference) {
        typed_trees::types::TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            ..
        } => program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == *base_symbol && definition.name == *base_name),
        _ => None,
    }
}

/// The declaration a field symbol already identifies. A subject projection is
/// produced by exact member resolution or not at all; when the enclosing type
/// is an opaque leaf the symbol is the only provenance available, and its own
/// declared field type resumes the bounded replay.
fn exact_declared_field<'program>(
    program: &'program TypedTrees,
    symbol: SymbolHandle,
) -> Option<&'program typed_trees::data::DataField> {
    if !symbol.is_valid() || program.symbols.get(symbol).kind != symbols::SymbolKind::Field {
        return None;
    }
    program
        .data_definitions()
        .iter()
        .flat_map(|data| program.data_members(data))
        .flat_map(|member| match member {
            DataMember::Field(field) => std::slice::from_ref(field),
            DataMember::Variant(variant) => program.data_payload_fields(variant),
        })
        .find(|field| field.symbol == symbol)
}
