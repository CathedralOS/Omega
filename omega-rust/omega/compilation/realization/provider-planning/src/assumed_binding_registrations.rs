//! Executed root Build intent for runtime assumed relationships.
//!
//! Registration rejoins two exact product requirement descriptions after the
//! interpreter checked original Build-cell custody. It conveys no provider
//! conformance, semantic premise, receiving acceptance or installed call. Those
//! independent relationship and trust consumers must admit the complete row
//! before its runtime use; Build cannot execute either selected operand here.

use crate::ProviderPlanningContext;
use checked_interpreter::{ExecutedAssumedBindingRegistration, ExecutedBuildDeclarationSite};
use checked_trees::typed_trees::expression::{ExpressionNode, StaticMachineArgument};
use checked_trees::typed_trees::static_applications::{
    StaticMachineApplication, StaticMachineApplicationHandle, StaticMachineBinding,
    StaticMachineLifetimeBinding,
};
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::BuildOperation;
use std::sync::Arc;
use symbols::SymbolHandle;

/// An exact Product requirement application and its retained payload owner.
/// A descriptor preserves invocation lifetime families and the separately
/// admitted static binder tuple. Its handles never index the final Product's
/// growing type/expression arenas; declaration symbols rejoin that source prefix.
#[derive(Debug, Clone)]
pub struct ProductBoundaryRequirementReference {
    pub symbol: SymbolHandle,
    application: StaticMachineApplicationHandle,
    owner: Arc<checked_trees::typed_trees::FrozenTypedTrees>,
    identity: checked_trees::typed_trees::static_applications::StaticApplicationIdentity,
}

impl PartialEq for ProductBoundaryRequirementReference {
    fn eq(&self, other: &Self) -> bool {
        self.symbol == other.symbol
            && Arc::ptr_eq(&self.owner, &other.owner)
            && self.identity == other.identity
    }
}

impl Eq for ProductBoundaryRequirementReference {}

impl ProductBoundaryRequirementReference {
    /// Source preparation receives the original immutable payload owner;
    /// this does not grant either requirement Build execution authority.
    pub fn application_demand(
        &self,
    ) -> checked_trees::typed_trees::static_applications::StaticApplicationDemand {
        checked_trees::typed_trees::static_applications::StaticApplicationDemand {
            declaration: self.symbol,
            owner: Arc::clone(&self.owner),
            application: self.application,
        }
    }

    pub fn application_owner(&self) -> &checked_trees::typed_trees::TypedTrees {
        &self.owner
    }
    /// Exact source-selected interface; top-level requirements have no trait
    /// owner. The original declaring interface is the direct absence meaning.
    pub fn selected_interface(&self) -> SymbolHandle {
        let selected = self.application().selected_interface;
        if selected.is_valid() {
            selected
        } else {
            self.owner
                .trait_machine_signature(self.symbol)
                .map_or(SymbolHandle::invalid(), |(owner, _)| owner.symbol)
        }
    }

    pub fn application_handle(&self) -> StaticMachineApplicationHandle {
        self.application
    }
    pub fn application(&self) -> &StaticMachineApplication {
        self.owner
            .expression_table
            .static_machine_application(self.application)
    }
    pub fn static_bindings(&self) -> &[StaticMachineBinding] {
        self.owner
            .expression_table
            .static_machine_application_bindings(self.application)
    }
    pub fn lifetime_bindings(&self) -> &[StaticMachineLifetimeBinding] {
        self.owner
            .expression_table
            .static_machine_application_lifetimes(self.application)
    }

    /// Equal lexical positions preserve explicitly selected regions. Bare
    /// invocation families are compared independently by declaration position.
    pub(crate) fn has_same_selected_regions(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.owner, &other.owner)
            && self.identity.lifetimes() == other.identity.lifetimes()
    }
}

/// A registration request, deliberately distinct from admitted installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssumedBindingRegistration {
    pub required: ProductBoundaryRequirementReference,
    pub supplied: ProductBoundaryRequirementReference,
    pub selecting_machine: SymbolHandle,
    pub source_span: source::SourceSpan,
}

/// Retain only actual executions, resolving operands under the occurrence's
/// product package authority rather than the selecting helper's Build imports.
pub fn collect_executed_assumed_binding_registrations(
    context: ProviderPlanningContext<'_>,
    executed: &[ExecutedAssumedBindingRegistration],
) -> Result<Vec<AssumedBindingRegistration>, Vec<Diagnostic>> {
    if context.scope != source::DependencyScope::Product {
        return Err(vec![Diagnostic::error(
            "assumed-binding registrations require the Product declaration context",
        )]);
    }
    let typed = context.typed;
    let mut registrations = Vec::with_capacity(executed.len());
    for receipt in executed {
        let (arguments, source_span, source_application): (&[StaticMachineArgument], _, _) =
            match receipt.site {
                ExecutedBuildDeclarationSite::Expression(expression) => {
                    let ExpressionNode::Call(call) = typed.expression_table.expression(expression)
                    else {
                        return Err(vec![Diagnostic::error(
                            "executed assumed-binding registration lost its exact call",
                        )]);
                    };
                    // The evaluated receipt follows the checked operation and
                    // original Build-cell check. Harvest its original operands;
                    // realization does not repeat the source classification.
                    (
                        &call.machine_arguments,
                        build_time_evaluation::provider_declarations::build_declaration_expression_source_span(
                            typed, expression,
                        ),
                        call.authored_machine_application,
                    )
                }
                ExecutedBuildDeclarationSite::Statement(statement) => {
                    let checked_trees::typed_trees::statement::StatementNode::Call(call) =
                        typed.statement_table.statement(statement)
                    else {
                        return Err(vec![Diagnostic::error(
                            "executed assumed-binding registration lost its exact call",
                        )]);
                    };
                    if call.target_symbol.is_valid()
                        || BuildOperation::from_call_target(call.target.as_str())
                            != Some(BuildOperation::AssumedBindingContract)
                    {
                        return Err(vec![Diagnostic::error(
                            "executed assumed-binding registration did not rejoin its admitted Build operation",
                        )]);
                    }
                    (
                        &call.machine_arguments,
                        call.source_span,
                        call.authored_machine_application,
                    )
                }
            };
        let [required, supplied] = arguments else {
            return Err(vec![Diagnostic::error(
                "assumed-binding registration requires exactly two product requirement descriptions",
            ).with_source_span(source_span)]);
        };
        let resolve = |argument_index: usize, argument: &StaticMachineArgument| {
            let selection = checked_trees::typed_trees::product_requirements::resolve_product_requirement(
                typed, argument, source_span,
            ).ok_or_else(|| vec![Diagnostic::error(
                "assumed-binding operand does not select one visible exact product boundary requirement",
            ).with_source_span(source_span)])?;
            let symbol = selection.declaration;
            if !ProviderPlanningContext::new(
                typed.product_declarations(),
                source::DependencyScope::Product,
            )
            .contains(symbol)
            {
                return Err(vec![
                    Diagnostic::error(
                        "assumed-binding requirement is outside the Product declaration context",
                    )
                    .with_source_span(source_span),
                ]);
            }
            let retained = typed
                .product_declaration_application(source_application, argument_index)
                .filter(|retained| {
                    retained.declaration == symbol
                        && retained
                            .owner
                            .expression_table
                            .static_machine_application(retained.application)
                            .selected_interface
                            == selection.selected_interface
                })
                .ok_or_else(|| {
                    vec![
                        Diagnostic::error(
                            "executed assumed-binding operand lost its owned typed application",
                        )
                        .with_source_span(source_span),
                    ]
                })?;
            Ok(ProductBoundaryRequirementReference {
                symbol,
                application: retained.application,
                owner: retained.owner.clone(),
                identity:
                    checked_trees::typed_trees::static_applications::static_application_identity(
                        &retained.owner,
                        retained.application,
                    )
                    .map_err(|diagnostic| vec![diagnostic.with_source_span(source_span)])?,
            })
        };
        registrations.push(AssumedBindingRegistration {
            required: resolve(0, required)?,
            supplied: resolve(1, supplied)?,
            selecting_machine: receipt.machine,
            source_span,
        });
    }
    Ok(registrations)
}
