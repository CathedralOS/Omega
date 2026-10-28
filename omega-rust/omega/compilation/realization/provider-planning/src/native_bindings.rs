//! Build-owned native candidate custody. A reached request binds its exact
//! requirement to an atomic target locator; no forwarding machine is invented.
//! Usage below sponsors the whole Build invocation, not a second locator run.

use checked_trees::typed_trees::TypedTrees;
use effects::provider_plan::{
    EvaluatedBindingEvaluationDigest, EvaluatedBindingMaterializationDigest,
    EvaluatedBindingProducerClosureDigest, EvaluatedBindingReceipt, EvaluatedBindingUsage,
    EvaluatedForeignImport, EvaluatedForeignSyscall, ProviderBinding,
};
use language_semantics::declaration_selection::{ExecutedNativeBinding, NativeBindingValue};
use symbols::SymbolHandle;

mod retention;
pub use retention::{
    capture_package_native_bindings, rejoin_package_native_bindings,
    unify_native_binding_identities,
};
use target::{ForeignLocatorCandidate, TargetProfile};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeFieldIdentity {
    pub table: SymbolHandle,
    pub field: SymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmittedNativeLocator {
    Unavailable {
        reason: String,
        receipt: EvaluatedBindingReceipt,
    },
    Import(EvaluatedForeignImport),
    Syscall(EvaluatedForeignSyscall),
    Table {
        identity: NativeFieldIdentity,
        table_name: String,
        field_name: String,
        drops_receiver: bool,
        receipt: EvaluatedBindingReceipt,
    },
}

/// An independently reached opaque-provider footprint declaration. The shared
/// Build activation joins it to one exact requirement/target/supplier row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeSameStackDemand {
    pub selecting_machine: Option<SymbolHandle>,
    pub source_span: source::SourceSpan,
    pub event_ordinal: usize,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedNativeBinding {
    pub closed_requirement: Option<SymbolHandle>,
    pub trait_scope: Option<checked_trees::typed_trees::trait_definition::RetainedTraitScope>,
    pub same_stack_demand: Option<NativeSameStackDemand>,
    pub raw_locator: NativeBindingValue,
    pub widths: [u64; 3],
    pub requirement: SymbolHandle,
    pub selecting_machine: Option<SymbolHandle>,
    pub source_span: source::SourceSpan,
    pub event_ordinal: usize,
    pub target: TargetProfile,
    pub locator: AdmittedNativeLocator,
    pub identities: terminal_psi::TerminalIdentityTable,
    /// Present for imported checked intent. Current-world author handles are
    /// absent on these rows; exact original source and receipt stay retained.
    pub retained: Option<
        std::sync::Arc<package_compilation::retained_native_bindings::RetainedNativeBinding>,
    >,
}

impl AdmittedNativeBinding {
    pub fn binding(&self) -> Option<ProviderBinding> {
        Some(match &self.locator {
            AdmittedNativeLocator::Unavailable { .. } => return None,
            AdmittedNativeLocator::Import(value) => ProviderBinding::Import {
                evaluated: value.clone(),
            },
            AdmittedNativeLocator::Syscall(value) => ProviderBinding::Syscall {
                number: value.number(),
            },
            AdmittedNativeLocator::Table {
                table_name,
                field_name,
                drops_receiver,
                ..
            } => {
                if *drops_receiver {
                    ProviderBinding::TableFunction {
                        table: table_name.clone(),
                        field: field_name.clone(),
                    }
                } else {
                    ProviderBinding::VtableField {
                        table: table_name.clone(),
                        field: field_name.clone(),
                    }
                }
            }
        })
    }
    pub const fn table_field(&self) -> Option<NativeFieldIdentity> {
        match self.locator {
            AdmittedNativeLocator::Table { identity, .. } => Some(identity),
            _ => None,
        }
    }
    pub fn receipt(&self) -> &EvaluatedBindingReceipt {
        match &self.locator {
            AdmittedNativeLocator::Unavailable { receipt, .. } => receipt,
            AdmittedNativeLocator::Import(value) => value.receipt(),
            AdmittedNativeLocator::Syscall(value) => value.receipt(),
            AdmittedNativeLocator::Table { receipt, .. } => receipt,
        }
    }
}

/// Admission follows Product joining and precedes generated-source extension.
/// The coordinator supplies only source-issued, activation-authenticated events.
pub fn admit_native_binding(
    typed: &TypedTrees,
    event: &ExecutedNativeBinding,
    requirement: SymbolHandle,
    selecting_machine: SymbolHandle,
    event_ordinal: usize,
    observation: &package_compilation::BuildObservationSummary,
    usage: package_compilation::BuildEvaluationUsage,
    table_field: Option<(SymbolHandle, SymbolHandle)>,
    same_stack_demand: Option<NativeSameStackDemand>,
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
) -> Result<AdmittedNativeBinding, String> {
    usage
        .validate()
        .map_err(|error| format!("native binding Build accounting is inconsistent: {error:?}"))?;
    let target = TargetProfile::from_canonical_target_name(event.target.canonical_name())
        .map_err(|error| error.to_string())?;
    let author = typed
        .machine(selecting_machine)
        .ok_or("native binding lost its selecting Build callable")?;
    let producer = typed
        .machine_callable_identity_text(author)
        .ok_or("native binding author has no callable identity")?;
    let requirement_identity = if let Some((owner, signature)) =
        typed.trait_machine_signature(requirement)
    {
        typed.trait_requirement_callable_identity_text(owner, signature)
    } else {
        let machine = typed
            .machine(requirement)
            .filter(|machine| {
                machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
                    && !machine.body_is_present
            })
            .ok_or("native binding operand is not a boundary requirement")?;
        typed
            .machine_callable_identity_text(machine)
            .ok_or("native requirement has no callable identity")?
    };
    let source_span = typed
        .symbols
        .rejoin_product_occurrence(event.request.source_span);
    let source = typed
        .symbols
        .source_file(source_span)
        .ok_or("native binding lost its source occurrence")?;
    let relative = source
        .path
        .strip_prefix(&source.package_root)
        .map_err(|_| "native binding source is outside its immutable package")?;
    let mut hash = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.build-native-binding.invocation.sha256.v3\0",
    );
    hash.raw(observation.identity().as_bytes());
    hash.bytes(producer.as_bytes());
    hash.bytes(requirement_identity.as_bytes());
    match &event.static_environment {
        None => hash.byte(0),
        Some(environment) => {
            if environment.identity.is_empty() {
                return Err(
                    "native helper environment lost its exact specialization identity".into(),
                );
            }
            hash.byte(1);
            hash.bytes(&environment.identity);
        }
    }
    hash.bytes(relative.to_string_lossy().as_bytes());
    hash.bytes(source.source.as_bytes());
    hash.u64(source_span.span.start as u64);
    hash.u64(source_span.span.end as u64);
    hash.u64(event_ordinal as u64);
    hash.bytes(target.target_name().as_bytes());
    for width in event.widths {
        hash.u64(width);
    }
    if let Some(package) = typed.symbols.symbol_package_identity(selecting_machine) {
        hash.byte(1);
        hash.raw(&package.digest());
    } else {
        hash.byte(0);
    }
    // Commit the complete engine-tagged accounting record, including sponsor
    // ceilings, rather than relabeling Terminal measurements as tree steps.
    let mut measured = Vec::new();
    package_compilation::build_execution::build_record_codec::encode_build_evaluation_usage(
        &usage,
        &mut measured,
    );
    hash.bytes(&measured);
    let table = if matches!(
        event.locator,
        NativeBindingValue::TableFunction | NativeBindingValue::VtableField
    ) {
        if event.widths != [0; 3] {
            return Err("typed table binding has unexpected locator widths".into());
        }
        let (table, field) =
            table_field.ok_or("typed table binding lost its exact selected field")?;
        let identity = prepare_native_table_field(typed, table, field)?;
        let table_name = typed.normalized_hermetic_symbol_identity(table)?;
        let field_name = typed
            .data_definition_by_symbol(table)
            .and_then(|definition| {
                typed
                    .data_members(definition)
                    .iter()
                    .filter_map(|member| member.as_field())
                    .find(|candidate| candidate.symbol == field)
            })
            .ok_or("typed table binding lost its selected runtime field")?
            .name
            .as_str()
            .to_owned();
        hash.bytes(table_name.as_bytes());
        hash.bytes(typed.normalized_hermetic_symbol_identity(field)?.as_bytes());
        Some((
            identity,
            table_name,
            field_name,
            matches!(event.locator, NativeBindingValue::TableFunction),
        ))
    } else {
        if table_field.is_some() {
            return Err("physical locator cannot carry a table declaration".into());
        }
        None
    };
    if let Some(demand) = same_stack_demand {
        hash.byte(1);
        hash.u64(demand.bytes);
        hash.u64(demand.event_ordinal as u64);
        hash.u64(demand.source_span.span.start as u64);
        hash.u64(demand.source_span.span.end as u64);
        let author = typed
            .machine(
                demand
                    .selecting_machine
                    .ok_or("native footprint has no current-world author")?,
            )
            .ok_or("native footprint lost its exact author")?;
        hash.bytes(
            typed
                .machine_callable_identity_text(author)
                .ok_or("native footprint author lost callable identity")?
                .as_bytes(),
        );
        let source = typed
            .symbols
            .source_file(demand.source_span)
            .ok_or("native footprint lost its source")?;
        hash.bytes(
            source
                .path
                .strip_prefix(&source.package_root)
                .map_err(|_| "native footprint source escaped its package")?
                .to_string_lossy()
                .as_bytes(),
        );
        hash.bytes(source.source.as_bytes());
    } else {
        hash.byte(0);
    }
    hash_native_value(&mut hash, &event.locator);
    let invocation = hash.finish();
    let candidate = match &event.locator {
        NativeBindingValue::PeByName { library, export } => {
            Some(ForeignLocatorCandidate::PeByName {
                library: library.clone(),
                export: export.clone(),
            })
        }
        NativeBindingValue::PeByOrdinal { library, ordinal } => {
            Some(ForeignLocatorCandidate::PeByOrdinal {
                library: library.clone(),
                ordinal: *ordinal,
            })
        }
        NativeBindingValue::ElfVersioned {
            object,
            symbol,
            version,
        } => Some(ForeignLocatorCandidate::ElfVersioned {
            object: object.clone(),
            symbol: symbol.clone(),
            version: version.clone(),
        }),
        NativeBindingValue::MachODylibSymbol {
            install_name,
            symbol,
        } => Some(ForeignLocatorCandidate::MachODylibSymbol {
            install_name: install_name.clone(),
            symbol: symbol.clone(),
        }),
        NativeBindingValue::SameStackDemand { .. } => {
            return Err("a footprint declaration cannot supply a native locator".into());
        }
        NativeBindingValue::Syscall { .. }
        | NativeBindingValue::TableFunction
        | NativeBindingValue::VtableField => None,
    };
    let mut unavailable = None;
    let normalized = candidate.and_then(|candidate| {
        match target::normalize_foreign_locator(candidate, target) {
            Ok(value) => Some(value),
            Err(error) => {
                unavailable = Some(format!(
                    "Build native locator is invalid for its selected target: {error}"
                ));
                None
            }
        }
    });
    let syscall = match event.locator {
        NativeBindingValue::Syscall { number } => {
            if !matches!(target, TargetProfile::LinuxX64 | TargetProfile::LinuxArm64) {
                unavailable = Some("Build native syscall requires a Linux target".into());
                None
            } else {
                match u32::try_from(number) {
                    Ok(number) => Some(number),
                    Err(_) => {
                        unavailable = Some("Build native syscall number does not fit u32".into());
                        None
                    }
                }
            }
        }
        _ => None,
    };
    let locator_identity = match (&normalized, syscall) {
        (Some(locator), _) => locator.identity_digest(),
        (_, Some(number)) => {
            effects::provider_plan::evaluated_syscall_identity_digest(target, number)
        }
        _ => target::ForeignLocatorIdentityDigest::from_digest(invocation),
    };
    let mut materialization = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.build-native-binding.materialization.sha256.v3\0",
    );
    materialization.raw(&invocation);
    materialization.raw(&locator_identity.as_bytes());
    let materialization = materialization.finish();
    let retained_usage = EvaluatedBindingUsage::from_evaluator(
        usage.usage_schema_version,
        usage.step_schedule_marker,
        usage.fuel_units,
        usage.invocation_fuel_ceiling,
        usage.build_log_bytes,
        usage.filesystem_operation_attempts,
        usage.peak_live_storage_cells,
        usage.peak_live_payload_bytes,
        usage.result_storage_cells,
        usage.result_payload_bytes,
    )?;
    let receipt = EvaluatedBindingReceipt::from_evaluation(
        typed.symbols.symbol_package_identity(selecting_machine),
        &producer,
        identities,
        EvaluatedBindingProducerClosureDigest::from_bytes(invocation)?,
        2, // Schema 2: the authenticated, engine-tagged sponsoring Build context.
        retained_usage,
        EvaluatedBindingEvaluationDigest::from_bytes(invocation)?,
        3,
        EvaluatedBindingMaterializationDigest::from_bytes(materialization)?,
        locator_identity,
    )?;
    let locator = if let Some(reason) = unavailable {
        AdmittedNativeLocator::Unavailable { reason, receipt }
    } else {
        match normalized {
            Some(locator) => AdmittedNativeLocator::Import(
                EvaluatedForeignImport::from_retained_evidence(locator, receipt)?,
            ),
            None if table.is_some() => {
                let (identity, table_name, field_name, drops_receiver) =
                    table.ok_or("typed table disappeared")?;
                AdmittedNativeLocator::Table {
                    identity,
                    table_name,
                    field_name,
                    drops_receiver,
                    receipt,
                }
            }
            None => {
                AdmittedNativeLocator::Syscall(EvaluatedForeignSyscall::from_retained_evidence(
                    target,
                    u64::from(syscall.ok_or("native syscall disappeared")?),
                    receipt,
                )?)
            }
        }
    };
    Ok(AdmittedNativeBinding {
        trait_scope: None,
        closed_requirement: None,
        same_stack_demand,
        raw_locator: event.locator.clone(),
        widths: event.widths,
        requirement,
        selecting_machine: Some(selecting_machine),
        source_span,
        event_ordinal,
        target,
        locator,
        identities: identities.as_table().clone(),
        retained: None,
    })
}

/// The requirement owns the first table borrow and its complete type; Build
/// only designates the exact field within that already-admitted nominal owner.
fn prepare_native_table_field(
    typed: &TypedTrees,
    table: SymbolHandle,
    field: SymbolHandle,
) -> Result<NativeFieldIdentity, String> {
    use checked_trees::typed_trees::types::PrimitiveType;
    let definition = typed
        .data_definition_by_symbol(table)
        .ok_or("table field lost its exact data declaration")?;
    let mut fields = typed
        .data_members(definition)
        .iter()
        .filter_map(|member| member.as_field())
        .filter(|candidate| candidate.symbol == field);
    let field = fields
        .next()
        .filter(|_| fields.next().is_none())
        .ok_or("table binding does not designate one exact field of the borrowed table")?;
    if field.relevance.is_erased()
        || typed.primitive_type_reference(field.type_reference) != Some(PrimitiveType::Addr)
    {
        return Err("table dispatch requires a runtime addr field".into());
    }
    Ok(NativeFieldIdentity {
        table,
        field: field.symbol,
    })
}

pub(crate) fn table_parameter_type(
    typed: &TypedTrees,
    requirement: SymbolHandle,
    table: SymbolHandle,
) -> Result<checked_trees::typed_trees::types::TypeReferenceHandle, String> {
    use checked_trees::typed_trees::types::TypeReferenceNode;
    let parameter = if let Some((_, signature)) = typed.trait_machine_signature(requirement) {
        typed
            .state_signature_parameters(signature)
            .first()
            .map(|parameter| parameter.type_reference)
    } else {
        typed
            .machine(requirement)
            .and_then(|machine| typed.machine_states(machine).first())
            .and_then(|state| typed.state_parameters(state).first())
            .map(|parameter| parameter.type_reference)
    }
    .ok_or("table requirement must have an explicit first table borrow")?;
    let strip = |mut reference| {
        while let TypeReferenceNode::Constrained { base_type, .. } =
            typed.type_reference_table.type_reference(reference)
        {
            reference = *base_type;
        }
        reference
    };
    let TypeReferenceNode::Reference {
        referee, access, ..
    } = typed.type_reference_table.type_reference(strip(parameter))
    else {
        return Err("table requirement first argument must be a table borrow".into());
    };
    if *access == language_semantics::ReferenceAccess::WriteOnly {
        return Err("table dispatch requires a readable table borrow".into());
    }
    let table_type = strip(*referee);
    let definition = typed
        .closed_data_definition(table_type)
        .ok_or("table requirement has no exact closed nominal table application")?;
    if definition.symbol != table {
        return Err("table requirement does not borrow the exact selected data declaration".into());
    }
    Ok(table_type)
}

fn hash_native_value(
    hash: &mut crate::digest_encoder::DomainDigestEncoder,
    value: &NativeBindingValue,
) {
    match value {
        NativeBindingValue::PeByName { library, export } => {
            hash.byte(0);
            hash.bytes(library);
            hash.bytes(export);
        }
        NativeBindingValue::PeByOrdinal { library, ordinal } => {
            hash.byte(1);
            hash.bytes(library);
            hash.u64(u64::from(*ordinal));
        }
        NativeBindingValue::ElfVersioned {
            object,
            symbol,
            version,
        } => {
            hash.byte(2);
            hash.bytes(object);
            hash.bytes(symbol);
            hash.bytes(version);
        }
        NativeBindingValue::MachODylibSymbol {
            install_name,
            symbol,
        } => {
            hash.byte(3);
            hash.bytes(install_name);
            hash.bytes(symbol);
        }
        NativeBindingValue::Syscall { number } => {
            hash.byte(4);
            hash.u64(*number);
        }
        NativeBindingValue::TableFunction => hash.byte(5),
        NativeBindingValue::VtableField => hash.byte(6),
        NativeBindingValue::SameStackDemand { bytes } => {
            hash.byte(7);
            hash.u64(*bytes);
        }
    }
}
