//! Transfer checked native intent between package worlds without replaying Build.
//! Source anchors and complete portable tuples rejoin independently; the old
//! receipt is retained byte-for-byte in its original callable namespace.
use super::*;
use language_semantics::declaration_selection::{BuildSelectionOrigin, ProductTarget};
use package_compilation::retained_native_bindings::*;
use symbols::RetainedDeclaration;

fn capture_anchor(typed: &TypedTrees, symbol: SymbolHandle) -> Result<RetainedDeclaration, String> {
    let anchor = RetainedDeclaration::capture(&typed.symbols, symbol).map_err(str::to_owned)?;
    RetainedDeclaration::from_retained_parts(anchor.source().clone(), anchor.span(), anchor.kind())
        .map_err(str::to_owned)
}
fn identity_text(
    typed: &TypedTrees,
    identity: checked_trees::typed_trees::type_identity::NormalizedTypeIdentity,
) -> Result<String, String> {
    typed
        .type_identity_text(identity)
        .ok_or_else(|| "native application lost its portable identity".into())
}
fn local_key(
    typed: &TypedTrees,
    symbol: SymbolHandle,
) -> Result<RetainedNativeLocalApplication, String> {
    let application = typed
        .closed_requirement_application(symbol)
        .ok_or("native selected method lost its closed application")?;
    let source = typed
        .expression_table
        .static_machine_application(application.application);
    let arguments = application
        .portable_local_arguments
        .iter()
        .map(|id| identity_text(typed, *id))
        .collect::<Result<Vec<_>, _>>()?;
    let lifetimes = typed
        .expression_table
        .static_machine_application_lifetimes(application.application)
        .iter()
        .map(|binding| {
            let identity =
                checked_trees::typed_trees::type_identity::portable_applied_lifetime_identity(
                    typed,
                    binding.argument.as_str(),
                    source.lexical_owner,
                    &[],
                )
                .ok_or("native selected lifetime lost its portable binder identity")?;
            identity_text(typed, identity)
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(RetainedNativeLocalApplication {
        arguments,
        lifetimes,
    })
}
fn live_application_key(
    typed: &TypedTrees,
    row: &AdmittedNativeBinding,
) -> Result<RetainedNativeApplication, String> {
    let outer = row
        .trait_scope
        .map(|scope| {
            identity_text(
                typed,
                scope
                    .scope(typed)
                    .and_then(|scope| scope.portable_application_identity(typed))
                    .ok_or("native trait scope lost its portable application")?,
            )
        })
        .transpose()?;
    let local = row
        .closed_requirement
        .map(|symbol| {
            let application = typed
                .closed_requirement_application(symbol)
                .ok_or("native selected method lost its formed application")?;
            if application.template != row.requirement {
                return Err("native selected method changes its requirement declaration".into());
            }
            let enclosing = match &outer {
                Some(identity) => identity.clone(),
                None => {
                    let owner = typed
                        .trait_definition_by_symbol(application.declaring_trait)
                        .ok_or("native selected method lost its declaring trait")?;
                    if !typed.trait_type_parameters(owner).is_empty()
                        || !owner.lifetime_parameters.is_empty()
                    {
                        return Err("native selected method lost its enclosing application".into());
                    }
                    let identity = checked_trees::typed_trees::trait_definition::AppliedTraitScope::abstract_scope(owner)
                        .portable_application_identity(typed)
                        .ok_or("native selected method lost its nongeneric trait identity")?;
                    identity_text(typed, identity)?
                }
            };
            if enclosing != identity_text(typed, application.portable_trait_application_identity)? {
                return Err(
                    "native selected method changes its enclosing trait application".into(),
                );
            }
            local_key(typed, symbol)
        })
        .transpose()?;
    if outer.is_none() && local.is_none() {
        Ok(RetainedNativeApplication::Universal)
    } else {
        Ok(RetainedNativeApplication::Applied { outer, local })
    }
}

/// Reconstruct the complete tuple in the current checked program. Imported
/// text selects a receiving occurrence, but never substitutes for that
/// occurrence's independently formed type, const and region application.
pub fn resolve_native_binding_application(
    typed: &TypedTrees,
    row: &AdmittedNativeBinding,
) -> Result<ResolvedNativeApplication, String> {
    let receiving = row
        .resolved_application(typed)
        .ok_or("native application has no independently formed receiving tuple")?;
    let actual = live_application_key(typed, &receiving)?;
    if row
        .retained
        .as_ref()
        .is_some_and(|retained| retained.application != actual)
    {
        return Err("native application changes its complete retained tuple".into());
    }
    Ok(ResolvedNativeApplication {
        key: actual,
        trait_scope: receiving.trait_scope,
        closed_requirement: receiving.closed_requirement,
    })
}

/// Invocation-local source coordinates and their independently rederived
/// portable tuple. This view carries no selection or execution authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedNativeApplication {
    pub key: RetainedNativeApplication,
    pub trait_scope: Option<checked_trees::typed_trees::trait_definition::RetainedTraitScope>,
    pub closed_requirement: Option<SymbolHandle>,
}

/// Called only after immutable Product checking, including unused operands.
/// Inherited rows are already exported by their own sponsoring handoff.
pub fn capture_package_native_bindings(
    typed: &TypedTrees,
    rows: &[AdmittedNativeBinding],
    activation: Option<SymbolHandle>,
    observation: Option<&package_compilation::BuildObservationSummary>,
    usage: Option<package_compilation::BuildEvaluationUsage>,
) -> Result<Vec<RetainedNativeBinding>, String> {
    rows.iter()
        .filter(|row| row.retained.is_none())
        .map(|row| {
            let activation =
                activation.ok_or("native export lost its original Build activation")?;
            let author = row
                .selecting_machine
                .ok_or("native export lost its original lexical author")?;
            let origin = BuildSelectionOrigin::capture(
                &typed.symbols,
                activation,
                author,
                row.source_span,
                Some(row.event_ordinal as u64),
            )
            .map_err(str::to_owned)?;
            let producer_callable = row
                .identities
                .text(row.receipt().producer_callable_identity())
                .ok_or("native receipt lost its original callable namespace")?
                .to_owned();
            // Reindex the namespace, preserving every receipt field and its digest.
            let receipt = reindex_receipt(
                row.receipt(),
                &producer_callable,
                &mut terminal_psi::TerminalIdentityTableBuilder::default(),
            )?;
            let table = row
                .table_field()
                .map(|field| {
                    Ok::<_, String>(RetainedNativeTable {
                        table: capture_anchor(typed, field.table)?,
                        table_identity: typed.normalized_hermetic_symbol_identity(field.table)?,
                        field: capture_anchor(typed, field.field)?,
                        field_identity: typed.normalized_hermetic_symbol_identity(field.field)?,
                    })
                })
                .transpose()?;
            let same_stack_demand = row
                .same_stack_demand
                .map(|demand| {
                    Ok::<_, String>(RetainedNativeSameStackDemand {
                        bytes: demand.bytes,
                        origin: BuildSelectionOrigin::capture(
                            &typed.symbols,
                            activation,
                            demand
                                .selecting_machine
                                .ok_or("native demand lost its lexical author")?,
                            demand.source_span,
                            Some(demand.event_ordinal as u64),
                        )
                        .map_err(str::to_owned)?,
                    })
                })
                .transpose()?;
            Ok(RetainedNativeBinding {
                requirement: capture_anchor(typed, row.requirement)?,
                requirement_identity: typed.normalized_hermetic_symbol_identity(row.requirement)?,
                application: resolve_native_binding_application(typed, row)?.key,
                origin,
                author_path: typed.symbols.display_path(author, "::"),
                target: ProductTarget::from_code(row.target_code())
                    .ok_or("native export has no canonical target")?,
                raw_locator: row.raw_locator.clone(),
                widths: row.widths,
                table,
                same_stack_demand,
                unavailable: match &row.locator {
                    AdmittedNativeLocator::Unavailable { reason, .. } => Some(reason.clone()),
                    _ => None,
                },
                usage: usage.ok_or("native export lost sponsoring Build usage")?,
                observation: observation
                    .ok_or("native export lost sponsoring Build observation")?
                    .identity()
                    .digest(),
                producer_callable,
                producer_recipe: row
                    .receipt_source
                    .as_ref()
                    .ok_or("own native export lost its immutable producer recipe")?
                    .producer_recipe(),
                receipt,
            })
        })
        .collect()
}

fn reindex_receipt(
    receipt: &EvaluatedBindingReceipt,
    producer: &str,
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
) -> Result<EvaluatedBindingReceipt, String> {
    let copied = EvaluatedBindingReceipt::from_evaluation(
        receipt.producer_package(),
        producer,
        identities,
        receipt.producer_closure_digest(),
        receipt.evaluator_semantics_marker(),
        receipt.evaluation_usage(),
        receipt.evaluation_digest(),
        receipt.materializer_schema_version(),
        receipt.materialization_digest(),
        receipt.locator_identity_digest(),
    )?;
    if copied.identity_digest() != receipt.identity_digest() {
        return Err("native receipt namespace transfer changed its commitment".into());
    }
    Ok(copied)
}
fn rejoin_anchor(
    typed: &TypedTrees,
    anchor: &RetainedDeclaration,
    identity: &str,
) -> Result<SymbolHandle, String> {
    anchor
        .rejoin(
            &typed.symbols,
            typed
                .symbols
                .symbols()
                .nodes()
                .iter()
                .map(|(symbol, _)| symbol)
                .filter(|symbol| {
                    typed.symbols.get(*symbol).kind == anchor.kind()
                        && typed
                            .symbols
                            .symbol_source_span(*symbol)
                            .is_some_and(|span| span.span == anchor.span())
                        && typed
                            .normalized_hermetic_symbol_identity(*symbol)
                            .is_ok_and(|actual| actual == identity)
                }),
        )
        .map_err(str::to_owned)
}

impl AdmittedNativeBinding {
    fn target_code(&self) -> u64 {
        ProductTarget::from_canonical_name(self.target.target_name())
            .map_or(u64::MAX, |target| target.code())
    }
    /// Resolve only exact applications already formed by ordinary receiving
    /// source preparation. Missing applied tuples remain unavailable intent.
    pub(crate) fn resolved_application(&self, typed: &TypedTrees) -> Option<Self> {
        let Some(retained) = &self.retained else {
            return Some(self.clone());
        };
        let RetainedNativeApplication::Applied { outer, local } = &retained.application else {
            return Some(self.clone());
        };
        let mut copy = self.clone();
        let found = typed
            .closed_requirement_applications()
            .find(|application| {
                application.template == self.requirement
                    && outer.as_ref().is_none_or(|expected| {
                        typed
                            .type_identity_text(application.portable_trait_application_identity)
                            .as_ref()
                            == Some(expected)
                    })
                    && local.as_ref().is_none_or(|expected| {
                        local_key(typed, application.signature.symbol)
                            .is_ok_and(|actual| actual == *expected)
                    })
            })?;
        copy.trait_scope = Some(found.trait_scope);
        copy.closed_requirement = local.as_ref().map(|_| found.signature.symbol);
        Some(copy)
    }
}

pub fn rejoin_package_native_bindings(
    typed: &TypedTrees,
    rows: Vec<RetainedNativeBinding>,
) -> Result<Vec<AdmittedNativeBinding>, String> {
    rows.into_iter()
        .map(|retained| {
            let requirement =
                rejoin_anchor(typed, &retained.requirement, &retained.requirement_identity)?;
            let target =
                TargetProfile::from_canonical_target_name(retained.target.canonical_name())
                    .map_err(|error| error.to_string())?;
            let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
            let receipt = reindex_receipt(
                &retained.receipt,
                &retained.producer_callable,
                &mut identities,
            )?;
            let locator = if let Some(reason) = &retained.unavailable {
                AdmittedNativeLocator::Unavailable {
                    reason: reason.clone(),
                    receipt,
                }
            } else if let Some(table) = &retained.table {
                let table_symbol = rejoin_anchor(typed, &table.table, &table.table_identity)?;
                let field = rejoin_anchor(typed, &table.field, &table.field_identity)?;
                let identity = super::prepare_native_table_field(typed, table_symbol, field)?;
                let field_name = typed.symbols.name(field).to_owned();
                AdmittedNativeLocator::Table {
                    identity,
                    table_name: table.table_identity.clone(),
                    field_name,
                    drops_receiver: matches!(
                        retained.raw_locator,
                        NativeBindingValue::TableFunction
                    ),
                    receipt,
                }
            } else if let NativeBindingValue::Syscall { number } = retained.raw_locator {
                AdmittedNativeLocator::Syscall(EvaluatedForeignSyscall::from_retained_evidence(
                    target, number, receipt,
                )?)
            } else {
                let candidate = match &retained.raw_locator {
                    NativeBindingValue::PeByName { library, export } => {
                        ForeignLocatorCandidate::PeByName {
                            library: library.clone(),
                            export: export.clone(),
                        }
                    }
                    NativeBindingValue::PeByOrdinal { library, ordinal } => {
                        ForeignLocatorCandidate::PeByOrdinal {
                            library: library.clone(),
                            ordinal: *ordinal,
                        }
                    }
                    NativeBindingValue::ElfVersioned {
                        object,
                        symbol,
                        version,
                    } => ForeignLocatorCandidate::ElfVersioned {
                        object: object.clone(),
                        symbol: symbol.clone(),
                        version: version.clone(),
                    },
                    NativeBindingValue::MachODylibSymbol {
                        install_name,
                        symbol,
                    } => ForeignLocatorCandidate::MachODylibSymbol {
                        install_name: install_name.clone(),
                        symbol: symbol.clone(),
                    },
                    _ => return Err("retained native intent has no physical locator".into()),
                };
                let normalized =
                    target::normalize_foreign_locator(candidate, target).map_err(|error| {
                        format!("retained native locator no longer normalizes: {error}")
                    })?;
                AdmittedNativeLocator::Import(EvaluatedForeignImport::from_retained_evidence(
                    normalized, receipt,
                )?)
            };
            let occurrence = typed
                .symbols
                .symbol_source_span(requirement)
                .ok_or("retained native requirement lost current source custody")?;
            Ok(AdmittedNativeBinding {
                receipt_source: None,
                requirement,
                trait_scope: None,
                closed_requirement: None,
                selecting_machine: None,
                // Diagnostics use the receiving requirement; original Build event
                // coordinates remain in retained.origin, never this SourceSpan.
                source_span: occurrence,
                event_ordinal: usize::try_from(
                    retained
                        .origin
                        .event_ordinal()
                        .ok_or("retained native event has no ordinal")?,
                )
                .map_err(|_| "native ordinal does not fit host")?,
                target,
                same_stack_demand: retained
                    .same_stack_demand
                    .as_ref()
                    .map(|demand| {
                        Ok::<_, String>(NativeSameStackDemand {
                            bytes: demand.bytes,
                            selecting_machine: None,
                            source_span: occurrence,
                            event_ordinal: usize::try_from(
                                demand
                                    .origin
                                    .event_ordinal()
                                    .ok_or("retained native demand has no ordinal")?,
                            )
                            .map_err(|_| "native demand ordinal does not fit host")?,
                        })
                    })
                    .transpose()?,
                raw_locator: retained.raw_locator.clone(),
                widths: retained.widths,
                locator,
                identities: identities.build(),
                retained: Some(std::sync::Arc::new(retained)),
            })
        })
        .collect()
}

/// One receipt namespace accompanies the complete current-plus-inherited ledger.
/// Reindexing changes only artifact-local indices, never original text/digests.
pub fn unify_native_binding_identities(rows: &mut [AdmittedNativeBinding]) -> Result<(), String> {
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    for row in rows.iter_mut() {
        let producer = row
            .identities
            .text(row.receipt().producer_callable_identity())
            .ok_or("native handoff receipt lost its original identity namespace")?
            .to_owned();
        let receipt = reindex_receipt(row.receipt(), &producer, &mut identities)?;
        row.locator = match &row.locator {
            AdmittedNativeLocator::Unavailable { reason, .. } => {
                AdmittedNativeLocator::Unavailable {
                    reason: reason.clone(),
                    receipt,
                }
            }
            AdmittedNativeLocator::Import(value) => AdmittedNativeLocator::Import(
                EvaluatedForeignImport::from_retained_evidence(value.locator().clone(), receipt)?,
            ),
            AdmittedNativeLocator::Syscall(value) => {
                AdmittedNativeLocator::Syscall(EvaluatedForeignSyscall::from_retained_evidence(
                    value.target(),
                    u64::try_from(value.number())
                        .map_err(|_| "native syscall number is negative")?,
                    receipt,
                )?)
            }
            AdmittedNativeLocator::Table {
                identity,
                table_name,
                field_name,
                drops_receiver,
                ..
            } => AdmittedNativeLocator::Table {
                identity: *identity,
                table_name: table_name.clone(),
                field_name: field_name.clone(),
                drops_receiver: *drops_receiver,
                receipt,
            },
        };
    }
    let identities = identities.build();
    for row in rows {
        row.identities = identities.clone();
    }
    Ok(())
}
