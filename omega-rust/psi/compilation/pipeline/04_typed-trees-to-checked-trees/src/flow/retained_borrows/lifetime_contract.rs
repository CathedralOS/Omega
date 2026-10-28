//! Rejoin a retained input edge to the original applied call declaration.
//! An older stored capture supplies provenance, not a stronger loan contract.
use checked_trees::{FlowControlFacts, FlowOperandRetainedSupplier, ViewReturnInputSource};
use typed_trees::types::TypeReferenceHandle;

pub(super) fn source_type(
    program: &dyn validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    control: &FlowControlFacts,
    supplier: &FlowOperandRetainedSupplier,
) -> Option<TypeReferenceHandle> {
    let contract = supplier.contract?;
    if !control.operands.is_valid(contract.input) || !control.operands.is_valid(supplier.operand) {
        return None;
    }
    let input = control.operands.get(contract.input);
    let machine = program.machine_holding_state(contract.state_symbol)?;
    let borrowed = borrow.call_at(
        machine.symbol,
        contract.state_symbol,
        contract.statement_index,
        contract.call_ordinal,
    )?;
    let borrowed = borrow.calls.get(borrowed);
    if borrowed.authored_expression != contract.invocation {
        return None;
    }
    let site = validation::find_call_site(
        program,
        machine.symbol,
        contract.state_symbol,
        contract.statement_index,
        contract.call_ordinal,
    )?;
    let call = contract
        .invocation
        .is_valid()
        .then(|| program.expression_table.call_node(contract.invocation))
        .flatten();
    let receiver = usize::from(call.is_some_and(|call| call.receiver.is_valid()));
    let arguments = if contract.output_parameter.is_none() {
        program.expression_table.expression_handles(call?.arguments)
    } else {
        validation::call_site_argument_expressions(program, &site)
    };
    if arguments.get(contract.parameter_index).copied() != Some(input.expression)
        || input.state_symbol != contract.state_symbol
        || input.statement_index != contract.statement_index
    {
        return None;
    }
    // The actual performed tuple, not an equal type or source spelling,
    // authenticates the input retained by this edge.
    let mut actual = control.calls.iter().filter(|(_, recorded)| {
        recorded.statement_index == contract.statement_index
            && recorded.call_ordinal == contract.call_ordinal
            && recorded.authored_expression == contract.invocation
            && recorded.target_symbol == borrowed.target_symbol
    });
    let (_, actual) = actual.next()?;
    if actual.operands.count() != arguments.len() + receiver
        || control
            .operands
            .span(actual.operands)?
            .get(contract.parameter_index + receiver)
            != Some(input)
    {
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
    let identity_type = program.normalized_type_identity(contract.source_type);
    if !identity_type.is_valid() {
        return None;
    }
    let (source, source_type, path) = if let Some(position) = contract.output_parameter {
        let parameters = site.parameters(program, machine.symbol)?;
        let parameter = parameters.get(position)?;
        let identity = facts::InvocationOutputIdentity {
            state_symbol: contract.state_symbol,
            statement_index: contract.statement_index,
            call_ordinal: contract.call_ordinal,
            parameter_symbol: parameter.symbol,
        };
        super::super::call_phases::completed_output_retention(
            borrow,
            machine.symbol,
            contract.state_symbol,
            contract.statement_index,
            borrowed,
            identity,
            parameter.type_reference,
            &crate::borrow::owner_path_from_place_segments(program, owner_path),
        )?;
        let relation =
            validation::borrow_contracts::resolve_provider_outputs(program, parameters).ok()?;
        if relation.readiness != validation::borrow_contracts::BorrowRelationReadiness::Complete {
            return None;
        }
        let output = relation.outputs.iter().find(|output| {
            output.destination
                == validation::borrow_contracts::ProviderOutputDestination::Parameter(position)
                && same_path(
                    program,
                    &crate::borrow::owner_place_segments(&output.owner_path),
                    owner_path,
                )
        })?;
        let edge = output.dependencies.iter().find(|edge| {
            edge.parameter_index == contract.parameter_index
                && edge.source == contract.source
                && program.normalized_type_identity(edge.source_type) == identity_type
                && same_path(
                    program,
                    &crate::borrow::owner_place_segments(&edge.source_path),
                    source_path,
                )
        })?;
        (edge.source, edge.source_type, source_path)
    } else {
        let edges =
            crate::borrow::call_reference_lifetime_sources(program, contract.state_symbol, call?)?;
        let edge = edges.iter().find(|edge| {
            edge.parameter_index == contract.parameter_index
                && edge.source == contract.source
                && program.normalized_type_identity(edge.source_type) == identity_type
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
        (edge.source, edge.source_type, source_path)
    };
    let carrier = match source {
        ViewReturnInputSource::Referent => source_type,
        ViewReturnInputSource::Stored => {
            validation::reference_result_custody::input_carrier_type(program, source_type)?
        }
    };
    validation::project_type_reference_from_segments(program, carrier, path)
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
