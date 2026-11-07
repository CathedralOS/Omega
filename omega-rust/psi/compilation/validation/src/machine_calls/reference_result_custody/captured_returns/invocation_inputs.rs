//! A completed call remains an origin carrier when its returned pointer has
//! no absolute address or exact descriptor supplier. Compose its body-derived
//! relation with the actual captured inputs; exclusion regions and lifetime
//! declarations cannot substitute for that relation.

use super::input_views::CapturedArgumentOrigin;
use super::{CapturedReturnPresentation, Captures, SymbolHandle};
use arena::Handle;
use checked_trees::{
    CheckedReferenceOriginProjection, CheckedReferenceOriginRoot, CheckedUnitStructuralPathSegment,
    FlowOperandCompletion, FlowOperandFact,
};
use semantic_vocabulary::ReferenceOriginExtent;

impl Captures<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn invocation_argument_sources(
        &self,
        state: &typed_trees::state::State,
        reference: typed_trees::types::TypeReferenceHandle,
        descriptor: &FlowOperandFact,
        relative: &crate::LocalWriteSource,
        presentation: &CapturedReturnPresentation,
        active: &mut Vec<SymbolHandle>,
        visiting: &mut Vec<Handle<FlowOperandFact>>,
    ) -> Option<Vec<CapturedArgumentOrigin>> {
        let FlowOperandCompletion::InvocationResult {
            invocation,
            result_type,
            path,
        } = descriptor.completion
        else {
            return None;
        };
        let result = (|| {
            let control = &self.facts.flow.control;
            let call = self
                .calls
                .invocation(state.symbol, invocation)
                .ok_or("returned descriptor has no exact performed invocation")?;
            if descriptor.state_symbol != state.symbol
                || descriptor.statement_index != call.statement_index
                || descriptor.expression != call.authored_expression
            {
                return Err("returned descriptor differs from its invocation occurrence");
            }
            let declared = crate::call_target_return_type(self.program, call.target_symbol)
                .ok_or("returned invocation has no result contract")?;
            if self.program.normalized_type_identity(declared)
                != self.program.normalized_type_identity(result_type)
            {
                return Err("returned descriptor changes its invocation result type");
            }
            let selected_path = control
                .operand_retained_paths
                .span(path)
                .ok_or("returned descriptor has an unavailable result path")?;
            let selected =
                crate::project_type_reference_from_segments(self.program, declared, selected_path)
                    .ok_or("returned descriptor has no typed result leaf")?;
            if self.program.normalized_type_identity(selected)
                != self
                    .program
                    .normalized_type_identity(descriptor.type_reference)
            {
                return Err("returned descriptor changes its selected result leaf");
            }
            let shell = crate::reference_shell(self.program, selected)
                .ok_or("returned invocation leaf is not a reference")?;
            let expected = crate::reference_shell(self.program, reference)
                .ok_or("returned invocation input is not a reference")?;
            if !crate::storage_origin_types_match(self.program, shell.referee, expected.referee)
                || !super::super::reference_access_can_supply(
                    shell.access.into(),
                    expected.access.into(),
                )
            {
                return Err("returned invocation changes the input's referent or access");
            }
            let (suffix, endpoint, extent) = super::super::reference_origin_path(
                self.program,
                shell.referee,
                &relative.segments,
            )
            .ok_or("returned invocation has no typed consuming projection")?;
            // A stored descriptor read belongs to the current consumer cut.
            // The ordinary storage query above owns it; projecting an old call
            // relation cannot rewind that read to the invocation's entry.
            if suffix.iter().any(|segment| {
                matches!(
                    segment,
                    CheckedReferenceOriginProjection::Exact(
                        CheckedUnitStructuralPathSegment::Referent
                    )
                )
            }) {
                return Err("returned invocation cannot replace a current descriptor-slot read");
            }
            if self.program.normalized_type_identity(endpoint) != presentation.source_type {
                return Err("returned invocation projection changes its receiving presentation");
            }
            let parameters = crate::call_target_parameters(self.program, call.target_symbol)
                .ok_or("returned invocation has no parameter roster")?;
            let operands = control
                .operands
                .span(call.argument_operands())
                .ok_or("returned invocation has no complete actual captures")?;
            if parameters.len() != operands.len() {
                return Err("returned invocation changes its actual parameter roster");
            }
            let structural_inputs = parameters
                .iter()
                .zip(call.argument_operands().handles())
                .enumerate()
                .filter(|(_, (parameter, _))| {
                    super::super::is_mutable_structural_parameter(self.program, parameter)
                })
                .map(|(position, (parameter, operand))| (position, parameter, operand))
                .collect::<Vec<_>>();
            let sources = self
                .target_sources(call.target_symbol, active)
                .ok_or("returned invocation has no checked origin relation")?;
            let mut output = Vec::new();
            for candidate in sources {
                if super::loan_place_segments(self.program, declared, &candidate.path)
                    .ok_or("returned invocation origin has no result path")?
                    != selected_path
                {
                    continue;
                }
                if !super::super::reference_access_can_supply(
                    candidate.source.access,
                    shell.access.into(),
                ) {
                    return Err("returned invocation origin changes reference access");
                }
                let CheckedReferenceOriginRoot::Parameter { index } = candidate.source.root else {
                    return Err("returned invocation has an independent, non-input origin");
                };
                let &(position, parameter, input) = structural_inputs
                    .get(index as usize)
                    .ok_or("returned invocation origin has no matching input")?;
                let operand = &operands[position];
                if operand.state_symbol != state.symbol
                    || operand.statement_index != call.statement_index
                {
                    return Err("returned invocation input belongs to another evaluation");
                }
                let (source, mut composed) = self
                    .checked_input_origin(
                        parameter,
                        &candidate.source,
                        self.program.normalized_type_identity(shell.referee),
                    )
                    .ok_or("returned invocation input has no checked typed projection")?;
                composed.path.extend(suffix.iter().cloned());
                composed.path.extend(presentation.path.iter().cloned());
                composed.endpoint_type = presentation.endpoint_type;
                composed.result_type = presentation.result_type;
                if extent == ReferenceOriginExtent::WithinRegion
                    || presentation.extent == ReferenceOriginExtent::WithinRegion
                {
                    composed.extent = ReferenceOriginExtent::WithinRegion;
                }
                let inputs = self
                    .argument_return_sources(
                        state,
                        parameter.type_reference,
                        input,
                        call.invocation_storage,
                        &source,
                        &composed,
                        active,
                        visiting,
                    )
                    .ok_or("returned invocation input has no complete captured origins")?;
                for mut input in inputs {
                    if composed.extent == ReferenceOriginExtent::WithinRegion {
                        input.extent = ReferenceOriginExtent::WithinRegion;
                    }
                    if !output.contains(&input) {
                        output.push(input);
                    }
                    if output.len() > super::super::MAX_REFERENCE_ORIGIN_LEAVES {
                        return Err("returned invocation exceeds the origin roster limit");
                    }
                }
            }
            Ok(output)
        })();
        if let Err(reason) = &result
            && std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some()
        {
            eprintln!(
                "CHECKSITE returned-invocation-input state={:?} statement={} expression={:?} invocation={invocation:?} path={path:?} refusal={reason}",
                state.symbol, descriptor.statement_index, descriptor.expression,
            );
        }
        result.ok()
    }
}
