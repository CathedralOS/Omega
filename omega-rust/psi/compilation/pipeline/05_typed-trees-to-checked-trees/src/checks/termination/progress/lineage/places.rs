//! Finite storage partitions for parameter values, not recursive referent snapshots.

use super::super::ProgressSubject;
use typed_trees::machine::Machine;
use validation::field_domains::exact_declared_field;

/// Select a finite owned prefix along this demand alone. Unused sibling fields
/// are never enumerated, even when their type graph shares large subtrees.
pub(super) fn partition(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    subject: &ProgressSubject,
) -> Option<ProgressSubject> {
    let parameter = program
        .machine_states(machine)
        .iter()
        .find_map(|state| program.state_parameter_by_symbol(state, subject.root))?;
    let mut partition = ProgressSubject {
        root: subject.root,
        projections: Vec::new(),
    };
    let mut current = parameter.type_reference;
    let mut visiting = Vec::new();
    for projection in &subject.projections {
        current = program.type_reference_table.peeled_type_reference(current);

        let field = match replay_partition_data(program, current) {
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
                    .flat_map(|member| program.data_member_fields(std::slice::from_ref(member)))
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

/// The data definition a partition may still replay: a resolved concrete
/// type's declaration, plus a generic application's own declaration. The
/// application arguments stay unresolved, so the declared members verify only
/// this projection's identity; the instantiated field type is itself an opaque
/// leaf for the next projection.
fn replay_partition_data(
    program: &dyn ::validation::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Option<&typed_trees::data::DataDefinition> {
    if let Some(data) = validation::declared_data_type(program, type_reference) {
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
