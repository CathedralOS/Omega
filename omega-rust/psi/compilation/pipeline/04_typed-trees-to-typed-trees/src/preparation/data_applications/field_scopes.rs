//! A field schema's lifetime names belong to its declaring data telescope.
//! Keep that source scope on fresh type rows before a projected value becomes
//! a static operand. Copying the raw field into a generic machine must not
//! recapture those names in the machine that happens to consume the value.

use arena::Handle;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{DataField, DataMember};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

enum FieldLocation {
    Member(Handle<DataMember>),
    Payload(Handle<DataField>),
}

pub(in crate::preparation) fn retain(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let mut fields = Vec::new();
    for definition in program.data_definitions() {
        for (position, member) in
            (0..definition.members.count()).zip(program.data_members(definition))
        {
            match member {
                DataMember::Field(field) => fields.push((
                    FieldLocation::Member(definition.members.handle_at(position)),
                    definition.symbol,
                    field.type_reference,
                )),
                DataMember::Variant(variant) => {
                    for (position, field) in
                        (0..variant.payload.count()).zip(program.data_payload_fields(variant))
                    {
                        fields.push((
                            FieldLocation::Payload(variant.payload.handle_at(position)),
                            definition.symbol,
                            field.type_reference,
                        ));
                    }
                }
            }
        }
    }
    for (location, owner, reference) in fields {
        if program
            .type_reference_table
            .operand_lifetime_owner(reference)
            .is_valid()
            || !contains_regions(program, reference)
        {
            continue;
        }
        let scoped = scoped_field_type(program, owner, reference)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        match location {
            FieldLocation::Member(handle) => {
                if let DataMember::Field(field) = program.data_members.get_mut(handle) {
                    field.type_reference = scoped;
                }
            }
            FieldLocation::Payload(handle) => {
                program.data_payload_fields.get_mut(handle).type_reference = scoped
            }
        }
    }
    Ok(())
}

fn contains_regions(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    crate::monomorphization::for_each_type_reference(
        program,
        &[reference],
        |_, node| {
            matches!(
                node,
                TypeReferenceNode::Reference {
                    lifetime: Some(_),
                    ..
                }
            ) || matches!(node, TypeReferenceNode::Generic { lifetime_arguments, .. } if !lifetime_arguments.is_empty())
        },
        |_, _, _| false,
    )
}

fn scoped_field_type(
    program: &mut TypedTrees,
    owner: SymbolHandle,
    reference: TypeReferenceHandle,
) -> Result<TypeReferenceHandle, &'static str> {
    // The ordinary application copier preserves independently supplied child
    // scopes and expression payloads. This is a fresh declaration-owned view,
    // not a mutation of interned input or a region granted by a caller name.
    let scoped = program.apply_callable_type(reference, &[], &[], owner)?;
    program
        .type_reference_table
        .retain_operand_lifetime_owner(scoped, owner)?;
    Ok(scoped)
}
