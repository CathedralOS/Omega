//! Carry Build's completed choices into application compilation.
//!
//! Provider choices must match their authored declaration requests before they
//! become selection intent. Other policies still lack their producers and
//! refuse requests for their own operations. An absent request needs no policy;
//! an unimplemented one cannot disappear into an empty result.

use language_semantics::declaration_selection::{
    BuildOperation, ExecutedProductSelection, ProductDeclarationRequest,
};

#[path = "product/provider_selections.rs"]
mod provider_selection_requests;

#[path = "product/retained_provider_selections.rs"]
mod retained_provider_selections;

#[cfg(test)]
#[path = "product/provider_selection_tests.rs"]
mod provider_selection_tests;

/// Whether a Build asked for one of `operations`: it authored such a
/// request (`declared`, executed or not) or executed one. Every admission
/// of this module, and the opaque-representation admission of the source
/// preparation, decides by this whether it has anything to admit.
pub fn build_requests_any(
    declared: &[std::sync::Arc<ProductDeclarationRequest>],
    executed: &[ExecutedProductSelection],
    operations: &[BuildOperation],
) -> bool {
    declared
        .iter()
        .any(|request| operations.contains(&request.operation))
        || executed
            .iter()
            .any(|event| operations.contains(&event.request.operation))
}

pub mod test_groups {
    use symbols::SymbolHandle;
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TestGroupRegistration {
        pub requirement: SymbolHandle,
        pub selecting_machine: SymbolHandle,
        pub source_span: source::SourceSpan,
        /// Service slots this group's runner backs with private per-test state,
        /// in selection order; empty runs the group without services.
        pub services: Vec<TestGroupService>,
    }
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TestGroupService {
        pub slot: SymbolHandle,
        pub source_span: source::SourceSpan,
    }
}
pub mod assumed_bindings {
    pub use language_semantics::declaration_selection::AssumedBindingContract;
}
#[path = "product/native_bindings.rs"]
pub mod native_bindings;
pub mod product_declarations {
    use crate::product::assumed_bindings::AssumedBindingContract;
    use crate::product::test_groups::TestGroupRegistration;
    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection::{
        BuildOperation, ExecutedProductSelection, ProductDeclarationRequest,
    };
    use language_semantics::wire::WireCompatibilityDemand;
    use std::sync::Arc;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ServiceAuditRegistration {
        pub identity: String,
        pub name: String,
    }
    pub struct AdmittedProductDeclarations {
        pub service_audits: Vec<ServiceAuditRegistration>,
        pub assumed_bindings: Vec<AssumedBindingContract>,
        pub wire_demands: Vec<WireCompatibilityDemand>,
        pub test_groups: Vec<TestGroupRegistration>,
    }
    pub fn admit(
        _typed: &TypedTrees,
        _activation: Option<SymbolHandle>,
        declared: &[Arc<ProductDeclarationRequest>],
        executed: &[ExecutedProductSelection],
    ) -> Result<AdmittedProductDeclarations, Vec<Diagnostic>> {
        if super::build_requests_any(
            declared,
            executed,
            &[
                BuildOperation::ServiceAudit,
                BuildOperation::AssumedBindingContract,
                BuildOperation::WireCompatibilityRequest,
                BuildOperation::TestGroupRegistration,
                BuildOperation::TestGroupService,
            ],
        ) {
            return Err(vec![Diagnostic::error(
                "no product route yet: Build product declaration admission",
            )]);
        }
        Ok(AdmittedProductDeclarations {
            service_audits: Vec::new(),
            assumed_bindings: Vec::new(),
            wire_demands: Vec::new(),
            test_groups: Vec::new(),
        })
    }
}
pub mod product_policy {
    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection::{
        AuthoredBehaviorExclusion, AuthoredRootGrant, BuildOperation, ExecutedEnumExclusion,
        ExecutedProductSelection, ProductDeclarationRequest,
    };
    use std::sync::Arc;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    pub fn collect_behavior_exclusions(
        _typed: &TypedTrees,
        _activation: Option<SymbolHandle>,
        declared: &[Arc<ProductDeclarationRequest>],
        executed: &[ExecutedProductSelection],
        enums: &[ExecutedEnumExclusion],
    ) -> Result<Vec<AuthoredBehaviorExclusion>, Vec<Diagnostic>> {
        // An executed enum exclusion is its own row kind; every one counts.
        if super::build_requests_any(declared, executed, &[BuildOperation::ServiceExclusion])
            || !enums.is_empty()
        {
            return Err(vec![Diagnostic::error(
                "no product route yet: Build behavior exclusion admission",
            )]);
        }
        Ok(Vec::new())
    }
    pub fn collect_product_root_grants(
        _typed: &TypedTrees,
        _activation: Option<SymbolHandle>,
        declared: &[Arc<ProductDeclarationRequest>],
        executed: &[ExecutedProductSelection],
    ) -> Result<Vec<AuthoredRootGrant>, Vec<Diagnostic>> {
        if super::build_requests_any(declared, executed, &[BuildOperation::BoundaryAcceptance]) {
            return Err(vec![Diagnostic::error(
                "no product route yet: Build root grant admission",
            )]);
        }
        Ok(Vec::new())
    }
}
pub mod provider_declarations {
    use diagnostics::Diagnostic;
    pub use language_semantics::declaration_selection::CompositionMode;
    use language_semantics::declaration_selection::product_operands::ProductStaticOperandGraph;
    use language_semantics::declaration_selection::{BuildSelectionOrigin, ProductTarget};
    use std::sync::Arc;
    use symbols::RetainedDeclaration;
    use symbols::SymbolHandle;
    pub use typed_trees::provider_declarations::ProviderPlanningContext;
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct RetainedProductApplication {
        pub graph: ProductStaticOperandGraph,
        pub sources: Arc<source::SourceMap>,
        pub identity: String,
    }
    impl RetainedProductApplication {
        pub fn declaration_spans(&self) -> impl Iterator<Item = source::SourceSpan> + '_ {
            self.graph
                .names
                .iter()
                .filter(|(_, name)| name.builtin_type.is_none() && name.builtin_function.is_none())
                .filter_map(|(_, name)| name.declaration)
        }
    }
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct RetainedProviderSelection {
        pub subject: RetainedDeclaration,
        pub subject_identity: String,
        pub subject_application: Option<RetainedProductApplication>,
        pub provider: RetainedDeclaration,
        pub provider_identity: String,
        pub provider_application: Option<RetainedProductApplication>,
        pub target: ProductTarget,
        pub composition_mode: CompositionMode,
        pub origin: BuildSelectionOrigin,
    }
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ProviderSelectionIdentity {
        pub symbol: SymbolHandle,
        pub application: arena::Handle<typed_trees::product_operand::ProductOperandApplication>,
        pub applied: bool,
        pub application_identity: Option<String>,
        pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
        pub canonical_path: String,
        pub authored_path: String,
    }
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ProviderOperatorFamilyCoordinate {
        pub symbol: SymbolHandle,
        pub requirement_identity: String,
        pub static_parameter_count: usize,
    }
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ProviderOperatorFamilySelection {
        pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
        pub canonical_path: String,
        pub authored_path: String,
        coordinates: Vec<ProviderOperatorFamilyCoordinate>,
    }
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum ProviderSelectionSubject {
        BoundaryTrait(ProviderSelectionIdentity),
        BoundaryRequirement(ProviderSelectionIdentity),
        BoundaryOperatorFamily(ProviderOperatorFamilySelection),
    }
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ProviderSelection {
        pub target: Option<language_semantics::declaration_selection::ProductTarget>,
        pub subject: ProviderSelectionSubject,
        pub provider_type: ProviderSelectionIdentity,
        pub composition_mode: CompositionMode,
        pub origin: language_semantics::declaration_selection::BuildSelectionOrigin,
    }
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TargetProviderDefaults {
        pub target: Option<String>,
        pub machines: Vec<symbols::SymbolHandle>,
        pub selections: Vec<ProviderSelection>,
    }
    impl ProviderOperatorFamilySelection {
        pub fn new(
            package: Option<semantic_vocabulary::PackageKeyIdentity>,
            canonical_path: String,
            authored_path: String,
            mut coordinates: Vec<ProviderOperatorFamilyCoordinate>,
        ) -> Result<Self, String> {
            coordinates
                .sort_by(|left, right| left.requirement_identity.cmp(&right.requirement_identity));
            if coordinates.is_empty() {
                return Err(format!(
                    "boundary-operator family `{authored_path}` contains no applicable coordinates"
                ));
            }
            for pair in coordinates.windows(2) {
                if pair[0].requirement_identity == pair[1].requirement_identity {
                    return Err(format!(
                        "boundary-operator family `{authored_path}` contains ambiguous coordinate `{}`",
                        pair[0].requirement_identity
                    ));
                }
            }
            Ok(Self {
                package,
                canonical_path,
                authored_path,
                coordinates,
            })
        }
        pub fn coordinates(&self) -> &[ProviderOperatorFamilyCoordinate] {
            &self.coordinates
        }
    }
    impl ProviderSelectionSubject {
        pub fn package(&self) -> Option<semantic_vocabulary::PackageKeyIdentity> {
            match self {
                Self::BoundaryTrait(identity) | Self::BoundaryRequirement(identity) => {
                    identity.package
                }
                Self::BoundaryOperatorFamily(family) => family.package,
            }
        }
        pub fn canonical_path(&self) -> &str {
            match self {
                Self::BoundaryTrait(identity) | Self::BoundaryRequirement(identity) => {
                    &identity.canonical_path
                }
                Self::BoundaryOperatorFamily(family) => &family.canonical_path,
            }
        }
        pub fn authored_path(&self) -> &str {
            match self {
                Self::BoundaryTrait(identity) | Self::BoundaryRequirement(identity) => {
                    &identity.authored_path
                }
                Self::BoundaryOperatorFamily(family) => &family.authored_path,
            }
        }
        pub fn selects_schema(
            &self,
            schema_symbol: SymbolHandle,
            requirement_identity: &str,
            application: Option<&str>,
        ) -> bool {
            match self {
                Self::BoundaryTrait(identity) | Self::BoundaryRequirement(identity) => {
                    identity.symbol == schema_symbol
                        && identity
                            .application_identity
                            .as_deref()
                            .is_none_or(|selected| Some(selected) == application)
                }
                Self::BoundaryOperatorFamily(family) => {
                    family.coordinates.iter().any(|coordinate| {
                        coordinate.symbol == schema_symbol
                            && coordinate.requirement_identity == requirement_identity
                    })
                }
            }
        }
        pub fn same_declaration_as(&self, other: &Self) -> bool {
            match (self, other) {
                (Self::BoundaryTrait(left), Self::BoundaryTrait(right)) => {
                    left.symbol == right.symbol
                        && left.application_identity == right.application_identity
                }
                (Self::BoundaryRequirement(left), Self::BoundaryRequirement(right)) => {
                    left.symbol == right.symbol
                        && left.application_identity == right.application_identity
                }
                (Self::BoundaryOperatorFamily(left), Self::BoundaryOperatorFamily(right)) => {
                    left.package == right.package && left.canonical_path == right.canonical_path
                }
                _ => false,
            }
        }
    }
    pub fn collect_product_provider_selections(
        context: ProviderPlanningContext<'_>,
        activation: Option<SymbolHandle>,
        declared: &[Arc<language_semantics::declaration_selection::ProductDeclarationRequest>],
        executed: &[language_semantics::declaration_selection::ExecutedProductSelection],
    ) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
        super::provider_selection_requests::collect(context, activation, declared, executed)
    }
    /// The provider defaults a program declares, one row per target.
    ///
    /// "The selected target package supplies ordinary named default provider
    /// implementations" (`wiki/spec/build/provider_selection.md`,
    /// "Declaration and choice"). A package declares them in a machine named
    /// `Type::provider_defaults`, whose body is a list of
    /// `Type::select_provider<Slot, Provider>(defaults)` calls. Each call
    /// becomes one selection, read from the typed trees: this is source
    /// policy, not code the Product runs. Machines of one target share a
    /// row, and rows come out ordered by target.
    pub fn retain_provider_defaults(
        context: ProviderPlanningContext<'_>,
    ) -> Result<Vec<TargetProviderDefaults>, Vec<Diagnostic>> {
        let mut rows: Vec<TargetProviderDefaults> = Vec::new();
        for machine in context.typed.machines().iter().filter(|machine| {
            machine.purpose == typed_trees::machine::MachinePurpose::ProviderDefaults
                && !machine.target_family_interface
                && context.contains(machine.symbol)
                && (context.scope == source::DependencyScope::Product || machine.target.is_none())
        }) {
            let target = machine
                .authored_target
                .as_ref()
                .map(|name| name.as_str().to_owned());
            let selections = defaults::harvest(context, machine)?;
            match rows.iter_mut().find(|row| row.target == target) {
                Some(row) => {
                    row.machines.push(machine.symbol);
                    row.selections.extend(selections);
                }
                None => rows.push(TargetProviderDefaults {
                    target,
                    machines: vec![machine.symbol],
                    selections,
                }),
            }
        }
        rows.sort_by(|left, right| left.target.cmp(&right.target));
        Ok(rows)
    }

    /// Reads the selections one `provider_defaults` machine declares.
    ///
    /// Only the forms the reader states are harvested. Anything else in such
    /// a machine is refused by name rather than skipped: a statement that is
    /// not a selection, an explicit composition mode, a slot or provider
    /// applied to static arguments, and a boundary-operator family as the
    /// slot. A skipped statement would be a default the Product silently
    /// does without.
    mod defaults {
        use super::{CompositionMode, ProviderPlanningContext, ProviderSelection, form, identity};
        use diagnostics::Diagnostic;
        use symbols::JoinedPath;
        use typed_trees::expression::{ExpressionNode, StaticMachineArgument};
        use typed_trees::machine::Machine;
        use typed_trees::product_operand::{ProductOperandKind, resolve_product_operand};
        use typed_trees::statement::StatementNode;

        /// One `select_provider` call as its statement holds it.
        struct Call<'typed> {
            target: &'typed str,
            resolved: bool,
            arguments: &'typed [StaticMachineArgument],
            values: usize,
            source_span: source::SourceSpan,
        }

        pub(super) fn harvest(
            context: ProviderPlanningContext<'_>,
            machine: &Machine,
        ) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
            let typed = context.typed;
            let name = machine.name.as_str();
            let refuse = |what: &str, source_span: source::SourceSpan| {
                vec![
                    Diagnostic::error(format!(
                        "no product route yet: source provider defaults: `{name}` {what}"
                    ))
                    .with_source_span(source_span),
                ]
            };
            let mut selections: Vec<ProviderSelection> = Vec::new();
            for state in typed.machine_states(machine) {
                for statement in typed.statement_table.statements(state.statement_nodes) {
                    let call = match statement {
                        StatementNode::Call(call) => Call {
                            target: call.target.as_str(),
                            resolved: call.target_symbol.is_valid(),
                            arguments: &call.machine_arguments,
                            values: typed
                                .statement_table
                                .expression_handles(call.arguments)
                                .len(),
                            source_span: call.source_span,
                        },
                        StatementNode::Expression(expression) => {
                            let source_span = typed.expression_table.source_span(*expression);
                            match typed.expression_table.expression(*expression) {
                                ExpressionNode::Call(call) => Call {
                                    target: call.target.as_str(),
                                    resolved: call.target_symbol.is_valid(),
                                    arguments: &call.machine_arguments,
                                    values: typed
                                        .expression_table
                                        .expression_handles(call.arguments)
                                        .len(),
                                    source_span,
                                },
                                _ => {
                                    return Err(refuse(
                                        "holds a statement that is not a `select_provider` call",
                                        source_span,
                                    ));
                                }
                            }
                        }
                        _ => {
                            return Err(refuse(
                                "holds a statement that is not a `select_provider` call",
                                typed
                                    .symbols
                                    .symbol_source_span(machine.symbol)
                                    .unwrap_or_default(),
                            ));
                        }
                    };
                    // Typing leaves the selection intrinsic unresolved; a
                    // call that resolved to a machine is an ordinary call.
                    if call.target != "select_provider" || call.resolved {
                        return Err(refuse(
                            "holds a statement that is not a `select_provider` call",
                            call.source_span,
                        ));
                    }
                    let [slot, provider] = call.arguments else {
                        return Err(refuse(
                            "selects a provider without exactly a slot and a provider",
                            call.source_span,
                        ));
                    };
                    // The one value argument is the defaults value itself. A
                    // second one is an explicit `CompositionMode`.
                    if call.values != 1 {
                        return Err(refuse(
                            "selects a provider with an explicit composition mode",
                            call.source_span,
                        ));
                    }
                    if slot.application.is_some() || provider.application.is_some() {
                        return Err(refuse(
                            "selects a slot or provider applied to static arguments",
                            call.source_span,
                        ));
                    }
                    let origin = super::BuildSelectionOrigin::capture(
                        &typed.symbols,
                        machine.symbol,
                        machine.symbol,
                        call.source_span,
                        None,
                    )
                    .map_err(|reason| refuse(reason, call.source_span))?;
                    let slot_symbol = resolve_product_operand(
                        typed,
                        slot,
                        call.source_span,
                        ProductOperandKind::BoundarySubject,
                    )
                    .ok_or_else(|| refuse(&format!(
                        "subject `{}` does not resolve to an exact boundary trait or requirement visible here",
                        slot.path.joined_path(),
                    ), call.source_span))?;
                    let provider_symbol = resolve_product_operand(
                        typed,
                        provider,
                        call.source_span,
                        ProductOperandKind::Provider,
                    )
                    .ok_or_else(|| refuse(&format!(
                        "provider `{}` does not resolve to an exact declaration visible here",
                        provider.path.joined_path(),
                    ), call.source_span))?;
                    let slot = identity(context, slot_symbol, &slot.path.joined_path());
                    let provider = identity(context, provider_symbol, &provider.path.joined_path());
                    let selection = form(
                        context,
                        slot,
                        provider,
                        origin,
                        None,
                        CompositionMode::Fused,
                        super::ProviderForm::DefaultClosure,
                    )
                    .map_err(|diagnostic| vec![diagnostic.with_source_span(call.source_span)])?;
                    if let Some(existing) = selections
                        .iter()
                        .find(|existing| existing.subject.same_declaration_as(&selection.subject))
                        && existing.provider_type.symbol != selection.provider_type.symbol
                    {
                        return Err(vec![
                            Diagnostic::error(format!(
                                "build selects two provider types for slot `{}`: `{}` and `{}`",
                                selection.subject.canonical_path(),
                                existing.provider_type.canonical_path,
                                selection.provider_type.canonical_path,
                            ))
                            .with_source_span(call.source_span),
                        ]);
                    }
                    selections.push(selection);
                }
            }
            Ok(selections)
        }
    }

    pub(super) fn identity(
        context: ProviderPlanningContext<'_>,
        symbol: symbols::SymbolHandle,
        authored_path: &str,
    ) -> ProviderSelectionIdentity {
        let symbols = &context.typed.symbols;
        ProviderSelectionIdentity {
            symbol,
            application: Default::default(),
            applied: false,
            application_identity: None,
            package: symbols.symbol_package_identity(symbol),
            canonical_path: symbols.display_path(symbol, "::"),
            authored_path: authored_path.to_owned(),
        }
    }

    /// What may stand in the Provider position of a selection.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(super) enum ProviderForm {
        /// A choice a Build makes: a static record a named implementation
        /// constructs. provider_selection.md, "Declaration and choice": "A
        /// data type or runtime provider descriptor is not such a value."
        StaticRecord,
        /// A default a target package declares: a named implementation, or
        /// the data type that owns the package's provider closure, whose
        /// attached machines each state the requirement they satisfy
        /// (`boundary machine Provider::exit_process(...) satisfies
        /// ProcessExit::exit_process`). The bundled library declares every
        /// one of its defaults this way, so refusing the form refuses every
        /// program; it stays served until the library is moved to named
        /// implementations (`TASKS.md`, **STATIC-MACHINE-MIGRATION**).
        DefaultClosure,
    }

    /// Classifies the slot, checks the provider is a declaration that can
    /// provide in `provider_form`, and states the selection with its
    /// authored origin.
    pub(super) fn form(
        context: ProviderPlanningContext<'_>,
        slot: ProviderSelectionIdentity,
        provider: ProviderSelectionIdentity,
        origin: BuildSelectionOrigin,
        target: Option<ProductTarget>,
        composition_mode: CompositionMode,
        provider_form: ProviderForm,
    ) -> Result<ProviderSelection, Diagnostic> {
        use symbols::SymbolKind;
        let typed = context.typed;
        if !slot.symbol.is_valid() || !provider.symbol.is_valid() {
            return Err(Diagnostic::error(format!(
                "provider selection `{}` for `{}` does not resolve to declarations",
                provider.authored_path, slot.authored_path
            )));
        }
        if !context.contains(slot.symbol) || !context.contains(provider.symbol) {
            return Err(Diagnostic::error(format!(
                "provider selection subject or provider is outside the {:?} planning context",
                context.scope
            )));
        }
        let slot_kind = typed.symbols.get(slot.symbol).kind;
        if typed
            .trait_definition_by_symbol(slot.symbol)
            .is_some_and(|definition| !definition.type_parameters.is_empty())
        {
            return Err(Diagnostic::error(format!(
                "no product route yet: provider slot `{}` requires its complete static application",
                slot.authored_path,
            )));
        }
        let subject = if slot_kind == SymbolKind::Trait
            && typed
                .traits()
                .iter()
                .any(|definition| definition.symbol == slot.symbol && definition.is_boundary)
        {
            ProviderSelectionSubject::BoundaryTrait(slot)
        } else if slot_kind == SymbolKind::Machine
            && typed.machines().iter().any(|requirement| {
                requirement.symbol == slot.symbol
                    && requirement.spelling.is_none()
                    && requirement.supply_mode
                        == language_semantics::MachineSupplyMode::TopLevelRequirement
            })
        {
            ProviderSelectionSubject::BoundaryRequirement(slot)
        } else if slot_kind == SymbolKind::Operator
            || typed
                .machine_token_bindings()
                .iter()
                .any(|binding| binding.is_boundary && binding.symbol == slot.symbol)
        {
            return Err(Diagnostic::error(format!(
                "no product route yet: source provider defaults: slot `{}` is a boundary-operator family",
                slot.authored_path
            )));
        } else {
            return Err(Diagnostic::error(format!(
                "provider selection subject `{}` does not resolve to an exact boundary trait, top-level boundary requirement, or boundary-operator family",
                slot.authored_path
            )));
        };
        let implementation = typed
            .conformances()
            .iter()
            .find(|definition| definition.symbol == provider.symbol);
        let owns_default_closure = provider_form == ProviderForm::DefaultClosure
            && typed.symbols.get(provider.symbol).kind == SymbolKind::Data;
        if implementation.is_none() && !owns_default_closure {
            return Err(Diagnostic::error(format!(
                "provider selection `{}` is not a static operation record from a named implementation; a data type is not a provider value",
                provider.authored_path
            )));
        }
        if implementation.is_some_and(|implementation| !implementation.type_parameters.is_empty()) {
            return Err(Diagnostic::error(format!(
                "no product route yet: provider `{}` requires its complete static application",
                provider.authored_path,
            )));
        }
        // A named implementation with lifetime parameters provides a
        // requirement its parent trait declares only when the parent's
        // lifetimes are spelled by the implementation's own.
        if let Some(implementation) =
            implementation.filter(|definition| definition.type_parameters.is_empty())
        {
            let root = typed.trait_definition_by_symbol(implementation.trait_symbol);
            let arguments = implementation.trait_lifetime_arguments.as_slice();
            let own = implementation.lifetime_parameters.len();
            let spelled = root.is_some_and(|root| {
                arguments.len() == root.lifetime_parameters.len()
                    && arguments
                        .iter()
                        .all(|ordinal| usize::try_from(*ordinal).is_ok_and(|ordinal| ordinal < own))
            });
            for row in &implementation.rows {
                let inherited_without_lifetimes = typed
                    .trait_definition_by_symbol(row.declaring_trait)
                    .is_some_and(|declaring| declaring.lifetime_parameters.is_empty());
                let direct = root.is_some_and(|root| root.symbol == row.declaring_trait);
                if !spelled || !(direct || inherited_without_lifetimes) {
                    return Err(Diagnostic::error(format!(
                        "named provider `{}` requires an unsupported inherited declaring-trait lifetime application for `{}`; the exact parent substitution is not retained",
                        provider.authored_path, row.requirement_name,
                    )));
                }
            }
        }
        Ok(ProviderSelection {
            target,
            subject,
            provider_type: provider,
            composition_mode,
            origin,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn request(operation: BuildOperation) -> Arc<ProductDeclarationRequest> {
        Arc::new(ProductDeclarationRequest {
            source_span: source::SourceSpan::default(),
            operation,
            operands: Box::new([]),
        })
    }

    fn executed(request: &Arc<ProductDeclarationRequest>) -> ExecutedProductSelection {
        ExecutedProductSelection {
            static_environment: None,
            target: None,
            request: Arc::clone(request),
            selecting_machine: None,
            composition: None,
        }
    }

    /// Runs every admission that reads product selections and reports which
    /// of them refused.
    fn refusals(
        declared: &[Arc<ProductDeclarationRequest>],
        executed: &[ExecutedProductSelection],
    ) -> Vec<String> {
        let typed = typed_trees::TypedTrees::default();
        // The Build ran: its activation is known in every case.
        let activation = Some(symbols::SymbolHandle::invalid());
        let context = provider_declarations::ProviderPlanningContext::new(
            &typed,
            source::DependencyScope::Product,
        );
        [
            native_bindings::collect(&typed, activation, declared, &[]).map(drop),
            product_declarations::admit(&typed, activation, declared, executed).map(drop),
            product_policy::collect_behavior_exclusions(
                &typed,
                activation,
                declared,
                executed,
                &[],
            )
            .map(drop),
            product_policy::collect_product_root_grants(&typed, activation, declared, executed)
                .map(drop),
            provider_declarations::collect_product_provider_selections(
                context, activation, declared, executed,
            )
            .map(drop),
        ]
        .into_iter()
        .filter_map(Result::err)
        .map(|diagnostics| diagnostics[0].message.clone())
        .collect()
    }

    /// A Build that ran and asked for nothing these admissions own is
    /// refused by none of them. `OptimizationSelection` belongs to none.
    #[test]
    fn a_build_that_asks_an_admission_for_nothing_is_not_refused_by_it() {
        assert_eq!(refusals(&[], &[]), Vec::<String>::new());
        let other = request(BuildOperation::OptimizationSelection);
        assert_eq!(
            refusals(&[Arc::clone(&other)], &[executed(&other)]),
            Vec::<String>::new()
        );
    }

    /// The twins: each unimplemented operation is refused by its own admission,
    /// whether the Build only authored the request or also executed it.
    #[test]
    fn each_request_is_refused_by_the_admission_that_owns_its_operation() {
        for (operation, owner) in [
            (
                BuildOperation::ServiceAudit,
                "Build product declaration admission",
            ),
            (
                BuildOperation::AssumedBindingContract,
                "Build product declaration admission",
            ),
            (
                BuildOperation::WireCompatibilityRequest,
                "Build product declaration admission",
            ),
            (
                BuildOperation::TestGroupRegistration,
                "Build product declaration admission",
            ),
            (
                BuildOperation::TestGroupService,
                "Build product declaration admission",
            ),
            (
                BuildOperation::ServiceExclusion,
                "Build behavior exclusion admission",
            ),
            (
                BuildOperation::BoundaryAcceptance,
                "Build root grant admission",
            ),
        ] {
            let asked = request(operation);
            let expected = [format!("no product route yet: {owner}")];
            assert_eq!(
                refusals(&[Arc::clone(&asked)], &[]),
                expected,
                "{operation:?}"
            );
            assert_eq!(
                refusals(&[], &[executed(&asked)]),
                expected,
                "{operation:?}"
            );
        }
    }

    /// The defaults a program declares, as (target, slot, provider) rows,
    /// or the first refusal.
    fn provider_defaults_of(source: &str) -> Result<Vec<(Option<String>, String, String)>, String> {
        let typed = crate::source_jobs::tests::typed(source);
        provider_declarations::retain_provider_defaults(
            provider_declarations::ProviderPlanningContext::new(
                &typed,
                source::DependencyScope::Product,
            ),
        )
        .map(|rows| {
            rows.into_iter()
                .flat_map(|row| {
                    let target = row.target.clone();
                    row.selections.into_iter().map(move |selection| {
                        (
                            target.clone(),
                            selection.subject.authored_path().to_owned(),
                            selection.provider_type.authored_path,
                        )
                    })
                })
                .collect()
        })
        .map_err(|diagnostics| diagnostics[0].message.clone())
    }

    const SLOT: &str = "boundary trait Service {\n    machine go() reaches none crashes none;\n}\ndata Native {}\nNativeProvider satisfies Service { machine go() reaches none crashes none {} }\n";

    /// A program that declares no provider-defaults machine has no defaults,
    /// and one that declares a default states it as slot and provider.
    #[test]
    fn a_provider_default_is_read_from_its_select_provider_call() {
        assert_eq!(
            provider_defaults_of(&format!(
                "{SLOT}machine Native::configure(defaults: &mut Native) {{ }}\n"
            )),
            Ok(Vec::new())
        );
        assert_eq!(
            provider_defaults_of(&format!(
                "{SLOT}machine Native::provider_defaults(defaults: &mut Native) {{\n    Native::select_provider<Service, NativeProvider>(defaults);\n}}\n"
            )),
            Ok(vec![(
                None,
                "Service".to_owned(),
                "NativeProvider".to_owned()
            )])
        );
    }

    /// The bundled library's form: the default names the data type that owns
    /// the provider closure, as `std` does for every target
    /// (`ProcessExitNativeProvider::select_provider<ProcessExit,
    /// ProcessExitNativeProvider>(defaults)`). It is a default, so it is read;
    /// the same data type chosen by a Build is refused
    /// (`provider_selection_tests`).
    #[test]
    fn a_provider_default_may_name_the_data_type_that_owns_its_closure() {
        assert_eq!(
            provider_defaults_of(&format!(
                "{SLOT}machine Native::provider_defaults(defaults: &mut Native) {{
    Native::select_provider<Service, Native>(defaults);
}}
"
            )),
            Ok(vec![(None, "Service".to_owned(), "Native".to_owned())])
        );
    }

    /// The twins: the same machine selecting for a slot that is not a
    /// boundary, and holding a statement that is not a selection. Neither is
    /// skipped.
    #[test]
    fn a_provider_default_that_is_not_a_selection_of_a_boundary_is_refused() {
        let refused = provider_defaults_of(&format!(
            "{SLOT}machine Native::provider_defaults(defaults: &mut Native) {{\n    Native::select_provider<Native, NativeProvider>(defaults);\n}}\n"
        ))
        .unwrap_err();
        assert!(
            refused.contains("subject `Native` does not resolve to an exact boundary trait"),
            "{refused}"
        );
        let refused = provider_defaults_of(&format!(
            "{SLOT}machine other() {{ }}\nmachine Native::provider_defaults(defaults: &mut Native) {{\n    other();\n}}\n"
        ))
        .unwrap_err();
        assert_eq!(
            refused,
            "no product route yet: source provider defaults: `Native::provider_defaults` holds a statement that is not a `select_provider` call"
        );
    }
}
