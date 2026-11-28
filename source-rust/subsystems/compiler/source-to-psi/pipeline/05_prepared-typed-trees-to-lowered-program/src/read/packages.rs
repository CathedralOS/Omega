//! A package ties each field to the same tuple of hidden types. Check that
//! relationship while both the authored field types and their concrete
//! occurrences are available, before selecting any physical representation.
use crate::Unserved;
use crate::source_state::TypedTrees;
use crate::source_state::data::{DataDefinition, DataMember};
use crate::source_state::expression::{PackageFieldType, TableStructLiteral};
use crate::source_state::type_identity::TypeIdentityRequest;
use crate::source_state::types::{TypeReferenceHandle, TypeReferenceNode};
use semantic_vocabulary::MachineId;
use symbols::SymbolHandle;
use terminal_check_tables::Checked;

fn symbol_key(symbol: SymbolHandle) -> (u32, u32) {
    (symbol.arena_index(), symbol.generation())
}

#[derive(Debug)]
pub(super) struct PackageFields<'a>(Vec<&'a PackageFieldType>);

impl PackageFields<'_> {
    pub(super) fn get(&self, symbol: &SymbolHandle) -> Option<&TypeReferenceHandle> {
        self.0
            .binary_search_by_key(&symbol_key(*symbol), |field| symbol_key(field.symbol))
            .ok()
            .map(|index| &self.0[index].ty)
    }
}

pub(super) fn check_construction<'a>(
    typed: &TypedTrees,
    machine: MachineId,
    data: &DataDefinition,
    literal: &'a TableStructLiteral,
) -> Checked<Option<PackageFields<'a>>> {
    let invalid = |reason| super::refuse_at(machine, Unserved::Typing(reason));
    if data.package_parameters.is_empty() {
        return if literal.package.is_none() {
            Ok(None)
        } else {
            Err(invalid("ordinary record literal retains package witnesses"))
        };
    }
    let package = literal
        .package
        .as_deref()
        .ok_or_else(|| invalid("package literal has no construction evidence"))?;
    let types = &typed.type_reference_table;
    let binders = typed
        .data_type_parameters
        .span(data.package_parameters)
        .ok_or_else(|| invalid("package declaration has stale hidden binders"))?;
    if package.witnesses.len() != binders.len() {
        return Err(invalid(
            "package witness count differs from the declaration",
        ));
    }
    let mut substitutions = Vec::new();
    if !data.type_parameters.is_empty() {
        let public = literal.application_type;
        if !types.contains_type_reference(public) {
            return Err(invalid("package literal has no public type application"));
        }
        let TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } = types.type_reference(public)
        else {
            return Err(invalid("package literal omits visible type arguments"));
        };
        let parameters = typed
            .data_type_parameters
            .span(data.type_parameters)
            .ok_or_else(|| invalid("package declaration has stale visible binders"))?;
        let visible = types.type_reference_handles(*arguments);
        if *base_symbol != data.symbol || parameters.len() != visible.len() {
            return Err(invalid(
                "package literal has another public type application",
            ));
        }
        substitutions.extend(
            parameters
                .iter()
                .zip(visible)
                .map(|(p, &ty)| (p.symbol, ty)),
        );
    }
    substitutions.extend(
        binders
            .iter()
            .zip(&package.witnesses)
            .map(|(p, &ty)| (p.symbol, ty)),
    );
    if substitutions.iter().any(|(binder, ty)| {
        !typed.symbols.symbols().nodes().is_valid(*binder)
            || !types.contains_type_reference(*ty)
            || !typed.normalized_type_identity(*ty).is_valid()
    }) {
        return Err(invalid(
            "package construction has an unresolved type argument",
        ));
    }
    let mut rows: Vec<_> = package.fields.iter().collect();
    rows.sort_unstable_by_key(|field| symbol_key(field.symbol));
    let members = typed
        .data_members
        .span(data.members)
        .ok_or_else(|| invalid("package declaration has a stale field roster"))?;
    if rows.len() != members.len() || rows.windows(2).any(|pair| pair[0].symbol == pair[1].symbol) {
        return Err(invalid(
            "package construction has missing, repeated or extra field types",
        ));
    }
    let fields = PackageFields(rows);
    for member in members {
        let DataMember::Field(field) = member else {
            return Err(invalid("package literal requires a record declaration"));
        };
        let Some(&concrete) = fields.get(&field.symbol) else {
            return Err(invalid(
                "package construction supplies a field from another declaration",
            ));
        };
        if !types.contains_type_reference(field.type_reference)
            || !types.contains_type_reference(concrete)
        {
            return Err(invalid("package construction has a stale field type"));
        }
        let expected = typed.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
            substitutions: &substitutions,
            ..TypeIdentityRequest::ordinary(field.type_reference)
        });
        if !expected.is_valid() || expected != typed.normalized_type_identity(concrete) {
            return Err(invalid(
                "package field type differs from its witness substitution",
            ));
        }
    }
    Ok(Some(fields))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_state::data::{DataField, TypeParameter};
    use crate::source_state::expression::{PackageConstruction, PackageFieldType};
    use crate::source_symbols::SymbolTableBuilder;
    use symbols::{SymbolKind, SymbolNameRef};

    // Stage 03 has already decided the witness here. Corrupt only one of the
    // two field types on its handoff, keeping the witness and owner unchanged.
    #[test]
    fn two_readers_keep_their_own_state_and_read_types() {
        let mut symbols = SymbolTableBuilder::with_sources(None);
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let [reader, read, file, socket] = SymbolTableBuilder::child_handles(
            symbols.insert_children(
                root,
                ["Reader", "Read", "File", "Socket"]
                    .map(|name| (SymbolKind::Data, SymbolNameRef::Static(name))),
            ),
        )
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
        let [witness, state, callback] =
            SymbolTableBuilder::child_handles(symbols.insert_children(
                reader,
                [
                    (SymbolKind::TypeParameter, SymbolNameRef::Static("S")),
                    (SymbolKind::Field, SymbolNameRef::Static("state")),
                    (SymbolKind::Field, SymbolNameRef::Static("read")),
                ],
            ))
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        let mut typed = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let types = &mut typed.type_reference_table;
        let [s, file, socket] =
            [(witness, "S"), (file, "File"), (socket, "Socket")].map(|(symbol, name)| {
                types.insert(TypeReferenceNode::Named {
                    symbol,
                    name: name.into(),
                })
            });
        let [abstract_fields, file_fields, socket_fields] = [s, file, socket].map(|ty| {
            let reference = types.insert(TypeReferenceNode::Reference {
                referee: ty,
                access: language_core::ReferenceAccess::Mutable,
                lifetime: None,
            });
            let arguments = types.insert_type_reference_handles([ty]);
            let operation = types.insert(TypeReferenceNode::Generic {
                base_symbol: read,
                base_name: "Read".into(),
                lifetime_arguments: vec![],
                arguments,
            });
            [reference, operation]
        });
        let [state_type, read_type] = abstract_fields;
        let package_parameters = typed.data_type_parameters.insert_many([TypeParameter {
            symbol: witness,
            name: "S".into(),
            ..Default::default()
        }]);
        let members = typed.data_members.insert_many(
            [(state, "state", state_type), (callback, "read", read_type)].map(
                |(symbol, name, type_reference)| {
                    DataMember::Field(DataField {
                        symbol,
                        name: name.into(),
                        type_reference,
                        ..Default::default()
                    })
                },
            ),
        );
        let data = DataDefinition {
            symbol: reader,
            package_parameters,
            members,
            ..Default::default()
        };
        let literal = |ty, [reference, operation]: [TypeReferenceHandle; 2]| TableStructLiteral {
            type_symbol: reader,
            package: Some(Box::new(PackageConstruction {
                witnesses: vec![ty],
                fields: vec![
                    PackageFieldType {
                        symbol: callback,
                        ty: operation,
                    },
                    PackageFieldType {
                        symbol: state,
                        ty: reference,
                    },
                ],
            })),
            ..Default::default()
        };
        let machine = MachineId::new(1).unwrap();
        for (witness, fields) in [(file, file_fields), (socket, socket_fields)] {
            check_construction(&typed, machine, &data, &literal(witness, fields)).unwrap();
        }
        for (replacement, expected) in [
            (callback, "repeated"),
            (
                SymbolHandle::from_parts(state.arena_index(), state.generation() + 1),
                "field from another declaration",
            ),
        ] {
            let mut changed = literal(file, file_fields);
            changed.package.as_mut().unwrap().fields[1].symbol = replacement;
            let error = check_construction(&typed, machine, &data, &changed).unwrap_err();
            assert!(error.detail.contains(expected), "{error:?}");
        }
        for fields in [
            [file_fields[0], socket_fields[1]],
            [socket_fields[0], file_fields[1]],
        ] {
            let error =
                check_construction(&typed, machine, &data, &literal(file, fields)).unwrap_err();
            assert!(error.detail.contains("witness substitution"), "{error:?}");
        }
    }
}
