//! Witnesses carried by a field use in its authored machine. Folding a body
//! retains that member and expression in SourceOrigin. The lowering that
//! materializes the operation supplies its actual structural place.
use crate::body::{Regions, ValueType};
use crate::source_state::SourceNode;
use crate::source_state::expression::ExpressionHandle;
use crate::source_state::package_openings::PackageOpeningHandle;
use crate::{SourceOrigin, Unserved};
use semantic_vocabulary::{MachineId, StructuralTypeId};
use std::collections::HashMap;
use symbols::SymbolHandle;
use terminal_check_tables::Checked;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FieldOpening {
    pub opening: PackageOpeningHandle,
    pub witnesses: Vec<StructuralTypeId>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct FieldOpenings(HashMap<(MachineId, ExpressionHandle), FieldOpening>);

impl FieldOpenings {
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn get(&self, source: SourceOrigin) -> Option<&FieldOpening> {
        let (_, SourceNode::Expression(expression)) = source.node()? else {
            return None;
        };
        self.0.get(&(source.source_member()?, expression))
    }
}

impl super::Program<'_> {
    pub(super) fn opened_field_type(
        &mut self,
        id: MachineId,
        field: SymbolHandle,
        record: StructuralTypeId,
        regions: Regions,
        source: SourceOrigin,
    ) -> Checked<Option<ValueType>> {
        let Some((_, SourceNode::Expression(expression))) = source.node() else {
            return Ok(None);
        };
        let Some(projection) = self.typed.package_projection(expression).copied() else {
            return Ok(None);
        };
        if projection.field != field {
            return Err(crate::reject(
                id,
                source,
                "package projection changed its selected field",
            ));
        }
        let mut witnesses = Vec::new();
        let mut rigid_parameters = Vec::new();
        for (_, witness) in &self.typed.package_opening(projection.opening).witnesses {
            let ValueType::Record(record, ..) = self.value_type(id, *witness)? else {
                return Err(super::refuse_at(
                    id,
                    Unserved::Typing("opened witness without a structural carrier"),
                ));
            };
            let symbol = self
                .typed
                .type_reference_table
                .type_reference(*witness)
                .as_named_symbol()
                .ok_or_else(|| {
                    crate::reject(id, source, "opened witness lost its nominal binder")
                })?;
            rigid_parameters.push(symbol);
            witnesses.push(record);
        }
        let prepared = FieldOpening {
            opening: projection.opening,
            witnesses,
        };
        let member = source.source_member().ok_or_else(|| {
            crate::reject(id, source, "package projection lost its machine occurrence")
        })?;
        if self
            .field_openings
            .0
            .insert((member, expression), prepared.clone())
            .is_some_and(|previous| previous != prepared)
        {
            return Err(crate::reject(
                id,
                source,
                "one projection has two opened field types",
            ));
        }
        // A projected Owner<S> carries this subject's fresh S, which is in
        // scope for the selected field just as a declaration binder is in
        // scope while reading its schema.
        let scope = std::sync::Arc::new(crate::source_state::SourceTypeScope {
            parent: Some(self.source_scope.clone()),
            substitutions: Vec::new(),
            rigid_parameters,
        });
        let outer = std::mem::replace(&mut self.source_scope, scope);
        let value_type = self.value_type(id, projection.result_type);
        self.source_scope = outer;
        let value_type = value_type?;
        let substitution = self.field_regions(id, record, regions)?;
        Ok(Some(self.substitute(value_type, &substitution)))
    }
}
