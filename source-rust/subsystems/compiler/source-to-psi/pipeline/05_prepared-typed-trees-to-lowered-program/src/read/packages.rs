//! A package ties each field to the same tuple of hidden types. Check that
//! relationship while both the authored field types and their concrete
//! occurrences are available, before selecting any physical representation.
use std::collections::HashMap;

use crate::Unserved;
use crate::source_state::TypedTrees;
use crate::source_state::data::{DataDefinition, DataMember};
use crate::source_state::expression::TableStructLiteral;
use crate::source_state::type_identity::TypeIdentityRequest;
use crate::source_state::types::{TypeReferenceHandle, TypeReferenceNode};
use semantic_vocabulary::MachineId;
use symbols::SymbolHandle;
use terminal_check_tables::Checked;

pub(super) fn check_construction(
    typed: &TypedTrees,
    machine: MachineId,
    data: &DataDefinition,
    literal: &TableStructLiteral,
) -> Checked<Option<HashMap<SymbolHandle, TypeReferenceHandle>>> {
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
    if package.witnesses.iter().any(|&witness| typed.package_witness_is_unsized(witness)) {
        return Err(invalid("package construction has an unsized witness"));
    }
    let fields: HashMap<_, _> = package
        .fields
        .iter()
        .map(|field| (field.symbol, field.ty))
        .collect();
    let members = typed
        .data_members
        .span(data.members)
        .ok_or_else(|| invalid("package declaration has a stale field roster"))?;
    if fields.len() != package.fields.len() || fields.len() != members.len() {
        return Err(invalid(
            "package construction has missing, repeated or extra field types",
        ));
    }
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

impl super::Program<'_> {
    /// A declared hidden binder and each fresh opening have different
    /// nominal types. Their storage rows have no readable fields or owned
    /// contents. Only a reference to the sized witness has public geometry.
    pub(super) fn package_witness_type(
        &mut self,
        machine: MachineId,
        reference: TypeReferenceHandle,
    ) -> Checked<Option<crate::body::ValueType>> {
        use super::places::RecordSource;
        use crate::body::{RecordType, StorageShape, ValueType};
        let typed = self.typed;
        let types = &typed.type_reference_table;
        if !types.contains_type_reference(reference) {
            return Ok(None);
        }
        let Some(symbol) = types.type_reference(reference).as_named_symbol() else {
            return Ok(None);
        };
        if !typed.symbols.symbols().nodes().is_valid(symbol)
            || typed.symbols.get(symbol).kind != symbols::SymbolKind::TypeParameter
        {
            return Ok(None);
        }
        if let Some(&row) = self.package_witnesses.get(&symbol) {
            return Ok(Some(ValueType::record(row)));
        }
        let owner = typed.symbols.get(symbol).parent;
        let declared = owner.is_valid()
            && typed
                .unique_data_definition_by_symbol(owner)
                .is_some_and(|data| {
                    typed
                        .data_type_parameters
                        .span_or_empty(data.package_parameters)
                        .iter()
                        .any(|parameter| parameter.symbol == symbol)
                });
        let opened = || {
            typed.package_openings.iter().any(|(_, opening)| {
                opening.witnesses.iter().any(|(_, ty)| {
                    types.contains_type_reference(*ty)
                        && types.type_reference(*ty).as_named_symbol() == Some(symbol)
                })
            })
        };
        if !declared && !opened() {
            return Ok(None);
        }
        let identity = typed.package_qualified_type_identity_text(reference);
        if identity.is_empty() {
            return Err(super::unavailable(
                machine,
                "package witness lacks its nominal identity",
            ));
        }
        let source = RecordSource {
            declaration: symbol,
            constant_input_owner: None,
            cleanup: None,
            application: Some(crate::source_state::SourceTypeApplication {
                reference,
                substitutions: Vec::new(),
                scope: Default::default(),
            }),
            region_owner: owner,
            fields: Vec::new(),
            cases: Vec::new(),
            erased: Vec::new(),
        };
        let row = self.push_record_row(
            RecordType {
                identity,
                multiplicity: terminal_psi::StructuralMultiplicity::Linear,
                cleanup: None,
                shape: StorageShape::Opaque,
                requires: Vec::new(),
                authored_layout: false,
                erased: Vec::new(),
            },
            source,
        );
        self.package_witnesses.insert(symbol, row);
        Ok(Some(ValueType::record(row)))
    }

    pub(super) fn retain_package_schema(
        &mut self,
        machine: MachineId,
        record: semantic_vocabulary::StructuralTypeId,
        data: &DataDefinition,
    ) -> Checked<()> {
        if data.package_parameters.is_empty() {
            return Ok(());
        }
        let parameters = self
            .typed
            .data_type_parameters
            .span(data.package_parameters)
            .ok_or_else(|| super::unavailable(machine, "package schema lost its binder roster"))?;
        let witnesses = parameters
            .iter()
            .map(|parameter| {
                self.package_witnesses
                    .get(&parameter.symbol)
                    .copied()
                    .ok_or_else(|| {
                        super::unavailable(
                            machine,
                            "package binder has no uniformly represented field",
                        )
                    })
            })
            .collect::<Checked<Vec<_>>>()?;
        self.package_schemas
            .push(terminal_psi::TerminalPackageSchema { record, witnesses });
        Ok(())
    }
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
