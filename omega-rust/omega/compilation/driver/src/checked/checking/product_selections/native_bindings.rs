//! Admit reached native rows against the original source-issued Product ledger.
//! Footprints and locators are separate reached operations; their complete
//! exact-key join precedes physical normalization and is independent of order.
use compilation_products::ComputedBuildConfig;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{BuildOperation, NativeBindingValue};

pub(super) fn admit(
    source: &mut psi_compiler::SourceContinuation,
    computed: &mut ComputedBuildConfig,
) -> Result<(), Vec<Diagnostic>> {
    let mut resolved = Vec::new();
    for (ordinal, event) in computed.native_binding_requests.iter().enumerate() {
        let typed = source.typed();
        let request = &event.request;
        let refuse =
            |reason: &str| vec![Diagnostic::error(reason).with_source_span(request.source_span)];
        request.validate_shape().map_err(|reason| refuse(&reason))?;
        if !matches!(
            request.operation,
            BuildOperation::NativeBinding
                | BuildOperation::NativeTableFunction
                | BuildOperation::NativeVtableField
                | BuildOperation::NativeSameStackDemand
        ) || !computed.declared_product_selections.contains(request)
            || computed
                .selected_build_machine_symbol
                .and_then(|symbol| typed.machine(symbol))
                .is_none()
        {
            return Err(refuse(
                "native binding lost its original Build activation or source-issued request",
            ));
        }
        let operand = request
            .operands
            .first()
            .ok_or_else(|| refuse("native binding lost its requirement operand"))?;
        let author = typed_trees::product_operand::resolve_build_request_author(
            typed,
            event.selecting_machine,
        )
        .map_err(|reason| refuse(&reason))?;
        let prepared = source.prepare_product_requirement_operand(
            operand,
            author,
            event.static_environment.as_deref(),
        )?;
        resolved.push((ordinal, event, prepared, author));
    }
    let typed = source.typed();
    let mut footprints = Vec::new();
    for (ordinal, event, prepared, author) in &resolved {
        let requirement = &prepared.requirement;
        let NativeBindingValue::SameStackDemand { bytes } = event.locator else {
            if event.request.operation == BuildOperation::NativeSameStackDemand {
                return Err(vec![
                    Diagnostic::error("native footprint request lost its evaluated bytes")
                        .with_source_span(event.request.source_span),
                ]);
            }
            continue;
        };
        let refuse =
            |reason| vec![Diagnostic::error(reason).with_source_span(event.request.source_span)];
        if event.request.operation != BuildOperation::NativeSameStackDemand
            || event.widths != [0; 3]
            || bytes == 0
        {
            return Err(refuse(
                "native same-stack demand must be one reached positive byte declaration",
            ));
        }
        let key = (
            *requirement,
            prepared.closed_requirement,
            prepared
                .trait_scope
                .and_then(|scope| scope.scope(typed))
                .and_then(|scope| scope.portable_application_identity(typed)),
            event.target,
            typed.symbols.symbol_package_identity(*author),
        );
        if footprints.iter().any(|(prior, _, _)| *prior == key) {
            return Err(refuse(
                "native same-stack demand is declared twice for one requirement, target, and supplier",
            ));
        }
        footprints.push((
            key,
            provider_planning::NativeSameStackDemand {
                selecting_machine: Some(*author),
                source_span: typed
                    .symbols
                    .rejoin_product_occurrence(event.request.source_span),
                event_ordinal: *ordinal,
                bytes,
            },
            false,
        ));
    }
    let mut admitted = Vec::new();
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    for (ordinal, event, prepared, author) in resolved {
        let requirement = prepared.requirement;
        if event.request.operation == BuildOperation::NativeSameStackDemand {
            continue;
        }
        let request = &event.request;
        let refuse =
            |reason: &str| vec![Diagnostic::error(reason).with_source_span(request.source_span)];
        let table_field = match (&event.locator, request.operation) {
            (NativeBindingValue::TableFunction, BuildOperation::NativeTableFunction)
            | (NativeBindingValue::VtableField, BuildOperation::NativeVtableField) => {
                let field = request
                    .operands
                    .get(1)
                    .ok_or_else(|| refuse("table binding lost its designated field"))?;
                Some(
                    typed_trees::product_operand::resolve_native_table_field(
                        typed,
                        &field.path,
                        field.source_span,
                    )
                    .ok_or_else(|| {
                        refuse("table binding must designate one exact visible data field")
                    })?,
                )
            }
            (
                NativeBindingValue::PeByName { .. }
                | NativeBindingValue::PeByOrdinal { .. }
                | NativeBindingValue::ElfVersioned { .. }
                | NativeBindingValue::MachODylibSymbol { .. }
                | NativeBindingValue::Syscall { .. },
                BuildOperation::NativeBinding,
            ) => None,
            _ => {
                return Err(refuse(
                    "native binding operation and evaluated locator disagree",
                ));
            }
        };
        let key = (
            requirement,
            prepared.closed_requirement,
            prepared
                .trait_scope
                .and_then(|scope| scope.scope(typed))
                .and_then(|scope| scope.portable_application_identity(typed)),
            event.target,
            typed.symbols.symbol_package_identity(author),
        );
        if admitted
            .iter()
            .any(|prior: &provider_planning::AdmittedNativeBinding| {
                prior.requirement == requirement
                    && prior.closed_requirement == prepared.closed_requirement
                    && prior
                        .trait_scope
                        .and_then(|scope| scope.scope(typed))
                        .and_then(|scope| scope.portable_application_identity(typed))
                        == key.2
                    && prior.target.target_name() == event.target.canonical_name()
                    && prior
                        .selecting_machine
                        .and_then(|author| typed.symbols.symbol_package_identity(author))
                        == key.4
            })
        {
            return Err(refuse(
                "Build binds the same native requirement more than once for one supplier and target",
            ));
        }
        let demand = footprints
            .iter_mut()
            .find(|(prior, _, _)| *prior == key)
            .map(|(_, demand, used)| {
                *used = true;
                *demand
            });
        let observation = computed
            .observation_summary
            .as_ref()
            .ok_or_else(|| refuse("native binding lost its sponsoring Build observation"))?;
        let usage = computed
            .evaluation_usage
            .ok_or_else(|| refuse("native binding lost its sponsoring Build accounting"))?;
        let mut row = provider_planning::admit_native_binding(
            typed,
            event,
            requirement,
            author,
            ordinal,
            observation,
            usage,
            table_field,
            demand,
            &mut identities,
        )
        .map_err(|reason| refuse(&reason))?;
        row.trait_scope = prepared.trait_scope;
        row.closed_requirement = prepared.closed_requirement;
        admitted.push(row);
    }
    if let Some((_, demand, _)) = footprints.iter().find(|(_, _, used)| !*used) {
        return Err(vec![Diagnostic::error("native same-stack demand has no exact native binding under the same activation, supplier, requirement, and target").with_source_span(demand.source_span)]);
    }
    let identities = identities.build();
    for row in &mut admitted {
        row.identities = identities.clone();
    }
    computed.config.admitted_native_bindings = admitted;
    Ok(())
}
