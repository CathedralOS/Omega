//! Public retained-result contracts supply lifetime debt, never pointer geometry.
//! Use the same applied return/input relation as loan attribution. Ordinary
//! checked results still come from their body; constructors cannot acquire a
//! protocol loan from a nominal lifetime or a same-shaped input.

use super::super::stored_calls::{SlotSource, owner_segments};
use super::super::stored_origins::{self, StoredOriginKind};
use super::*;
use crate::borrow_contracts::{ViewReturnSource, resolve_applied_signature_view_return_source};

pub(super) fn call_result(
    program: &dyn crate::CheckingInput,
    caller: &Machine,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    include_shared: bool,
    inputs: &dyn ArgumentOrigins,
) -> Option<AggregateOrigins> {
    let call = expression_table(program).call_node(expression)?;
    let signature = program.requirement_call_signature(caller.symbol, call)?;
    let parameters = program.state_signature_parameters(signature);
    let inventory = stored_origins::declared_origins_for_query(
        program,
        SymbolHandle::invalid(),
        "result",
        signature.return_type,
        true,
    )?;
    if !inventory
        .references
        .iter()
        .any(|leaf| leaf.kind == StoredOriginKind::LifetimeDependency)
    {
        return None;
    }
    let fields = match resolve_applied_signature_view_return_source(
        program,
        signature,
        &[],
        &[],
        caller.symbol,
    ) {
        ViewReturnSource::Fields { fields } => fields,
        ViewReturnSource::NotApplicable => Vec::new(),
        _ => return None,
    };
    let mut relative = AggregateOrigins::default();
    for leaf in inventory.references {
        let mut dependencies = Vec::new();
        for field in fields.iter().filter(|field| field.lifetime_only) {
            if owner_segments(&field.owner_path)? != leaf.local_segments {
                continue;
            }
            let parameter = parameters.get(field.parameter_index)?;
            let source = super::super::FrameSourcePlace {
                segments: owner_segments(&field.source_path)?,
                ..super::super::FrameSourcePlace::parameter_root(parameter)
            };
            let access = match field.kind {
                checked_trees::BorrowAccessKind::Read => {
                    language_semantics::ReferenceAccess::Shared
                }
                checked_trees::BorrowAccessKind::Mutable => {
                    language_semantics::ReferenceAccess::Mutable
                }
                checked_trees::BorrowAccessKind::WriteOnly => {
                    language_semantics::ReferenceAccess::WriteOnly
                }
            };
            let dependency =
                SlotSource::from_place(program, source, FramePathPrecision::Exact, Some(access));
            if !dependencies.contains(&dependency) {
                dependencies.push(dependency);
            }
        }
        relative.references.push(ReferenceLeaf {
            kind: leaf.kind,
            local_suffix: leaf.local_path.strip_prefix("result")?.to_owned(),
            local_segments: leaf.local_segments,
            may_select: leaf.may_select,
            origin: None,
            lifetime_dependencies: Some(dependencies),
        });
    }
    instantiate_result_origins(
        program,
        caller,
        parameters,
        program.expression_table.expression_handles(call.arguments),
        &mut relative,
        symbols,
        inference,
        include_shared,
        inputs,
    )
}
