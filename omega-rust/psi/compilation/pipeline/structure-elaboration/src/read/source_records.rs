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

fn source_application(
    typed: &typed_trees::TypedTrees,
    data: &DataDefinition,
    reference: TypeReferenceHandle,
    machine: MachineId,
) -> Checked<SourceTypeApplication> {
    // Empty substitutions are justified by this exact declaration, not by a
    // missing application record. Applied data needs its retained bindings.
    if !data.lifetime_parameters.is_empty()
        || !data.type_parameters.is_empty()
        || data.generic_instance.is_some()
    {
        return Err(refuse(
            machine,
            Construct::Machine,
            Unserved::Type("applied source record"),
        ));
    }
    if !reference.is_valid() || !data.symbol.is_valid() {
        return Err(refuse(
            machine,
            Construct::Machine,
            Unserved::Spelling("record source application"),
        ));
    }
    match typed.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { symbol, .. } if *symbol == data.symbol => {}
        _ => {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Typing("record application names another declaration"),
            ));
        }
    }
    Ok(SourceTypeApplication {
        reference,
        substitutions: Vec::new(),
    })
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
        let data = self.data_declaration(source.declaration).ok_or_else(|| {
            refuse(
                machine,
                Construct::Machine,
                Unserved::Spelling("record source declaration"),
            )
        })?;
        let application = source_application(self.typed, data, reference, machine)?;
        // This reader admits natural, no-carry data only. A selected plan must
        // bring its real schema, policy, field mapping and target realizations.
        if data.properties.carry.is_some()
            || self.typed.plan_laid_layout_for_type(reference).is_some()
        {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Type("planned source record"),
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
        self.records
            .iter()
            .map(|(record, declaration)| {
                let source = terminal_check_tables::row(
                    &self.record_sources,
                    record,
                    machine,
                    Construct::Machine,
                )?;
                correspondence(record, declaration, source, machine)
            })
            .collect()
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
    let application = source.application.clone().ok_or_else(|| {
        refuse(
            machine,
            Construct::Machine,
            Unserved::Spelling("record without a source application"),
        )
    })?;
    if source.fields.len() != declaration.fields.len()
        || source.cases.len() != declaration.cases.len()
        || source
            .cases
            .iter()
            .zip(&declaration.cases)
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
    append(&declaration.fields, &source.fields)?;
    for (case, (symbol, symbols)) in declaration.cases.iter().zip(&source.cases) {
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
        let scalar = ValueType::Scalar(ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        ));
        let declaration = RecordType {
            copyable: false,
            identity: "source-owned-storage".into(),
            fields: vec![
                RecordField {
                    id: StructuralFieldId::new(3).unwrap(),
                    identity: "first".into(),
                    value_type: scalar,
                },
                RecordField {
                    id: StructuralFieldId::new(7).unwrap(),
                    identity: "second".into(),
                    value_type: scalar,
                },
            ],
            cases: Vec::new(),
            requires: Vec::new(),
        };
        let source = RecordSource {
            declaration: data.symbol,
            application: Some(SourceTypeApplication {
                reference,
                substitutions: Vec::new(),
            }),
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
    fn export_refuses_missing_applications_and_truncated_field_rosters() {
        let (declaration, mut source) = record_fixture();
        let application = source.application.take();
        assert!(
            correspondence(
                StructuralTypeId::new(2).unwrap(),
                &declaration,
                &source,
                machine()
            )
            .is_err()
        );
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
}
