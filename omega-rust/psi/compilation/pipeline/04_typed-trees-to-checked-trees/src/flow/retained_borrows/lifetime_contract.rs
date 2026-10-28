//! Rejoin a retained input edge to the original applied call declaration.
//! An older stored capture supplies provenance, not a stronger loan contract.
use checked_trees::{FlowControlFacts, FlowOperandRetainedSupplier, ViewReturnInputSource};
use typed_trees::types::TypeReferenceHandle;

pub(super) fn source_type(
    program: &dyn validation::CheckingInput,
    control: &FlowControlFacts,
    supplier: &FlowOperandRetainedSupplier,
) -> Option<TypeReferenceHandle> {
    let contract = supplier.contract?;
    if !control.operands.is_valid(contract.input) || !control.operands.is_valid(supplier.operand) {
        return None;
    }
    let input = control.operands.get(contract.input);
    let machine = program.machine_holding_state(contract.state_symbol)?;
    let call = program.expression_table.call_node(contract.invocation)?;
    let target = validation::call_target_entry_symbol(
        program,
        validation::call_target_symbol(program, call),
    );
    let arguments = program.expression_table.expression_handles(call.arguments);
    if arguments.get(contract.parameter_index).copied() != Some(input.expression)
        || input.state_symbol != contract.state_symbol
        || input.statement_index != contract.statement_index
    {
        return None;
    }
    // The source profile must still name this actual performed argument, not
    // another invocation with a compatible result or matching type spelling.
    let receiver = usize::from(call.receiver.is_valid());
    let actual = control.calls.iter().any(|(_, recorded)| {
        recorded.statement_index == contract.statement_index
            && recorded.authored_expression == contract.invocation
            && recorded.target_symbol == target
            && validation::find_call_site(program, machine.symbol, contract.state_symbol,
                contract.statement_index, recorded.call_ordinal).is_some_and(|site| {
                    matches!(site, validation::CallSite::Expression { expression, .. } if expression == contract.invocation)
                })
            && control.operands.span(recorded.operands)
                .and_then(|operands| operands.get(contract.parameter_index + receiver)) == Some(input)
    });
    if !actual {
        return None;
    }
    let source_path = control.operand_retained_paths.span(contract.source_path)?;
    let owner_path = control.operand_retained_paths.span(contract.owner_path)?;
    let projection = control.operand_retained_paths.span(supplier.projection)?;
    let (expected_operand, mut expected_projection) = if contract.source
        == ViewReturnInputSource::Stored
        && validation::reference_shell(program, input.type_reference).is_some()
    {
        if !control.operand_retentions.is_valid(input.retention) {
            return None;
        }
        let profile = control.operand_retentions.get(input.retention);
        (
            profile.stored_value,
            control
                .operand_retained_paths
                .span(profile.stored_projection)?
                .to_vec(),
        )
    } else {
        (contract.input, Vec::new())
    };
    expected_projection.extend_from_slice(source_path);
    if expected_operand != supplier.operand || !same_path(program, projection, &expected_projection)
    {
        return None;
    }
    let identity = program.normalized_type_identity(contract.source_type);
    if !identity.is_valid() {
        return None;
    }
    let edges =
        crate::borrow::call_reference_lifetime_sources(program, contract.state_symbol, call)?;
    let edge = edges.iter().find(|edge| {
        edge.parameter_index == contract.parameter_index
            && edge.source == contract.source
            && program.normalized_type_identity(edge.source_type) == identity
            && same_path(
                program,
                &crate::borrow::owner_place_segments(&edge.source_path),
                source_path,
            )
            && same_path(
                program,
                &crate::borrow::owner_place_segments(&edge.owner_path),
                owner_path,
            )
    })?;
    let carrier = match edge.source {
        ViewReturnInputSource::Referent => edge.source_type,
        ViewReturnInputSource::Stored => {
            validation::reference_result_custody::input_carrier_type(program, edge.source_type)?
        }
    };
    validation::project_type_reference_from_segments(program, carrier, source_path)
}

fn same_path(
    program: &dyn validation::CheckingInput,
    left: &[facts::PlaceSegment],
    right: &[facts::PlaceSegment],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            validation::storage_path_relations::canonical_place_segments_equal(
                program, *left, *right,
            )
        })
}
