//! Read the canonical catalogue already retained by an actual source-query
//! invocation. Geometry is computation, not value membership or access rights.

use crate::{InvocationTarget, PureTerminalEvaluationOutcome, PureTerminalEvaluationReceipt};
use terminal_psi::{
    StructuralTypeShape, TargetLayoutSchemaArgument, TargetLayoutSchemaField,
    TargetLayoutSchemaFieldKind,
};
use typed_trees::{SourceRecordCorrespondence, types::TypeReferenceHandle};

// The receipt retains its actual generated call before execution in the same
// frozen owner. A later catalogue selection cannot replace that operand.
pub(super) fn query_reference(
    receipt: &PureTerminalEvaluationReceipt,
) -> Result<TypeReferenceHandle, String> {
    use language_core::core_vocabulary::{CoreVocabulary, ReflectionDeclaration};
    use typed_trees::references::StaticArgumentCall;
    let source = receipt.source();
    let call = receipt.call()?;
    let declaration = source
        .machine_by_symbol(call.target_symbol)
        .or_else(|| source.machine_holding_state(call.target_symbol))
        .ok_or("policy schema request lost its selected declaration")?;
    if source.core_vocabulary(declaration.symbol)
        != Some(CoreVocabulary::Reflection(ReflectionDeclaration::Schema))
    {
        return Err("policy schema request is not the actual core schema query".into());
    }
    let request =
        StaticArgumentCall::Expression(typed_trees::expression::ExpressionHandle::invalid(), call);
    if request.lexical_owner(source) != receipt.requester() {
        return Err("policy schema request belongs to another lexical author".into());
    }
    let [operand] = request.arguments(source) else {
        return Err("policy schema request lost its exact single type operand".into());
    };
    if !source
        .type_reference_table
        .contains_type_reference(operand.type_reference)
        || source
            .closed_data_definition(operand.type_reference)
            .is_none()
    {
        return Err("policy schema request has no actual closed type operand".into());
    }
    Ok(operand.type_reference)
}

pub(super) fn source_row(
    receipt: &PureTerminalEvaluationReceipt,
) -> Result<&SourceRecordCorrespondence, String> {
    let source = receipt.source();
    let semantic_type = query_reference(receipt)?;
    let semantic = source
        .closed_data_definition(semantic_type)
        .ok_or("policy schema has no exact closed semantic application")?;
    let mut matching = receipt.program().source_records().iter().filter(|row| {
        row.application
            .as_ref()
            .is_some_and(|source| source.reference == semantic_type)
    });
    let row = matching
        .next()
        .ok_or("policy schema query did not publish this actual source application")?;
    if matching.next().is_some() || row.declaration != semantic.symbol {
        return Err("policy schema query has ambiguous or foreign source correspondence".into());
    }
    Ok(row)
}

pub(super) fn schema_geometry(
    receipt: &PureTerminalEvaluationReceipt,
    target: InvocationTarget<'_>,
) -> Result<TargetLayoutSchemaArgument, String> {
    // The receipt is the existing source/code/result owner, not a decoded
    // description label. Never reconstruct a structural identity from bytes.
    if !matches!(
        receipt.outcome(),
        PureTerminalEvaluationOutcome::Materialized { .. }
    ) {
        return Err("policy schema query did not complete with an actual value".into());
    }
    if receipt.admission().target() != target.name {
        return Err("policy schema query belongs to another target".into());
    }
    let facts = target
        .layout_target
        .ok_or("policy schema requires its actual immutable target facts")?;
    facts
        .validate()
        .map_err(|reason| format!("policy schema target facts: {reason}"))?;
    if facts.profile != target.name {
        return Err("policy schema target facts name another profile".into());
    }
    if let Some(carriers) = target.carrier_layout
        && (carriers.pointer_byte_size != facts.pointer_size
            || carriers.pointer_byte_alignment != facts.pointer_alignment
            || carriers.case_tag_byte_size != facts.case_tag_size
            || carriers.case_tag_byte_alignment != facts.case_tag_alignment)
    {
        return Err("policy schema target carrier model differs from its actual facts".into());
    }
    let source = receipt.source();
    let semantic_type = query_reference(receipt)?;
    let semantic = source
        .closed_data_definition(semantic_type)
        .ok_or("policy schema lost its closed semantic declaration")?;
    let row = source_row(receipt)?;
    let module = receipt.program().view().module();
    let declaration = module
        .structural_type(row.record)
        .ok_or("policy schema lost its actual structural declaration")?;
    if !matches!(declaration.shape, StructuralTypeShape::Record { .. }) {
        return Err("policy schema requires its complete sum/case geometry producer".into());
    }
    let mut geometry = terminal_semantics::ReferenceStorageGeometry::new(
        module.structural_types.iter(),
        |application| {
            Err(terminal_semantics::StorageGeometryError::UnresolvedExtent(
                application,
            ))
        },
    )
    .with_target_carriers(target.carrier_layout);
    // A pending selected child is an unmet dependency, not the default layout.
    // The existing resolver refuses it until its actual target realization is
    // supplied; this first record path never substitutes guessed geometry.
    let fields = geometry
        .record_fields(row.record)
        .map_err(|reason| format!("policy schema field geometry: {reason}"))?;
    if fields.len() != row.fields.len() || fields.len() > 32 {
        return Err("policy schema differs from its complete 32-field source roster".into());
    }
    let mut retained = Vec::with_capacity(fields.len());
    for field in fields {
        let mut correspondences = row
            .fields
            .iter()
            .filter(|source| source.field == field.declaration.id);
        let correspondence = correspondences
            .next()
            .ok_or("policy schema field has no exact source correspondence")?;
        if correspondences.next().is_some() {
            return Err("policy schema field has duplicate source correspondence".into());
        }
        let mut members = source
            .data_members(semantic)
            .iter()
            .filter_map(typed_trees::data::DataMember::as_field)
            .filter(|member| member.symbol == correspondence.declaration);
        let member = members
            .next()
            .ok_or("policy schema field belongs to another source declaration")?;
        if members.next().is_some() || correspondence.schema != member.symbol {
            return Err("policy schema field lost its source/schema member join".into());
        }
        let kind = if field.declaration.field_type.scalar_type().is_some() {
            TargetLayoutSchemaFieldKind::Scalar
        } else if matches!(
            field.declaration.field_type.byte_sequence_carrier(),
            Some(terminal_psi::ByteSequenceCarrier::BoundedOwned { .. })
        ) || field
            .declaration
            .field_type
            .as_structural()
            .and_then(|identity| module.structural_type(identity))
            .is_some_and(|child| matches!(child.shape, StructuralTypeShape::FixedArray { .. }))
        {
            TargetLayoutSchemaFieldKind::Repeated
        } else {
            TargetLayoutSchemaFieldKind::Nested
        };
        let layout_source = terminal_semantics::schema_field_layout_source(
            module,
            &field.declaration.field_type,
            &terminal_semantics::UnselectedRecordStorage,
        )
        .map_err(|reason| format!("policy schema geometry source: {reason}"))?;
        retained.push(TargetLayoutSchemaField {
            record_field: field.declaration.id,
            schema_field: field.declaration.id,
            key: 0, // Assigned below, after the input's stable identity ordering.
            identity: member.identity,
            byte_size: field.byte_size,
            byte_alignment: field.byte_alignment,
            kind,
            layout_source,
        });
    }
    if retained.iter().any(|field| field.identity.is_some()) {
        if retained.iter().any(|field| field.identity.is_none()) {
            return Err(
                "canonical layout schema does not serve mixed numbered and unnumbered fields"
                    .into(),
            );
        }
        retained.sort_by_key(|field| field.identity);
        if retained
            .windows(2)
            .any(|pair| pair[0].identity == pair[1].identity)
        {
            return Err("policy schema repeats a stable member identity".into());
        }
    }
    // Keys are local to this exact input/receipt. They grant no issuer,
    // backing or field access authority, and output decoding uses this roster.
    for (ordinal, field) in retained.iter_mut().enumerate() {
        field.key = (ordinal as u64) + 1;
    }
    let mut retired = semantic.retired_identities.clone();
    retired.sort_unstable();
    if retired.len() > 32
        || retired.windows(2).any(|pair| pair[0] == pair[1])
        || retained
            .iter()
            .filter_map(|field| field.identity)
            .any(|identity| retired.binary_search(&identity).is_ok())
    {
        return Err("policy schema retired identities differ from its declared field scope".into());
    }
    Ok(TargetLayoutSchemaArgument {
        abi_version: 4,
        fields: retained,
        retired_field_identities: retired,
    })
}
