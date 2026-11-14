//! A product keeps the source identities that minted its nominal storage.
//! Retain an actual application from the same typed owner while reading; the
//! product factory must not reconstruct one from a name after checking.

use semantic_vocabulary::{MachineId, StructuralTypeId};
use symbols::SymbolHandle;
use terminal_check_tables::{Checked, Construct, Refusal, RefusalReason};
use typed_trees::data::DataDefinition;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use typed_trees::{SourceFieldCorrespondence, SourceRecordCorrespondence, SourceTypeApplication};

use super::{Program, places::RecordSource};
use crate::body::RecordType;
use crate::{Unserved, refuse};

pub(super) fn source_application(
    typed: &typed_trees::TypedTrees,
    data: &DataDefinition,
    reference: TypeReferenceHandle,
    machine: MachineId,
) -> Checked<SourceTypeApplication> {
    use typed_trees::data::TypeParameterKind;
    let unavailable = |detail| refuse(machine, Construct::Machine, Unserved::Type(detail));
    if !typed
        .type_reference_table
        .contains_type_reference(reference)
        || !data.symbol.is_valid()
    {
        return Err(malformed(
            machine,
            "record source application has no retained reference or declaration",
        ));
    }
    // Closure supplies a substituted declaration, not evidence that its
    // generic arguments satisfy additional requirements. Those premises must
    // remain expressible at this entrance; unsupported ones are refused.
    // A lifetime binder is not an unresolved binder of the record: lifetimes.md,
    // "Lifetime arguments are erased regions, separate from runtime generic
    // arity, layout and monomorphization", so every application of the
    // declaration, elided or named, is this one record. Whether the lifetimes
    // an occurrence names are declared is judged where its type is read
    // (`lifetime_application`); the storage correspondence has nothing to keep.
    if !data.type_parameters.is_empty() {
        return Err(unavailable(
            "record application with unresolved type binders",
        ));
    }
    let selected = typed
        .closed_data_definition(reference)
        .ok_or_else(|| unavailable("record application without its closed retained declaration"))?;
    if selected != data {
        return Err(malformed(
            machine,
            "record application selects another retained declaration",
        ));
    }
    if data.properties.carry.is_some() || typed.plan_laid_layout_for_type(reference).is_some() {
        return Err(unavailable("planned source record"));
    }
    let mut substitutions = Vec::new();
    if let Some(origin) = data.generic_instance {
        if !typed.type_reference_table.contains_type_reference(origin)
            || typed.normalized_type_identity(origin) != typed.normalized_type_identity(reference)
        {
            return Err(malformed(
                machine,
                "record instance lost its complete originating application",
            ));
        }
        let TypeReferenceNode::Generic {
            base_symbol,
            lifetime_arguments,
            arguments,
            ..
        } = typed.type_reference_table.type_reference(origin)
        else {
            return Err(malformed(
                machine,
                "record instance origin is not a generic application",
            ));
        };
        let template = typed
            .unique_data_definition_by_symbol(*base_symbol)
            .ok_or_else(|| malformed(machine, "record application has no unique template"))?;
        if !lifetime_arguments.is_empty() || !template.lifetime_parameters.is_empty() {
            return Err(unavailable(
                "record application with type and lifetime arguments",
            ));
        }
        if !template.conformance_bounds.is_empty()
            || !data.conformance_bounds.is_empty()
            || !template.requires_facts.is_empty()
            || !data.requires_facts.is_empty()
        {
            return Err(unavailable(
                "record application whose generic premises are not lowered",
            ));
        }
        if template.properties.carry.is_some()
            || template.cleanup_selection.is_some()
            || data.cleanup_selection.is_some()
            || template.quotient.is_some()
            || data.quotient.is_some()
        {
            return Err(unavailable(
                "record application with unlowered carry, cleanup or quotient premises",
            ));
        }
        if template.supply_mode != data.supply_mode
            || template.properties != data.properties
            || template.zero_gated != data.zero_gated
            || template.retired_identities != data.retired_identities
        {
            return Err(malformed(
                machine,
                "record application changed its declaration's storage obligations",
            ));
        }
        let parameters = typed.data_type_parameters(template);
        let arguments = typed
            .type_reference_table
            .type_reference_handles(*arguments);
        if parameters.len() != arguments.len() {
            return Err(malformed(
                machine,
                "record application lost its complete argument tuple",
            ));
        }
        let mut binders = Vec::with_capacity(parameters.len());
        for (parameter, argument) in parameters.iter().zip(arguments) {
            if !parameter.symbol.is_valid()
                || typed.symbols.get(parameter.symbol).parent != template.symbol
                || typed.symbols.get(parameter.symbol).kind != symbols::SymbolKind::TypeParameter
                || !typed
                    .type_reference_table
                    .contains_type_reference(*argument)
            {
                return Err(malformed(
                    machine,
                    "record application has a foreign binder or argument",
                ));
            }
            if !matches!(parameter.kind, TypeParameterKind::Type)
                || parameter.bounds != typed_trees::data::DataProperties::default()
            {
                return Err(unavailable(
                    "record application with constrained or non-type parameters",
                ));
            }
            if !typed_trees::domain::index_argument_is_closed(typed, *argument) {
                return Err(unavailable(
                    "record application with an unresolved argument",
                ));
            }
            // Ordinary nominal identities erase regions. Do not let a new
            // byte/reference reader turn that storage key into lifetime
            // correspondence until this same application owner retains it.
            if typed
                .region_free_scoped_type_identity(
                    typed_trees::type_identity::TypeIdentityRequest::ordinary(*argument),
                    &[],
                )
                .is_none()
            {
                return Err(unavailable(
                    "record application whose reference regions are not retained",
                ));
            }
            binders.push(parameter.symbol);
            substitutions.push((parameter.symbol, *argument));
        }
        binders.sort_unstable_by_key(|binder| (binder.arena_index(), binder.generation()));
        if binders.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(malformed(
                machine,
                "record application repeats a template binder",
            ));
        }
        validate_applied_members(typed, template, data, reference, &substitutions, machine)?;
    } else {
        match typed.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } if *symbol == data.symbol => {}
            // `Inner<'a>`: lifetime arguments only, one per declared binder.
            TypeReferenceNode::Generic {
                base_symbol,
                lifetime_arguments,
                arguments,
                ..
            } if *base_symbol == data.symbol
                && arguments.is_empty()
                && lifetime_arguments.len() == data.lifetime_parameters.len() => {}
            _ => {
                return Err(malformed(
                    machine,
                    "record application has no retained instantiated owner",
                ));
            }
        }
    }
    Ok(SourceTypeApplication {
        reference,
        substitutions,
    })
}

fn validate_applied_members(
    typed: &typed_trees::TypedTrees,
    template: &DataDefinition,
    selected: &DataDefinition,
    reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    machine: MachineId,
) -> Checked<()> {
    use typed_trees::data::{DataField, DataMember};
    use typed_trees::type_identity::TypeIdentityRequest;
    let same_field = |expected: &DataField, actual: &DataField| {
        typed.data_constructor_field(reference, template.symbol, expected.symbol) == Some(actual)
            && expected.symbol == actual.symbol
            && expected.identity == actual.identity
            && expected.name == actual.name
            && expected.relevance == actual.relevance
            && typed
                .type_reference_table
                .contains_type_reference(expected.type_reference)
            && typed
                .type_reference_table
                .contains_type_reference(actual.type_reference)
            && typed.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
                type_reference: expected.type_reference,
                binders: &[],
                substitutions,
                ..TypeIdentityRequest::ordinary(expected.type_reference)
            }) == typed.normalized_type_identity(actual.type_reference)
    };
    let expected = typed.data_members(template);
    let actual = typed.data_members(selected);
    if expected.len() != actual.len()
        || expected
            .iter()
            .zip(actual)
            .any(|(expected, actual)| match (expected, actual) {
                (DataMember::Field(expected), DataMember::Field(actual)) => {
                    !same_field(expected, actual)
                }
                (DataMember::Variant(expected), DataMember::Variant(actual)) => {
                    let expected_fields = typed.data_payload_fields(expected);
                    let actual_fields = typed.data_payload_fields(actual);
                    expected.symbol != actual.symbol
                        || expected.identity != actual.identity
                        || expected.name != actual.name
                        || expected.requires_facts != actual.requires_facts
                        || expected.retired_payload_identities != actual.retired_payload_identities
                        || expected_fields.len() != actual_fields.len()
                        || expected_fields
                            .iter()
                            .zip(actual_fields)
                            .any(|(expected, actual)| !same_field(expected, actual))
                }
                _ => true,
            })
    {
        return Err(malformed(
            machine,
            "record application changed its exact substituted member roster",
        ));
    }
    Ok(())
}

/// Both references were authenticated against this immutable typed owner.
/// Canonical type identities join aliases while the tuple retains each exact
/// template binder; a matching printed name cannot supply either association.
pub(super) fn same_application(
    typed: &typed_trees::TypedTrees,
    left: &SourceTypeApplication,
    right: &SourceTypeApplication,
) -> bool {
    typed.normalized_type_identity(left.reference)
        == typed.normalized_type_identity(right.reference)
        && left.substitutions.len() == right.substitutions.len()
        && left.substitutions.iter().zip(&right.substitutions).all(
            |((left_binder, left_type), (right_binder, right_type))| {
                left_binder == right_binder
                    && typed.normalized_type_identity(*left_type)
                        == typed.normalized_type_identity(*right_type)
            },
        )
}

impl Program<'_> {
    pub(super) fn record_application(
        &mut self,
        machine: MachineId,
        record: StructuralTypeId,
        reference: TypeReferenceHandle,
    ) -> Checked<()> {
        let source =
            terminal_check_tables::row(&self.record_sources, record, machine, Construct::Machine)?;
        let data = self
            .typed
            .closed_data_definition(reference)
            .ok_or_else(|| {
                malformed(
                    machine,
                    "record source application has no closed declaration",
                )
            })?;
        if data.symbol != source.declaration {
            return Err(malformed(
                machine,
                "record source application changed its declaration",
            ));
        }
        let application = source_application(self.typed, data, reference, machine)?;
        if let Some(retained) = &source.application
            && !same_application(self.typed, retained, &application)
        {
            return Err(malformed(
                machine,
                "one record identity was reused for another complete application",
            ));
        }
        let source = self
            .record_sources
            .get_mut(record)
            .ok_or_else(|| malformed(machine, "record source is outside its table"))?;
        if source.application.is_none() {
            source.application = Some(application);
        }
        Ok(())
    }

    pub(crate) fn source_record_correspondence(
        &self,
        machine: MachineId,
    ) -> Checked<Vec<SourceRecordCorrespondence>> {
        if self.records.len() != self.record_sources.len() {
            return Err(malformed(machine, "record/source catalogs differ"));
        }
        let mut found = Vec::new();
        for (record, declaration) in self.records.iter() {
            let source = terminal_check_tables::row(
                &self.record_sources,
                record,
                machine,
                Construct::Machine,
            )?;
            if let crate::StorageShape::ByteSequence(carrier) = declaration.shape {
                let source_is_bytes = if let Some(application) = &source.application {
                    application.substitutions.is_empty()
                        && super::bytes::source_byte_view(
                            self.typed,
                            application.reference,
                            source.region_owner,
                            machine,
                        )?
                        .is_some_and(|view| {
                            view.carrier == carrier && view.identity == declaration.identity
                        })
                } else {
                    carrier.is_shared_readable_view()
                };
                if source.declaration.is_valid()
                    || !source.fields.is_empty()
                    || !source.cases.is_empty()
                    || !source_is_bytes
                {
                    return Err(malformed(
                        machine,
                        "byte storage has nominal or invalid source custody",
                    ));
                }
                // A literal defines its own claim-free structural byte type.
                // Its source and contents stay on the establishment operation;
                // neither that type nor a source view mints a nominal schema.
                continue;
            }
            if matches!(
                declaration.shape,
                crate::StorageShape::FixedArray { .. }
                    | crate::StorageShape::PrimitiveScalar { .. }
            ) {
                // These physical shapes have no nominal declaration or
                // field schema. Their actual type reference is retained,
                // but cannot mint nominal source authority.
                if source.declaration.is_valid()
                    || !source.fields.is_empty()
                    || !source.cases.is_empty()
                    || !source.application.as_ref().is_some_and(|application| {
                        self.typed
                            .type_reference_table
                            .contains_type_reference(application.reference)
                            && application.substitutions.is_empty()
                    })
                {
                    return Err(malformed(
                        machine,
                        "physical array storage has nominal or missing source custody",
                    ));
                }
                continue;
            }
            found.push(correspondence(record, declaration, source, machine)?);
        }
        Ok(found)
    }
}

fn malformed(machine: MachineId, detail: &'static str) -> Refusal {
    Refusal {
        reason: RefusalReason::ForeignIdentity,
        machine,
        concerns: Construct::Machine,
        detail: detail.into(),
    }
}

fn correspondence(
    record: StructuralTypeId,
    declaration: &RecordType,
    source: &RecordSource,
    machine: MachineId,
) -> Checked<SourceRecordCorrespondence> {
    // A declaration nothing names as a type has no occurrence to retain
    // (`data Unused { .. }` in a judged source). `record_definition` reads
    // one only for a declaration with no type parameter that is no generic
    // instance, so the declaration alone identifies the record.
    let application = source.application.clone();
    if source.fields.len() != declaration.fields().len()
        || source.cases.len() != declaration.cases().len()
        || source
            .cases
            .iter()
            .zip(declaration.cases())
            .any(|((_, fields), case)| fields.len() != case.fields.len())
    {
        return Err(malformed(machine, "record/source member rosters differ"));
    }
    let mut fields = Vec::new();
    let mut append =
        |declared: &[crate::body::RecordField], symbols: &[SymbolHandle]| -> Checked<()> {
            for (field, symbol) in declared.iter().zip(symbols) {
                if !symbol.is_valid() {
                    return Err(malformed(machine, "record field has no declaration"));
                }
                fields.push(SourceFieldCorrespondence {
                    field: field.id,
                    declaration: *symbol,
                    schema: *symbol,
                });
            }
            Ok(())
        };
    append(declaration.fields(), &source.fields)?;
    for (case, (symbol, symbols)) in declaration.cases().iter().zip(&source.cases) {
        if !symbol.is_valid() {
            return Err(malformed(machine, "record case has no declaration"));
        }
        append(&case.fields, symbols)?;
    }
    Ok(SourceRecordCorrespondence {
        record,
        declaration: source.declaration,
        application,
        storage_identity: declaration.identity.clone(),
        schema: source.declaration,
        // The absence of carry and selected layout was checked while retaining
        // the application. Natural data has no selected policy or realization.
        policy: SymbolHandle::invalid(),
        layout: None,
        fields,
        realizations: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::{RecordField, ValueType};
    use arena::{Handle, HandleSpan};
    use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, StructuralFieldId};

    fn machine() -> MachineId {
        MachineId::new(1).unwrap()
    }
    fn symbol(raw: u32) -> SymbolHandle {
        SymbolHandle::from_parts(raw, 1)
    }

    fn application_fixture() -> (typed_trees::TypedTrees, DataDefinition, TypeReferenceHandle) {
        let mut typed = typed_trees::TypedTrees::default();
        let data = DataDefinition {
            symbol: symbol(1),
            ..Default::default()
        };
        let reference = typed.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: data.symbol,
            name: "Storage".into(),
        });
        typed.push_data_definition(data.clone());
        (typed, data, reference)
    }

    #[test]
    fn an_application_retains_its_source_reference_and_proven_empty_bindings() {
        let (typed, data, reference) = application_fixture();
        let application = source_application(&typed, &data, reference, machine()).unwrap();
        assert_eq!(application.reference, reference);
        assert!(application.substitutions.is_empty());
    }

    #[test]
    fn missing_wrong_owner_and_unbound_applications_do_not_gain_identities() {
        let (mut typed, mut data, reference) = application_fixture();
        assert!(
            source_application(&typed, &data, TypeReferenceHandle::invalid(), machine()).is_err()
        );
        let other = typed.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: symbol(2),
            name: "Storage".into(),
        });
        assert!(
            source_application(&typed, &data, other, machine()).is_err(),
            "same spelling is not the owner"
        );
        data.type_parameters = HandleSpan::from_parts(Handle::from_parts(1, 1), 1);
        assert!(
            source_application(&typed, &data, reference, machine()).is_err(),
            "missing substitutions are not empty substitutions"
        );
    }

    fn record_fixture() -> (RecordType, RecordSource) {
        let (_, data, reference) = application_fixture();
        let scalar = ValueType::plain(ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        ));
        let declaration = RecordType {
            copyable: false,
            identity: "source-owned-storage".into(),
            shape: crate::StorageShape::Record(vec![
                RecordField {
                    id: StructuralFieldId::new(3).unwrap(),
                    identity: "first".into(),
                    value_type: scalar,
                    byte_predicates: semantic_vocabulary::ByteSequencePredicateSet::EMPTY,
                },
                RecordField {
                    id: StructuralFieldId::new(7).unwrap(),
                    identity: "second".into(),
                    value_type: scalar,
                    byte_predicates: semantic_vocabulary::ByteSequencePredicateSet::EMPTY,
                },
            ]),
            requires: Vec::new(),
        };
        let source = RecordSource {
            declaration: data.symbol,
            application: Some(SourceTypeApplication {
                reference,
                substitutions: Vec::new(),
            }),
            region_owner: SymbolHandle::invalid(),
            fields: vec![symbol(4), symbol(9)],
            cases: Vec::new(),
        };
        (declaration, source)
    }

    #[test]
    fn natural_storage_retains_exact_field_ids_and_source_schema() {
        let (declaration, source) = record_fixture();
        let output = correspondence(
            StructuralTypeId::new(2).unwrap(),
            &declaration,
            &source,
            machine(),
        )
        .unwrap();
        assert_eq!(output.declaration, source.declaration);
        assert_eq!(output.schema, source.declaration);
        assert_eq!(output.storage_identity, declaration.identity);
        assert_eq!(
            output
                .fields
                .iter()
                .map(|f| (f.field.get(), f.declaration, f.schema))
                .collect::<Vec<_>>(),
            vec![(3, symbol(4), symbol(4)), (7, symbol(9), symbol(9))]
        );
    }

    #[test]
    fn export_keeps_an_unnamed_declaration_and_refuses_truncated_field_rosters() {
        let (declaration, mut source) = record_fixture();
        let application = source.application.take();
        let unnamed = correspondence(
            StructuralTypeId::new(2).unwrap(),
            &declaration,
            &source,
            machine(),
        )
        .unwrap();
        assert_eq!(unnamed.declaration, source.declaration);
        assert_eq!(unnamed.application, None);
        source.application = application;
        source.fields.pop();
        assert_eq!(
            correspondence(
                StructuralTypeId::new(2).unwrap(),
                &declaration,
                &source,
                machine()
            )
            .unwrap_err()
            .reason,
            RefusalReason::ForeignIdentity
        );
    }
    /// Deliberately supplied rows challenge the receiving owner here. The
    /// parsed-source controls cover the real producer rather than this fixture.
    struct AppliedFixture {
        typed: typed_trees::TypedTrees,
        template: DataDefinition,
        parameter: SymbolHandle,
        u32_type: TypeReferenceHandle,
        u64_type: TypeReferenceHandle,
    }

    impl AppliedFixture {
        fn new(bounds: typed_trees::data::DataProperties) -> Self {
            use semantic_vocabulary::PackageKeyIdentity;
            use source::{SourceMap, SourceOrigin, SourceSpan, Span};
            use std::{path::PathBuf, sync::Arc};
            use symbols::{BuiltinTypeAtom, SymbolKind, SymbolNameRef, SymbolTableBuilder};
            use typed_trees::data::{DataField, DataMember, DataVariant, TypeParameter};
            let text = "Container T None Some value";
            let mut sources = SourceMap::default();
            let source = sources
                .add_with_metadata(
                    PathBuf::from("application.omg"),
                    text.into(),
                    PathBuf::from("package"),
                    PackageKeyIdentity::from_digest([42; 32]),
                    SourceOrigin::User,
                )
                .source_id;
            let token = |name: &str| {
                let start = text.find(name).unwrap();
                SymbolNameRef::Source(SourceSpan::new(
                    source,
                    Span::new(start, start + name.len()),
                ))
            };
            let mut builder = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
            let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
            let mut builtins = SymbolTableBuilder::child_handles(
                builder.insert_children(
                    root,
                    BuiltinTypeAtom::ALL
                        .iter()
                        .map(|atom| {
                            (
                                SymbolKind::BuiltinType,
                                SymbolNameRef::Static(atom.symbol_name()),
                            )
                        })
                        .chain([(SymbolKind::Data, token("Container"))]),
                ),
            )
            .collect::<Vec<_>>();
            let owner = builtins.pop().unwrap();
            let children = SymbolTableBuilder::child_handles(builder.insert_children(
                owner,
                [
                    (SymbolKind::TypeParameter, token("T")),
                    (SymbolKind::Variant, token("None")),
                    (SymbolKind::Variant, token("Some")),
                ],
            ))
            .collect::<Vec<_>>();
            let field = SymbolTableBuilder::child_handles(
                builder.insert_children(children[2], [(SymbolKind::Field, token("value"))]),
            )
            .next()
            .unwrap();
            let mut typed = typed_trees::TypedTrees {
                symbols: builder.finish(),
                ..Default::default()
            };
            let reference = |typed: &mut typed_trees::TypedTrees, symbol, name: &str| {
                typed.type_reference_table.insert(TypeReferenceNode::Named {
                    symbol,
                    name: name.into(),
                })
            };
            let u32_type = reference(
                &mut typed,
                builtins[BuiltinTypeAtom::Integer(
                    numerics::integer_carrier::FixedInteger::unsigned(32),
                )
                .ordinal()],
                "u32",
            );
            let u64_type = reference(&mut typed, builtins[BuiltinTypeAtom::U64.ordinal()], "u64");
            let formal = reference(&mut typed, children[0], "T");
            let parameters = typed.data_type_parameters.insert_many([TypeParameter {
                symbol: children[0],
                name: "T".into(),
                bounds,
                ..Default::default()
            }]);
            let payload = typed.data_payload_fields.insert_many([DataField {
                symbol: field,
                identity: Some(0),
                name: "value".into(),
                type_reference: formal,
                ..Default::default()
            }]);
            let members = typed.data_members.insert_many([
                DataMember::Variant(DataVariant {
                    symbol: children[1],
                    name: "None".into(),
                    identity: Some(0),
                    ..Default::default()
                }),
                DataMember::Variant(DataVariant {
                    symbol: children[2],
                    name: "Some".into(),
                    identity: Some(1),
                    payload,
                    ..Default::default()
                }),
            ]);
            let template = DataDefinition {
                symbol: owner,
                name: "Container".into(),
                type_parameters: parameters,
                members,
                ..Default::default()
            };
            typed.push_data_definition(template.clone());
            Self {
                typed,
                template,
                parameter: children[0],
                u32_type,
                u64_type,
            }
        }

        fn apply(
            &mut self,
            argument: TypeReferenceHandle,
            field_type: TypeReferenceHandle,
        ) -> TypeReferenceHandle {
            use typed_trees::data::DataMember;
            let arguments = self
                .typed
                .type_reference_table
                .insert_type_reference_handles([argument]);
            let reference = self
                .typed
                .type_reference_table
                .insert(TypeReferenceNode::Generic {
                    base_symbol: self.template.symbol,
                    base_name: "Container".into(),
                    lifetime_arguments: Vec::new(),
                    arguments,
                });
            let mut members = self.typed.data_members(&self.template).to_vec();
            let DataMember::Variant(case) = &mut members[1] else {
                panic!("Some case");
            };
            let mut payload = self.typed.data_payload_fields(case).to_vec();
            payload[0].type_reference = field_type;
            case.payload = self.typed.data_payload_fields.insert_many(payload);
            let mut selected = self.template.clone();
            selected.type_parameters = HandleSpan::empty();
            selected.generic_instance = Some(reference);
            selected.members = self.typed.data_members.insert_many(members);
            self.typed
                .retain_data_application_definition(reference, selected, true);
            reference
        }
    }

    #[test]
    fn complete_applications_separate_arguments_and_allow_finite_nesting() {
        let mut fixture = AppliedFixture::new(Default::default());
        let first = fixture.apply(fixture.u32_type, fixture.u32_type);
        let second = fixture.apply(fixture.u64_type, fixture.u64_type);
        let nested = fixture.apply(second, second);
        let alias = fixture.apply(fixture.u64_type, fixture.u64_type);
        let mut program = Program::new(&fixture.typed, false);
        let ValueType::Record(first_record, _, _) = program.record_at(machine(), first).unwrap()
        else {
            panic!("record");
        };
        let ValueType::Record(second_record, _, _) = program.record_at(machine(), second).unwrap()
        else {
            panic!("record");
        };
        let ValueType::Record(nested_record, _, _) = program.record_at(machine(), nested).unwrap()
        else {
            panic!("record");
        };
        assert_ne!(first_record, second_record);
        assert_ne!(nested_record, second_record);
        assert_eq!(
            program.record_at(machine(), alias).unwrap(),
            ValueType::record(second_record)
        );
        let source = program.record_sources.get(second_record).unwrap();
        assert_eq!(source.application.as_ref().unwrap().reference, second);
        assert_eq!(
            source.application.as_ref().unwrap().substitutions,
            [(fixture.parameter, fixture.u64_type)]
        );
        assert_eq!(
            program.records.get(nested_record).unwrap().cases()[1].fields[0].value_type,
            ValueType::record(second_record)
        );
        assert_eq!(
            program
                .source_record_correspondence(machine())
                .unwrap()
                .len(),
            3
        );
    }

    #[test]
    fn a_closed_marker_cannot_authorize_the_wrong_substituted_payload() {
        let mut fixture = AppliedFixture::new(Default::default());
        let wrong = fixture.apply(fixture.u64_type, fixture.u32_type);
        let mut program = Program::new(&fixture.typed, false);
        let refusal = program.record_at(machine(), wrong).unwrap_err();
        assert!(
            refusal.detail.contains("substituted member roster"),
            "{refusal:?}"
        );
        let valid = fixture.apply(fixture.u64_type, fixture.u64_type);
        let original = fixture.typed.closed_data_definition(valid).unwrap().clone();
        let mut forged = original.clone();
        let mut members = fixture.typed.data_members(&forged).to_vec();
        let typed_trees::data::DataMember::Variant(some) = &mut members[1] else {
            panic!("the retained Some case");
        };
        let replacement =
            symbols::SymbolTableBuilder::child_handles(fixture.typed.symbols.append_children(
                some.symbol,
                [(
                    symbols::SymbolKind::Field,
                    symbols::SymbolNameRef::Static("value"),
                )],
            ))
            .next()
            .unwrap();
        let mut fields = fixture.typed.data_payload_fields(some).to_vec();
        assert_ne!(fields[0].symbol, replacement);
        fields[0].symbol = replacement;
        some.payload = fixture.typed.data_payload_fields.insert_many(fields);
        forged.members = fixture.typed.data_members.insert_many(members);
        fixture
            .typed
            .retain_data_application_definition(valid, forged, true);
        let mut program = Program::new(&fixture.typed, false);
        let refusal = program.record_at(machine(), valid).unwrap_err();
        assert!(
            refusal.detail.contains("substituted member roster"),
            "{refusal:?}"
        );
        let mut false_copy = original;
        false_copy.properties.multiplicity = language_semantics::Multiplicity::Unrestricted;
        fixture
            .typed
            .retain_data_application_definition(valid, false_copy, true);
        let mut program = Program::new(&fixture.typed, false);
        let refusal = program.record_at(machine(), valid).unwrap_err();
        assert!(
            refusal.detail.contains("storage obligations"),
            "{refusal:?}"
        );
        let borrowed = fixture
            .typed
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: fixture.u64_type,
                access: language_core::ReferenceAccess::Shared,
                lifetime: None,
            });
        let reference_application = fixture.apply(borrowed, borrowed);
        let mut program = Program::new(&fixture.typed, false);
        let refusal = program
            .record_at(machine(), reference_application)
            .unwrap_err();
        assert!(
            refusal
                .detail
                .contains("reference regions are not retained"),
            "{refusal:?}"
        );
        let formal = fixture
            .typed
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: fixture.parameter,
                name: "T".into(),
            });
        let open = fixture.apply(formal, formal);
        let mut program = Program::new(&fixture.typed, false);
        // An ambient interpretation of T cannot close the source tuple.
        program.scope.push((
            fixture.parameter,
            ValueType::plain(ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
            )),
        ));
        let refusal = program.record_at(machine(), open).unwrap_err();
        assert!(
            refusal.detail.contains("unresolved argument"),
            "{refusal:?}"
        );
    }

    #[test]
    fn an_unlowered_generic_premise_is_not_an_empty_application_tuple() {
        let mut fixture = AppliedFixture::new(typed_trees::data::DataProperties {
            multiplicity: language_semantics::Multiplicity::Unrestricted,
            ..Default::default()
        });
        let constrained = fixture.apply(fixture.u64_type, fixture.u64_type);
        let mut program = Program::new(&fixture.typed, false);
        let refusal = program.record_at(machine(), constrained).unwrap_err();
        assert!(
            refusal
                .detail
                .contains("constrained or non-type parameters"),
            "{refusal:?}"
        );
    }
}
