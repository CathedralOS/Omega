//! Evaluated exclusions are ordinary authenticated Build boundary effects.
//! Capture only actual emitted calls to the exact toolchain protocol; this is
//! no static selection scan and grants no authority to an unexecuted body.

use super::{TerminalBuildBindingCall, TerminalBuildBindingOperation, TerminalBuildExclusionCases};
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    BuildBehaviorExclusionKind, BuildBehaviorExclusionRequest, BuildProductRequest,
};

pub(super) fn capture(
    checked: &checked_trees::CheckedTrees,
    lowered: &lowered_psi::LoweredPsi,
) -> Result<Vec<TerminalBuildBindingCall>, Diagnostic> {
    crate::build_protocol::capture(checked, lowered, "BuildBehaviorOperations", |name| {
        Some(match name {
            "exclude_crash" => BuildBehaviorExclusionKind::Crash,
            "exclude_physical_authority" => BuildBehaviorExclusionKind::PhysicalAuthority,
            _ => return None,
        })
    })?
    .into_iter()
    .map(|(operation, boundary, kind)| {
        let mut occurrences = lowered.source_call_occurrences.iter().filter(|occurrence| {
            occurrence.terminal_site == semantic_vocabulary::InvocationSite::Operation(operation)
        });
        let occurrence = occurrences.next().ok_or_else(|| {
            Diagnostic::error("Build exclusion lost its exact emitted occurrence")
        })?;
        if occurrences.next().is_some() {
            return Err(Diagnostic::error(
                "Build exclusion has ambiguous source custody",
            ));
        }
        let source_span = match occurrence.source_site {
            Some(checked_trees::NominalMachineUseSite::Statement(statement)) => {
                match checked.statement_table.statement(statement) {
                    typed_trees::statement::StatementNode::Call(call) => call.source_span,
                    _ => return Err(Diagnostic::error("Build exclusion lost its authored call")),
                }
            }
            Some(checked_trees::NominalMachineUseSite::Expression(expression)) => {
                checked.expression_table.source_span(expression)
            }
            None => {
                return Err(Diagnostic::error(
                    "Build exclusion has no authored occurrence",
                ));
            }
        };
        // The internal boundary only observes identity. Mutable Build
        // authority belongs to its exact public Prelude wrapper, before this
        // shared borrow is formed. Direct user calls cannot acquire a host
        // binding even if they can name the toolchain requirement.
        let machine = checked
            .machine_holding_state(occurrence.source_state)
            .ok_or_else(|| Diagnostic::error("Build exclusion lost its selecting machine"))?;
        let state = checked
            .state_by_symbol(occurrence.source_state)
            .ok_or_else(|| Diagnostic::error("Build exclusion lost its selecting state"))?;
        let expected_method = match kind {
            BuildBehaviorExclusionKind::Crash => "exclude_crash",
            BuildBehaviorExclusionKind::PhysicalAuthority => "exclude_physical_authority",
        };
        if !checked.symbols.is_build_prelude_symbol(machine.symbol)
            || !checked.symbols.is_build_prelude_symbol(state.symbol)
            || state.name.as_str() != expected_method
            || !checked.data_definitions().iter().any(|declaration| {
                declaration.symbol == machine.attached_data_symbol
                    && language_semantics::build_requests::is_build_protocol(
                        &checked.symbols,
                        declaration.symbol,
                        "Build",
                    )
            })
        {
            return Err(Diagnostic::error(
                "Build behavior protocol requires its source-owned mutable Build selection helper",
            )
            .with_source_span(source_span));
        }
        let author = checked
            .symbols
            .symbol_source_span(machine.symbol)
            .ok_or_else(|| {
                Diagnostic::error("Build exclusion lost its authored selecting machine")
            })?;
        Ok(TerminalBuildBindingCall {
            operation,
            boundary,
            action: TerminalBuildBindingOperation::ExcludeBehavior,
            request: BuildProductRequest::BehaviorExclusion(BuildBehaviorExclusionRequest {
                source_span,
                kind,
            }),
            author,
            composition: None,
            exclusion: Some(cases(checked, lowered, boundary, kind)?),
        })
    })
    .collect()
}

fn cases(
    checked: &checked_trees::CheckedTrees,
    lowered: &lowered_psi::LoweredPsi,
    boundary: semantic_vocabulary::BoundaryMachineId,
    kind: BuildBehaviorExclusionKind,
) -> Result<TerminalBuildExclusionCases, Diagnostic> {
    let refuse = || Diagnostic::error("Build exclusion lost its exact checked Prelude cases");
    let mut declarations = checked.data_definitions().iter().filter(|declaration| {
        language_semantics::build_requests::is_build_protocol(
            &checked.symbols,
            declaration.symbol,
            kind.type_name(),
        )
    });
    let declaration = declarations.next().ok_or_else(refuse)?;
    if declarations.next().is_some() {
        return Err(refuse());
    }
    let module = &lowered.semantic_module;
    let parameters = &module
        .boundary_machines
        .iter()
        .find(|declaration| declaration.id == boundary)
        .ok_or_else(refuse)?
        .structural_parameters;
    let [receiver, parameter] = parameters.as_slice() else {
        return Err(refuse());
    };
    if receiver.access != terminal_psi::StructuralAccess::SharedBorrow
        || parameter.access != terminal_psi::StructuralAccess::Owned
    {
        return Err(refuse());
    }
    let definition = module
        .structural_types
        .iter()
        .find(|definition| definition.id == parameter.structural_type)
        .ok_or_else(refuse)?;
    let terminal_psi::StructuralTypeShape::Sum { cases } = &definition.shape else {
        return Err(refuse());
    };
    let variants = checked
        .data_members(declaration)
        .iter()
        .filter_map(|member| match member {
            typed_trees::data::DataMember::Variant(variant) => Some(variant),
            _ => None,
        })
        .collect::<Vec<_>>();
    if cases.is_empty()
        || cases.len() != variants.len()
        || cases.iter().any(|case| !case.fields.is_empty())
    {
        return Err(refuse());
    }
    let mut selected = Vec::new();
    for variant in variants {
        if !checked.data_payload_fields(variant).is_empty() {
            return Err(refuse());
        }
        let identity = variant.path_identity();
        let mut matching = cases
            .iter()
            .filter(|case| module.identities.text(case.identity) == Some(identity.as_str()));
        let case = matching.next().ok_or_else(refuse)?;
        if matching.next().is_some() || selected.iter().any(|(id, _)| *id == case.id) {
            return Err(refuse());
        }
        selected.push((
            case.id,
            checked
                .symbols
                .symbol_source_span(variant.symbol)
                .ok_or_else(refuse)?,
        ));
    }
    Ok(TerminalBuildExclusionCases {
        structural_type: parameter.structural_type,
        cases: selected,
    })
}
