//! Boundary-call signature resolution: the "receiver field -> attached data
//! -> field's declared trait -> called signature" chain shared by the proof
//! engines (the checker's R4 witness mints and psi-proof's
//! bounded-assignment containment walk both consume it; psi-validation's
//! sibling resolves through its crate-local symbol caches, which also cover
//! `contains`-clause receivers). Lives here because the chain is a pure
//! `TypedTrees` walk and every consumer already depends on this crate.

use crate::TypedTrees;
use crate::data::DataMember;
use crate::machine::Machine;
use crate::signature::StateSignature;
use crate::statement::TableCall;
use crate::types::TypeReferenceNode;

/// The boundary-trait signature a call statement resolves to
/// (`self.fw.get_size(..)` -> trait `Firmware`'s `get_size`), or `None` for
/// every other receiver class (free calls, machine-field receivers,
/// non-trait field types).
pub fn called_boundary_signature<'program>(
    program: &'program TypedTrees,
    machine: &Machine,
    call: &TableCall,
) -> Option<&'program StateSignature> {
    let receiver = program
        .statement_table
        .name_path_members(call.receiver)
        .last()?;
    let attached = machine.attached_data.as_ref()?;
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == attached.as_str())?;
    let field_type = program
        .data_members(data)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == receiver.as_str() => field
                .type_reference
                .is_valid()
                .then_some(field.type_reference),
            _ => None,
        })?;
    let trait_definition = if let Some(requirement) =
        crate::service::exact_bound_service_requirement(program, field_type)
    {
        program
            .traits()
            .iter()
            .find(|definition| definition.symbol == requirement)?
    } else {
        let TypeReferenceNode::Named {
            name: trait_name, ..
        } = program.type_reference_table.type_reference(field_type)
        else {
            return None;
        };
        program
            .traits()
            .iter()
            .find(|definition| definition.name.as_str() == trait_name.as_str())?
    };
    program
        .trait_machine_signatures(trait_definition)
        .iter()
        .find(|signature| signature.name == call.target)
}
