//! Retain source coordinates while the reader still owns each declaration.
//! Products pair these with the exact checked module; no later consumer has to
//! rediscover a source declaration from an identity string or a field name.

use semantic_vocabulary::{BoundaryMachineId, IdTable, MachineId};
use symbols::SymbolHandle;
use terminal_check_tables::{Checked, Construct};
use typed_trees::{SourceFieldCorrespondence, SourceRecordCorrespondence, SourceTypeApplication};

use super::Program;
use crate::{Unserved, refuse};

impl Program<'_> {
    pub(crate) fn source_records(
        &self,
        context: MachineId,
    ) -> Checked<Vec<SourceRecordCorrespondence>> {
        let unavailable = |reason| refuse(context, Construct::Machine, Unserved::Type(reason));
        let mut result = Vec::with_capacity(self.records.len());
        for (record, source) in self.record_sources.iter() {
            if !self
                .typed
                .symbols
                .symbols()
                .nodes()
                .is_valid(source.declaration)
            {
                return Err(unavailable("source record has a stale declaration"));
            }
            let data = self
                .typed
                .data_definition_by_symbol(source.declaration)
                .ok_or_else(|| unavailable("source record lost its data declaration"))?;
            // This reader admits closed, ordinary checked data. Its canonical
            // self type is an exact application with no substitutions.
            let reference = if self
                .typed
                .type_reference_table
                .contains_type_reference(data.predicate_subject_type)
            {
                data.predicate_subject_type
            } else {
                self.typed
                    .type_reference_table
                    .find_named_type_reference(source.declaration)
                    .ok_or_else(|| unavailable("source record has no retained type application"))?
            };
            if !matches!(self.typed.type_reference_table.type_reference(reference),
                typed_trees::types::TypeReferenceNode::Named { symbol, .. } if *symbol == source.declaration)
            {
                return Err(unavailable(
                    "source record application names another declaration",
                ));
            }
            if self.typed.plan_laid_layout_for_type(reference).is_some() {
                return Err(unavailable(
                    "explicit record layout needs its checked source and geometry correspondence",
                ));
            }
            let declared =
                terminal_check_tables::row(&self.records, record, context, Construct::Machine)?;
            if declared.fields.len() != source.fields.len()
                || declared.cases.len() != source.cases.len()
            {
                return Err(unavailable(
                    "source record field roster differs from its checked shape",
                ));
            }
            let mut fields = Vec::new();
            let mut retain =
                |field: &crate::body::RecordField, declaration: SymbolHandle| -> Checked<()> {
                    if !self.typed.symbols.symbols().nodes().is_valid(declaration) {
                        return Err(unavailable("source record field has a stale declaration"));
                    }
                    fields.push(SourceFieldCorrespondence {
                        field: field.id,
                        declaration,
                        schema: declaration,
                    });
                    Ok(())
                };
            for (field, declaration) in declared.fields.iter().zip(&source.fields) {
                retain(field, *declaration)?;
            }
            for (case, (case_source, payload)) in declared.cases.iter().zip(&source.cases) {
                if !self.typed.symbols.symbols().nodes().is_valid(*case_source)
                    || case.fields.len() != payload.len()
                {
                    return Err(unavailable(
                        "source record case roster differs from its checked payload",
                    ));
                }
                for (field, declaration) in case.fields.iter().zip(payload) {
                    retain(field, *declaration)?;
                }
            }
            result.push(SourceRecordCorrespondence {
                record,
                declaration: source.declaration,
                application: SourceTypeApplication {
                    reference,
                    substitutions: Vec::new(),
                },
                storage_identity: declared.identity.clone(),
                schema: source.declaration,
                // No layout or policy is invented: explicit plans were refused
                // above, and record formation refused a carry policy.
                policy: SymbolHandle::invalid(),
                layout: None,
                fields,
                realizations: Vec::new(),
            });
        }
        Ok(result)
    }

    pub(crate) fn boundary_sources(
        &self,
        context: MachineId,
    ) -> Checked<IdTable<BoundaryMachineId, SymbolHandle>> {
        let mut sources = IdTable::default();
        for (id, boundary) in self.boundaries.rows.iter() {
            if !self
                .typed
                .symbols
                .symbols()
                .nodes()
                .is_valid(boundary.source)
            {
                return Err(refuse(
                    context,
                    Construct::Machine,
                    Unserved::Call("boundary source has a stale declaration"),
                ));
            }
            let retained = sources.push(boundary.source);
            if retained != id {
                return Err(refuse(
                    context,
                    Construct::Machine,
                    Unserved::Call("boundary source coordinates changed during publication"),
                ));
            }
        }
        Ok(sources)
    }
}
