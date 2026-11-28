//! Exact typed expression occurrences that project a hidden package field.
//! Folding a body retains its original member and expression in SourceOrigin.
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
pub(crate) struct PreparedProjection {
    pub opening: PackageOpeningHandle,
    pub witnesses: Vec<StructuralTypeId>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct PackageProjections(HashMap<(MachineId, ExpressionHandle), PreparedProjection>);

impl PackageProjections {
    pub(crate) fn get(&self, source: SourceOrigin) -> Option<&PreparedProjection> {
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
        for (_, witness) in &self.typed.package_opening(projection.opening).witnesses {
            let ValueType::Record(record, ..) = self.value_type(id, *witness)? else {
                return Err(super::refuse_at(
                    id,
                    Unserved::Typing("opened witness without a structural carrier"),
                ));
            };
            witnesses.push(record);
        }
        let prepared = PreparedProjection {
            opening: projection.opening,
            witnesses,
        };
        let member = source.source_member().ok_or_else(|| {
            crate::reject(id, source, "package projection lost its machine occurrence")
        })?;
        if self
            .package_projections
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
        let value_type = self.value_type(id, projection.result_type)?;
        let substitution = self.field_regions(id, record, regions)?;
        Ok(Some(self.substitute(value_type, &substitution)))
    }
}
